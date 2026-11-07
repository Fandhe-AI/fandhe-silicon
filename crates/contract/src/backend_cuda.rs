//! CUDA の背後実装（下-1）。`backend-cuda` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! ドライバは `cudarc` 経由で実行時に動的に読み込み、`CudaDeviceProvider::request` が device の
//! 取得・診断・確保を担う `CudaDevice`（非公開）を返す。`cudarc` の型は公開 API に出さない。
//! wasm32 ではコンパイル対象から外す（`lib.rs` の `cfg`）。
//!
//! spec との対応: TASK-5（ポインタ表記）。
//!
//! `CudaDeviceProvider` は CUDA の `DeviceProvider` の入口で、device を作る前に要求を検査し、
//! 通れば device を作る（TASK-160 のポインタ）。

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use cudarc::driver::{CudaContext, CudaSlice, CudaStream, DriverError};
use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, UnsupportedReason,
};
use fandhe_silicon_core::execution::{
    __backend_next_encoder_id, Binding, CommandEncoder, CommandQueue, ConstantBinding,
    ImportedSemaphoreHandle, PipelineHandle, SubmitToken, ThreadgroupMemory,
};
use fandhe_silicon_core::memory::{
    BufferHandle, BufferUsage, MemoryOps, validate_allocation_size, validate_buffer_range,
};

/// この provider が受け持つ背後実装。
const BACKEND: BackendKind = BackendKind::Cuda;

/// `DeviceDiagnostics::backend_name` に載せる名前。
const BACKEND_NAME: &str = "cuda";

/// device の識別子の採番。`take_next` で 1 から単調増加に取る。
static NEXT_DEVICE_ID: AtomicU64 = AtomicU64::new(1);

/// device の世代の採番。背後実装の中で 1 から単調増加に取る。
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// カウンタの現在値を返して 1 進める。使い切ったら `None`。`request_device` から呼ばれる。
fn take_next(counter: &AtomicU64) -> Option<u64> {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1)?;
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some(current),
            Err(seen) => current = seen,
        }
    }
}

/// 要求の組み合わせを device を作る前に照らす。`DeviceProvider::request` から呼ばれる。
///
/// 照らす順は、背後実装、プロファイル、所有形態。最初に当たったものを返す。
/// `Graphics` は `graphics` feature の有無によらず拒む。
fn check_request(req: &DeviceRequest) -> Result<(), ContractError> {
    if req.backend != BACKEND {
        return Err(ContractError::InvalidCombination(format!(
            "CUDA provider cannot serve backend {:?}",
            req.backend
        )));
    }
    match req.profile {
        Profile::ComputeOnly => {}
        Profile::Graphics | Profile::CpuOnly => {
            return Err(ContractError::InvalidCombination(format!(
                "backend {BACKEND:?} does not serve profile {:?}",
                req.profile
            )));
        }
        _ => {
            return Err(ContractError::InvalidCombination(
                "unknown device profile".into(),
            ));
        }
    }
    match req.ownership {
        DeviceOwnership::ProductIndependent => Ok(()),
        DeviceOwnership::Shared => Err(ContractError::Unsupported {
            op: "request(DeviceOwnership::Shared)",
            backend: BACKEND,
        }),
        DeviceOwnership::Imported { .. } => Err(ContractError::Unsupported {
            op: "request(DeviceOwnership::Imported)",
            backend: BACKEND,
        }),
        _ => Err(ContractError::InvalidCombination(
            "unknown device ownership".into(),
        )),
    }
}

/// GPU の名前と compute capability から診断用の `adapter_name` を組み立てる。
fn adapter_name_of(name: &str, capability: (i32, i32)) -> String {
    format!(
        "{name} (cuda compute_capability={}.{})",
        capability.0, capability.1
    )
}

/// context から読み取った device の属性。
struct Attributes {
    adapter_name: String,
    total_mem: u64,
}

/// 名前・compute capability・メモリの大きさを順に読み、1 つでも失敗したら `DeviceUnavailable` にする。
/// 読み取りは閉包で受けるので、単体試験からは device に触らず呼べる。
fn read_attributes<E: std::fmt::Debug>(
    name: impl FnOnce() -> Result<String, E>,
    capability: impl FnOnce() -> Result<(i32, i32), E>,
    total_mem: impl FnOnce() -> Result<usize, E>,
) -> Result<Attributes, ContractError> {
    let unavailable =
        |call: &str, e: &E| ContractError::DeviceUnavailable(format!("{call}: {e:?}"));
    let name = name().map_err(|e| unavailable("CudaContext::name", &e))?;
    let capability =
        capability().map_err(|e| unavailable("CudaContext::compute_capability", &e))?;
    let total = total_mem().map_err(|e| unavailable("CudaContext::total_mem", &e))?;
    let total_mem = u64::try_from(total).map_err(|_| {
        ContractError::DeviceUnavailable("CudaContext::total_mem: value exceeds u64".into())
    })?;
    Ok(Attributes {
        adapter_name: adapter_name_of(&name, capability),
        total_mem,
    })
}

/// grid の各軸の上限を属性の閉包で順に読み、1 つでも失敗か負の値なら `DeviceUnavailable` にする。
/// 値は `u32` に収まる範囲で返す。
fn read_grid_dims<E: std::fmt::Debug>(
    mut attr: impl FnMut(usize) -> Result<i32, E>,
) -> Result<[u32; 3], ContractError> {
    let mut dims = [0_u32; 3];
    for (axis, slot) in dims.iter_mut().enumerate() {
        let v = attr(axis).map_err(|e| {
            ContractError::DeviceUnavailable(format!(
                "CudaContext::attribute(MAX_GRID_DIM[{axis}]): {e:?}"
            ))
        })?;
        *slot = u32::try_from(v).map_err(|_| {
            ContractError::DeviceUnavailable(format!(
                "CudaContext::attribute(MAX_GRID_DIM[{axis}]): invalid value {v}"
            ))
        })?;
    }
    Ok(dims)
}

/// ドライバを呼ぶ閉包の panic を `DeviceUnavailable` に変える。panic の payload は載せない。
/// unwind するビルドで効く（`panic=abort` のビルドでは panic はそのまま abort になる）。
fn guard_driver_panic<T>(
    call: &str,
    f: impl FnOnce() -> Result<T, ContractError>,
) -> Result<T, ContractError> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(ContractError::DeviceUnavailable(format!(
            "{call}: driver call panicked"
        )))
    })
}

/// CUDA ドライバの共有ライブラリが読み込めるかを返す。`request_device` が context を作る前に呼ぶ。
fn driver_library_present() -> bool {
    // SAFETY: `is_culib_present` は CUDA の共有ライブラリの名前の候補を順に読み込み、1 つでも読めれば
    // その場で閉じて `true` を返す。ポインタを受けず、呼び出しの前に満たす不変条件は無い。
    // 読み込みが走らせる初期化子は、以後の `CudaContext::new` が読み込むものと同じライブラリのものだけ。
    unsafe { cudarc::driver::sys::is_culib_present() }
}

/// 要求を検査し、ドライバが読めれば device 0 の context と stream を作って `CudaDevice` を返す。
/// 作るまでの失敗はすべて `DeviceUnavailable`。`CudaDeviceProvider::request` から呼ばれる。
fn request_device(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
    check_request(&req)?;
    if !driver_library_present() {
        return Err(ContractError::DeviceUnavailable(
            "CudaContext::new(0): CUDA driver library is not loadable".into(),
        ));
    }
    guard_driver_panic("CudaContext::new(0)", || {
        let ctx = CudaContext::new(0)
            .map_err(|e| ContractError::DeviceUnavailable(format!("CudaContext::new(0): {e:?}")))?;
        let attributes = read_attributes(
            || ctx.name(),
            || ctx.compute_capability(),
            || ctx.total_mem(),
        )?;
        use cudarc::driver::sys::CUdevice_attribute as Attr;
        let grid_dims = read_grid_dims(|axis| {
            ctx.attribute(match axis {
                0 => Attr::CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_X,
                1 => Attr::CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Y,
                _ => Attr::CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Z,
            })
        })?;
        let stream = ctx.new_stream().map_err(|e| {
            ContractError::DeviceUnavailable(format!("CudaContext::new_stream: {e:?}"))
        })?;
        let exhausted =
            || ContractError::DeviceUnavailable("device counter space is exhausted".into());
        let id = take_next(&NEXT_DEVICE_ID).ok_or_else(exhausted)?;
        let generation = take_next(&NEXT_GENERATION).ok_or_else(exhausted)?;
        Ok(Box::new(CudaDevice {
            id,
            generation,
            ownership: DeviceOwnership::ProductIndependent,
            adapter_name: attributes.adapter_name,
            limits: LimitsTable {
                max_buffer_bytes: attributes.total_mem,
                max_dispatch_dims: grid_dims,
                max_texture_size: 0,
            },
            stream,
            buffers: Mutex::new(Buffers::default()),
        }) as Box<dyn Device>)
    })
}

/// CUDA の背後実装の `DeviceProvider`。
///
/// `request` は、背後実装が CUDA でない・`Graphics`・`CpuOnly` を `InvalidCombination` で、
/// `Shared`・`Imported` を `Unsupported` で拒む。検査を通った後は device を作り、ドライバが
/// 読めない・context や属性の取得に失敗したときは `DeviceUnavailable` を返す。
#[derive(Debug, Clone, Copy, Default)]
pub struct CudaDeviceProvider;

impl DeviceProvider for CudaDeviceProvider {
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        request_device(req)
    }
}

/// `alloc_zeros` の失敗を写す。メモリ不足は `AllocationFailed`、それ以外は `BackendFailure`。
fn allocation_failure(requested_bytes: u64, e: DriverError) -> ContractError {
    if e.0 == cudarc::driver::sys::CUresult::CUDA_ERROR_OUT_OF_MEMORY {
        ContractError::AllocationFailed { requested_bytes }
    } else {
        backend_failure("alloc_zeros", e)
    }
}

/// ドライバ呼び出しの失敗を `BackendFailure` に写す。
fn backend_failure(op: &'static str, e: impl std::fmt::Debug) -> ContractError {
    ContractError::BackendFailure {
        op,
        backend: BACKEND,
        detail: format!("{e:?}"),
    }
}

/// device が持つバッファの表。ハンドルの id から `CudaSlice` を引く。
#[derive(Default)]
struct Buffers {
    next_id: u64,
    map: BTreeMap<u64, CudaSlice<u8>>,
}

/// `request_device` が返す device。`Device`・`Capabilities`・`MemoryOps`・`CommandQueue` を実装する。
struct CudaDevice {
    id: u64,
    generation: u64,
    ownership: DeviceOwnership,
    adapter_name: String,
    limits: LimitsTable,
    stream: Arc<CudaStream>,
    buffers: Mutex<Buffers>,
}

impl CudaDevice {
    /// バッファの表のロックを取る。poison されていても中身を使う。
    fn lock(&self) -> MutexGuard<'_, Buffers> {
        self.buffers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// ハンドルの所属と範囲を検査して `[start, end)` を返す。`cudarc` の境界の panic に届く前に拒む。
    fn range(
        &self,
        table: &BTreeMap<u64, CudaSlice<u8>>,
        handle: &BufferHandle,
        offset: u64,
        len: u64,
    ) -> Result<(usize, usize), ContractError> {
        if handle.__backend_device_id() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        if handle.__backend_generation() != self.generation {
            return Err(ContractError::InvalidArgument(
                "buffer handle generation does not match the device".into(),
            ));
        }
        let slice = table
            .get(&handle.__backend_id())
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))?;
        validate_buffer_range(offset, len, slice.len() as u64)?;
        let start = usize::try_from(offset)
            .map_err(|_| ContractError::InvalidArgument("offset exceeds usize".into()))?;
        let len = usize::try_from(len)
            .map_err(|_| ContractError::InvalidArgument("length exceeds usize".into()))?;
        let end = start
            .checked_add(len)
            .ok_or_else(|| ContractError::InvalidArgument("range exceeds usize".into()))?;
        Ok((start, end))
    }

    /// バッファの範囲を host へ読む。同期まで同じ関数の中で終える。
    fn read(&self, src: &BufferHandle, offset: u64, len: u64) -> Result<Vec<u8>, ContractError> {
        let buffers = self.lock();
        let (start, end) = self.range(&buffers.map, src, offset, len)?;
        let mut out = vec![0_u8; end - start];
        if out.is_empty() {
            return Ok(out);
        }
        let slice = buffers
            .map
            .get(&src.__backend_id())
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))?;
        self.stream
            .memcpy_dtoh(&slice.slice(start..end), out.as_mut_slice())
            .and_then(|()| self.stream.synchronize())
            .map_err(|e| backend_failure("memcpy_dtoh", e))?;
        Ok(out)
    }
}

impl Capabilities for CudaDevice {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::Unsupported
    }

    fn limits(&self) -> &LimitsTable {
        &self.limits
    }

    fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
        CapabilityQuery {
            level: CapabilityLevel::Unsupported,
            unsupported_reason: Some(UnsupportedReason::UnknownFeature),
            evidence: None,
        }
    }
}

impl Device for CudaDevice {
    fn backend(&self) -> BackendKind {
        BACKEND
    }
    fn ownership(&self) -> &DeviceOwnership {
        &self.ownership
    }
    fn generation(&self) -> u64 {
        self.generation
    }
    fn memory(&self) -> &dyn MemoryOps {
        self
    }
    fn queue(&self) -> &dyn CommandQueue {
        self
    }
    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: self.adapter_name.clone(),
            backend_name: BACKEND_NAME,
        }
    }
}

impl MemoryOps for CudaDevice {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        if byte_len == 0 {
            return Err(ContractError::InvalidArgument(
                "allocation length must not be zero".into(),
            ));
        }
        validate_allocation_size(byte_len, self.limits.max_buffer_bytes)?;
        let len = usize::try_from(byte_len)
            .map_err(|_| ContractError::InvalidArgument("length exceeds usize".into()))?;
        let slice = self
            .stream
            .alloc_zeros::<u8>(len)
            .map_err(|e| allocation_failure(byte_len, e))?;
        self.stream
            .synchronize()
            .map_err(|e| allocation_failure(byte_len, e))?;
        let mut buffers = self.lock();
        buffers.next_id = buffers
            .next_id
            .checked_add(1)
            .ok_or_else(|| ContractError::InvalidArgument("buffer ids exhausted".into()))?;
        let id = buffers.next_id;
        buffers.map.insert(id, slice);
        Ok(BufferHandle::__backend_new(
            self.generation,
            id,
            self.id,
            byte_len,
            usage,
        ))
    }

    fn upload(
        &self,
        dst: &BufferHandle,
        dst_offset: u64,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        let mut buffers = self.lock();
        let (start, end) = self.range(&buffers.map, dst, dst_offset, bytes.len() as u64)?;
        if bytes.is_empty() {
            return Ok(());
        }
        let slice = buffers
            .map
            .get_mut(&dst.__backend_id())
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))?;
        self.stream
            .memcpy_htod(bytes, &mut slice.slice_mut(start..end))
            .and_then(|()| self.stream.synchronize())
            .map_err(|e| backend_failure("memcpy_htod", e))
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        self.read(src, src_offset, byte_len)
    }

    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        // ロックは `read` の中で離れているので、コールバックはロックの外で呼ばれる。
        callback(self.read(src, src_offset, byte_len));
    }

    fn free(&self, buffer: BufferHandle) {
        if buffer.__backend_device_id() != self.id
            || buffer.__backend_generation() != self.generation
        {
            return;
        }
        self.lock().map.remove(&buffer.__backend_id());
    }
}

impl CommandQueue for CudaDevice {
    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(__backend_next_encoder_id().unwrap_or(0), None)
    }
    fn submit(
        &self,
        _commands: CommandEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Err(ContractError::Unsupported {
            op: "submit",
            backend: BACKEND,
        })
    }
    fn submit_many(
        &self,
        _commands: Vec<CommandEncoder>,
        _after: &[SubmitToken],
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Err(ContractError::Unsupported {
            op: "submit_many",
            backend: BACKEND,
        })
    }
    fn poll(&self, _token: SubmitToken) -> CompletionState {
        CompletionState::Lost
    }
    fn poll_ex(&self, _token: SubmitToken) -> CompletionStateEx {
        CompletionStateEx::Unknown
    }
    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        callback(CompletionState::Lost);
    }
    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        _token: SubmitToken,
        _timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        Err(ContractError::Unsupported {
            op: "wait_blocking",
            backend: BACKEND,
        })
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
            backend: BACKEND,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ドライバのライブラリが読めない環境では、`request` は panic せず `DeviceUnavailable` を返す
    /// （DEV-213）。読める環境では device に触らず戻る。
    #[test]
    fn absent_library_is_device_unavailable_without_panicking() {
        if driver_library_present() {
            println!("not-run: CUDA driver library is present");
            return;
        }
        let r = CudaDeviceProvider::request(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        });
        assert!(matches!(r, Err(ContractError::DeviceUnavailable(_))));
    }

    #[test]
    fn a_panic_in_the_guard_is_device_unavailable() {
        let r: Result<(), _> = guard_driver_panic("call", || panic!("boom"));
        assert!(matches!(r, Err(ContractError::DeviceUnavailable(_))));
    }

    #[test]
    fn grid_dims_come_from_attributes_and_reject_failures() {
        let ok = read_grid_dims(|a| Ok::<_, &str>([2_147_483_647, 65_535, 65_534][a]));
        assert_eq!(ok.ok(), Some([2_147_483_647, 65_535, 65_534]));
        assert!(matches!(
            read_grid_dims(|a| if a == 1 { Err("x") } else { Ok(1) }),
            Err(ContractError::DeviceUnavailable(_))
        ));
        assert!(matches!(
            read_grid_dims(|_| Ok::<_, &str>(-1)),
            Err(ContractError::DeviceUnavailable(_))
        ));
    }

    #[test]
    fn attribute_failures_are_device_unavailable() {
        let ok_name = || Ok::<_, &str>("GPU".to_string());
        let ok_cap = || Ok::<_, &str>((12, 1));
        let ok_mem = || Ok::<_, &str>(1024_usize);
        let a = read_attributes(ok_name, ok_cap, ok_mem).ok().unwrap();
        assert_eq!(a.adapter_name, "GPU (cuda compute_capability=12.1)");
        assert!(matches!(
            read_attributes(|| Err("x"), ok_cap, ok_mem),
            Err(ContractError::DeviceUnavailable(_))
        ));
        assert!(matches!(
            read_attributes(ok_name, || Err("x"), ok_mem),
            Err(ContractError::DeviceUnavailable(_))
        ));
        assert!(matches!(
            read_attributes(ok_name, ok_cap, || Err("x")),
            Err(ContractError::DeviceUnavailable(_))
        ));
    }
}
