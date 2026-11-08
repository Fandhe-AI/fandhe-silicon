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
//!
//! `CudaDevice` は `ComputeOps` を実装し、パイプラインの生成の口 `create_pipeline` を持つ
//! （TASK-803 のポインタ）。
//!
//! `CommandQueue` の実装は投入の窓口で、記録先の開いている集合と投入済みの表を持つ（TASK-604 のポインタ）。
//!
//! `free` したバッファは stream にゼロ化を積んでから再利用の一覧の待つ側に入り、`allocate` の入口で
//! 完了を確かめたものだけが、同じ長さ・同じ用途の確保に再利用される（TASK-340 のポインタ）。
//! 一覧の上限は件数と大きさの合計で、入れられないバッファは解放する。新規の確保は全バイトを 0 にし終えてから返す。
//! 記録先に命令を記録する口（`dispatch_ex`）は `Unsupported` を返し、投入（`submit`・`submit_many`）は
//! stream に event を記録するだけでバッファを参照しないので、`free` したバッファを参照する未完了の投入は無い。
//! バッファを参照する記録を足す変更では、`free` から一覧への回し方を見直すこと。
//!
//! `query`・`query_ex` は、`request` の時点で捕捉した compute capability とホストメモリのマップの可否の属性と、
//! 能力の表から答える（TASK-175）。

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use cudarc::driver::{
    CudaContext, CudaEvent, CudaFunction, CudaModule, CudaSlice, CudaStream, DriverError,
};
use cudarc::nvrtc::{CompileError, CompileOptions, Ptx};
use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
    LimitsTable,
};
use fandhe_silicon_core::capability_table::{FeatureRow, query_from_table};
use fandhe_silicon_core::device::{
    __backend_next_device_id, Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider,
    DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError, UnsupportedReason};
use fandhe_silicon_core::execution::{
    __BackendSharedEncoderCloser, __backend_next_encoder_id, Binding, BindingLayout,
    CommandEncoder, CommandQueue, ComputeOps, ConstantBinding, ImportedSemaphoreHandle,
    PipelineHandle, PipelineInput, SubmitToken, ThreadgroupMemory,
};
use fandhe_silicon_core::feature_key::{
    FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX, FEATURE_MAPPABLE_PRIMARY_BUFFERS, FEATURE_SHADER_F16,
    FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_SUBGROUP, FEATURE_SUBGROUP_BARRIER,
};
use fandhe_silicon_core::memory::{
    BufferHandle, BufferUsage, MemoryOps, validate_allocation_size, validate_buffer_range,
};

/// この provider が受け持つ背後実装。
const BACKEND: BackendKind = BackendKind::Cuda;

/// `DeviceDiagnostics::backend_name` に載せる名前。
const BACKEND_NAME: &str = "cuda";

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
    compute_capability: (i32, i32),
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
        compute_capability: capability,
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
    // 読み込み先は OS の検索規則で決まるので、検索パスの内容をプロセスの環境が信頼している前提に立つ。
    unsafe { cudarc::driver::sys::is_culib_present() }
}

/// 要求を検査し、ドライバが読めれば device 0 の context と stream を作って `CudaDevice` を返す。
/// 作るまでの失敗はすべて `DeviceUnavailable`。`CudaDeviceProvider::request` から呼ばれる。
fn request_device(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
    open_device(req).map(|d| Box::new(d) as Box<dyn Device>)
}

/// `request_device` の本体。具体の型 `CudaDevice` を返し、device の要る試験がこれを呼ぶ。
fn open_device(req: DeviceRequest) -> Result<CudaDevice, ContractError> {
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
        // 読み取りの失敗は device の作成を失敗させず、その機能の値だけを `S0` にする。
        let can_map_host_memory =
            can_map_host_memory_of(|| ctx.attribute(Attr::CU_DEVICE_ATTRIBUTE_CAN_MAP_HOST_MEMORY));
        let stream = ctx.new_stream().map_err(|e| {
            ContractError::DeviceUnavailable(format!("CudaContext::new_stream: {e:?}"))
        })?;
        let exhausted =
            || ContractError::DeviceUnavailable("device counter space is exhausted".into());
        let id = __backend_next_device_id().ok_or_else(exhausted)?;
        let generation = take_next(&NEXT_GENERATION).ok_or_else(exhausted)?;
        Ok(CudaDevice {
            id,
            generation,
            ownership: DeviceOwnership::ProductIndependent,
            adapter_name: attributes.adapter_name,
            limits: LimitsTable {
                max_buffer_bytes: attributes.total_mem,
                max_dispatch_dims: grid_dims,
                max_texture_size: 0,
            },
            compute_capability: attributes.compute_capability,
            capabilities: CapabilitySnapshot {
                compute_capability: attributes.compute_capability,
                can_map_host_memory,
            },
            stream,
            buffers: Mutex::new(Buffers::default()),
            pipelines: Mutex::new(Pipelines::default()),
            open: Arc::new(OpenEncoders::default()),
            submissions: Mutex::new(Submissions::new()),
        })
    })
}

/// 能力の判定の材料。`open_device` が device を作るときに 1 回だけ読み、`query` は読み直さない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CapabilitySnapshot {
    /// device の compute capability（major, minor）。
    compute_capability: (i32, i32),
    /// ホストメモリのマップの可否。読み取りに失敗したときは `None`。
    can_map_host_memory: Option<bool>,
}

/// 能力の表の行の規則。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CudaRule {
    /// compute capability がこの値以上なら `S2`、未満なら `Unsupported`（`NotSupported`）。
    MinComputeCapability { major: i32, minor: i32 },
    /// ホストメモリのマップの可否の属性で決める。読み取りに失敗していれば `S0`。
    CanMapHostMemory,
}

/// compute capability がこれより低い device では、表のどの行も `Unsupported`（`NotSupported`）を返す。
const MIN_COMPUTE_CAPABILITY: (i32, i32) = (7, 5);

/// CUDA の能力の表。行の無い鍵は `UNLISTED_FEATURE_QUERY` になる。
static CUDA_FEATURE_TABLE: [FeatureRow<CudaRule>; 6] = [
    FeatureRow {
        feature: FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX,
        rule: CudaRule::MinComputeCapability { major: 7, minor: 0 },
        source: "CUDA C++ Programming Guide: Warp Matrix Functions",
    },
    FeatureRow {
        feature: FEATURE_SHADER_F16,
        rule: CudaRule::MinComputeCapability { major: 5, minor: 3 },
        source: "CUDA C++ Programming Guide: Feature Support per Compute Capability",
    },
    FeatureRow {
        feature: FEATURE_SUBGROUP,
        rule: CudaRule::MinComputeCapability { major: 5, minor: 0 },
        source: "CUDA C++ Programming Guide: Warp Shuffle Functions",
    },
    FeatureRow {
        feature: FEATURE_SUBGROUP_BARRIER,
        rule: CudaRule::MinComputeCapability { major: 7, minor: 0 },
        source: "CUDA C++ Programming Guide: Independent Thread Scheduling",
    },
    FeatureRow {
        feature: FEATURE_SHADER_FLOAT32_ATOMIC,
        rule: CudaRule::MinComputeCapability { major: 2, minor: 0 },
        source: "CUDA C++ Programming Guide: atomicAdd()",
    },
    FeatureRow {
        feature: FEATURE_MAPPABLE_PRIMARY_BUFFERS,
        rule: CudaRule::CanMapHostMemory,
        source: "CUDA Driver API: CU_DEVICE_ATTRIBUTE_CAN_MAP_HOST_MEMORY",
    },
];

/// ホストメモリのマップの可否の属性の値を写す。1 は `true`、0 は `false`、読み取りの失敗とほかの値は `None`。
fn can_map_host_memory_of<E>(attr: impl FnOnce() -> Result<i32, E>) -> Option<bool> {
    match attr() {
        Ok(1) => Some(true),
        Ok(0) => Some(false),
        _ => None,
    }
}

/// 根拠が実行時の問合せで、`S2` か `Unsupported`（`NotSupported`）の値。
fn runtime_decided(supported: bool) -> CapabilityQuery {
    if supported {
        CapabilityQuery {
            level: CapabilityLevel::S2,
            unsupported_reason: None,
            evidence: Some(CapabilityEvidence::RuntimeApi),
        }
    } else {
        CapabilityQuery {
            level: CapabilityLevel::Unsupported,
            unsupported_reason: Some(UnsupportedReason::NotSupported),
            evidence: Some(CapabilityEvidence::RuntimeApi),
        }
    }
}

/// 1 つの行の規則を捕捉した値に当てる。compute capability が `MIN_COMPUTE_CAPABILITY` より低ければ、
/// 規則によらず `Unsupported`（`NotSupported`）を返す。
fn evaluate(rule: &CudaRule, snapshot: &CapabilitySnapshot) -> CapabilityQuery {
    if snapshot.compute_capability < MIN_COMPUTE_CAPABILITY {
        return runtime_decided(false);
    }
    match *rule {
        CudaRule::MinComputeCapability { major, minor } => {
            runtime_decided(snapshot.compute_capability >= (major, minor))
        }
        CudaRule::CanMapHostMemory => match snapshot.can_map_host_memory {
            Some(v) => runtime_decided(v),
            None => CapabilityQuery {
                level: CapabilityLevel::S0,
                unsupported_reason: None,
                evidence: None,
            },
        },
    }
}

/// 捕捉した値と表から 1 つの機能の値を決める純粋な関数。下位の API を呼ばない。
/// `CudaDevice` の `query`・`query_ex` から呼ばれる。
fn cuda_capability_of(snapshot: &CapabilitySnapshot, feature: FeatureId) -> CapabilityQuery {
    query_from_table(&CUDA_FEATURE_TABLE, feature, |rule| {
        evaluate(rule, snapshot)
    })
}

/// CUDA の背後実装の `DeviceProvider`。
///
/// `request` は、背後実装が CUDA でない・`Graphics`・`CpuOnly` を `InvalidCombination` で、
/// `Shared`・`Imported` を `Unsupported` で拒む。検査を通った後は device を作り、ドライバが
/// 読めない・context や属性の取得に失敗したときは `DeviceUnavailable` を返す。
/// ホストメモリのマップの可否の属性の読み取りの失敗は `request` を失敗させず、その機能の `query` を `S0` にする。
#[derive(Debug, Clone, Copy, Default)]
pub struct CudaDeviceProvider;

impl DeviceProvider for CudaDeviceProvider {
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        request_device(req)
    }
}

/// 差し込みの失敗の文字列に載せる印。
#[cfg(feature = "fault-injection")]
const INJECTED: &str = "injected";

/// 指定した属性の読み取りだけを失敗させ、`read_attributes` の写し方を通した値を返す。
#[cfg(feature = "fault-injection")]
fn injected_attribute_failure(
    name_ok: bool,
    capability_ok: bool,
    total_mem_ok: bool,
) -> ContractError {
    let attributes = read_attributes(
        || name_ok.then(|| "GPU".to_string()).ok_or(INJECTED),
        || capability_ok.then_some((0, 0)).ok_or(INJECTED),
        || total_mem_ok.then_some(0_usize).ok_or(INJECTED),
    );
    match attributes {
        Err(e) => e,
        Ok(_) => ContractError::DeviceUnavailable(INJECTED.into()),
    }
}

// 初期化の失敗を差し込む provider を作る。各 `request` は検査の後に差し込みの失敗を返し、
// 実の `request` の経路（`CudaDeviceProvider`）には触れない。
macro_rules! injected_provider {
    ($($(#[$doc:meta])* $name:ident => $failure:expr;)*) => {$(
        $(#[$doc])*
        #[cfg(feature = "fault-injection")]
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        #[cfg(feature = "fault-injection")]
        impl DeviceProvider for $name {
            fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
                check_request(&req)?;
                Err($failure)
            }
        }
    )*};
}

injected_provider! {
    /// ドライバのライブラリが読めない失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    CudaDriverMissingProvider => ContractError::DeviceUnavailable(
        "CudaContext::new(0): CUDA driver library is not loadable (injected)".into(),
    );
    /// context の作成の失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    CudaContextNewFailsProvider => ContractError::DeviceUnavailable(
        "CudaContext::new(0): injected".into(),
    );
    /// GPU の名前の読み取りの失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    CudaNameFailsProvider => injected_attribute_failure(false, true, true);
    /// compute capability の読み取りの失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    CudaComputeCapabilityFailsProvider => injected_attribute_failure(true, false, true);
    /// メモリの大きさの読み取りの失敗を差し込む入口。実際の故障ではなく、feature `fault-injection` の裏にある。
    CudaTotalMemFailsProvider => injected_attribute_failure(true, true, false);
}

/// `allocate` の中のドライバ呼び出しの失敗を写す。メモリ不足は `AllocationFailed`、それ以外は `BackendFailure`。
fn allocation_failure(op: &'static str, requested_bytes: u64, e: DriverError) -> ContractError {
    if e.0 == cudarc::driver::sys::CUresult::CUDA_ERROR_OUT_OF_MEMORY {
        ContractError::AllocationFailed { requested_bytes }
    } else {
        backend_failure(op, e)
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

/// メモリ操作のドライバを呼ぶ閉包の panic を `BackendFailure` に変える。panic の payload は載せない。
/// unwind するビルドで効く（`panic=abort` のビルドでは panic はそのまま abort になる）。
fn guard_memory_panic<T>(
    op: &'static str,
    f: impl FnOnce() -> Result<T, ContractError>,
) -> Result<T, ContractError> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(ContractError::BackendFailure {
            op,
            backend: BACKEND,
            detail: "driver call panicked".into(),
        })
    })
}

/// `create_pipeline` の入力を分類し、受け付ける CUDA C のソースと入口名を返す。
/// device にも下位の API にも触らない。`ComputeOps::create_pipeline` から最初に呼ばれる。
fn classify_pipeline_input<'a>(
    input: &PipelineInput<'a>,
) -> Result<(&'a str, &'a str), ContractError> {
    let variant = match input {
        PipelineInput::CudaC {
            source,
            entry_point,
        } => return Ok((source, entry_point)),
        PipelineInput::Ptx { .. } | PipelineInput::Wgsl { .. } => {
            return Err(ContractError::Unsupported {
                op: "create_pipeline",
                backend: BACKEND,
            });
        }
        PipelineInput::Msl { .. } => "Msl",
        PipelineInput::SpirV { .. } => "SpirV",
        PipelineInput::Dxil { .. } => "Dxil",
        PipelineInput::MslSpecialized { .. } => "MslSpecialized",
        PipelineInput::MslWithOptions { .. } => "MslWithOptions",
        PipelineInput::MslSpecializedWithOptions { .. } => "MslSpecializedWithOptions",
        _ => {
            return Err(ContractError::Unsupported {
                op: "create_pipeline",
                backend: BACKEND,
            });
        }
    };
    Err(ContractError::ArtifactRejected {
        kind: "unsupported-input",
        reason: format!(
            "pipeline input {variant} is not accepted by the CUDA backend; accepted input is CudaC"
        ),
    })
}

/// `layout.slots` に重複した番号が無いことを照らす。最初に重複した番号で `InvalidArgument`。
/// `create_pipeline` が入力の分類の次に呼ぶ。
fn check_unique_slots(slots: &[u32]) -> Result<(), ContractError> {
    let mut seen = HashSet::new();
    for &slot in slots {
        if !seen.insert(slot) {
            return Err(ContractError::InvalidArgument(format!(
                "duplicate slot in binding layout: {slot}"
            )));
        }
    }
    Ok(())
}

/// ソースと入口名に NUL のバイトが無いことを照らす。下位の API へ文字列を渡す前の検査で、
/// `create_pipeline` が最後に呼ぶ。ソースを先に見る。
fn check_no_nul(source: &str, entry_point: &str) -> Result<(), ContractError> {
    if source.contains('\0') {
        return Err(ContractError::ArtifactRejected {
            kind: "cuda-c",
            reason: "CUDA C source contains a NUL byte".to_string(),
        });
    }
    if entry_point.contains('\0') {
        return Err(ContractError::ArtifactRejected {
            kind: "function",
            reason: "entry point contains a NUL byte".to_string(),
        });
    }
    Ok(())
}

/// NVRTC に渡すオプションを compute capability から組み立てる。`compile_cuda_c` から呼ばれる。
/// 渡すのは `--gpu-architecture` の 1 つだけ。負の値は `BackendFailure`。
fn compile_options(capability: (i32, i32)) -> Result<CompileOptions, ContractError> {
    let (major, minor) = capability;
    if major < 0 || minor < 0 {
        return Err(ContractError::BackendFailure {
            op: "create_pipeline",
            backend: BACKEND,
            detail: format!("invalid compute capability {major}.{minor}"),
        });
    }
    Ok(CompileOptions {
        options: vec![format!("--gpu-architecture=compute_{major}{minor}")],
        ..CompileOptions::default()
    })
}

/// NVRTC の共有ライブラリが読み込めるかを返す。`compile_cuda_c` がコンパイルの前に呼ぶ。
fn nvrtc_library_present() -> bool {
    // SAFETY: `is_culib_present` は NVRTC の共有ライブラリの名前の候補を順に読み込み、1 つでも読めれば
    // その場で閉じて `true` を返す。ポインタを受けず、呼び出しの前に満たす不変条件は無い。
    // 読み込みが走らせる初期化子は、以後のコンパイルが読み込むものと同じライブラリのものだけ。
    // 読み込み先は OS の検索規則で決まるので、検索パスの内容をプロセスの環境が信頼している前提に立つ。
    unsafe { cudarc::nvrtc::sys::is_culib_present() }
}

/// 成果物の読み込み系の失敗を写す。入力の成果物に原因があるときだけ `ArtifactRejected`、
/// それ以外（メモリ不足・device や context の異常・内部エラーなど）は `BackendFailure`。
fn artifact_failure(
    kind: &'static str,
    call: &'static str,
    caused_by_artifact: bool,
    detail: impl std::fmt::Debug,
) -> ContractError {
    if caused_by_artifact {
        ContractError::ArtifactRejected {
            kind,
            reason: format!("{call}: {detail:?}"),
        }
    } else {
        backend_failure(call, detail)
    }
}

/// ドライバのコードが入力の成果物に原因のあるものかを返す。`load_module` / `load_function` の失敗の分類に使う。
fn driver_error_is_artifact_cause(e: &DriverError) -> bool {
    use cudarc::driver::sys::CUresult;
    matches!(
        e.0,
        CUresult::CUDA_ERROR_INVALID_PTX
            | CUresult::CUDA_ERROR_INVALID_IMAGE
            | CUresult::CUDA_ERROR_NO_BINARY_FOR_GPU
            | CUresult::CUDA_ERROR_UNSUPPORTED_PTX_VERSION
            | CUresult::CUDA_ERROR_NOT_FOUND
    )
}

/// NVRTC のコードが入力のソースやオプションに原因のあるものかを返す。`nvrtc_failure` の分類に使う。
fn nvrtc_code_is_artifact_cause(code: cudarc::nvrtc::sys::nvrtcResult) -> bool {
    use cudarc::nvrtc::sys::nvrtcResult;
    matches!(
        code,
        nvrtcResult::NVRTC_ERROR_COMPILATION
            | nvrtcResult::NVRTC_ERROR_INVALID_PROGRAM
            | nvrtcResult::NVRTC_ERROR_INVALID_OPTION
            | nvrtcResult::NVRTC_ERROR_INVALID_INPUT
    )
}

/// NVRTC の失敗を、失敗した呼び出しの名前を付けて写す。`compile_cuda_c` から呼ばれる。
fn nvrtc_failure(e: CompileError) -> ContractError {
    let (call, code) = match &e {
        CompileError::CreationError(n) => ("nvrtcCreateProgram", n.0),
        CompileError::CompileError { nvrtc, .. } => ("nvrtcCompileProgram", nvrtc.0),
        CompileError::GetLogError(n) => ("nvrtcGetProgramLog", n.0),
        CompileError::GetPtxError(n) => ("nvrtcGetPTX", n.0),
        CompileError::DestroyError(n) => ("nvrtcDestroyProgram", n.0),
    };
    artifact_failure("cuda-c", call, nvrtc_code_is_artifact_cause(code), e)
}

/// パイプラインの生成でドライバや NVRTC を呼ぶ閉包の panic を `BackendFailure` に変える。
/// panic の payload は載せない。unwind するビルドで効く（`panic=abort` のビルドでは abort になる）。
fn guard_pipeline_panic<T>(
    f: impl FnOnce() -> Result<T, ContractError>,
) -> Result<T, ContractError> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(ContractError::BackendFailure {
            op: "create_pipeline",
            backend: BACKEND,
            detail: "driver call panicked".into(),
        })
    })
}

/// CUDA C のソースを NVRTC で PTX にする。NVRTC を呼ぶのはこの関数だけ。
/// ライブラリが無ければ `Unsupported`。`ComputeOps::create_pipeline` から呼ばれる。
fn compile_cuda_c(source: &str, capability: (i32, i32)) -> Result<Ptx, ContractError> {
    let options = compile_options(capability)?;
    if !nvrtc_library_present() {
        return Err(ContractError::Unsupported {
            op: "create_pipeline",
            backend: BACKEND,
        });
    }
    guard_pipeline_panic(|| {
        cudarc::nvrtc::compile_ptx_with_opts(source, options).map_err(nvrtc_failure)
    })
}

/// 投入済みの表の 1 件の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupState {
    Pending,
    Completed,
    Failed,
}

/// event の完了を待たずに問い合わせる。問合せの失敗と panic は `Failed`。
fn event_state(event: &CudaEvent) -> GroupState {
    match catch_unwind(AssertUnwindSafe(|| event.try_is_complete())) {
        Ok(Ok(true)) => GroupState::Completed,
        Ok(Ok(false)) => GroupState::Pending,
        Ok(Err(_)) | Err(_) => GroupState::Failed,
    }
}

/// 投入済みの表。投入の通し番号は device ごとに 1 から払い出す。
///
/// `G` は 1 回の投入の完了を表す値で、本物では投入の最後に記録した `CudaEvent`、試験では偽の状態。
/// 回収されるまで保持する。
struct Submissions<G> {
    /// 次に払い出す通し番号。
    next_sequence: u64,
    /// この番号以下は回収済み（失敗の集合にあるものを除き完了）。
    watermark: u64,
    /// 通し番号から完了の値への表。
    pending: BTreeMap<u64, G>,
    /// 失敗として回収した通し番号。
    failed: BTreeSet<u64>,
    /// 終端を待つ登録済みの callback。通し番号と組で持つ。
    callbacks: Vec<(u64, CompletionCallback)>,
    /// 直近の `reclaim` で未完了のまま止まった先頭の番号。同じ錠の間はその判定を使い、再び問い合わせない。
    stalled: Option<u64>,
}

/// `on_complete` に渡された callback。
type CompletionCallback = Box<dyn FnOnce(CompletionState) + Send>;

impl<G> Submissions<G> {
    /// 何も投入していない表。
    fn new() -> Self {
        Self {
            next_sequence: 1,
            watermark: 0,
            pending: BTreeMap::new(),
            failed: BTreeSet::new(),
            callbacks: Vec::new(),
            stalled: None,
        }
    }

    /// 通し番号の完了状態。呼ぶ前に `reclaim` しておく。
    ///
    /// `reclaim` が未完了で止まった先頭は、再び問い合わせずに `Pending` とする
    /// （問合せの再読みで表に残ったまま完了と判定されるのを防ぐ）。
    ///
    /// 表にあれば列の状態、失敗の集合にある番号と払い出していない番号（0 を含む）は `Lost`、
    /// 回収済みの他は `Complete`。
    fn state_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> CompletionState {
        if let Some(group) = self.pending.get(&sequence) {
            if self.stalled == Some(sequence) {
                return CompletionState::Pending;
            }
            return match classify(group) {
                GroupState::Pending => CompletionState::Pending,
                GroupState::Completed => CompletionState::Complete,
                GroupState::Failed => CompletionState::Lost,
            };
        }
        if self.failed.contains(&sequence) || sequence == 0 || sequence >= self.next_sequence {
            CompletionState::Lost
        } else {
            CompletionState::Complete
        }
    }

    /// 回収してから通し番号の完了状態を返す。`poll` と `on_complete` から呼ぶ。
    fn reclaim_and_state_of(
        &mut self,
        sequence: u64,
        classify: impl Fn(&G) -> GroupState,
    ) -> CompletionState {
        self.reclaim(&classify);
        self.state_of(sequence, classify)
    }

    /// 終端になった登録済みの callback を表から外して返す。呼ぶのは錠を放してから。
    fn take_ready(
        &mut self,
        classify: impl Fn(&G) -> GroupState,
    ) -> Vec<(CompletionCallback, CompletionState)> {
        let mut ready = Vec::new();
        for (sequence, callback) in std::mem::take(&mut self.callbacks) {
            match self.state_of(sequence, &classify) {
                CompletionState::Pending => self.callbacks.push((sequence, callback)),
                CompletionState::Complete => ready.push((callback, CompletionState::Complete)),
                CompletionState::Lost => ready.push((callback, CompletionState::Lost)),
            }
        }
        ready
    }

    /// 登録済みの callback を全て外して返す。device の破棄で `Lost` を通知するのに使う。
    fn take_all_callbacks(&mut self) -> Vec<CompletionCallback> {
        std::mem::take(&mut self.callbacks)
            .into_iter()
            .map(|(_, callback)| callback)
            .collect()
    }

    /// `make` が成功したときだけ通し番号を払い出して表に入れる。
    ///
    /// 番号が尽きていれば `make` を呼ばずに `BackendFailure`。`make` の失敗は番号を消費しない。
    fn issue(
        &mut self,
        make: impl FnOnce() -> Result<G, ContractError>,
    ) -> Result<u64, ContractError> {
        let sequence = self.next_sequence;
        let next = sequence
            .checked_add(1)
            .ok_or_else(|| ContractError::BackendFailure {
                op: "submit",
                backend: BACKEND,
                detail: "submission sequence space is exhausted".into(),
            })?;
        let group = make()?;
        self.pending.insert(sequence, group);
        self.next_sequence = next;
        Ok(sequence)
    }

    /// 通し番号の昇順に、決着した先頭から表を外して watermark を進め、止まった先頭の番号を返す。
    ///
    /// 先頭が未完了なら後ろが完了していても回収しない。失敗は失敗の集合に移す。
    fn reclaim(&mut self, classify: impl Fn(&G) -> GroupState) -> Option<u64> {
        self.stalled = None;
        while let Some(entry) = self.pending.first_entry() {
            let sequence = *entry.key();
            match classify(entry.get()) {
                GroupState::Pending => {
                    self.stalled = Some(sequence);
                    return Some(sequence);
                }
                GroupState::Completed => {
                    entry.remove();
                }
                GroupState::Failed => {
                    entry.remove();
                    self.failed.insert(sequence);
                }
            }
            self.watermark = sequence;
        }
        None
    }
}

/// 開いている記録先の識別子の集合。`CommandEncoder` の破棄の hook を兼ねる。
#[derive(Default)]
struct OpenEncoders {
    ids: Mutex<BTreeSet<u64>>,
}

impl OpenEncoders {
    /// 集合の錠を取る。poison されていても中身を使う。
    fn lock(&self) -> MutexGuard<'_, BTreeSet<u64>> {
        self.ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 識別子を集合に入れる。0 は入れない。
    fn open(&self, id: u64) {
        if id != 0 {
            self.lock().insert(id);
        }
    }

    /// 渡された識別子が全部開いていれば集合から外す。
    ///
    /// 1 つでも無い（0 を含む）・重複していれば `InvalidArgument` で、集合は変えない。
    fn claim(&self, ids: &[u64]) -> Result<(), ContractError> {
        let mut open = self.lock();
        let mut seen = BTreeSet::new();
        for id in ids {
            if !open.contains(id) || !seen.insert(*id) {
                return Err(ContractError::InvalidArgument(
                    "encoder is not open on this device".into(),
                ));
            }
        }
        for id in ids {
            open.remove(id);
        }
        Ok(())
    }
}

impl __BackendSharedEncoderCloser for OpenEncoders {
    /// 投入されずに破棄された記録先を集合から外す。無ければ何もしない。
    fn close(&self, id: u64) {
        self.lock().remove(&id);
    }
}

/// 印の持ち主の照合。別 device の印は `DeviceMismatch`、世代違いの印は `DeviceLost`（印の世代）を返す。
fn check_token_owner(
    token: &SubmitToken,
    device_id: u64,
    generation: u64,
) -> Result<(), ContractError> {
    if token.__backend_device_id() != device_id {
        return Err(ContractError::DeviceMismatch);
    }
    if token.generation != generation {
        return Err(ContractError::DeviceLost {
            generation: token.generation,
        });
    }
    Ok(())
}

/// 空でない `wait` を、何も投入せずに `Unsupported` で拒む。
fn reject_wait(op: &'static str, wait: &[ImportedSemaphoreHandle]) -> Result<(), ContractError> {
    if wait.is_empty() {
        Ok(())
    } else {
        Err(ContractError::Unsupported {
            op,
            backend: BACKEND,
        })
    }
}

/// `submit_many` が記録先と device に触る前に通す検査。順は、`after` の各印、空の列、`wait`。
fn precheck_submit_many(
    command_count: usize,
    after: &[SubmitToken],
    wait: &[ImportedSemaphoreHandle],
    device_id: u64,
    generation: u64,
) -> Result<(), ContractError> {
    for token in after {
        check_token_owner(token, device_id, generation)?;
    }
    if command_count == 0 {
        return Err(ContractError::InvalidArgument(
            "submit_many requires at least one encoder".into(),
        ));
    }
    reject_wait("submit_many(wait)", wait)
}

/// 再利用の一覧に入れられるバッファの件数の上限（ゼロ化を待つ側と使える側の合計）。
const POOL_MAX_ENTRIES: usize = 16;

/// 再利用の一覧に入れられるバッファの大きさの合計の上限（バイト。待つ側と使える側の合計）。
const POOL_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// 再利用の鍵。バッファの長さと、確保のときに受けた `BufferUsage::bits()`。
type PoolKey = (u64, u32);

/// ゼロ化を積み終えて完了を待っているバッファ 1 件。`ReusePool::retire` が作り、`settle` が見る。
struct Zeroing<B, E> {
    key: PoolKey,
    slot: u64,
    buffer: B,
    done: E,
}

/// 再利用の一覧。`CudaDevice::free` が入れ、`allocate` の入口（`settle_pool`）が完了を確かめて
/// 使える側へ移し、`allocate` が `take` で取り出す。
///
/// `B` はバッファ、`E` は完了の印。本物では `CudaSlice<u8>` と `CudaEvent`、試験では素の値。
struct ReusePool<B, E> {
    zeroing: Vec<Zeroing<B, E>>,
    ready: BTreeMap<PoolKey, Vec<(u64, B)>>,
    entries: usize,
    bytes: u64,
    last_put_slot: Option<u64>,
    reuse_count: u64,
    slot_match_count: u64,
}

impl<B, E> Default for ReusePool<B, E> {
    fn default() -> Self {
        Self {
            zeroing: Vec::new(),
            ready: BTreeMap::new(),
            entries: 0,
            bytes: 0,
            last_put_slot: None,
            reuse_count: 0,
            slot_match_count: 0,
        }
    }
}

impl<B, E> ReusePool<B, E> {
    /// 1 件足したときの件数と合計。あふれるか上限を超えるなら `None`。
    fn next_totals(&self, byte_len: u64) -> Option<(usize, u64)> {
        let entries = self.entries.checked_add(1)?;
        let bytes = self.bytes.checked_add(byte_len)?;
        (entries <= POOL_MAX_ENTRIES && bytes <= POOL_MAX_BYTES).then_some((entries, bytes))
    }

    /// 長さ `byte_len` のバッファを一覧に入れられるか。`free` がゼロ化を積む前に呼ぶ。
    fn would_accept(&self, byte_len: u64) -> bool {
        self.next_totals(byte_len).is_some()
    }

    /// ゼロ化を積んだバッファを待つ側に入れる。入れられなければ引数をそのまま返す。
    fn retire(&mut self, key: PoolKey, slot: u64, buffer: B, done: E) -> Option<(B, E)> {
        let Some((entries, bytes)) = self.next_totals(key.0) else {
            return Some((buffer, done));
        };
        self.entries = entries;
        self.bytes = bytes;
        self.last_put_slot = Some(slot);
        self.zeroing.push(Zeroing {
            key,
            slot,
            buffer,
            done,
        });
        None
    }

    /// 待つ側の完了を確かめる。`Ok(true)` は使える側へ移し、`Ok(false)` は待つ側に残す。
    /// `Err` は一覧から外し、そのバッファを返す（呼び出し側がロックの外で解放する）。
    fn settle<X>(&mut self, mut is_done: impl FnMut(&E) -> Result<bool, X>) -> Vec<B> {
        let mut dropped = Vec::new();
        for item in std::mem::take(&mut self.zeroing) {
            match is_done(&item.done) {
                Ok(true) => self
                    .ready
                    .entry(item.key)
                    .or_default()
                    .push((item.slot, item.buffer)),
                Ok(false) => self.zeroing.push(item),
                Err(_) => {
                    self.entries = self.entries.saturating_sub(1);
                    self.bytes = self.bytes.saturating_sub(item.key.0);
                    dropped.push(item.buffer);
                }
            }
        }
        dropped
    }

    /// 使える側から鍵の一致するバッファを後に入れたものから 1 件取り出す。
    fn take(&mut self, key: PoolKey) -> Option<(u64, B)> {
        let list = self.ready.get_mut(&key)?;
        let (slot, buffer) = list.pop()?;
        if list.is_empty() {
            self.ready.remove(&key);
        }
        self.entries = self.entries.saturating_sub(1);
        self.bytes = self.bytes.saturating_sub(key.0);
        self.reuse_count = self.reuse_count.saturating_add(1);
        if self.last_put_slot == Some(slot) {
            self.slot_match_count = self.slot_match_count.saturating_add(1);
        }
        Some((slot, buffer))
    }
}

/// `free` の流れ。入れられない（`accepts` が偽）ならゼロ化を積まずに、ゼロ化が積めなければ
/// 一覧に入れずに、バッファを返す（呼び出し側が解放する）。入れたら `None`。
fn retire_flow<B, E, X>(
    byte_len: u64,
    mut buffer: B,
    accepts: impl FnOnce(u64) -> bool,
    zero: impl FnOnce(&mut B) -> Result<E, X>,
    put: impl FnOnce(B, E) -> Option<(B, E)>,
) -> Option<B> {
    if !accepts(byte_len) {
        return Some(buffer);
    }
    let Ok(done) = zero(&mut buffer) else {
        return Some(buffer);
    };
    put(buffer, done).map(|(buffer, _)| buffer)
}

/// バッファの表の 1 件。`slot` は device の中で確保ごとに払い出す番号、`usage` は確保のときの用途。
struct BufferEntry {
    slice: CudaSlice<u8>,
    slot: u64,
    usage: BufferUsage,
}

/// device が持つバッファの表と再利用の一覧。ハンドルの id から `BufferEntry` を引く。
#[derive(Default)]
struct Buffers {
    next_id: u64,
    next_slot: u64,
    map: BTreeMap<u64, BufferEntry>,
    pool: ReusePool<CudaSlice<u8>, CudaEvent>,
}

/// device が持つパイプラインの表。識別子は device ごとに 1 から払い出し、関数と配置の写しを引く。
#[derive(Default)]
struct Pipelines {
    next_id: u64,
    map: BTreeMap<u64, PipelineEntry>,
}

/// パイプラインの表の 1 件。`CudaFunction` はモジュールを保持しないので、モジュールを同じ値に持たせて
/// 関数より長く生かす（`CudaModule` の drop が `cuModuleUnload` を呼ぶ）。
/// フィールドは宣言順に drop されるので、関数、配置、モジュールの順で解放される。
struct PipelineEntry {
    _function: CudaFunction,
    _layout: BindingLayout,
    _module: Arc<CudaModule>,
}

/// `request_device` が返す device。`Device`・`Capabilities`・`MemoryOps`・`ComputeOps`・`CommandQueue` を実装する。
struct CudaDevice {
    id: u64,
    generation: u64,
    ownership: DeviceOwnership,
    adapter_name: String,
    limits: LimitsTable,
    compute_capability: (i32, i32),
    /// `query`・`query_ex` が読む能力の判定の材料。`open_device` で 1 回だけ捕捉する。
    capabilities: CapabilitySnapshot,
    stream: Arc<CudaStream>,
    buffers: Mutex<Buffers>,
    pipelines: Mutex<Pipelines>,
    open: Arc<OpenEncoders>,
    submissions: Mutex<Submissions<CudaEvent>>,
}

impl Drop for CudaDevice {
    /// 終端を待つ登録済みの callback を、`Lost` でちょうど 1 回ずつ呼んでから破棄する。
    fn drop(&mut self) {
        let callbacks = self
            .submissions
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_all_callbacks();
        // 1 つが panic しても残りを呼び切る。巻き戻し中の二重 panic を避けるため、panic は外へ伝えず捨てる。
        let ready = callbacks
            .into_iter()
            .map(|callback| (callback, CompletionState::Lost))
            .collect();
        if let Some(payload) = run_callbacks(ready) {
            discard_panic(payload);
        }
    }
}

/// 使わない panic の中身を捨てる。中身の `Drop` が panic しても、そこから巻き戻らない。
fn discard_panic(payload: Box<dyn std::any::Any + Send>) {
    if let Err(inner) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
        std::mem::forget(inner);
    }
}

/// 表から外した callback を全部呼ぶ。1 つが panic しても残りを呼び切り、最初の panic の中身を返す。
///
/// 各 callback は呼ぶ前に表から外れているので、取りこぼしと二重呼び出しは起きない。錠を放してから呼ぶこと。
/// 呼ぶ側が通常経路では返った中身を `resume_unwind` で再送出し、`Drop` では捨てる。
/// 2 個目以降の panic の中身は `discard_panic` で捨てる。
fn run_callbacks<C: FnOnce(CompletionState)>(
    ready: Vec<(C, CompletionState)>,
) -> Option<Box<dyn std::any::Any + Send>> {
    let mut first_panic = None;
    for (callback, state) in ready {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| callback(state))) {
            if first_panic.is_none() {
                first_panic = Some(payload);
            } else {
                discard_panic(payload);
            }
        }
    }
    first_panic
}

impl CudaDevice {
    /// 投入済みの表の錠を取る。poison されていても中身を使う。
    fn lock_submissions(&self) -> MutexGuard<'_, Submissions<CudaEvent>> {
        self.submissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 試験専用。投入に成功した回数の通算。
    #[cfg(test)]
    fn test_submitted_count(&self) -> u64 {
        self.lock_submissions().next_sequence - 1
    }

    /// 試験専用。開いている記録先の数。
    #[cfg(test)]
    fn test_open_encoder_count(&self) -> usize {
        self.open.lock().len()
    }

    /// 試験専用。再利用の一覧から取り出された回数の通算。
    #[cfg(all(test, feature = "native-blocking-wait"))]
    fn test_pool_reuse_count(&self) -> u64 {
        self.lock().pool.reuse_count
    }

    /// 試験専用。取り出した slot が直前に一覧へ入れた slot と一致した回数の通算。
    #[cfg(all(test, feature = "native-blocking-wait"))]
    fn test_pool_slot_match_count(&self) -> u64 {
        self.lock().pool.slot_match_count
    }

    /// 試験専用。再利用の一覧の件数（待つ側と使える側の合計）。
    #[cfg(all(test, feature = "native-blocking-wait"))]
    fn test_pool_len(&self) -> usize {
        self.lock().pool.entries
    }

    /// 試験専用。表にあるハンドルの slot。表に無いハンドルは `None`。
    #[cfg(all(test, feature = "native-blocking-wait"))]
    fn test_slot_of(&self, handle: &BufferHandle) -> Option<u64> {
        self.lock()
            .map
            .get(&handle.__backend_id())
            .map(|entry| entry.slot)
    }

    /// stream に event を記録して通し番号を払い出し、印を返す。呼ぶ側が投入済みの表の錠を持つ。
    ///
    /// 記録の失敗は `BackendFailure` で、番号を消費せず表を変えない。完了は待たない。
    fn issue_event(&self, subs: &mut Submissions<CudaEvent>) -> Result<SubmitToken, ContractError> {
        let sequence = subs.issue(|| {
            guard_memory_panic("record_event", || {
                self.stream
                    .record_event(None)
                    .map_err(|e| backend_failure("record_event", e))
            })
        })?;
        Ok(SubmitToken::__backend_new(
            self.generation,
            sequence,
            self.id,
        ))
    }

    /// 回収してから、終端になった登録済みの callback を錠の外で呼ぶ。
    ///
    /// callback が panic しても残りを呼び切り、最初の panic は呼び切った後に再送出する。
    fn drive_callbacks(&self) {
        if let Some(payload) = self.drive_callbacks_catching() {
            std::panic::resume_unwind(payload);
        }
    }

    /// `drive_callbacks` の本体。最初の panic の中身を再送出せずに返す。
    fn drive_callbacks_catching(&self) -> Option<Box<dyn std::any::Any + Send>> {
        let ready = {
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            subs.take_ready(event_state)
        };
        run_callbacks(ready)
    }

    /// バッファの表のロックを取る。poison されていても中身を使う。
    fn lock(&self) -> MutexGuard<'_, Buffers> {
        self.buffers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 再利用の一覧の待つ側の完了を確かめ、確かめられないバッファを解放する。`allocate` の入口から呼ばれる。
    fn settle_pool(&self) {
        let dropped = catch_unwind(AssertUnwindSafe(|| {
            self.lock().pool.settle(|event| match event_state(event) {
                GroupState::Completed => Ok(true),
                GroupState::Pending => Ok(false),
                GroupState::Failed => Err(()),
            })
        }))
        .unwrap_or_default();
        // ドライバへの解放はロックの外で呼ぶ。
        let _ = catch_unwind(AssertUnwindSafe(move || drop(dropped)));
    }

    /// ハンドルの所属と範囲を検査して `[start, end)` を返す。`cudarc` の境界の panic に届く前に拒む。
    fn range(
        &self,
        table: &BTreeMap<u64, BufferEntry>,
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
        let entry = table
            .get(&handle.__backend_id())
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))?;
        validate_buffer_range(offset, len, entry.slice.len() as u64)?;
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
        // 確保できなければ abort せず `AllocationFailed` で返す。
        let mut out: Vec<u8> = Vec::new();
        out.try_reserve_exact(end - start)
            .map_err(|_| ContractError::AllocationFailed {
                requested_bytes: len,
            })?;
        out.resize(end - start, 0);
        if out.is_empty() {
            return Ok(out);
        }
        let slice = &buffers
            .map
            .get(&src.__backend_id())
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))?
            .slice;
        // 転送が終わったか、転送が始まらなかったと分かったときだけ `true` にする。
        let mut settled = false;
        let copied = guard_memory_panic("memcpy_dtoh", || {
            if let Err(e) = self
                .stream
                .memcpy_dtoh(&slice.slice(start..end), out.as_mut_slice())
            {
                settled = true;
                return Err(backend_failure("memcpy_dtoh", e));
            }
            self.stream
                .synchronize()
                .map_err(|e| backend_failure("synchronize", e))?;
            settled = true;
            Ok(())
        });
        if let Err(e) = copied {
            if !settled {
                // 転送が終わったと確かめられないので、書き込み先を解放しない。
                std::mem::forget(out);
            }
            return Err(e);
        }
        Ok(out)
    }
}

impl Capabilities for CudaDevice {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        cuda_capability_of(&self.capabilities, feature).level
    }

    fn limits(&self) -> &LimitsTable {
        &self.limits
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        cuda_capability_of(&self.capabilities, feature)
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
        self.settle_pool();
        // id は確保の前に払い出す。尽きたときに device のメモリを確保しない。
        // 一覧からの取り出しは id の払い出しの後に行い、id が尽きたときに一覧のバッファを減らさない。
        let key = (byte_len, usage.bits());
        let (id, reused) = {
            let mut buffers = self.lock();
            buffers.next_id =
                buffers
                    .next_id
                    .checked_add(1)
                    .ok_or(ContractError::AllocationFailed {
                        requested_bytes: byte_len,
                    })?;
            (buffers.next_id, buffers.pool.take(key))
        };
        let (slice, slot) =
            match reused {
                Some((slot, slice)) => (slice, slot),
                None => {
                    let slice = guard_memory_panic("alloc_zeros", || {
                        let slice = self
                            .stream
                            .alloc_zeros::<u8>(len)
                            .map_err(|e| allocation_failure("alloc_zeros", byte_len, e))?;
                        self.stream
                            .synchronize()
                            .map_err(|e| allocation_failure("synchronize", byte_len, e))?;
                        Ok(slice)
                    })?;
                    let mut buffers = self.lock();
                    buffers.next_slot = buffers.next_slot.checked_add(1).ok_or(
                        ContractError::AllocationFailed {
                            requested_bytes: byte_len,
                        },
                    )?;
                    (slice, buffers.next_slot)
                }
            };
        self.lock()
            .map
            .insert(id, BufferEntry { slice, slot, usage });
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
        let slice = &mut buffers
            .map
            .get_mut(&dst.__backend_id())
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))?
            .slice;
        guard_memory_panic("memcpy_htod", || {
            self.stream
                .memcpy_htod(bytes, &mut slice.slice_mut(start..end))
                .and_then(|()| self.stream.synchronize())
                .map_err(|e| backend_failure("memcpy_htod", e))
        })
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
        let Some(entry) = self.lock().map.remove(&buffer.__backend_id()) else {
            return;
        };
        // 鍵は表の項目から決める。ゼロ化は stream に積むだけで host は待たない。
        let BufferEntry { slice, slot, usage } = entry;
        let byte_len = slice.len() as u64;
        let key = (byte_len, usage.bits());
        // ドライバへの解放はロックの外で呼ぶ。`free` は値を返さないので panic も外へ出さない。
        let rejected = catch_unwind(AssertUnwindSafe(|| {
            retire_flow(
                byte_len,
                slice,
                |len| self.lock().pool.would_accept(len),
                |slice| {
                    self.stream.memset_zeros(slice)?;
                    self.stream.record_event(None)
                },
                |slice, done| self.lock().pool.retire(key, slot, slice, done),
            )
        }));
        if let Ok(Some(slice)) = rejected {
            let _ = catch_unwind(AssertUnwindSafe(move || drop(slice)));
        }
    }
}

impl ComputeOps for CudaDevice {
    /// 入力の分類、`slots` の重複、NUL のバイトの順に照らし、最初に当たったものを返す。
    /// 通った `CudaC` は NVRTC で PTX にし、モジュールの読み込みと関数の取得を経て、
    /// 関数と配置の写しを device のパイプラインの表に登録してハンドルを返す。
    /// NVRTC のライブラリが無ければ `Unsupported`。
    fn create_pipeline(
        &self,
        input: PipelineInput<'_>,
        layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        let (source, entry_point) = classify_pipeline_input(&input)?;
        check_unique_slots(&layout.slots)?;
        check_no_nul(source, entry_point)?;
        let ptx = compile_cuda_c(source, self.compute_capability)?;
        let ctx = self.stream.context();
        let (module, function) = guard_pipeline_panic(|| {
            ctx.bind_to_thread()
                .map_err(|e| backend_failure("bind_to_thread", e))?;
            let module = ctx.load_module(ptx).map_err(|e| {
                artifact_failure(
                    "module",
                    "cuModuleLoadData",
                    driver_error_is_artifact_cause(&e),
                    e,
                )
            })?;
            let function = module.load_function(entry_point).map_err(|e| {
                artifact_failure(
                    "function",
                    "cuModuleGetFunction",
                    driver_error_is_artifact_cause(&e),
                    e,
                )
            })?;
            Ok((module, function))
        })?;
        let mut pipelines = self
            .pipelines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = pipelines
            .next_id
            .checked_add(1)
            .ok_or_else(|| ContractError::BackendFailure {
                op: "create_pipeline",
                backend: BACKEND,
                detail: "pipeline id space is exhausted".into(),
            })?;
        pipelines.next_id = id;
        pipelines.map.insert(
            id,
            PipelineEntry {
                _function: function,
                _layout: layout.clone(),
                _module: module,
            },
        );
        Ok(PipelineHandle::__backend_new(id, self.id))
    }
}

impl CommandQueue for CudaDevice {
    /// 記録先の識別子を払い出し、この device の開いている集合に入れる。何も投入しない。
    fn create_encoder(&self) -> CommandEncoder {
        let closer: Arc<dyn __BackendSharedEncoderCloser> = self.open.clone();
        match __backend_next_encoder_id() {
            Some(id) => {
                self.open.open(id);
                CommandEncoder::__backend_new_shared(id, closer)
            }
            // 識別子が尽きたときの 0 は集合に入れない。投入は `InvalidArgument` で拒まれる。
            None => CommandEncoder::__backend_new_shared(0, closer),
        }
    }

    /// 記録先を投入し、完了を待たずに印を返す。
    ///
    /// 最初に投入済みの表を回収する。空でない `wait` は `Unsupported`、記録先がこの device の
    /// 開いている集合に無ければ `InvalidArgument` で、どちらも何も投入しない。通れば stream に event を 1 つ記録する。
    fn submit(
        &self,
        mut commands: CommandEncoder,
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        let result = {
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            reject_wait("submit(wait)", wait)
                .and_then(|()| self.open.claim(&[commands.__backend_id()]))
                .and_then(|()| {
                    commands.__backend_disarm_closer();
                    self.issue_event(&mut subs)
                })
        };
        self.drive_callbacks();
        result
    }
    /// 複数の記録先を 1 回で投入し、印を 1 つ返す。
    ///
    /// 最初に投入済みの表を回収する。検査の順は、`after` の各印（別 device は `DeviceMismatch`、
    /// 世代違いは `DeviceLost`）、空の列（`InvalidArgument`）、`wait`（`Unsupported`）、全記録先の所属
    /// （`InvalidArgument`）。検査に当たると何も投入しない。1 つの stream に順に積むので、`after` は照合だけで
    /// 追加の待ちは入れない。通れば、錠を放してから終端になった登録済みの callback を呼ぶ（`submit` と同じ）。
    fn submit_many(
        &self,
        mut commands: Vec<CommandEncoder>,
        after: &[SubmitToken],
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        let result = {
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            precheck_submit_many(commands.len(), after, wait, self.id, self.generation)
                .and_then(|()| {
                    let ids: Vec<u64> = commands.iter().map(CommandEncoder::__backend_id).collect();
                    self.open.claim(&ids)
                })
                .and_then(|()| {
                    for command in &mut commands {
                        command.__backend_disarm_closer();
                    }
                    self.issue_event(&mut subs)
                })
        };
        self.drive_callbacks();
        result
    }
    /// 完了の問合せ。待たない。
    ///
    /// 別 device・世代違いの印、払い出していない番号、失敗した列は `Lost`。
    /// 回収のあとで、終端になった登録済みの callback を錠の外で呼ぶ。
    fn poll(&self, token: SubmitToken) -> CompletionState {
        if check_token_owner(&token, self.id, self.generation).is_err() {
            return CompletionState::Lost;
        }
        let state = {
            let mut subs = self.lock_submissions();
            subs.reclaim_and_state_of(token.sequence, event_state)
        };
        self.drive_callbacks();
        state
    }
    /// 終端の状態で `callback` をちょうど 1 回呼ぶ。待たない。
    ///
    /// 終端ならこの呼び出しの中で呼ぶ。未完了なら登録して戻り、`submit`・`submit_many`・`poll`・`poll_ex`・`on_complete` の
    /// どれかの中で終端と分かった時に、その呼び出しを行ったスレッドで呼ぶ。
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        if check_token_owner(&token, self.id, self.generation).is_err() {
            callback(CompletionState::Lost);
            return;
        }
        let terminal = {
            let mut subs = self.lock_submissions();
            match subs.reclaim_and_state_of(token.sequence, event_state) {
                CompletionState::Pending => {
                    subs.callbacks.push((token.sequence, callback));
                    None
                }
                CompletionState::Complete => Some((callback, CompletionState::Complete)),
                CompletionState::Lost => Some((callback, CompletionState::Lost)),
            }
        };
        let own_panic =
            terminal.and_then(|(callback, state)| run_callbacks(vec![(callback, state)]));
        let drive_panic = self.drive_callbacks_catching();
        if let Some(payload) = own_panic {
            if let Some(extra) = drive_panic {
                discard_panic(extra);
            }
            std::panic::resume_unwind(payload);
        }
        if let Some(payload) = drive_panic {
            std::panic::resume_unwind(payload);
        }
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

    fn snapshot(cc: (i32, i32), map: Option<bool>) -> CapabilitySnapshot {
        CapabilitySnapshot {
            compute_capability: cc,
            can_map_host_memory: map,
        }
    }

    const THRESHOLD_ROWS: [FeatureId; 5] = [
        FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX,
        FEATURE_SHADER_F16,
        FEATURE_SUBGROUP,
        FEATURE_SUBGROUP_BARRIER,
        FEATURE_SHADER_FLOAT32_ATOMIC,
    ];

    /// 閾値の 5 行は、下限以上の compute capability で `S2`、未満で `Unsupported` になる（DEV-85）。
    #[test]
    fn threshold_rows_follow_the_compute_capability() {
        for f in THRESHOLD_ROWS {
            assert_eq!(
                cuda_capability_of(&snapshot((7, 5), Some(true)), f),
                runtime_decided(true)
            );
            assert_eq!(
                cuda_capability_of(&snapshot((9, 0), Some(true)), f),
                runtime_decided(true)
            );
            assert_eq!(
                cuda_capability_of(&snapshot((7, 4), Some(true)), f),
                runtime_decided(false)
            );
        }
    }

    /// 規則の下限を下回る compute capability では `Unsupported` になる。
    #[test]
    fn a_rule_below_its_threshold_is_not_supported() {
        let rule = CudaRule::MinComputeCapability { major: 8, minor: 0 };
        assert_eq!(
            evaluate(&rule, &snapshot((7, 5), None)),
            runtime_decided(false)
        );
        assert_eq!(
            evaluate(&rule, &snapshot((8, 0), None)),
            runtime_decided(true)
        );
    }

    /// ホストメモリのマップの行は属性に従い、読み取りに失敗していれば `S0` になる（DEV-85）。
    #[test]
    fn mappable_follows_the_attribute_and_is_s0_when_unread() {
        let f = FEATURE_MAPPABLE_PRIMARY_BUFFERS;
        assert_eq!(
            cuda_capability_of(&snapshot((9, 0), Some(true)), f),
            runtime_decided(true)
        );
        assert_eq!(
            cuda_capability_of(&snapshot((9, 0), Some(false)), f),
            runtime_decided(false)
        );
        let failed = can_map_host_memory_of(|| Err::<i32, _>("injected"));
        let q = cuda_capability_of(&snapshot((9, 0), failed), f);
        assert_eq!(q.level, CapabilityLevel::S0);
        assert_eq!(q.unsupported_reason, None);
        assert_eq!(q.evidence, None);
        assert_eq!(
            cuda_capability_of(&snapshot((7, 4), None), f),
            runtime_decided(false)
        );
        assert_eq!(can_map_host_memory_of(|| Ok::<_, &str>(1)), Some(true));
        assert_eq!(can_map_host_memory_of(|| Ok::<_, &str>(0)), Some(false));
        assert_eq!(can_map_host_memory_of(|| Ok::<_, &str>(2)), None);
    }

    /// どの鍵・compute capability・属性の組でも、`S3` 以上は返らない（DEV-89）。
    #[test]
    fn no_key_returns_s3_or_above() {
        for &key in fandhe_silicon_core::feature_key::FEATURE_QUERY_KEYS {
            for cc in [(1, 0), (5, 3), (7, 4), (7, 5), (9, 0), (99, 9)] {
                for map in [None, Some(false), Some(true)] {
                    let q = cuda_capability_of(&snapshot(cc, map), key);
                    assert!(matches!(
                        q.level,
                        CapabilityLevel::S0 | CapabilityLevel::S2 | CapabilityLevel::Unsupported
                    ));
                }
            }
        }
    }

    /// 表に行の無い鍵は、core の `UNLISTED_FEATURE_QUERY` と等しい値を返す。
    #[test]
    fn keys_without_a_row_are_unlisted() {
        for &key in fandhe_silicon_core::feature_key::FEATURE_QUERY_KEYS {
            if fandhe_silicon_core::capability_table::find_row(&CUDA_FEATURE_TABLE, key).is_none() {
                assert_eq!(
                    cuda_capability_of(&snapshot((9, 0), Some(true)), key),
                    fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY
                );
            }
        }
    }

    /// 表は core の共通の規則の検査（鍵の一覧との突き合わせと、問合せの値）を通る（DEV-87）。
    #[cfg(feature = "test-support")]
    #[test]
    fn table_passes_the_common_checks() {
        use fandhe_silicon_core::capability_table_check::{check_table_keys, check_table_queries};
        use fandhe_silicon_core::feature_key::FEATURE_QUERY_KEYS;

        assert_eq!(
            check_table_keys(&CUDA_FEATURE_TABLE, FEATURE_QUERY_KEYS),
            Ok(())
        );
        for cc in [(5, 3), (7, 4), (7, 5), (9, 0)] {
            for map in [None, Some(false), Some(true)] {
                let s = snapshot(cc, map);
                assert_eq!(
                    check_table_queries(&CUDA_FEATURE_TABLE, FEATURE_QUERY_KEYS, |rule| {
                        evaluate(rule, &s)
                    }),
                    Ok(())
                );
            }
        }
    }

    /// 完了を確かめられたものだけが、鍵の一致する確保に取り出される。
    #[test]
    fn pool_takes_only_settled_buffers_with_the_same_key() {
        let mut pool = ReusePool::<Vec<u8>, bool>::default();
        assert!(pool.retire((8, 3), 1, vec![0; 8], true).is_none());
        assert!(pool.take((8, 3)).is_none());
        assert!(pool.settle(|done| Ok::<bool, ()>(*done)).is_empty());
        assert!(pool.take((8, 4)).is_none());
        assert!(pool.take((16, 3)).is_none());
        assert_eq!(pool.reuse_count, 0);
        let (slot, buffer) = pool.take((8, 3)).expect("settled buffer");
        assert_eq!((slot, buffer.len()), (1, 8));
        assert_eq!((pool.reuse_count, pool.slot_match_count), (1, 1));
        assert_eq!((pool.entries, pool.bytes), (0, 0));
        assert!(pool.take((8, 3)).is_none());
        // 直前に入れた slot でないものは slot の一致に数えない。
        assert!(pool.retire((8, 3), 2, vec![0; 8], true).is_none());
        assert!(pool.retire((8, 3), 3, vec![0; 8], true).is_none());
        pool.settle(|done| Ok::<bool, ()>(*done));
        assert_eq!(pool.take((8, 3)).map(|(slot, _)| slot), Some(3));
        assert_eq!(pool.take((8, 3)).map(|(slot, _)| slot), Some(2));
        assert_eq!((pool.reuse_count, pool.slot_match_count), (3, 2));
    }

    /// 完了を確かめられないバッファは一覧から外れて返り、件数と合計から引かれる。
    #[test]
    fn pool_drops_buffers_whose_zeroing_cannot_be_confirmed() {
        let mut pool = ReusePool::<u8, u8>::default();
        assert!(pool.retire((4, 1), 1, 10, 0).is_none());
        assert!(pool.retire((4, 1), 2, 11, 1).is_none());
        assert!(pool.retire((4, 1), 3, 12, 2).is_none());
        let dropped = pool.settle(|done| match *done {
            0 => Err("lost"),
            1 => Ok(false),
            _ => Ok(true),
        });
        assert_eq!(dropped, vec![10]);
        assert_eq!((pool.entries, pool.bytes), (2, 8));
        assert_eq!(pool.take((4, 1)).map(|(_, buffer)| buffer), Some(12));
        assert!(pool.take((4, 1)).is_none());
        let dropped = pool.settle(|done| {
            if *done == 1 {
                Ok::<bool, ()>(true)
            } else {
                Ok(false)
            }
        });
        assert!(dropped.is_empty());
        assert_eq!(pool.take((4, 1)).map(|(_, buffer)| buffer), Some(11));
    }

    /// 待つ側の分も件数と合計に数え、上限を超えるものは入れずに返す。
    #[test]
    fn pool_counts_zeroing_entries_toward_the_limits() {
        let mut pool = ReusePool::<u8, ()>::default();
        for i in 0..POOL_MAX_ENTRIES as u64 {
            assert!(pool.would_accept(1));
            assert!(pool.retire((1, 0), i, 0, ()).is_none());
        }
        assert!(!pool.would_accept(1));
        assert_eq!(pool.retire((1, 0), 99, 7, ()), Some((7, ())));
        assert_eq!(pool.entries, POOL_MAX_ENTRIES);

        let mut pool = ReusePool::<u8, ()>::default();
        assert!(pool.would_accept(POOL_MAX_BYTES));
        assert!(!pool.would_accept(POOL_MAX_BYTES + 1));
        assert!(!pool.would_accept(u64::MAX));
        assert!(pool.retire((POOL_MAX_BYTES, 0), 1, 0, ()).is_none());
        assert!(!pool.would_accept(1));
        assert_eq!(pool.retire((1, 0), 2, 5, ()), Some((5, ())));
    }

    /// 入れられないときはゼロ化を積まず、ゼロ化の失敗では一覧に入れず、バッファを返す。
    #[test]
    fn retire_flow_skips_zeroing_when_rejected_and_keeps_failed_buffers_out() {
        let rejected = retire_flow(
            4,
            1u8,
            |_| false,
            |_| -> Result<u8, ()> { panic!("zero must not run") },
            |_, _| panic!("put must not run"),
        );
        assert_eq!(rejected, Some(1));
        let failed = retire_flow(
            4,
            2u8,
            |_| true,
            |_| Err::<u8, _>("zero failed"),
            |_, _| panic!("put must not run"),
        );
        assert_eq!(failed, Some(2));
        let accepted = retire_flow(4, 3u8, |_| true, |_| Ok::<u8, ()>(9), |_, _| None);
        assert_eq!(accepted, None);
        let refused = retire_flow(4, 4u8, |_| true, |_| Ok::<u8, ()>(9), |b, e| Some((b, e)));
        assert_eq!(refused, Some(4));
    }

    /// 一覧の観測できる状態。件数・合計・待つ側の数・使える側の鍵ごとの slot の並び・直前の slot・2 つの回数。
    type PoolState = (
        usize,
        u64,
        usize,
        Vec<(PoolKey, Vec<u64>)>,
        Option<u64>,
        u64,
        u64,
    );

    fn pool_state<B, E>(pool: &ReusePool<B, E>) -> PoolState {
        (
            pool.entries,
            pool.bytes,
            pool.zeroing.len(),
            pool.ready
                .iter()
                .map(|(key, list)| (*key, list.iter().map(|(slot, _)| *slot).collect()))
                .collect(),
            pool.last_put_slot,
            pool.reuse_count,
            pool.slot_match_count,
        )
    }

    /// 件数の上限は待つ側だけ・使える側だけ・両方の混在のどれでもちょうどで効く。
    #[test]
    fn pool_entry_limit_is_exact_on_the_zeroing_side_the_ready_side_and_both() {
        let max = POOL_MAX_ENTRIES as u64;
        // 待つ側だけ。
        let mut pool = ReusePool::<u8, ()>::default();
        for i in 0..max - 1 {
            assert!(pool.retire((1, 0), i, 0, ()).is_none());
        }
        assert!(pool.would_accept(1));
        assert!(pool.retire((1, 0), max - 1, 0, ()).is_none());
        assert!(!pool.would_accept(1));
        let before = pool_state(&pool);
        assert_eq!(pool.retire((1, 0), 99, 7, ()), Some((7, ())));
        assert_eq!(pool_state(&pool), before);

        // 使える側だけ。
        let mut pool = ReusePool::<u8, ()>::default();
        for i in 0..max {
            assert!(pool.retire((1, 0), i, 0, ()).is_none());
        }
        assert!(pool.settle(|_| Ok::<bool, ()>(true)).is_empty());
        assert_eq!((pool.zeroing.len(), pool.entries), (0, POOL_MAX_ENTRIES));
        let before = pool_state(&pool);
        assert!(!pool.would_accept(1));
        assert_eq!(pool.retire((1, 0), 99, 7, ()), Some((7, ())));
        assert_eq!(pool_state(&pool), before);
        assert!(pool.take((1, 0)).is_some());
        assert!(pool.retire((1, 0), 100, 0, ()).is_none());
        assert_eq!(pool.retire((1, 0), 101, 8, ()), Some((8, ())));

        // 混在。
        let mut pool = ReusePool::<u8, u8>::default();
        for i in 0..max / 2 {
            assert!(pool.retire((1, 0), i, 0, 1).is_none());
        }
        assert!(pool.settle(|_| Ok::<bool, ()>(true)).is_empty());
        for i in max / 2..max - 1 {
            assert!(pool.retire((1, 0), i, 0, 2).is_none());
        }
        assert!(pool.retire((1, 0), max - 1, 5, 3).is_none());
        assert_eq!((pool.zeroing.len(), pool.ready.len()), (8, 1));
        let before = pool_state(&pool);
        assert!(!pool.would_accept(1));
        assert_eq!(pool.retire((1, 0), 99, 7, 4), Some((7, 4)));
        assert_eq!(pool_state(&pool), before);
        let dropped = pool.settle(|done| if *done == 3 { Err(()) } else { Ok(false) });
        assert_eq!(dropped, vec![5]);
        assert!(pool.retire((1, 0), 100, 0, 4).is_none());
        assert_eq!(pool.retire((1, 0), 101, 9, 4), Some((9, 4)));
    }

    /// 大きさの合計の上限は合計がちょうどのときまで入り、あふれる足し算は一覧を変えずに拒む。
    #[test]
    fn pool_byte_limit_is_exact_and_an_overflowing_sum_leaves_the_pool_unchanged() {
        let mut pool = ReusePool::<u8, ()>::default();
        assert!(pool.retire((POOL_MAX_BYTES - 1, 0), 1, 0, ()).is_none());
        assert!(pool.would_accept(1));
        assert!(pool.retire((1, 0), 2, 0, ()).is_none());
        assert_eq!((pool.entries, pool.bytes), (2, POOL_MAX_BYTES));
        let before = pool_state(&pool);
        assert!(!pool.would_accept(1));
        assert_eq!(pool.retire((1, 0), 3, 5, ()), Some((5, ())));
        assert_eq!(pool_state(&pool), before);

        let half = POOL_MAX_BYTES / 2;
        let mut pool = ReusePool::<u8, ()>::default();
        assert!(pool.retire((half, 0), 1, 0, ()).is_none());
        pool.settle(|_| Ok::<bool, ()>(true));
        assert!(pool.retire((half, 1), 2, 0, ()).is_none());
        let before = pool_state(&pool);
        assert!(!pool.would_accept(1));
        assert_eq!(pool.retire((1, 0), 3, 6, ()), Some((6, ())));
        assert_eq!(pool_state(&pool), before);

        let mut pool = ReusePool::<u8, ()>::default();
        assert!(pool.retire((1, 0), 1, 0, ()).is_none());
        let before = pool_state(&pool);
        assert!(!pool.would_accept(u64::MAX));
        assert_eq!(pool.retire((u64::MAX, 0), 2, 4, ()), Some((4, ())));
        assert_eq!(pool_state(&pool), before);

        pool.entries = usize::MAX;
        let before = pool_state(&pool);
        assert!(!pool.would_accept(1));
        assert_eq!(pool.retire((1, 0), 3, 3, ()), Some((3, ())));
        assert_eq!(pool_state(&pool), before);
    }

    /// 取り出しは長さと用途のビットの両方が一致するときだけ。外れた問い合わせは何も変えない。
    #[test]
    fn pool_take_matches_only_when_both_length_and_usage_bits_are_equal() {
        let bits = BufferUsage::STORAGE.bits();
        let other = (BufferUsage::STORAGE | BufferUsage::UPLOAD).bits();
        let mut pool = ReusePool::<u8, ()>::default();
        assert!(pool.retire((8, bits), 1, 3, ()).is_none());
        pool.settle(|_| Ok::<bool, ()>(true));
        let before = pool_state(&pool);
        assert!(pool.take((16, bits)).is_none());
        assert!(pool.take((8, other)).is_none());
        assert!(pool.take((16, other)).is_none());
        assert_eq!(pool_state(&pool), before);
        assert_eq!(pool.take((8, bits)), Some((1, 3)));
        assert_eq!((pool.entries, pool.bytes, pool.reuse_count), (0, 0, 1));
    }

    /// 完了の問い合わせの 3 値の扱い。未完了は残り、使える側のものは再び問い合わせない。
    #[test]
    fn pool_settle_keeps_pending_moves_them_later_and_subtracts_only_failed_entries() {
        let calls = std::cell::Cell::new(0u32);
        let mut pool = ReusePool::<u32, u8>::default();
        for (slot, len) in [(1, 4u8), (2, 8), (3, 16)] {
            assert!(
                pool.retire((u64::from(len), 0), slot, u32::from(len), len)
                    .is_none()
            );
        }
        let dropped = pool.settle(|done| {
            calls.set(calls.get() + 1);
            if *done == 8 { Err(()) } else { Ok(false) }
        });
        assert_eq!((dropped, calls.get()), (vec![8], 3));
        assert_eq!((pool.entries, pool.bytes, pool.zeroing.len()), (2, 20, 2));
        assert!(pool.ready.is_empty());
        assert!(pool.take((4, 0)).is_none());

        let before = pool_state(&pool);
        assert!(pool.settle(|_| Ok::<bool, ()>(false)).is_empty());
        assert_eq!(pool_state(&pool), before);

        assert!(pool.settle(|_| Ok::<bool, ()>(true)).is_empty());
        assert_eq!((pool.entries, pool.bytes, pool.zeroing.len()), (2, 20, 0));

        // 使える側のものは閉包に渡らない。
        assert!(pool.retire((32, 0), 4, 32, 32).is_none());
        calls.set(0);
        assert!(
            pool.settle(|_| {
                calls.set(calls.get() + 1);
                Ok::<bool, ()>(false)
            })
            .is_empty()
        );
        assert_eq!((calls.get(), pool.entries, pool.bytes), (1, 3, 52));
        assert_eq!(pool.take((4, 0)).map(|(_, b)| b), Some(4));
        assert_eq!(pool.take((16, 0)).map(|(_, b)| b), Some(16));
        assert_eq!((pool.entries, pool.bytes), (1, 32));

        let mut empty = ReusePool::<u8, u8>::default();
        assert!(
            empty
                .settle(|_| -> Result<bool, ()> { panic!("no item to ask") })
                .is_empty()
        );
    }

    /// slot の一致は、直前に入れた slot を取り出したときだけ数える。拒まれた入れ方は直前の slot を動かさない。
    #[test]
    fn pool_counts_a_slot_match_only_for_the_most_recently_put_slot() {
        let key = (8, 0);
        let mut pool = ReusePool::<u8, ()>::default();
        for slot in 1..=3 {
            assert!(pool.retire(key, slot, 0, ()).is_none());
        }
        pool.settle(|_| Ok::<bool, ()>(true));
        let slots: Vec<u64> = (0..3)
            .filter_map(|_| pool.take(key).map(|(s, _)| s))
            .collect();
        assert_eq!(slots, vec![3, 2, 1]);
        assert_eq!((pool.reuse_count, pool.slot_match_count), (3, 1));

        let mut pool = ReusePool::<u8, u8>::default();
        assert!(pool.retire(key, 1, 0, 1).is_none());
        pool.settle(|_| Ok::<bool, ()>(true));
        assert!(pool.retire(key, 2, 0, 2).is_none());
        assert_eq!(pool.take(key).map(|(s, _)| s), Some(1));
        assert_eq!((pool.reuse_count, pool.slot_match_count), (1, 0));

        let mut pool = ReusePool::<u8, ()>::default();
        assert!(pool.retire((8, 0), 1, 0, ()).is_none());
        assert!(pool.retire((16, 0), 2, 0, ()).is_none());
        pool.settle(|_| Ok::<bool, ()>(true));
        assert_eq!(pool.take((8, 0)).map(|(s, _)| s), Some(1));
        assert_eq!(pool.slot_match_count, 0);
        assert_eq!(pool.take((16, 0)).map(|(s, _)| s), Some(2));
        assert_eq!(pool.slot_match_count, 1);

        let mut pool = ReusePool::<u8, ()>::default();
        for slot in 0..POOL_MAX_ENTRIES as u64 {
            assert!(pool.retire((1, 0), slot, 0, ()).is_none());
        }
        assert_eq!(pool.retire((1, 0), 99, 1, ()), Some((1, ())));
        assert_eq!(pool.last_put_slot, Some(POOL_MAX_ENTRIES as u64 - 1));
        pool.settle(|_| Ok::<bool, ()>(true));
        assert_eq!(
            pool.take((1, 0)).map(|(s, _)| s),
            Some(POOL_MAX_ENTRIES as u64 - 1)
        );
        assert_eq!((pool.reuse_count, pool.slot_match_count), (1, 1));
    }

    /// 空の一覧・別の鍵・待つ側にしか無い鍵の取り出しは `None` で、何も数えず何も作らない。
    #[test]
    fn pool_take_on_an_empty_pool_or_another_key_returns_none_without_counting() {
        let mut pool = ReusePool::<u8, ()>::default();
        let empty = pool_state(&pool);
        assert!(pool.take((8, 0)).is_none());
        assert_eq!(pool_state(&pool), empty);

        assert!(pool.retire((8, 0), 1, 0, ()).is_none());
        let waiting = pool_state(&pool);
        assert!(pool.take((8, 0)).is_none());
        assert_eq!(pool_state(&pool), waiting);

        pool.settle(|_| Ok::<bool, ()>(true));
        let ready = pool_state(&pool);
        assert!(pool.take((16, 0)).is_none());
        assert_eq!(pool_state(&pool), ready);

        assert!(pool.take((8, 0)).is_some());
        assert!(pool.ready.is_empty());
        assert!(pool.take((8, 0)).is_none());
        assert_eq!((pool.reuse_count, pool.entries, pool.bytes), (1, 0, 0));
    }

    /// 流れの関数の呼び分け。受け入れないと `zero`・`put` を呼ばず、`zero` の失敗では `put` を呼ばない。
    #[test]
    fn retire_flow_calls_zero_and_put_only_when_allowed_and_returns_what_put_returns() {
        use std::cell::RefCell;
        let log = RefCell::new(Vec::<String>::new());
        let rejected = retire_flow(
            12,
            vec![1u8],
            |n| {
                log.borrow_mut().push(format!("accepts {n}"));
                false
            },
            |_| -> Result<u8, &str> { panic!("zero must not run") },
            |_, _| panic!("put must not run"),
        );
        assert_eq!(rejected, Some(vec![1]));
        assert_eq!(*log.borrow(), vec!["accepts 12".to_string()]);

        log.borrow_mut().clear();
        let failed = retire_flow(
            5,
            vec![1u8],
            |n| {
                log.borrow_mut().push(format!("accepts {n}"));
                true
            },
            |b| {
                b.push(2);
                log.borrow_mut().push("zero".into());
                Err::<u8, _>("zero failed")
            },
            |_, _| panic!("put must not run"),
        );
        assert_eq!(failed, Some(vec![1, 2]));
        assert_eq!(*log.borrow(), vec!["accepts 5".to_string(), "zero".into()]);

        log.borrow_mut().clear();
        let returned = retire_flow(
            5,
            vec![1u8],
            |_| true,
            |b| {
                b.push(2);
                Ok::<u8, &str>(7)
            },
            |b, e| {
                log.borrow_mut().push(format!("put {b:?} {e}"));
                Some((b, e))
            },
        );
        assert_eq!(returned, Some(vec![1, 2]));
        assert_eq!(*log.borrow(), vec!["put [1, 2] 7".to_string()]);
        let kept = retire_flow(5, vec![1u8], |_| true, |_| Ok::<u8, &str>(7), |_, _| None);
        assert_eq!(kept, None);

        let pool = RefCell::new(ReusePool::<Vec<u8>, u8>::default());
        for _ in 0..POOL_MAX_ENTRIES {
            let r = retire_flow(
                1,
                vec![0u8],
                |n| pool.borrow().would_accept(n),
                |_| Ok::<u8, &str>(0),
                |b, e| pool.borrow_mut().retire((1, 0), 1, b, e),
            );
            assert_eq!(r, None);
        }
        let before = pool_state(&pool.borrow());
        let full = retire_flow(
            1,
            vec![9u8],
            |n| pool.borrow().would_accept(n),
            |_| -> Result<u8, &str> { panic!("zero must not run") },
            |b, e| pool.borrow_mut().retire((1, 0), 2, b, e),
        );
        assert_eq!(full, Some(vec![9]));
        assert_eq!(pool_state(&pool.borrow()), before);
    }

    /// 最初の通し番号は 1 で、失敗した払い出しは番号を消費しない（CMD-87）。
    #[test]
    fn the_first_sequence_is_one_and_a_failed_issue_does_not_consume_a_number() {
        let mut t = Submissions::<GroupState>::new();
        assert!(
            t.issue(|| Err(ContractError::InvalidArgument("x".into())))
                .is_err()
        );
        assert_eq!(t.issue(|| Ok(GroupState::Pending)).ok(), Some(1));
        assert_eq!(t.issue(|| Ok(GroupState::Pending)).ok(), Some(2));
    }

    #[test]
    fn an_exhausted_sequence_does_not_call_make() {
        let mut t = Submissions::<GroupState>::new();
        t.next_sequence = u64::MAX;
        let mut called = false;
        let r = t.issue(|| {
            called = true;
            Ok(GroupState::Pending)
        });
        assert!(matches!(r, Err(ContractError::BackendFailure { .. })));
        assert!(!called);
    }

    #[test]
    fn reclaim_stops_at_a_pending_head_and_moves_failures_aside() {
        let mut t = Submissions::<GroupState>::new();
        for g in [
            GroupState::Completed,
            GroupState::Failed,
            GroupState::Pending,
            GroupState::Completed,
        ] {
            let _ = t.issue(|| Ok(g));
        }
        assert_eq!(t.reclaim(|g| *g), Some(3));
        assert_eq!(t.watermark, 2);
        assert!(t.failed.contains(&2));
        assert_eq!(t.pending.len(), 2);
    }

    #[test]
    fn states_cover_pending_complete_failed_reclaimed_and_unissued() {
        let mut t = Submissions::<GroupState>::new();
        for g in [
            GroupState::Completed,
            GroupState::Failed,
            GroupState::Pending,
        ] {
            let _ = t.issue(|| Ok(g));
        }
        let state = |t: &mut Submissions<GroupState>, n| t.reclaim_and_state_of(n, |g| *g);
        assert_eq!(state(&mut t, 1), CompletionState::Complete);
        assert_eq!(state(&mut t, 2), CompletionState::Lost);
        assert_eq!(state(&mut t, 3), CompletionState::Pending);
        assert_eq!(state(&mut t, 0), CompletionState::Lost);
        assert_eq!(state(&mut t, 4), CompletionState::Lost);
        assert_eq!(
            t.reclaim_and_state_of(3, |_| GroupState::Completed),
            CompletionState::Complete
        );
        assert!(t.pending.is_empty());
    }

    #[test]
    fn the_owner_check_rejects_foreign_and_stale_tokens() {
        let foreign = SubmitToken::__backend_new(1, 1, 99);
        let stale = SubmitToken::__backend_new(2, 1, 7);
        let own = SubmitToken::__backend_new(1, 1, 7);
        assert!(matches!(
            check_token_owner(&foreign, 7, 1),
            Err(ContractError::DeviceMismatch)
        ));
        assert!(matches!(
            check_token_owner(&stale, 7, 1),
            Err(ContractError::DeviceLost { generation: 2 })
        ));
        assert!(check_token_owner(&own, 7, 1).is_ok());
    }

    #[test]
    fn run_callbacks_calls_every_callback_once_despite_a_panic_and_returns_the_first() {
        use std::sync::atomic::AtomicUsize;
        let calls = Arc::new(AtomicUsize::new(0));
        let mut ready: Vec<(CompletionCallback, CompletionState)> = Vec::new();
        for i in 0..4 {
            let calls = Arc::clone(&calls);
            ready.push((
                Box::new(move |_| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    if i == 1 || i == 2 {
                        std::panic::panic_any(i);
                    }
                }),
                CompletionState::Complete,
            ));
        }
        let first = run_callbacks(ready).expect("panic is returned");
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert_eq!(*first.downcast::<i32>().unwrap(), 1);
    }

    #[test]
    fn discard_panic_does_not_unwind_even_if_the_payload_drop_panics() {
        struct PanicOnDrop;
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                std::panic::panic_any(0_u8);
            }
        }
        discard_panic(Box::new(PanicOnDrop));
    }

    #[test]
    fn take_ready_keeps_pending_callbacks_and_releases_terminal_ones() {
        let mut t = Submissions::<GroupState>::new();
        for g in [GroupState::Completed, GroupState::Pending] {
            let _ = t.issue(|| Ok(g));
        }
        t.callbacks.push((1, Box::new(|_| {})));
        t.callbacks.push((2, Box::new(|_| {})));
        let ready = t.take_ready(|g| *g);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].1, CompletionState::Complete);
        assert_eq!(t.callbacks.len(), 1);
    }

    #[test]
    fn a_stalled_head_stays_pending_even_if_the_group_reads_complete_afterwards() {
        let mut t = Submissions::<GroupState>::new();
        let _ = t.issue(|| Ok(GroupState::Completed));
        let _ = t.issue(|| Ok(GroupState::Completed));
        t.callbacks.push((2, Box::new(|_| {})));
        // 回収の時点では 1 件目が未完了だったと見なす。
        let calls = std::cell::Cell::new(0);
        let first = t.reclaim(|_| {
            calls.set(calls.get() + 1);
            GroupState::Pending
        });
        assert_eq!(first, Some(1));
        // 再読みでは完了と答えるが、止まった先頭は Pending のまま配信もされない。
        assert_eq!(t.state_of(1, |g| *g), CompletionState::Pending);
        t.callbacks.push((1, Box::new(|_| {})));
        let ready = t.take_ready(|g| *g);
        assert_eq!(ready.len(), 1);
        assert_eq!(t.callbacks.len(), 1);
        assert_eq!(t.callbacks[0].0, 1);
        assert!(t.pending.contains_key(&1));
    }

    #[test]
    fn take_all_callbacks_returns_every_registered_callback_once() {
        let mut t = Submissions::<GroupState>::new();
        t.callbacks.push((1, Box::new(|_| {})));
        t.callbacks.push((2, Box::new(|_| {})));
        assert_eq!(t.take_all_callbacks().len(), 2);
        assert!(t.callbacks.is_empty());
        assert!(t.take_all_callbacks().is_empty());
    }

    #[test]
    fn claim_rejects_unknown_zero_and_duplicate_ids_without_changing_the_set() {
        let open = OpenEncoders::default();
        open.open(0);
        open.open(5);
        open.open(6);
        for ids in [&[0_u64][..], &[7], &[5, 5], &[5, 7]] {
            assert!(matches!(
                open.claim(ids),
                Err(ContractError::InvalidArgument(_))
            ));
            assert_eq!(open.lock().len(), 2);
        }
        assert!(open.claim(&[5, 6]).is_ok());
        assert!(open.lock().is_empty());
    }

    #[test]
    fn close_removes_an_open_id_and_ignores_unknown_ones() {
        let open = OpenEncoders::default();
        open.open(9);
        open.close(1);
        assert_eq!(open.lock().len(), 1);
        open.close(9);
        assert!(open.lock().is_empty());
    }

    #[test]
    fn a_nonempty_wait_is_unsupported() {
        assert!(reject_wait("submit(wait)", &[]).is_ok());
        assert!(matches!(
            reject_wait("submit(wait)", &[ImportedSemaphoreHandle::__backend_new(1)]),
            Err(ContractError::Unsupported {
                op: "submit(wait)",
                ..
            })
        ));
    }

    /// CMD-67。
    #[test]
    fn submit_many_checks_after_then_empty_then_wait() {
        let foreign = SubmitToken::__backend_new(1, 1, 99);
        let stale = SubmitToken::__backend_new(2, 1, 7);
        let wait = [ImportedSemaphoreHandle::__backend_new(1)];
        assert!(matches!(
            precheck_submit_many(0, &[foreign], &wait, 7, 1),
            Err(ContractError::DeviceMismatch)
        ));
        assert!(matches!(
            precheck_submit_many(0, &[stale], &wait, 7, 1),
            Err(ContractError::DeviceLost { generation: 2 })
        ));
        assert!(matches!(
            precheck_submit_many(0, &[], &wait, 7, 1),
            Err(ContractError::InvalidArgument(_))
        ));
        assert!(matches!(
            precheck_submit_many(1, &[], &wait, 7, 1),
            Err(ContractError::Unsupported { .. })
        ));
        let own = SubmitToken::__backend_new(1, 1, 7);
        assert!(precheck_submit_many(1, &[own], &[], 7, 1).is_ok());
    }

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
    fn a_panic_in_the_memory_guard_is_backend_failure() {
        let r: Result<(), _> = guard_memory_panic("op", || panic!("boom"));
        match r {
            Err(ContractError::BackendFailure { op, backend, .. }) => {
                assert_eq!(op, "op");
                assert_eq!(backend, BACKEND);
            }
            other => panic!("unexpected result: {other:?}"),
        }
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

    /// compute capability は `read_attributes` が読んだ値のまま持つ。
    #[test]
    fn attributes_keep_the_compute_capability() {
        let a = read_attributes(
            || Ok::<_, &str>("GPU".to_string()),
            || Ok((12, 1)),
            || Ok(1024_usize),
        )
        .ok()
        .unwrap();
        assert_eq!(a.compute_capability, (12, 1));
    }

    /// NVRTC に渡すオプションは `--gpu-architecture` の 1 つだけ（COMP-22）。
    #[test]
    fn compile_options_are_only_the_gpu_architecture() {
        let o = compile_options((12, 1)).ok().unwrap();
        assert_eq!(
            o,
            CompileOptions {
                options: vec!["--gpu-architecture=compute_121".into()],
                ..CompileOptions::default()
            }
        );
        let o = compile_options((8, 6)).ok().unwrap();
        assert_eq!(o.options, vec!["--gpu-architecture=compute_86".to_string()]);
        assert!(matches!(
            compile_options((-1, 0)),
            Err(ContractError::BackendFailure {
                op: "create_pipeline",
                ..
            })
        ));
    }

    /// NVRTC のライブラリが読めない環境では、panic せず `Unsupported` を返す。
    /// 読める環境では device に触らず戻る。
    #[test]
    fn absent_nvrtc_library_is_unsupported_without_panicking() {
        if nvrtc_library_present() {
            println!("not-run: NVRTC library is present");
            return;
        }
        let r = compile_cuda_c("extern \"C\" __global__ void k() {}", (8, 6));
        assert!(matches!(
            r,
            Err(ContractError::Unsupported {
                op: "create_pipeline",
                backend: BackendKind::Cuda
            })
        ));
    }

    #[test]
    fn nvrtc_failures_map_to_artifact_rejected_or_backend_failure() {
        use cudarc::nvrtc::result::NvrtcError;
        use cudarc::nvrtc::sys::nvrtcResult;
        let compile = CompileError::CompileError {
            nvrtc: NvrtcError(nvrtcResult::NVRTC_ERROR_COMPILATION),
            options: vec![],
            log: std::ffi::CString::new("log").unwrap(),
        };
        assert!(matches!(
            nvrtc_failure(compile),
            ContractError::ArtifactRejected { kind: "cuda-c", .. }
        ));
        for code in [
            nvrtcResult::NVRTC_ERROR_OUT_OF_MEMORY,
            nvrtcResult::NVRTC_ERROR_INTERNAL_ERROR,
            nvrtcResult::NVRTC_ERROR_PROGRAM_CREATION_FAILURE,
        ] {
            assert!(matches!(
                nvrtc_failure(CompileError::CreationError(NvrtcError(code))),
                ContractError::BackendFailure {
                    op: "nvrtcCreateProgram",
                    ..
                }
            ));
        }
        for code in [
            nvrtcResult::NVRTC_ERROR_INVALID_PROGRAM,
            nvrtcResult::NVRTC_ERROR_INVALID_OPTION,
            nvrtcResult::NVRTC_ERROR_INVALID_INPUT,
        ] {
            assert!(matches!(
                nvrtc_failure(CompileError::GetPtxError(NvrtcError(code))),
                ContractError::ArtifactRejected { kind: "cuda-c", .. }
            ));
        }
        assert!(matches!(
            artifact_failure("module", "cuModuleLoadData", true, "x"),
            ContractError::ArtifactRejected { kind: "module", .. }
        ));
        assert!(matches!(
            artifact_failure("module", "cuModuleLoadData", false, "x"),
            ContractError::BackendFailure {
                op: "cuModuleLoadData",
                ..
            }
        ));
    }

    /// ドライバのコードは、成果物が原因のものだけ `ArtifactRejected` の側に分類される。
    #[test]
    fn driver_codes_are_classified_by_artifact_cause() {
        use cudarc::driver::sys::CUresult;
        for code in [
            CUresult::CUDA_ERROR_INVALID_PTX,
            CUresult::CUDA_ERROR_INVALID_IMAGE,
            CUresult::CUDA_ERROR_NO_BINARY_FOR_GPU,
            CUresult::CUDA_ERROR_UNSUPPORTED_PTX_VERSION,
            CUresult::CUDA_ERROR_NOT_FOUND,
        ] {
            assert!(driver_error_is_artifact_cause(&DriverError(code)));
        }
        for code in [
            CUresult::CUDA_ERROR_OUT_OF_MEMORY,
            CUresult::CUDA_ERROR_INVALID_CONTEXT,
            CUresult::CUDA_ERROR_ILLEGAL_ADDRESS,
            CUresult::CUDA_ERROR_LAUNCH_FAILED,
        ] {
            assert!(!driver_error_is_artifact_cause(&DriverError(code)));
        }
    }

    #[test]
    fn a_panic_in_the_pipeline_guard_is_backend_failure() {
        let r: Result<(), _> = guard_pipeline_panic(|| panic!("boom"));
        assert!(matches!(
            r,
            Err(ContractError::BackendFailure {
                op: "create_pipeline",
                ..
            })
        ));
    }

    /// 入力の分類: `CudaC` だけが通り、ほかは `Unsupported` か `ArtifactRejected` になる。
    #[test]
    fn only_cuda_c_is_accepted_and_other_inputs_are_classified() {
        use fandhe_silicon_core::execution::{MslFloatFunctions, MslMathMode};
        let ok = classify_pipeline_input(&PipelineInput::CudaC {
            source: "s",
            entry_point: "e",
        });
        assert!(matches!(ok, Ok(("s", "e"))));
        for input in [
            PipelineInput::Ptx {
                source: "p",
                entry_point: "e",
            },
            PipelineInput::Wgsl {
                source: "w",
                entry_point: "e",
            },
        ] {
            assert!(matches!(
                classify_pipeline_input(&input),
                Err(ContractError::Unsupported {
                    op: "create_pipeline",
                    backend: BackendKind::Cuda
                })
            ));
        }
        for input in [
            PipelineInput::Msl {
                source: "m",
                entry_point: "e",
            },
            PipelineInput::SpirV {
                words: &[],
                entry_point: "e",
            },
            PipelineInput::Dxil {
                bytes: &[],
                entry_point: "e",
            },
            PipelineInput::MslSpecialized {
                source: "m",
                entry_point: "e",
                function_constants: &[],
            },
            PipelineInput::MslWithOptions {
                source: "m",
                entry_point: "e",
                math_mode: MslMathMode::Fast,
            },
            PipelineInput::MslSpecializedWithOptions {
                source: "m",
                entry_point: "e",
                function_constants: &[],
                math_mode: MslMathMode::Fast,
                float_functions: MslFloatFunctions::Precise,
            },
        ] {
            assert!(matches!(
                classify_pipeline_input(&input),
                Err(ContractError::ArtifactRejected {
                    kind: "unsupported-input",
                    ..
                })
            ));
        }
        // 理由文は variant 名と受け付ける入力だけを含む。
        let Err(ContractError::ArtifactRejected { reason, .. }) =
            classify_pipeline_input(&PipelineInput::Msl {
                source: "m",
                entry_point: "e",
            })
        else {
            panic!("Msl must be rejected");
        };
        assert_eq!(
            reason,
            "pipeline input Msl is not accepted by the CUDA backend; accepted input is CudaC"
        );
        let Err(ContractError::ArtifactRejected { reason, .. }) =
            classify_pipeline_input(&PipelineInput::MslSpecializedWithOptions {
                source: "m",
                entry_point: "e",
                function_constants: &[],
                math_mode: MslMathMode::Fast,
                float_functions: MslFloatFunctions::Precise,
            })
        else {
            panic!("MslSpecializedWithOptions must be rejected");
        };
        assert_eq!(
            reason,
            "pipeline input MslSpecializedWithOptions is not accepted by the CUDA backend; accepted input is CudaC"
        );
    }

    /// `slots` の重複は `InvalidArgument`（BIND-3）。
    #[test]
    fn duplicate_slots_are_invalid_argument() {
        assert!(check_unique_slots(&[]).is_ok());
        assert!(check_unique_slots(&[0, 1, 4]).is_ok());
        assert!(matches!(
            check_unique_slots(&[0, 1, 0]),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[test]
    fn nul_bytes_are_rejected_before_compilation() {
        assert!(check_no_nul("a", "b").is_ok());
        assert!(matches!(
            check_no_nul("a\0", "b"),
            Err(ContractError::ArtifactRejected { kind: "cuda-c", .. })
        ));
        assert!(matches!(
            check_no_nul("a", "b\0"),
            Err(ContractError::ArtifactRejected {
                kind: "function",
                ..
            })
        ));
    }
}
/// CUDA の device に触る試験。device の無い環境では流さない。
#[cfg(test)]
mod device_tests {
    use super::*;

    /// 要素に 1 を足す小さなカーネル。
    const ADD_ONE: &str = "extern \"C\" __global__ void add_one(float* out, const float* inp, unsigned int n) { unsigned int i = blockIdx.x * blockDim.x + threadIdx.x; if (i < n) { out[i] = inp[i] + 1.0f; } }";

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    fn add_one(entry_point: &str) -> PipelineInput<'_> {
        PipelineInput::CudaC {
            source: ADD_ONE,
            entry_point,
        }
    }

    fn table_len(d: &CudaDevice) -> usize {
        d.pipelines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map
            .len()
    }

    /// CUDA C のカーネルから、この device のパイプラインが作られる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn cuda_c_kernel_creates_a_pipeline_of_this_device() {
        let d = device();
        let p = d
            .create_pipeline(add_one("add_one"), &BindingLayout::new(vec![0, 1, 2]))
            .expect("pipeline");
        assert_eq!(p.__backend_device_id(), d.id);
    }

    /// 同じ device で 2 回作ると、識別子が異なる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn two_pipelines_on_one_device_get_distinct_ids() {
        let d = device();
        let layout = BindingLayout::new(vec![0, 1, 2]);
        let a = d.create_pipeline(add_one("add_one"), &layout).expect("a");
        let b = d.create_pipeline(add_one("add_one"), &layout).expect("b");
        assert_ne!(a.__backend_id(), b.__backend_id());
    }

    /// `slots` がカーネルの引数と合わなくても作られる（COMP-25）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn slots_that_differ_from_the_kernel_arguments_still_create_a_pipeline() {
        let d = device();
        assert!(
            d.create_pipeline(add_one("add_one"), &BindingLayout::new(vec![]))
                .is_ok()
        );
        assert!(
            d.create_pipeline(add_one("add_one"), &BindingLayout::new(vec![0, 1, 2, 3, 9]))
                .is_ok()
        );
    }

    /// 表は渡した `layout` の写しを持つ。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn the_pipeline_table_keeps_a_copy_of_the_layout() {
        let d = device();
        let h = d
            .create_pipeline(add_one("add_one"), &BindingLayout::new(vec![4, 5]))
            .expect("pipeline");
        let pipelines = d
            .pipelines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = pipelines.map.get(&h.__backend_id()).expect("entry");
        assert_eq!(entry._layout, BindingLayout::new(vec![4, 5]));
    }

    /// 重複した `slots` は `InvalidArgument` で、表の件数は増えない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn duplicate_slots_are_invalid_argument_and_add_no_pipeline() {
        let d = device();
        d.create_pipeline(add_one("add_one"), &BindingLayout::new(vec![0]))
            .expect("pipeline");
        let before = table_len(&d);
        let r = d.create_pipeline(add_one("add_one"), &BindingLayout::new(vec![0, 0]));
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))));
        assert_eq!(table_len(&d), before);
    }

    /// コンパイルできないソースは `ArtifactRejected { kind: "cuda-c" }`。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn uncompilable_source_is_artifact_rejected_cuda_c() {
        let d = device();
        let r = d.create_pipeline(
            PipelineInput::CudaC {
                source: "this is not CUDA C",
                entry_point: "k",
            },
            &BindingLayout::new(vec![0]),
        );
        assert!(matches!(
            r,
            Err(ContractError::ArtifactRejected { kind: "cuda-c", .. })
        ));
    }

    /// 無い入口名は `ArtifactRejected { kind: "function" }`。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn missing_entry_point_is_artifact_rejected_function() {
        let d = device();
        let r = d.create_pipeline(add_one("absent"), &BindingLayout::new(vec![0]));
        assert!(matches!(
            r,
            Err(ContractError::ArtifactRejected {
                kind: "function",
                ..
            })
        ));
    }

    /// PTX は `Unsupported`、MSL は `ArtifactRejected { kind: "unsupported-input" }`。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn ptx_is_unsupported_and_msl_is_unsupported_input() {
        let d = device();
        let layout = BindingLayout::new(vec![0]);
        let ptx = d.create_pipeline(
            PipelineInput::Ptx {
                source: ".version 7.0",
                entry_point: "k",
            },
            &layout,
        );
        assert!(matches!(
            ptx,
            Err(ContractError::Unsupported {
                op: "create_pipeline",
                backend: BackendKind::Cuda,
                ..
            })
        ));
        let msl = d.create_pipeline(
            PipelineInput::Msl {
                source: "kernel void k() {}",
                entry_point: "k",
            },
            &layout,
        );
        assert!(matches!(
            msl,
            Err(ContractError::ArtifactRejected {
                kind: "unsupported-input",
                ..
            })
        ));
    }

    /// device は読み取った compute capability を持つ。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn compute_capability_is_read_from_the_device() {
        let d = device();
        assert_ne!(d.compute_capability, (0, 0));
        println!(
            "compute_capability={}.{}",
            d.compute_capability.0, d.compute_capability.1
        );
    }

    /// 記録先を作って捨てても、開いている数は戻り、投入は 0 回のまま。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dropping_an_encoder_closes_it_and_submits_nothing() {
        let d = device();
        let encoder = d.create_encoder();
        assert_eq!(d.test_open_encoder_count(), 1);
        drop(encoder);
        assert_eq!(d.test_open_encoder_count(), 0);
        assert_eq!(d.test_submitted_count(), 0);
    }

    /// 拒まれた投入は、投入の回数も開いている記録先の数も変えない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn rejected_submissions_do_not_change_the_submitted_count() {
        let d = device();
        let other = device();
        assert!(matches!(
            d.submit(other.create_encoder(), &[]),
            Err(ContractError::InvalidArgument(_))
        ));
        assert!(matches!(
            d.submit(
                d.create_encoder(),
                &[ImportedSemaphoreHandle::__backend_new(1)]
            ),
            Err(ContractError::Unsupported { .. })
        ));
        assert_eq!(d.test_submitted_count(), 0);
        assert_eq!(d.test_open_encoder_count(), 0);
        assert_eq!(other.test_open_encoder_count(), 0);
    }
}

/// CUDA の device に触る、バッファの再利用とゼロ化の試験。device の無い環境では流さない。
#[cfg(all(test, feature = "native-blocking-wait"))]
mod pool_device_tests {
    use super::*;

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 全バイトを読み、0 以外のバイトの数を返す。
    fn nonzero_bytes(d: &CudaDevice, h: &BufferHandle, len: u64) -> usize {
        d.download_blocking(h, 0, len)
            .expect("download")
            .iter()
            .filter(|b| **b != 0)
            .count()
    }

    /// 新しい device の確保は、直後に全バイトが 0 で読める（MEM-2）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_fresh_allocation_reads_back_all_zero() {
        let d = device();
        for len in [1u64, 4096, 4097, 65_536, 1024 * 1024] {
            let h = d.allocate(len, BufferUsage::STORAGE).expect("allocate");
            assert_eq!(nonzero_bytes(&d, &h, len), 0, "len={len}");
        }
        assert_eq!(d.test_pool_reuse_count(), 0);
    }

    /// 確保・upload・download_blocking の往復で同じ列が読める（MEM-1）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn allocate_upload_and_download_blocking_round_trip() {
        let d = device();
        let max = d.limits().max_buffer_bytes;
        for len in [1u64, 4097, 1024 * 1024, 16 * 1024 * 1024] {
            if len > max {
                println!("not-run: len={len} exceeds max_buffer_bytes={max}");
                continue;
            }
            let h = d.allocate(len, BufferUsage::STORAGE).expect("allocate");
            let src: Vec<u8> = (0..len)
                .map(|i| u8::try_from(i % 251).unwrap_or(0) + 1)
                .collect();
            d.upload(&h, 0, &src).expect("upload");
            assert_eq!(d.download_blocking(&h, 0, len).expect("download"), src);
        }
    }

    /// 1,000 回の upload・free・allocate のくり返しで、確保した内容は毎回全バイト 0（MEM-17）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn reused_buffers_never_expose_previous_contents_over_1000_rounds() {
        let d = device();
        let len = 4097u64;
        let mut h = d.allocate(len, BufferUsage::STORAGE).expect("allocate");
        for round in 0..1000u32 {
            let fill = u8::try_from(round % 255).unwrap_or(0) + 1;
            d.upload(&h, 0, &vec![fill; 4097]).expect("upload");
            d.free(h);
            h = d.allocate(len, BufferUsage::STORAGE).expect("allocate");
            assert_eq!(nonzero_bytes(&d, &h, len), 0, "round={round}");
        }
        let n = d.test_pool_reuse_count();
        let m = d.test_pool_slot_match_count();
        println!("pool_reuse_count={n} pool_slot_match_count={m}");
        assert!(n >= 1);
    }

    /// 用途が違う確保は再利用せず、同じ用途の確保だけが同じ slot を取り出す。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_different_usage_is_not_reused() {
        let d = device();
        let len = 4096u64;
        let a = d.allocate(len, BufferUsage::STORAGE).expect("allocate");
        let slot_a = d.test_slot_of(&a).expect("slot");
        d.free(a);
        d.stream.synchronize().expect("synchronize");
        let reused_before = d.test_pool_reuse_count();
        let b = d.allocate(len, BufferUsage::UPLOAD).expect("allocate");
        assert_eq!(d.test_pool_reuse_count(), reused_before);
        assert_eq!(b.usage(), BufferUsage::UPLOAD);
        assert_eq!(nonzero_bytes(&d, &b, len), 0);
        let again = d.allocate(len, BufferUsage::STORAGE).expect("allocate");
        assert_eq!(d.test_pool_reuse_count(), reused_before + 1);
        assert_eq!(d.test_slot_of(&again), Some(slot_a));
    }

    /// 解放済みハンドルの写しは、再利用されたバッファに当たらない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_freed_handle_does_not_alias_the_reused_buffer() {
        let d = device();
        let h = d.allocate(64, BufferUsage::STORAGE).expect("allocate");
        let stale = BufferHandle::__backend_new(
            d.generation,
            h.__backend_id(),
            d.id,
            64,
            BufferUsage::STORAGE,
        );
        let slot = d.test_slot_of(&h).expect("slot");
        d.free(h);
        d.stream.synchronize().expect("synchronize");
        let fresh = d.allocate(64, BufferUsage::STORAGE).expect("allocate");
        assert_eq!(d.test_slot_of(&fresh), Some(slot));
        assert_ne!(fresh.__backend_id(), stale.__backend_id());
        assert_eq!(d.test_pool_reuse_count(), 1);
        assert!(matches!(
            d.upload(&stale, 0, &[1u8]),
            Err(ContractError::InvalidArgument(_))
        ));
        let len_before = d.test_pool_len();
        d.free(stale);
        assert_eq!(d.test_pool_len(), len_before);
        assert_eq!(d.test_pool_len(), 0);
    }

    /// 一覧の件数は上限までで、長さが違う確保は一覧から取り出さない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn the_pool_is_bounded_and_matches_only_the_same_length() {
        let d = device();
        let handles: Vec<BufferHandle> = (0..POOL_MAX_ENTRIES + 4)
            .map(|_| d.allocate(32, BufferUsage::STORAGE).expect("allocate"))
            .collect();
        for h in handles {
            d.free(h);
        }
        assert_eq!(d.test_pool_len(), POOL_MAX_ENTRIES);
        let other = d.allocate(33, BufferUsage::STORAGE).expect("allocate");
        assert_eq!(d.test_pool_reuse_count(), 0);
        assert_eq!(d.test_pool_len(), POOL_MAX_ENTRIES);
        d.free(other);
    }
}
