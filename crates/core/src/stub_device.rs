//! feature `test-support` を有効にしたときだけ公開される、試験用の device と provider。
//!
//! GPU も背後実装も使わず、ホストのメモリだけで `Device`・`MemoryOps`・`CommandQueue` を満たす（TASK-156 のポインタ）。
//! 製品の経路からは使わない。メモリ・投入・compute・描画の口は本 module の中の非公開の型で、`StubDevice` 越しにだけ届く。
//!
//! `Device::compute()` は要求のプロファイルで答えが変わる。`CpuOnly` は `None`、`ComputeOnly`・`Graphics` は `Some` を返す
//! （所有形態によらない。TASK-2956 のポインタ）。
//!
//! `Device::textures()` も要求のプロファイルで答えが変わる。`CpuOnly` は `None`、`ComputeOnly`・`Graphics` は `Some` を返す
//! （所有形態によらない。TASK-150 のポインタ）。
//!
//! feature `graphics` のビルドでは `Device::graphics()` も要求のプロファイルで答えが変わる。`CpuOnly`・`ComputeOnly` は `None`、
//! `Graphics` は `Some` を返す（所有形態によらない。TASK-837 のポインタ）。描画の口は描画先の handle とパイプラインの
//! device の識別子を `render_target::check_render_target_device` などで照らす（TASK-375・TEX-12 のポインタ）。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
    WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT, WEBGPU_PORT_UNSUPPORTED,
};
use crate::capability_table::{FeatureRow, query_from_table};
use crate::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use crate::error::{CompletionState, ContractError, UnsupportedReason};
use crate::execution::{
    __BackendSharedEncoderCloser, __backend_next_encoder_id, Binding, BindingLayout,
    CommandEncoder, CommandQueue, ComputeOps, ConstantBinding, ImportedSemaphoreHandle,
    PipelineHandle, PipelineInput, SubmitToken, ThreadgroupMemory,
};
#[cfg(feature = "graphics")]
use crate::graphics::{DepthConfig, DepthFormat, GraphicsOps, PixelFormat, RenderEncoder};
use crate::memory::{
    BufferHandle, BufferUsage, MemoryOps, validate_allocation_size, validate_buffer_range,
};
#[cfg(feature = "graphics")]
use crate::render_target::{RenderTargetHandle, check_render_target_device};
use crate::sampler::{SamplerDescriptor, SamplerHandle};
use crate::texture::{TextureDescriptor, TextureHandle, TextureOps, TextureView};
use crate::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;

/// 試験用の device の上限の表。確保の上限だけ持ち、dispatch・テクスチャは 0。
const LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1 << 28,
    max_dispatch_dims: [0, 0, 0],
    max_texture_size: 0,
};

/// 形式の問合せの鍵（`TEXTURE_FORMAT_QUERY_KEYS`）の件数。表の長さに使う。
const TEXTURE_FORMAT_KEY_COUNT: usize = TEXTURE_FORMAT_QUERY_KEYS.len();

/// 形式の問合せの鍵の行に持たせる値。`Unsupported`・理由 `NotSupported`・根拠なし。
const TEXTURE_FORMAT_ROW_QUERY: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::Unsupported,
    unsupported_reason: Some(UnsupportedReason::NotSupported),
    evidence: None,
};

/// 形式の問合せの鍵ごとの行。鍵は `TEXTURE_FORMAT_QUERY_KEYS` の並びのまま。
/// `StubDevice::query_ex` が `query_from_table` で引く。
static TEXTURE_FORMAT_QUERY_TABLE: [FeatureRow<CapabilityQuery>; TEXTURE_FORMAT_KEY_COUNT] =
    texture_format_query_table();

/// 鍵の一覧から `TEXTURE_FORMAT_QUERY_TABLE` の行を作る。鍵を手で書き写さないための関数。
const fn texture_format_query_table() -> [FeatureRow<CapabilityQuery>; TEXTURE_FORMAT_KEY_COUNT] {
    let mut rows = [FeatureRow {
        feature: FeatureId(""),
        rule: TEXTURE_FORMAT_ROW_QUERY,
        source: "stub",
    }; TEXTURE_FORMAT_KEY_COUNT];
    let mut i = 0;
    while i < TEXTURE_FORMAT_KEY_COUNT {
        rows[i].feature = TEXTURE_FORMAT_QUERY_KEYS[i];
        i += 1;
    }
    rows
}

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

/// 投入の印を 1 つ払い出す。queue と描画の口が同じ源を共有し、尽きたら `BackendFailure { op, .. }` を返す。
fn issue_token(
    sequence: &AtomicU64,
    backend: BackendKind,
    id: u64,
    op: &'static str,
) -> Result<SubmitToken, ContractError> {
    let mut current = sequence.load(Ordering::Relaxed);
    loop {
        let Some(next) = current.checked_add(1) else {
            return Err(ContractError::BackendFailure {
                op,
                backend,
                detail: "submission sequence space is exhausted".to_string(),
            });
        };
        match sequence.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Ok(SubmitToken::__backend_new(id, current, id)),
            Err(actual) => current = actual,
        }
    }
}

/// 投入を即時に完了済みとして扱う `CommandQueue`。`StubDevice` が持つ。
struct StubQueue {
    backend: BackendKind,
    id: u64,
    open: Arc<OpenEncoders>,
    next_sequence: Arc<AtomicU64>,
}

impl StubQueue {
    fn new(backend: BackendKind, id: u64) -> Self {
        Self {
            backend,
            id,
            open: Arc::new(OpenEncoders {
                ids: Mutex::new(HashSet::new()),
            }),
            next_sequence: Arc::new(AtomicU64::new(1)),
        }
    }

    /// 投入の印を 1 つ払い出す。
    fn issue(&self) -> Result<SubmitToken, ContractError> {
        issue_token(&self.next_sequence, self.backend, self.id, "submit")
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
        if token.sequence == 0 || token.sequence >= self.next_sequence.load(Ordering::Relaxed) {
            return Err(ContractError::InvalidArgument(
                "submit token sequence was not issued by this queue".to_string(),
            ));
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
        // 印の検査は `poll` が Lost を返す条件と同じ `check_token` に一本化している。
        // Lost になる印は、ここで種類別のエラー（DeviceMismatch / DeviceLost / InvalidArgument）になる。
        self.check_token(&token)?;
        debug_assert_eq!(self.poll(token), CompletionState::Complete);
        Ok(())
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

/// パイプラインの印を払い出す `ComputeOps`。`StubDevice` が持つ。
///
/// 成果物と slot の並びは検査も解釈もしない。印の device の識別子は device の `generation()` と同じ値で、
/// 通し番号は device ごとに 1 から増える。尽きたら `BackendFailure { op: "create_pipeline", .. }` を返す（panic しない）。
struct StubCompute {
    backend: BackendKind,
    id: u64,
    next_pipeline: AtomicU64,
}

impl StubCompute {
    fn new(backend: BackendKind, id: u64) -> Self {
        Self {
            backend,
            id,
            next_pipeline: AtomicU64::new(1),
        }
    }
}

impl ComputeOps for StubCompute {
    fn create_pipeline(
        &self,
        _input: PipelineInput<'_>,
        _layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        let mut current = self.next_pipeline.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_add(1) else {
                return Err(ContractError::BackendFailure {
                    op: "create_pipeline",
                    backend: self.backend,
                    detail: "pipeline id space is exhausted".to_string(),
                });
            };
            match self.next_pipeline.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(PipelineHandle::__backend_new(current, self.id)),
                Err(actual) => current = actual,
            }
        }
    }
}

/// テクスチャの handle を払い出す `TextureOps`。`StubDevice` が持つ。
///
/// 記述子は検査せず、handle の device の識別子と世代は device の `generation()` と同じ値で、
/// 通し番号は device ごとに 1 から増える。尽きたら `BackendFailure { op: "create_texture", .. }` を返す（panic しない）。
/// 作ったテクスチャを表に持ち、`destroy_texture` で表から除く。別 device の handle・作っていない handle の破棄と
/// `destroy_view` は何もしない。破棄した識別子は再び払い出さない。
/// `create_sampler` も記述子を検査せず、handle の device の識別子と世代は `generation()` と同じ値で、
/// 通し番号はテクスチャとは別に device ごとに 1 から増える。尽きたら `BackendFailure { op: "create_sampler", .. }` を返す（panic しない）。
struct StubTextures {
    backend: BackendKind,
    id: u64,
    next_texture: AtomicU64,
    /// サンプラーの通し番号の源（テクスチャとは別）。
    next_sampler: AtomicU64,
    /// 作って破棄していないテクスチャの識別子。
    live: Mutex<HashSet<u64>>,
}

impl StubTextures {
    fn new(backend: BackendKind, id: u64) -> Self {
        Self {
            backend,
            id,
            next_texture: AtomicU64::new(1),
            next_sampler: AtomicU64::new(1),
            live: Mutex::new(HashSet::new()),
        }
    }
}

impl TextureOps for StubTextures {
    fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Result<TextureHandle, ContractError> {
        let mut current = self.next_texture.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_add(1) else {
                return Err(ContractError::BackendFailure {
                    op: "create_texture",
                    backend: self.backend,
                    detail: "texture id space is exhausted".to_string(),
                });
            };
            match self.next_texture.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    lock(&self.live).insert(current);
                    return Ok(TextureHandle::__backend_new(
                        self.id, current, self.id, desc,
                    ));
                }
                Err(actual) => current = actual,
            }
        }
    }

    fn destroy_texture(&self, texture: TextureHandle) {
        if texture.__backend_device_id() == self.id && texture.__backend_generation() == self.id {
            lock(&self.live).remove(&texture.__backend_id());
        }
    }

    fn destroy_view<'t>(&self, view: TextureView<'t>) {
        let _ = view;
    }

    fn create_sampler(&self, desc: &SamplerDescriptor<'_>) -> Result<SamplerHandle, ContractError> {
        let _ = desc;
        let mut current = self.next_sampler.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_add(1) else {
                return Err(ContractError::BackendFailure {
                    op: "create_sampler",
                    backend: self.backend,
                    detail: "sampler id space is exhausted".to_string(),
                });
            };
            match self.next_sampler.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(SamplerHandle::__backend_new(self.id, current, self.id)),
                Err(actual) => current = actual,
            }
        }
    }
}

/// 描画の操作を満たす `GraphicsOps`。`StubDevice` が feature `graphics` のビルドで持つ。
///
/// 入力・配置・形式・大きさは検査も解釈もせず、印の device の識別子は device の `generation()` と同じ値にする。
/// パイプライン・描画先・記録先の通し番号は 1 つの源から 1 から増え、尽きたら `BackendFailure` を返す（panic しない）。
/// 描画先の handle（`create_render_encoder*` の描画先、`copy_target_to_buffer` の描画先）と `draw`・`draw_ex` のパイプラインは
/// `check_render_target_device` などで device の識別子を照らし、違えば `DeviceMismatch` を返す。
/// `create_render_encoder*` は通し番号を取らず、`DeviceMismatch` を持つ識別子 `0` の記録先を返し、
/// `draw`・`draw_ex`・`submit_render` がその値を返す。照合を通ったときの `draw`・`draw_ex`・`copy_target_to_buffer` は何もせず `Unsupported` を返す。
/// `submit_render` の印は `CommandQueue::poll` で `Complete` になる。
/// 作っていない描画先・破棄した描画先を渡すと `InvalidArgument` を返す（記録先の生成ではその値を持つ記録先を返す）。
/// 破棄した描画先を使う記録先を `draw`・`draw_ex`・`submit_render` に渡すと `InvalidArgument` を返す。別の device で作った記録先を `draw`・`draw_ex`・`submit_render` に渡すと `DeviceMismatch` を返す。
/// `submit_render` に渡さずに捨てた記録先の分の記録は残る（試験用で、後始末の経路は持たない）。
/// 別の device の描画先の破棄は何もしない。
#[cfg(feature = "graphics")]
struct StubGraphics {
    backend: BackendKind,
    id: u64,
    next_serial: AtomicU64,
    sequence: Arc<AtomicU64>,
    /// 生きている描画先の識別子。
    targets: Mutex<HashSet<u64>>,
    /// 記録先の識別子から、その記録先が使う描画先の識別子の列。
    recorded: Mutex<HashMap<u64, Vec<u64>>>,
}

#[cfg(feature = "graphics")]
impl StubGraphics {
    fn new(backend: BackendKind, id: u64, sequence: Arc<AtomicU64>) -> Self {
        Self {
            backend,
            id,
            next_serial: AtomicU64::new(1),
            sequence,
            targets: Mutex::new(HashSet::new()),
            recorded: Mutex::new(HashMap::new()),
        }
    }

    fn is_live(&self, target: &RenderTargetHandle) -> bool {
        lock(&self.targets).contains(&target.__backend_id())
    }

    fn not_live() -> ContractError {
        ContractError::InvalidArgument("render target is not live".to_string())
    }

    /// device の識別子の照合の後に、描画先が生きていることを照らす。
    fn check_target(&self, target: &RenderTargetHandle) -> Result<(), ContractError> {
        check_render_target_device(target, self.id, self.id, false)?;
        if self.is_live(target) {
            Ok(())
        } else {
            Err(Self::not_live())
        }
    }

    /// 通し番号を 1 つ取る。尽きたら `None`。
    fn next(&self) -> Option<u64> {
        let mut current = self.next_serial.load(Ordering::Relaxed);
        loop {
            let next = current.checked_add(1)?;
            match self.next_serial.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(current),
                Err(actual) => current = actual,
            }
        }
    }

    fn exhausted(&self, op: &'static str) -> ContractError {
        ContractError::BackendFailure {
            op,
            backend: self.backend,
            detail: "graphics id space is exhausted".to_string(),
        }
    }

    fn pipeline(&self, op: &'static str) -> Result<PipelineHandle, ContractError> {
        let serial = self.next().ok_or_else(|| self.exhausted(op))?;
        Ok(PipelineHandle::__backend_new(serial, self.id))
    }

    fn target(&self, op: &'static str) -> Result<RenderTargetHandle, ContractError> {
        let serial = self.next().ok_or_else(|| self.exhausted(op))?;
        Ok(RenderTargetHandle::__backend_new(self.id, serial, self.id))
    }

    /// 描画先を 1 つ作って表に入れる。
    fn live_target(&self, op: &'static str) -> Result<RenderTargetHandle, ContractError> {
        let handle = self.target(op)?;
        lock(&self.targets).insert(handle.__backend_id());
        Ok(handle)
    }

    /// `targets` を順に照らし、最初の不一致の値を持つ識別子 `0` の記録先を返す。通れば `encoder(op)`。
    fn checked_encoder(&self, targets: &[&RenderTargetHandle], op: &'static str) -> RenderEncoder {
        for target in targets {
            if let Err(e) = self.check_target(target) {
                return RenderEncoder::__backend_new_rejected(self.id, 0, self.id, e);
            }
        }
        let encoder = self.encoder(op);
        if encoder.__backend_rejected().is_none() {
            let ids = targets.iter().map(|t| t.__backend_id()).collect();
            lock(&self.recorded).insert(encoder.__backend_id(), ids);
        }
        encoder
    }

    /// 記録先が持つ拒否の値、無ければパイプラインの device の識別子の不一致を返す。
    fn check_draw(
        &self,
        encoder: &RenderEncoder,
        pipeline: &PipelineHandle,
    ) -> Result<(), ContractError> {
        if let Some(err) = encoder.__backend_rejected() {
            return Err(err.clone());
        }
        if pipeline.__backend_device_id() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        if encoder.__backend_device_id() != self.id || encoder.__backend_generation() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        self.check_recorded_live(encoder)
    }

    /// 記録先が使う描画先がすべて生きていることを照らす（破棄済みは `InvalidArgument`）。
    fn check_recorded_live(&self, encoder: &RenderEncoder) -> Result<(), ContractError> {
        if let Some(ids) = lock(&self.recorded).get(&encoder.__backend_id()) {
            let targets = lock(&self.targets);
            if ids.iter().any(|id| !targets.contains(id)) {
                return Err(Self::not_live());
            }
        }
        Ok(())
    }

    fn encoder(&self, op: &'static str) -> RenderEncoder {
        match self.next() {
            Some(serial) => RenderEncoder::__backend_new(self.id, serial, self.id),
            None => RenderEncoder::__backend_new_rejected(self.id, 0, self.id, self.exhausted(op)),
        }
    }
}

#[cfg(feature = "graphics")]
impl GraphicsOps for StubGraphics {
    fn create_render_pipeline(
        &self,
        _vs: PipelineInput<'_>,
        _fs: PipelineInput<'_>,
        _layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        self.pipeline("create_render_pipeline")
    }

    fn create_render_pipeline_with_depth(
        &self,
        _vs: PipelineInput<'_>,
        _fs: PipelineInput<'_>,
        _layout: &BindingLayout,
        _depth_format: DepthFormat,
    ) -> Result<PipelineHandle, ContractError> {
        self.pipeline("create_render_pipeline_with_depth")
    }

    fn create_offscreen_target(
        &self,
        _width: u32,
        _height: u32,
        _format: PixelFormat,
    ) -> Result<RenderTargetHandle, ContractError> {
        self.live_target("create_offscreen_target")
    }

    fn create_offscreen_target_with_depth(
        &self,
        _width: u32,
        _height: u32,
        _color_format: PixelFormat,
        _depth: DepthConfig,
    ) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError> {
        let op = "create_offscreen_target_with_depth";
        let color = self.target(op)?;
        let depth_target = self.target(op)?;
        let mut targets = lock(&self.targets);
        targets.insert(color.__backend_id());
        targets.insert(depth_target.__backend_id());
        Ok((color, depth_target))
    }

    fn destroy_offscreen_target(&self, target: RenderTargetHandle) {
        if check_render_target_device(&target, self.id, self.id, false).is_ok() {
            lock(&self.targets).remove(&target.__backend_id());
        }
    }

    fn create_render_encoder(&self, target: &RenderTargetHandle) -> RenderEncoder {
        self.checked_encoder(&[target], "create_render_encoder")
    }

    fn create_render_encoder_with_depth(
        &self,
        target: &RenderTargetHandle,
        depth_target: &RenderTargetHandle,
        _clear_color: [f32; 4],
    ) -> RenderEncoder {
        self.checked_encoder(&[target, depth_target], "create_render_encoder_with_depth")
    }

    fn draw(
        &self,
        encoder: &mut RenderEncoder,
        pipeline: &PipelineHandle,
        _vertex_buffer: &BufferHandle,
        _vertex_count: u32,
    ) -> Result<(), ContractError> {
        self.check_draw(encoder, pipeline)?;
        Err(ContractError::Unsupported {
            op: "draw",
            backend: self.backend,
        })
    }

    fn draw_ex(
        &self,
        encoder: &mut RenderEncoder,
        pipeline: &PipelineHandle,
        _vertex_buffer: &BufferHandle,
        _vertex_count: u32,
        _vertex_constants: &[ConstantBinding<'_>],
    ) -> Result<(), ContractError> {
        self.check_draw(encoder, pipeline)?;
        Err(ContractError::Unsupported {
            op: "draw_ex",
            backend: self.backend,
        })
    }

    fn submit_render(
        &self,
        encoder: RenderEncoder,
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        if let Some(err) = encoder.__backend_rejected() {
            return Err(err.clone());
        }
        if encoder.__backend_device_id() != self.id || encoder.__backend_generation() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        let recorded = lock(&self.recorded).remove(&encoder.__backend_id());
        if let Some(ids) = recorded {
            let targets = lock(&self.targets);
            if ids.iter().any(|id| !targets.contains(id)) {
                return Err(Self::not_live());
            }
        }
        if !wait.is_empty() {
            return Err(ContractError::Unsupported {
                op: "submit_render(wait)",
                backend: self.backend,
            });
        }
        issue_token(&self.sequence, self.backend, self.id, "submit_render")
    }

    fn copy_target_to_buffer(
        &self,
        _encoder: &mut CommandEncoder,
        target: &RenderTargetHandle,
        _dst: &BufferHandle,
        _dst_offset: u64,
    ) -> Result<(), ContractError> {
        self.check_target(target)?;
        Err(ContractError::Unsupported {
            op: "copy_target_to_buffer",
            backend: self.backend,
        })
    }
}

/// GPU も背後実装も使わない試験用の device。
///
/// 要求の `backend` と所有形態をそのまま持つ。
/// 能力問合せは `query` がどの機能も `Unsupported` を返し、`query_ex` は鍵で返す値が変わる。
/// WebGPU の口の 2 鍵には `WEBGPU_PORT_UNSUPPORTED`、形式の問合せの鍵には `Unsupported`・理由 `NotSupported`・根拠なし、
/// それ以外の鍵には `UNLISTED_FEATURE_QUERY` を返す。
/// メモリはホストの `Vec<u8>`、投入は即時に完了済みとして扱う。
/// `compute()` は要求の `profile` で変わり、`CpuOnly` は `None`、`ComputeOnly`・`Graphics` は `Some` を返す。
/// `Some` の `create_pipeline` は `Ok(PipelineHandle)` を返し、印の device の識別子は `generation()` と等しい。
/// `textures()` も `profile` で変わり、`CpuOnly` は `None`、`ComputeOnly`・`Graphics` は `Some` を返す。
/// `Some` の `create_texture` は記述子を検査せず `Ok(TextureHandle)` を返し、handle の device の識別子は `generation()` と等しい。
/// `Some` の `destroy_texture` は作ったテクスチャを表から除き、別 device・作っていないテクスチャの破棄と `destroy_view` は何もしない。
/// `Some` の `create_sampler` は記述子を検査せず `Ok(SamplerHandle)` を返し、handle の device の識別子は `generation()` と等しい。
///
/// feature `graphics` のビルドでは `graphics()` も `profile` で変わり、`CpuOnly`・`ComputeOnly` は `None`、`Graphics` は `Some` を返す。
/// `Some` の描画パイプライン・描画先の生成は入力を検査せず `Ok` を返し、印の device の識別子は `generation()` と等しい。
/// 別 device の描画先やパイプラインを渡すと `DeviceMismatch` を返す（記録先の生成では、その値を持つ記録先を返す）。
/// `submit_render` の印は `queue()` の `poll` で `Complete` になる。照合を通った `draw`・`draw_ex`・`copy_target_to_buffer` と、
/// 待ちつきの `submit_render` は `Unsupported` を返す。通し番号が尽きたら `BackendFailure` を返す（記録先は拒否の値を持つ）。
/// 作っていない描画先・破棄した描画先を渡すと `InvalidArgument` を返す。破棄した描画先を使う記録先を `draw`・`draw_ex`・`submit_render` に渡しても同じ。別 device で作った記録先を `draw`・`draw_ex`・`submit_render` に渡すと `DeviceMismatch` を返す。
pub struct StubDevice {
    backend: BackendKind,
    ownership: DeviceOwnership,
    profile: Profile,
    id: u64,
    memory: StubMemory,
    queue: StubQueue,
    compute: StubCompute,
    textures: StubTextures,
    #[cfg(feature = "graphics")]
    graphics: StubGraphics,
}

impl StubDevice {
    /// 要求から device を作る。組み合わせの検査はせず、どの要求でも成功する。
    pub fn new(req: DeviceRequest) -> Self {
        let id = next_id();
        let queue = StubQueue::new(req.backend, id);
        #[cfg(feature = "graphics")]
        let graphics = StubGraphics::new(req.backend, id, Arc::clone(&queue.next_sequence));
        Self {
            backend: req.backend,
            ownership: req.ownership,
            profile: req.profile,
            id,
            memory: StubMemory::new(id),
            queue,
            compute: StubCompute::new(req.backend, id),
            textures: StubTextures::new(req.backend, id),
            #[cfg(feature = "graphics")]
            graphics,
        }
    }
}

impl Capabilities for StubDevice {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::Unsupported
    }

    /// WebGPU の口の 2 つの鍵には背後実装と同じ値（`WEBGPU_PORT_UNSUPPORTED`）を返し、
    /// 共通の検査 `check_webgpu_ports` を通す。形式の問合せの鍵（`TEXTURE_FORMAT_QUERY_KEYS`）には
    /// `Unsupported`・理由 `NotSupported`・根拠なしを返す。それ以外の鍵には `UNLISTED_FEATURE_QUERY` を返す。
    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        if feature == WEBGPU_DEVICE_PORT || feature == WEBGPU_HANDLE_PORT {
            return WEBGPU_PORT_UNSUPPORTED;
        }
        query_from_table(&TEXTURE_FORMAT_QUERY_TABLE, feature, |q| *q)
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

    fn compute(&self) -> Option<&dyn ComputeOps> {
        match self.profile {
            Profile::CpuOnly => None,
            Profile::ComputeOnly | Profile::Graphics => Some(&self.compute),
        }
    }

    fn textures(&self) -> Option<&dyn TextureOps> {
        match self.profile {
            Profile::CpuOnly => None,
            Profile::ComputeOnly | Profile::Graphics => Some(&self.textures),
        }
    }

    #[cfg(feature = "graphics")]
    fn graphics(&self) -> Option<&dyn GraphicsOps> {
        match self.profile {
            Profile::CpuOnly => None,
            Profile::ComputeOnly => None,
            Profile::Graphics => Some(&self.graphics),
        }
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
    use crate::texture::{TextureDimension, TextureExtent, TextureUsage};
    use crate::texture_format::TextureFormat;

    fn request() -> DeviceRequest {
        DeviceRequest {
            backend: BackendKind::Vulkan,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::Shared,
        }
    }

    use crate::capability_table::UNLISTED_FEATURE_QUERY;
    use crate::capability_table_check::{check_table_keys, check_table_queries};

    fn table_device() -> StubDevice {
        StubDevice::new(request())
    }

    #[test]
    fn texture_format_table_follows_query_key_list() {
        assert_eq!(
            TEXTURE_FORMAT_QUERY_TABLE.len(),
            TEXTURE_FORMAT_QUERY_KEYS.len()
        );
        for (row, key) in TEXTURE_FORMAT_QUERY_TABLE
            .iter()
            .zip(TEXTURE_FORMAT_QUERY_KEYS)
        {
            assert_eq!(row.feature, *key);
            assert_eq!(row.source, "stub");
        }
    }

    #[test]
    fn texture_format_table_keys_and_queries_pass_shared_checks() {
        assert_eq!(
            check_table_keys(&TEXTURE_FORMAT_QUERY_TABLE, TEXTURE_FORMAT_QUERY_KEYS),
            Ok(())
        );
        assert_eq!(
            check_table_queries(
                &TEXTURE_FORMAT_QUERY_TABLE,
                TEXTURE_FORMAT_QUERY_KEYS,
                |q| *q
            ),
            Ok(())
        );
    }

    #[test]
    fn query_ex_returns_row_value_for_texture_format_keys() {
        let device = table_device();
        for key in TEXTURE_FORMAT_QUERY_KEYS {
            let q = device.query_ex(*key);
            assert_eq!(q.level, CapabilityLevel::Unsupported);
            assert_eq!(q.unsupported_reason, Some(UnsupportedReason::NotSupported));
            assert_eq!(q.evidence, None);
            assert_ne!(q, UNLISTED_FEATURE_QUERY);
        }
    }

    #[test]
    fn query_ex_returns_unlisted_value_for_keys_without_rows() {
        let device = table_device();
        for key in ["texformat:Astc", "", "any-feature"] {
            assert_eq!(device.query_ex(FeatureId(key)), UNLISTED_FEATURE_QUERY);
        }
    }

    #[test]
    fn query_level_matches_query_ex_level() {
        let device = table_device();
        let mut keys: Vec<FeatureId> = TEXTURE_FORMAT_QUERY_KEYS.to_vec();
        keys.extend([
            FeatureId("texformat:Astc"),
            FeatureId("any-feature"),
            WEBGPU_DEVICE_PORT,
            WEBGPU_HANDLE_PORT,
        ]);
        for key in keys {
            assert_eq!(device.query(key), device.query_ex(key).level);
        }
    }

    #[test]
    fn webgpu_port_keys_are_not_texture_format_keys() {
        let device = table_device();
        for key in [WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT] {
            assert!(!TEXTURE_FORMAT_QUERY_KEYS.contains(&key));
            assert_eq!(device.query_ex(key), WEBGPU_PORT_UNSUPPORTED);
        }
    }

    #[test]
    fn create_pipeline_reports_exhausted_id_space() {
        let compute = StubCompute::new(BackendKind::Vulkan, 5);
        compute.next_pipeline.store(u64::MAX - 1, Ordering::Relaxed);
        let input = || PipelineInput::Msl {
            source: "",
            entry_point: "main",
        };
        let layout = BindingLayout::new(vec![]);
        let handle = compute.create_pipeline(input(), &layout).expect("pipeline");
        assert_eq!(handle.__backend_id(), u64::MAX - 1);
        assert_eq!(handle.__backend_device_id(), 5);
        assert!(matches!(
            compute.create_pipeline(input(), &layout),
            Err(ContractError::BackendFailure {
                op: "create_pipeline",
                ..
            })
        ));
    }

    fn texture_desc() -> TextureDescriptor<'static> {
        TextureDescriptor::new(
            TextureDimension::D2,
            TextureExtent {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            TextureFormat::Rgba8Unorm,
            TextureUsage::TEXTURE_BINDING,
        )
    }

    fn request_with(profile: Profile) -> DeviceRequest {
        DeviceRequest {
            profile,
            ..request()
        }
    }

    #[test]
    fn textures_is_none_for_cpu_only() {
        let device = StubDevice::new(request_with(Profile::CpuOnly));
        assert!(device.textures().is_none());
    }

    #[test]
    fn textures_is_some_for_compute_only_and_graphics() {
        for profile in [Profile::ComputeOnly, Profile::Graphics] {
            let device = StubDevice::new(request_with(profile));
            assert!(device.textures().is_some());
        }
    }

    // DEV-153: 通し番号は 1 から増え、device の識別子と世代は generation() と等しい。
    #[test]
    fn create_texture_numbers_from_one_and_uses_generation() {
        let device = StubDevice::new(request());
        let textures = device.textures().expect("textures");
        let desc = texture_desc();
        let a = textures.create_texture(&desc).expect("a");
        let b = textures.create_texture(&desc).expect("b");
        assert_eq!(a.__backend_id(), 1);
        assert_eq!(b.__backend_id(), 2);
        for h in [&a, &b] {
            assert_eq!(h.__backend_device_id(), device.generation());
            assert_eq!(h.__backend_generation(), device.generation());
        }
    }

    #[test]
    fn create_texture_reports_exhausted_id_space() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        textures.next_texture.store(u64::MAX - 1, Ordering::Relaxed);
        let desc = texture_desc();
        let handle = textures.create_texture(&desc).expect("texture");
        assert_eq!(handle.__backend_id(), u64::MAX - 1);
        assert_eq!(handle.__backend_device_id(), 5);
        assert!(matches!(
            textures.create_texture(&desc),
            Err(ContractError::BackendFailure {
                op: "create_texture",
                ..
            })
        ));
    }

    // TEX-19
    #[test]
    fn destroy_texture_removes_own_entry() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        let h = textures.create_texture(&texture_desc()).expect("texture");
        let id = h.__backend_id();
        assert!(lock(&textures.live).contains(&id));
        textures.destroy_texture(h);
        assert!(!lock(&textures.live).contains(&id));
    }

    // TEX-19
    #[test]
    fn destroy_texture_of_other_device_keeps_own_entry() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        let desc = texture_desc();
        let h = textures.create_texture(&desc).expect("texture");
        let id = h.__backend_id();
        textures.destroy_texture(TextureHandle::__backend_new(9, id, 9, &desc));
        assert!(lock(&textures.live).contains(&id));
    }

    // TEX-19
    #[test]
    fn destroy_texture_with_other_generation_keeps_own_entry() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        let desc = texture_desc();
        let h = textures.create_texture(&desc).expect("texture");
        let id = h.__backend_id();
        textures.destroy_texture(TextureHandle::__backend_new(5, id, 9, &desc));
        assert!(lock(&textures.live).contains(&id));
    }

    // TEX-19
    #[test]
    fn destroy_texture_with_other_device_id_keeps_own_entry() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        let desc = texture_desc();
        let h = textures.create_texture(&desc).expect("texture");
        let id = h.__backend_id();
        textures.destroy_texture(TextureHandle::__backend_new(9, id, 5, &desc));
        assert!(lock(&textures.live).contains(&id));
    }

    // TEX-19
    #[test]
    fn destroy_unknown_texture_keeps_entries() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        let desc = texture_desc();
        let h = textures.create_texture(&desc).expect("texture");
        let id = h.__backend_id();
        textures.destroy_texture(TextureHandle::__backend_new(5, id + 100, 5, &desc));
        let live = lock(&textures.live);
        assert_eq!(live.len(), 1);
        assert!(live.contains(&id));
    }

    // TEX-19
    #[test]
    fn destroy_view_keeps_texture_entry() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        let h = textures.create_texture(&texture_desc()).expect("texture");
        let view = TextureView::__backend_new(&h, 5, 1, 5);
        textures.destroy_view(view);
        assert!(lock(&textures.live).contains(&h.__backend_id()));
    }

    // TEX-19
    #[test]
    fn destroyed_texture_id_is_not_reused() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        let desc = texture_desc();
        let first = textures.create_texture(&desc).expect("texture");
        let first_id = first.__backend_id();
        textures.destroy_texture(first);
        let second = textures.create_texture(&desc).expect("texture");
        assert_ne!(second.__backend_id(), first_id);
    }

    // DEV-154: 通し番号は 1 から増え、device の識別子と世代は generation() と同じ値。
    #[test]
    fn create_sampler_numbers_from_one_and_uses_generation() {
        let device = StubDevice::new(request());
        let textures = device.textures().expect("textures");
        let desc = SamplerDescriptor::default();
        let first = textures.create_sampler(&desc).expect("sampler");
        let second = textures.create_sampler(&desc).expect("sampler");
        assert_eq!(first.__backend_id(), 1);
        assert_eq!(second.__backend_id(), 2);
        assert_eq!(first.__backend_device_id(), device.generation());
        assert_eq!(first.__backend_generation(), device.generation());
    }

    // DEV-154: サンプラーの通し番号はテクスチャとは別の源。
    #[test]
    fn sampler_numbering_is_separate_from_textures() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        textures.create_texture(&texture_desc()).expect("texture");
        let sampler = textures
            .create_sampler(&SamplerDescriptor::default())
            .expect("sampler");
        assert_eq!(sampler.__backend_id(), 1);
    }

    // DEV-154: サンプラーを先に作っても、テクスチャの最初の通し番号は 1 のまま。
    #[test]
    fn texture_numbering_is_unaffected_by_earlier_samplers() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        textures
            .create_sampler(&SamplerDescriptor::default())
            .expect("sampler");
        let texture = textures.create_texture(&texture_desc()).expect("texture");
        assert_eq!(texture.__backend_id(), 1);
    }

    // DEV-154: 通し番号が尽きたら panic せず BackendFailure を返す。
    #[test]
    fn create_sampler_reports_exhausted_id_space() {
        let textures = StubTextures::new(BackendKind::Vulkan, 5);
        textures.next_sampler.store(u64::MAX - 1, Ordering::Relaxed);
        let desc = SamplerDescriptor::default();
        let handle = textures.create_sampler(&desc).expect("sampler");
        assert_eq!(handle.__backend_id(), u64::MAX - 1);
        assert_eq!(handle.__backend_device_id(), 5);
        assert!(matches!(
            textures.create_sampler(&desc),
            Err(ContractError::BackendFailure {
                op: "create_sampler",
                ..
            })
        ));
    }

    #[cfg(feature = "graphics")]
    fn pipeline_input() -> PipelineInput<'static> {
        PipelineInput::Msl {
            source: "",
            entry_point: "main",
        }
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn graphics_create_reports_exhausted_id_space() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        g.next_serial.store(u64::MAX - 1, Ordering::Relaxed);
        let layout = BindingLayout::new(vec![]);
        let handle = g
            .create_render_pipeline(pipeline_input(), pipeline_input(), &layout)
            .expect("pipeline");
        assert_eq!(handle.__backend_id(), u64::MAX - 1);
        assert_eq!(handle.__backend_device_id(), 5);
        assert!(matches!(
            g.create_render_pipeline(pipeline_input(), pipeline_input(), &layout),
            Err(ContractError::BackendFailure {
                op: "create_render_pipeline",
                ..
            })
        ));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn render_encoder_carries_backend_failure_when_ids_are_exhausted() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let target = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        g.next_serial.store(u64::MAX - 1, Ordering::Relaxed);
        let first = g.create_render_encoder(&target);
        assert_eq!(first.__backend_rejected(), None);
        let second = g.create_render_encoder(&target);
        assert!(matches!(
            second.__backend_rejected(),
            Some(ContractError::BackendFailure {
                op: "create_render_encoder",
                ..
            })
        ));
        assert!(matches!(
            g.submit_render(second, &[]),
            Err(ContractError::BackendFailure {
                op: "create_render_encoder",
                ..
            })
        ));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn destroyed_target_is_removed_from_table() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let t = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        assert!(g.is_live(&t));
        let copy = RenderTargetHandle::__backend_new(5, t.__backend_id(), 5);
        g.destroy_offscreen_target(t);
        assert!(!g.is_live(&copy));
        assert!(matches!(
            g.create_render_encoder(&copy).__backend_rejected(),
            Some(ContractError::InvalidArgument(_))
        ));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn destroying_other_device_handle_keeps_own_target() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let t = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        g.destroy_offscreen_target(RenderTargetHandle::__backend_new(9, t.__backend_id(), 9));
        assert!(g.is_live(&t));
    }

    #[cfg(feature = "graphics")]
    fn is_invalid_argument(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn submit_render_rejects_encoder_whose_target_was_destroyed() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let t = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        let copy = RenderTargetHandle::__backend_new(5, t.__backend_id(), 5);
        let encoder = g.create_render_encoder(&copy);
        assert_eq!(encoder.__backend_rejected(), None);
        g.destroy_offscreen_target(t);
        assert!(matches!(
            g.submit_render(encoder, &[]),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn draw_rejects_foreign_encoder_before_looking_up_table() {
        let a = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let b = StubGraphics::new(BackendKind::Vulkan, 6, Arc::new(AtomicU64::new(1)));
        let ta = a
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target a");
        let tb = b
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target b");
        let _ea = a.create_render_encoder(&ta);
        let mut eb = b.create_render_encoder(&tb);
        let pipeline = PipelineHandle::__backend_new(1, 5);
        let buf = BufferHandle::__backend_new(5, 1, 1, 16, BufferUsage::VERTEX);
        a.destroy_offscreen_target(ta);
        assert!(matches!(
            a.draw(&mut eb, &pipeline, &buf, 3),
            Err(ContractError::DeviceMismatch)
        ));
        assert!(matches!(
            a.draw_ex(&mut eb, &pipeline, &buf, 3, &[]),
            Err(ContractError::DeviceMismatch)
        ));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn submit_render_rejects_foreign_encoder_without_touching_table() {
        let a = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let b = StubGraphics::new(BackendKind::Vulkan, 6, Arc::new(AtomicU64::new(1)));
        let ta = a
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target a");
        let tb = b
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target b");
        let ea = a.create_render_encoder(&ta);
        let eb = b.create_render_encoder(&tb);
        assert_eq!(ea.__backend_id(), eb.__backend_id());
        assert!(matches!(
            a.submit_render(eb, &[]),
            Err(ContractError::DeviceMismatch)
        ));
        a.destroy_offscreen_target(ta);
        assert!(matches!(
            a.submit_render(ea, &[]),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn draw_rejects_encoder_whose_target_was_destroyed() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let t = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        let copy = RenderTargetHandle::__backend_new(5, t.__backend_id(), 5);
        let mut enc = g.create_render_encoder(&copy);
        g.destroy_offscreen_target(t);
        let buf = BufferHandle::__backend_new(5, 1, 5, 4, BufferUsage::STORAGE);
        let pipeline = PipelineHandle::__backend_new(1, 5);
        assert!(is_invalid_argument(g.draw(&mut enc, &pipeline, &buf, 3)));
        assert!(is_invalid_argument(g.draw_ex(
            &mut enc,
            &pipeline,
            &buf,
            3,
            &[]
        )));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn copy_and_draw_reject_destroyed_and_unknown_targets() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let live = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        let copy = RenderTargetHandle::__backend_new(5, live.__backend_id(), 5);
        let unknown = RenderTargetHandle::__backend_new(5, 9999, 5);
        g.destroy_offscreen_target(live);
        let buf = BufferHandle::__backend_new(5, 1, 5, 4, BufferUsage::STORAGE);
        let pipeline = PipelineHandle::__backend_new(1, 5);
        let mut cmd = CommandEncoder::__backend_new(0, None);
        for t in [&copy, &unknown] {
            assert!(is_invalid_argument(
                g.copy_target_to_buffer(&mut cmd, t, &buf, 0)
            ));
            let mut enc = g.create_render_encoder(t);
            assert!(is_invalid_argument(g.draw(&mut enc, &pipeline, &buf, 3)));
            assert!(is_invalid_argument(g.draw_ex(
                &mut enc,
                &pipeline,
                &buf,
                3,
                &[]
            )));
        }
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn unknown_target_gives_rejected_encoder() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let unknown = RenderTargetHandle::__backend_new(5, 9999, 5);
        assert!(matches!(
            g.create_render_encoder(&unknown).__backend_rejected(),
            Some(ContractError::InvalidArgument(_))
        ));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn depth_pair_targets_can_each_be_destroyed() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let make = || {
            g.create_offscreen_target_with_depth(
                1,
                1,
                PixelFormat::Rgba8Unorm,
                DepthConfig {
                    format: DepthFormat::Depth32Float,
                    clear_value: 1.0,
                    compare: crate::graphics::CompareFunction::Less,
                    write_enabled: true,
                },
            )
            .expect("pair")
        };
        let (color, depth) = make();
        let color_copy = RenderTargetHandle::__backend_new(5, color.__backend_id(), 5);
        let depth_copy = RenderTargetHandle::__backend_new(5, depth.__backend_id(), 5);
        assert!(g.is_live(&color_copy) && g.is_live(&depth_copy));
        g.destroy_offscreen_target(depth);
        assert!(g.is_live(&color_copy));
        assert!(!g.is_live(&depth_copy));
        g.destroy_offscreen_target(color);
        assert!(!g.is_live(&color_copy));
        let (c2, d2) = make();
        assert!(g.is_live(&c2) && g.is_live(&d2));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn destroyed_ids_are_not_reused() {
        let g = StubGraphics::new(BackendKind::Vulkan, 5, Arc::new(AtomicU64::new(1)));
        let a = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        let first = a.__backend_id();
        g.destroy_offscreen_target(a);
        let b = g
            .create_offscreen_target(1, 1, PixelFormat::Rgba8Unorm)
            .expect("target");
        assert_ne!(b.__backend_id(), first);
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
    fn submit_many_rejects_out_of_range_sequence_without_claiming() {
        let device = StubDeviceProvider::request(request()).expect("device");
        let queue = device.queue();
        let token = queue.submit(queue.create_encoder(), &[]).expect("submit");
        for bad in [0, u64::MAX, token.sequence + 1] {
            let mut forged = token;
            forged.sequence = bad;
            let enc = queue.create_encoder();
            let r = queue.submit_many(vec![enc], &[forged], &[]);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{bad}");
        }
        // 記録先は claim されておらず、新しい encoder で正常に投入できる。
        queue
            .submit_many(vec![queue.create_encoder()], &[token], &[])
            .expect("valid after");
    }

    #[cfg(feature = "native-blocking-wait")]
    #[test]
    fn wait_blocking_errors_match_poll_lost_tokens() {
        let device = StubDeviceProvider::request(request()).expect("device");
        let queue = device.queue();
        let token = queue.submit(queue.create_encoder(), &[]).expect("submit");
        assert_eq!(queue.poll(token), CompletionState::Complete);
        assert!(queue.wait_blocking(token, None).is_ok());
        // poll が Lost を返す印は、wait_blocking では種類別のエラーになる。
        let mut bad_seq = token;
        bad_seq.sequence = 0;
        let mut bad_gen = token;
        bad_gen.generation = token.generation.wrapping_add(1);
        for forged in [bad_seq, bad_gen] {
            assert_eq!(queue.poll(forged), CompletionState::Lost);
        }
        assert!(matches!(
            queue.wait_blocking(bad_seq, None),
            Err(ContractError::InvalidArgument(_))
        ));
        assert!(matches!(
            queue.wait_blocking(bad_gen, None),
            Err(ContractError::DeviceLost { .. })
        ));
        let other = StubDeviceProvider::request(request()).expect("device");
        let foreign = other
            .queue()
            .submit(other.queue().create_encoder(), &[])
            .expect("submit");
        assert_eq!(queue.poll(foreign), CompletionState::Lost);
        assert!(matches!(
            queue.wait_blocking(foreign, None),
            Err(ContractError::DeviceMismatch)
        ));
    }

    #[test]
    fn stub_device_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StubDevice>();
    }
}
