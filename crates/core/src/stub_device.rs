//! feature `test-support` を有効にしたときだけ公開される、試験用の device と provider。
//!
//! GPU も背後実装も使わず、ホストのメモリだけで `Device`・`MemoryOps`・`CommandQueue` を満たす（TASK-156 のポインタ）。
//! 製品の経路からは使わない。メモリと投入の口は本 module の中の非公開の型で、`StubDevice` 越しにだけ届く。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::capability::{BackendKind, Capabilities, CapabilityLevel, FeatureId, LimitsTable};
use crate::device::{Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest};
use crate::error::{CompletionState, ContractError};
use crate::execution::{
    __BackendSharedEncoderCloser, __backend_next_encoder_id, Binding, CommandEncoder, CommandQueue,
    ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, SubmitToken, ThreadgroupMemory,
};
use crate::memory::{
    BufferHandle, BufferUsage, MemoryOps, validate_allocation_size, validate_buffer_range,
};

/// 試験用の device の上限の表。確保の上限だけ持ち、dispatch・テクスチャは 0。
const LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1 << 28,
    max_dispatch_dims: [0, 0, 0],
    max_texture_size: 0,
};

/// device の識別子の源。device の世代とハンドル・印の `device_id` に同じ値を使う。
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// 識別子を 1 つ払い出す。尽きたら `u64::MAX` を返す（panic しない）。
fn next_id() -> u64 {
    let mut current = NEXT_ID.load(Ordering::Relaxed);
    loop {
        let Some(next) = current.checked_add(1) else {
            return u64::MAX;
        };
        match NEXT_ID.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return current,
            Err(actual) => current = actual,
        }
    }
}

/// 錠を取る。毒化していても中身を使い続ける。
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// バッファの表。
struct Buffers {
    next_id: u64,
    map: HashMap<u64, Vec<u8>>,
}

/// ホストのメモリで動く `MemoryOps`。`StubDevice` が持つ。
struct StubMemory {
    id: u64,
    buffers: Mutex<Buffers>,
}

impl StubMemory {
    fn new(id: u64) -> Self {
        Self {
            id,
            buffers: Mutex::new(Buffers {
                next_id: 1,
                map: HashMap::new(),
            }),
        }
    }

    /// ハンドルが自分の払い出したものか照合し、表の中の記録を返す。
    fn check<'a>(
        &self,
        map: &'a mut HashMap<u64, Vec<u8>>,
        handle: &BufferHandle,
    ) -> Result<&'a mut Vec<u8>, ContractError> {
        if handle.__backend_device_id() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        let Some(data) = map.get_mut(&handle.__backend_id()) else {
            return Err(ContractError::InvalidArgument(
                "buffer handle is not live".to_string(),
            ));
        };
        if handle.__backend_generation() != self.id {
            return Err(ContractError::InvalidArgument(
                "buffer handle generation does not match".to_string(),
            ));
        }
        if u64::try_from(data.len()).ok() != Some(handle.size()) {
            return Err(ContractError::InvalidArgument(
                "buffer handle size does not match".to_string(),
            ));
        }
        Ok(data)
    }

    /// 範囲を検査して読み出す。`download_*` が共有する。
    fn read_range(
        &self,
        src: &BufferHandle,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        let mut guard = lock(&self.buffers);
        let data = self.check(&mut guard.map, src)?;
        let alloc_len = u64::try_from(data.len()).unwrap_or(u64::MAX);
        validate_buffer_range(offset, len, alloc_len)?;
        let alloc_failed = || ContractError::AllocationFailed {
            requested_bytes: len,
        };
        let start = usize::try_from(offset).map_err(|_| alloc_failed())?;
        let n = usize::try_from(len).map_err(|_| alloc_failed())?;
        let end = start.checked_add(n).ok_or_else(alloc_failed)?;
        let Some(slice) = data.get(start..end) else {
            return Err(ContractError::InvalidArgument(
                "range is outside the buffer".to_string(),
            ));
        };
        let mut out = Vec::new();
        out.try_reserve_exact(n).map_err(|_| alloc_failed())?;
        out.extend_from_slice(slice);
        Ok(out)
    }
}

impl MemoryOps for StubMemory {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        validate_allocation_size(byte_len, LIMITS.max_buffer_bytes)?;
        let failed = || ContractError::AllocationFailed {
            requested_bytes: byte_len,
        };
        let len = usize::try_from(byte_len).map_err(|_| failed())?;
        let mut data = Vec::new();
        data.try_reserve_exact(len).map_err(|_| failed())?;
        data.resize(len, 0);
        let mut guard = lock(&self.buffers);
        let id = guard.next_id;
        guard.next_id = id.checked_add(1).ok_or_else(failed)?;
        guard.map.insert(id, data);
        Ok(BufferHandle::__backend_new(
            self.id, id, self.id, byte_len, usage,
        ))
    }

    fn upload(
        &self,
        dst: &BufferHandle,
        dst_offset: u64,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        let mut guard = lock(&self.buffers);
        let data = self.check(&mut guard.map, dst)?;
        let len = u64::try_from(bytes.len())
            .map_err(|_| ContractError::InvalidArgument("upload length overflows".to_string()))?;
        let alloc_len = u64::try_from(data.len()).unwrap_or(u64::MAX);
        validate_buffer_range(dst_offset, len, alloc_len)?;
        let out_of_range =
            || ContractError::InvalidArgument("range is outside the buffer".to_string());
        let start = usize::try_from(dst_offset).map_err(|_| out_of_range())?;
        let end = start.checked_add(bytes.len()).ok_or_else(out_of_range)?;
        let Some(slice) = data.get_mut(start..end) else {
            return Err(out_of_range());
        };
        slice.copy_from_slice(bytes);
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        self.read_range(src, src_offset, byte_len)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        let result = self.read_range(src, src_offset, byte_len);
        callback(result);
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        let result = self.read_range(src, src_offset, byte_len);
        callback(result);
    }

    fn free(&self, buffer: BufferHandle) {
        let mut guard = lock(&self.buffers);
        if self.check(&mut guard.map, &buffer).is_ok() {
            guard.map.remove(&buffer.__backend_id());
        }
    }
}

/// 開いている記録先の集合。破棄時の hook を兼ねる。
struct OpenEncoders {
    ids: Mutex<HashSet<u64>>,
}

impl OpenEncoders {
    /// 全部が開いていて重複が無いときだけ、まとめて閉じる。
    fn claim(&self, ids: &[u64]) -> Result<(), ContractError> {
        let mut set = lock(&self.ids);
        let mut seen = HashSet::new();
        for id in ids {
            if !set.contains(id) || !seen.insert(*id) {
                return Err(ContractError::InvalidArgument(
                    "command encoder is not open on this queue".to_string(),
                ));
            }
        }
        for id in ids {
            set.remove(id);
        }
        Ok(())
    }
}

impl __BackendSharedEncoderCloser for OpenEncoders {
    fn close(&self, id: u64) {
        lock(&self.ids).remove(&id);
    }
}

/// 投入を即時に完了済みとして扱う `CommandQueue`。`StubDevice` が持つ。
struct StubQueue {
    backend: BackendKind,
    id: u64,
    open: Arc<OpenEncoders>,
    next_sequence: AtomicU64,
}

impl StubQueue {
    fn new(backend: BackendKind, id: u64) -> Self {
        Self {
            backend,
            id,
            open: Arc::new(OpenEncoders {
                ids: Mutex::new(HashSet::new()),
            }),
            next_sequence: AtomicU64::new(1),
        }
    }

    /// 投入の印を 1 つ払い出す。
    fn issue(&self) -> Result<SubmitToken, ContractError> {
        let mut current = self.next_sequence.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_add(1) else {
                return Err(ContractError::BackendFailure {
                    op: "submit",
                    backend: self.backend,
                    detail: "submission sequence space is exhausted".to_string(),
                });
            };
            match self.next_sequence.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(SubmitToken::__backend_new(self.id, current, self.id)),
                Err(actual) => current = actual,
            }
        }
    }

    /// 印が自分の device のものか照合する。
    fn check_token(&self, token: &SubmitToken) -> Result<(), ContractError> {
        if token.__backend_device_id() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        if token.generation != self.id {
            return Err(ContractError::DeviceLost {
                generation: token.generation,
            });
        }
        Ok(())
    }
}

impl CommandQueue for StubQueue {
    fn create_encoder(&self) -> CommandEncoder {
        match __backend_next_encoder_id() {
            Some(id) => {
                lock(&self.open.ids).insert(id);
                CommandEncoder::__backend_new_shared(
                    id,
                    self.open.clone() as Arc<dyn __BackendSharedEncoderCloser>,
                )
            }
            None => CommandEncoder::__backend_new(0, None),
        }
    }

    fn submit(
        &self,
        mut commands: CommandEncoder,
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        if !wait.is_empty() {
            return Err(ContractError::Unsupported {
                op: "submit(wait)",
                backend: self.backend,
            });
        }
        self.open.claim(&[commands.__backend_id()])?;
        commands.__backend_disarm_closer();
        self.issue()
    }

    fn submit_many(
        &self,
        mut commands: Vec<CommandEncoder>,
        after: &[SubmitToken],
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        if commands.is_empty() {
            return Err(ContractError::InvalidArgument(
                "submit_many requires at least one command encoder".to_string(),
            ));
        }
        if !wait.is_empty() {
            return Err(ContractError::Unsupported {
                op: "submit_many(wait)",
                backend: self.backend,
            });
        }
        for token in after {
            self.check_token(token)?;
        }
        let ids: Vec<u64> = commands.iter().map(|c| c.__backend_id()).collect();
        self.open.claim(&ids)?;
        for c in &mut commands {
            c.__backend_disarm_closer();
        }
        self.issue()
    }

    fn poll(&self, token: SubmitToken) -> CompletionState {
        if token.__backend_device_id() != self.id
            || token.generation != self.id
            || token.sequence == 0
            || token.sequence >= self.next_sequence.load(Ordering::Relaxed)
        {
            return CompletionState::Lost;
        }
        CompletionState::Complete
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        let state = self.poll(token);
        callback(state);
    }

    #[cfg(target_arch = "wasm32")]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState)>) {
        let state = self.poll(token);
        callback(state);
    }

    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        token: SubmitToken,
        _timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        self.check_token(&token)?;
        match self.poll(token) {
            CompletionState::Lost => Err(ContractError::BackendFailure {
                op: "wait_blocking",
                backend: self.backend,
                detail: "submit token was not issued by this queue".to_string(),
            }),
            _ => Ok(()),
        }
    }

    fn dispatch_ex(
        &self,
        _encoder: &mut CommandEncoder,
        _pipeline: &PipelineHandle,
        _bindings: &[Binding<'_>],
        _consts: &[ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        Err(ContractError::Unsupported {
            op: "dispatch_ex",
            backend: self.backend,
        })
    }
}

/// GPU も背後実装も使わない試験用の device。
///
/// 要求の `backend` と所有形態をそのまま持ち、能力問合せはどの機能も `Unsupported` を返す。
/// メモリはホストの `Vec<u8>`、投入は即時に完了済みとして扱う。
pub struct StubDevice {
    backend: BackendKind,
    ownership: DeviceOwnership,
    id: u64,
    memory: StubMemory,
    queue: StubQueue,
}

impl StubDevice {
    /// 要求から device を作る。組み合わせの検査はせず、どの要求でも成功する。
    pub fn new(req: DeviceRequest) -> Self {
        let id = next_id();
        Self {
            backend: req.backend,
            ownership: req.ownership,
            id,
            memory: StubMemory::new(id),
            queue: StubQueue::new(req.backend, id),
        }
    }
}

impl Capabilities for StubDevice {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::Unsupported
    }

    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }
}

impl Device for StubDevice {
    fn backend(&self) -> BackendKind {
        self.backend
    }

    fn ownership(&self) -> &DeviceOwnership {
        &self.ownership
    }

    fn generation(&self) -> u64 {
        self.id
    }

    fn memory(&self) -> &dyn MemoryOps {
        &self.memory
    }

    fn queue(&self) -> &dyn CommandQueue {
        &self.queue
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: "stub".to_string(),
            backend_name: "stub",
        }
    }
}

/// `StubDevice` を返す provider。どの要求にも `Ok` を返す。
#[derive(Debug, Clone, Copy, Default)]
pub struct StubDeviceProvider;

impl DeviceProvider for StubDeviceProvider {
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        Ok(Box::new(StubDevice::new(req)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Profile;

    fn request() -> DeviceRequest {
        DeviceRequest {
            backend: BackendKind::Vulkan,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::Shared,
        }
    }

    #[test]
    fn stub_device_request_returns_ok_and_echoes_backend_and_ownership() {
        let device = StubDeviceProvider::request(request()).expect("device");
        assert_eq!(device.backend(), BackendKind::Vulkan);
        assert!(matches!(device.ownership(), DeviceOwnership::Shared));
    }

    #[test]
    fn stub_device_round_trip_allocate_upload_download() {
        let device = StubDeviceProvider::request(request()).expect("device");
        let mem = device.memory();
        let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
        let buf = mem.allocate(8, usage).expect("alloc");
        mem.upload(&buf, 2, &[1, 2, 3]).expect("upload");
        type Downloaded = Vec<Result<Vec<u8>, ContractError>>;
        let slot: Arc<Mutex<Downloaded>> = Arc::default();
        let sink = Arc::clone(&slot);
        mem.download_async(
            &buf,
            0,
            8,
            Box::new(move |r| sink.lock().expect("lock").push(r)),
        );
        let got = slot.lock().expect("lock");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].as_ref().expect("ok"), &[0, 0, 1, 2, 3, 0, 0, 0]);
    }

    #[test]
    fn stub_device_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StubDevice>();
    }
}
