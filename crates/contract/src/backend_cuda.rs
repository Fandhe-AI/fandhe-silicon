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
//! 公開の `Device` からは `compute()` でこの実装へ届く（TASK-166 のポインタ）。
//!
//! `CommandQueue` の実装は投入の窓口で、記録先の開いている集合と投入済みの表を持つ（TASK-604 のポインタ）。
//!
//! `free` したバッファは stream にゼロ化を積んでから再利用の一覧の待つ側に入り、`allocate` の入口で
//! 完了を確かめたものだけが、同じ長さ・同じ用途の確保に再利用される（TASK-340 のポインタ）。
//! 一覧の上限は件数と大きさの合計で、入れられないバッファは解放する。新規の確保は全バイトを 0 にし終えてから返す。
//! `copy_buffer_to_buffer`・`clear_buffer` は記録の時点に照らして記録先に積み、投入（`submit`・`submit_many`）が
//! 記録の順に stream へ積んでから event を記録する。投入はコピー・クリアの各バッファに最後に使った投入の番号を付け、
//! その投入が投入済みの表に残っている間の `free` は解放を保留し、`submit`・`submit_many`・`poll`・`allocate`・
//! `wait_blocking`（feature `native-blocking-wait` の下）の中で投入済みの表から外れたものを再利用の一覧へ回す
//! （TASK-624・TASK-606）。
//!
//! `query`・`query_ex` は、`request` の時点で捕捉した compute capability とホストメモリのマップの可否の属性と、
//! 能力の表から答える（TASK-175）。
//!
//! `ext-interop` のビルドでは device の作成で UUID を 1 回読み、`physical_gpu_id` が返す（TASK-197）。
//!
//! `CudaDevice` は検証の報告の受け口の登録を受け付けて保持し、報告は出さない（TASK-607）。
//!
//! device の世代は書き換えられる値で、照合と払い出しは今の値を読む。`free` は表の項目を払い出したときの世代で照らし、
//! 完了の callback の登録は登録した印の世代を持つ（TASK-2934）。
//!
//! feature `fault-injection` のビルドでは `FaultHooks::simulate_device_lost` が模擬のロストを起こし、`DeviceLostNotify` で
//! 登録したコールバックを 1 回呼び、その後に登録の世代が古い完了の callback を `Lost` で 1 回ずつ呼ぶ（TASK-2934）。
//!
//! `CudaDevice` は `Device::destroy` を上書きし、登録済みの完了の callback を `Lost` で呼び切ってから stream を同期し、
//! `fault-injection` のビルドではロストの通知を `Destroyed` で 1 回呼んでから解放する（TASK-191）。
//!
//! `upload` は確保のときの用途に `UPLOAD` が無ければ、`download_blocking`・`download_async` は `DOWNLOAD` が無ければ、
//! 範囲を見ずに `InvalidArgument` で拒み、下位の API を呼ばない。照らす順は別の device、世代、表に無いハンドル、用途、範囲で、
//! 判定には表に記録した用途を使いハンドルの申告は使わない（TASK-342）。
//!
//! `fault-injection` のビルドでは `arm_gate` が門を仕込み、次に成功する投入の event の前に stream へ門を待つ host の関数を積む。
//! `open_gate` が門を開け、門の待ちには上限があって、過ぎると門は開かずに stream が先へ進む。
//! `Drop`・`Device::destroy` は同期の前に門を開ける（TASK-345）。
//!
//! `pending_release_count` は `free` が解放を保留しているバッファの数を返し、読むだけで回収はしない（TASK-345）。
//!
//! feature `native-blocking-wait` のビルドの `wait_blocking` は、照らしの後に投入済みの表から待つかを決め、表の錠を放してから
//! `None` は投入の event を同期し、`Some` は期限まで event の完了を確かめ直す。戻る前に `submit` と同じく、解放を保留した
//! バッファを回し、終端になった登録済みの callback を呼ぶ（TASK-606）。
//!
//! `create_pipeline` は `Ptx` の入力を NVRTC に渡さず、テキストをそのままモジュールとして読み込む。`CudaC` の入力だけを
//! device の NVRTC の呼び出し口 `compile_with_nvrtc` に渡し、試験のビルドの device はその呼び出し口に入った回数を持つ（TASK-804）。
//!
//! `dispatch_ex`（簡易版の `dispatch` を含む）は記録の時点に記録先・パイプライン・束縛のバッファ・slot・定数・共有メモリの合計を
//! 照らして記録先に積み、束縛のポインタは投入の時に解決する。投入は記録したバッファの解放済みの確かめの後に、利用者のカーネルを
//! 走らせる門（`DISPATCH_EXECUTION_ENABLED`。単体の試験のビルドだけで開く）に照らし、閉じていれば dispatch の記録を含む記録先を
//! `Unsupported` で拒んで何も積まない。開いていれば、全 dispatch のポインタの解決とカーネルの仮引数の大きさの照らしを積む前に済ませ、
//! 記録の順に起動する（TASK-805）。
//! `dispatch_ex` は束縛のバッファの後・slot の前に、`dispatch_dims` の 0 と `limits().max_dispatch_dims` を超える要素も
//! 記録の時点に照らす（TASK-806）。
//!
//! feature `fault-injection` のビルドで模擬のロストの後は、`allocate`・`upload`・`download_blocking`・`download_async`・
//! `copy_buffer_to_buffer`・`clear_buffer`・`dispatch_ex`・`submit`・`submit_many`・`wait_blocking` が、呼んだ時点の世代を
//! 載せた `DeviceLost` を返す（`download_async` は callback に渡す）。`allocate` の 0 の長さ、`upload` の表に無いハンドル、
//! `copy_buffer_to_buffer`・`clear_buffer` の開いていない記録先、`dispatch_ex` の開いていない記録先と 0 の要素、`submit` の
//! 空でない `wait`、空の `submit_many` でも同じ値を返す。`poll`・`poll_ex` はロストの前の印に、`poll` は今の世代の印にも
//! `Lost` を返す。`arm_gate` で止めた投入を待っている間に模擬のロストが起き、`open_gate` で門を開けると、`wait_blocking` は
//! `DeviceLost` を返す。`download_blocking`・`wait_blocking` は feature `native-blocking-wait` も有効なビルドにある（TASK-188）。
//!
//! feature `fault-injection` のビルドで模擬のロストの後は、`create_pipeline` と、引数の宣言が合う `create_pipeline_checked` も、
//! 呼んだ時点の世代を載せた `DeviceLost` を返し、NVRTC の呼び出し口とモジュールの読み込みに入らない。模擬のロストは、理由に
//! `Unknown`・`Unrecoverable` のどちらを指定してもその理由で通知し、検証の報告の受け口に登録した関数を呼ばない（TASK-561）。

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(feature = "fault-injection")]
use std::sync::{Condvar, PoisonError};
#[cfg(feature = "fault-injection")]
use std::time::Duration;

use cudarc::driver::{
    CudaContext, CudaEvent, CudaFunction, CudaModule, CudaSlice, CudaStream, DevicePtr, DriverError,
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
#[cfg(feature = "fault-injection")]
use fandhe_silicon_core::device_lost::{DeviceLostNotify, LostCallback};
#[cfg(feature = "fault-injection")]
use fandhe_silicon_core::error::DeviceLostReason;
use fandhe_silicon_core::error::{CompletionState, ContractError, UnsupportedReason};
use fandhe_silicon_core::execution::{
    __BackendSharedEncoderCloser, __backend_next_encoder_id, Binding, BindingLayout,
    CommandEncoder, CommandQueue, ComputeOps, ConstantBinding, ImportedSemaphoreHandle,
    PipelineHandle, PipelineInput, SubmitToken, ThreadgroupMemory,
};
#[cfg(feature = "fault-injection")]
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::feature_key::{
    FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX, FEATURE_MAPPABLE_PRIMARY_BUFFERS, FEATURE_SHADER_F16,
    FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_SUBGROUP, FEATURE_SUBGROUP_BARRIER,
};
#[cfg(feature = "ext-interop")]
use fandhe_silicon_core::interop::{
    ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle, PhysicalGpuId,
};
use fandhe_silicon_core::memory::{
    BufferHandle, BufferUsage, MemoryOps, UsageRole, validate_allocation_size,
    validate_buffer_range, validate_usage_for_role,
};
use fandhe_silicon_core::validation::{ValidationCallback, ValidationIntake, ValidationReporter};

/// この provider が受け持つ背後実装。
const BACKEND: BackendKind = BackendKind::Cuda;

/// `DeviceDiagnostics::backend_name` に載せる名前。
const BACKEND_NAME: &str = "cuda";

/// device の世代の採番。背後実装の中で 1 から単調増加に取る。
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// この背後実装の provider の `request` が呼ばれた回数（feature `fault-injection` のビルドだけで数える）。
#[cfg(feature = "fault-injection")]
static REQUEST_CALLS: AtomicU64 = AtomicU64::new(0);

/// provider の `request` の入口で最初に呼ぶ。feature `fault-injection` の無いビルドでは何もしない。
fn note_request_call() {
    #[cfg(feature = "fault-injection")]
    REQUEST_CALLS.fetch_add(1, Ordering::SeqCst);
}

/// この背後実装の provider（`CudaDeviceProvider` と差し込みの provider）の `request` が、プロセスの開始から呼ばれた回数。
///
/// 要求の検査で拒んだ呼び出しも数える。数は戻せない。試験・診断用で、feature `fault-injection` の裏にある。
#[cfg(feature = "fault-injection")]
pub fn request_call_count() -> u64 {
    REQUEST_CALLS.load(Ordering::SeqCst)
}

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

/// 世代の源が尽きて、ロストの世代を取り直せなかったことを表す。状態は変わっていない。
#[cfg(feature = "fault-injection")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GenerationExhausted;

/// 門の待ちの上限。過ぎると門が閉じたままでも `gate_host_fn` が戻り、stream が先へ進む。
#[cfg(feature = "fault-injection")]
const GATE_EXPIRY: Duration = Duration::from_millis(500);

/// 門 1 つの開閉の印。`gate_host_fn` が待ち、`open` が開ける。
///
/// device に触らない純粋な型で、`GateState` が仕込みと積んだ門として持つ（TASK-345）。
#[cfg(feature = "fault-injection")]
#[derive(Default)]
struct GateLatch {
    opened: Mutex<bool>,
    wake: Condvar,
}

#[cfg(feature = "fault-injection")]
impl GateLatch {
    /// 門を開けて、待っているものを全部起こす。何度呼んでもよい。
    fn open(&self) {
        let mut opened = self.opened.lock().unwrap_or_else(PoisonError::into_inner);
        *opened = true;
        self.wake.notify_all();
    }

    /// 門が開いているか。
    fn is_open(&self) -> bool {
        *self.opened.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 門が開くか `limit` が過ぎるまで待つ。開いていれば `true`、上限が過ぎたら `false`。
    fn wait_open(&self, limit: Duration) -> bool {
        let guard = self.opened.lock().unwrap_or_else(PoisonError::into_inner);
        let (guard, _) = self
            .wake
            .wait_timeout_while(guard, limit, |opened| !*opened)
            .unwrap_or_else(PoisonError::into_inner);
        *guard
    }
}

/// 試験用の門の状態。仕込んだ門と、stream に積んだ門を錠の中に持つ。
///
/// `CudaDevice` が 1 つ持ち、`FaultHooks::arm_gate`・`open_gate` と `issue_event`・`Drop`・`Device::destroy` が使う。
/// device に触らない純粋な型なので、積む手段は呼び出し側が渡す（TASK-345）。
#[cfg(feature = "fault-injection")]
#[derive(Default)]
struct GateState {
    inner: Mutex<GateInner>,
}

#[cfg(feature = "fault-injection")]
#[derive(Default)]
struct GateInner {
    /// 仕込んだ門。次に成功する投入が積んで空になる。
    armed: Option<Arc<GateLatch>>,
    /// 積んだ門。`open_all` が開けて空にする。
    issued: Vec<Arc<GateLatch>>,
}

#[cfg(feature = "fault-injection")]
impl GateState {
    /// 錠を取る。poison されていても中身を使う。
    fn lock(&self) -> MutexGuard<'_, GateInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 門を仕込む。仕込み済みなら何もしない。
    fn arm(&self) {
        let mut inner = self.lock();
        if inner.armed.is_none() {
            inner.armed = Some(Arc::new(GateLatch::default()));
        }
    }

    /// 仕込んだ門と積んだ門を全部開ける。錠は開ける前に放す。何度呼んでもよい。
    fn open_all(&self) {
        let (armed, issued) = {
            let mut inner = self.lock();
            (inner.armed.take(), std::mem::take(&mut inner.issued))
        };
        for latch in armed.iter().chain(issued.iter()) {
            latch.open();
        }
    }

    /// 仕込んだ門があれば `launch` に渡して積む。仕込みが無ければ何もしない。
    ///
    /// `launch` が `Ok` のときだけ仕込みを消費して積んだ門の一覧に移し、開いた門は一覧から外す。
    /// `Err` のときは仕込みを残して返す。
    fn attach_with<E>(
        &self,
        launch: impl FnOnce(&Arc<GateLatch>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut inner = self.lock();
        let Some(latch) = inner.armed.clone() else {
            return Ok(());
        };
        launch(&latch)?;
        inner.armed = None;
        inner.issued.retain(|issued| !issued.is_open());
        inner.issued.push(latch);
        Ok(())
    }

    /// 試験専用。門が仕込まれているか。
    #[cfg(test)]
    fn is_armed(&self) -> bool {
        self.lock().armed.is_some()
    }

    /// 試験専用。積んだ門の数。
    #[cfg(test)]
    fn issued_len(&self) -> usize {
        self.lock().issued.len()
    }
}

/// stream に積む host の関数。門が開くか `GATE_EXPIRY` が過ぎるまで待つ。
///
/// CUDA の API を呼ばない。panic しない。`arg` の参照は借りるだけで、落とさない。
///
/// # Safety
///
/// `arg` は `CudaDevice::launch_gate` が `Arc::<GateLatch>::into_raw` で作ったポインタで、その参照はどこでも
/// 取り戻さないこと（指す `GateLatch` はプロセスの終わりまで生きる）。
#[cfg(feature = "fault-injection")]
unsafe extern "C" fn gate_host_fn(arg: *mut c_void) {
    // SAFETY: `arg` は `launch_gate` が `Arc::into_raw` で作ったポインタで、その参照はどこでも取り戻さない
    // （上の `# Safety`）。指す `GateLatch` は解放されないので、ここで借りる参照は呼ばれた回数に依らず有効。
    // `GateLatch` は `Sync` で、ここでは共有の参照だけを作る。
    let latch = unsafe { &*arg.cast_const().cast::<GateLatch>() };
    let _ = latch.wait_open(GATE_EXPIRY);
}

/// 模擬のロストの状態。最初のロストの理由と、登録されたコールバック 1 つを錠の中に持つ。
///
/// device に触らない純粋な型で、CUDA の device の型 `CudaDevice` が 1 つ持つ（TASK-2934）。
/// 世代の格納先と取り直しの源は呼び出し側が渡すので、device が無い環境でも単体で確かめられる。
/// `Drop` ではコールバックを呼ばない。
#[cfg(feature = "fault-injection")]
#[derive(Default)]
struct LostState {
    inner: Mutex<LostInner>,
}

#[cfg(feature = "fault-injection")]
#[derive(Default)]
struct LostInner {
    /// 最初のロストの理由。`Some` になったら 2 回目以降のロストは無視する。
    reason: Option<DeviceLostReason>,
    /// 登録されたコールバック。ロストで `take` されるので、ロストの後に登録したものは呼ばれない。
    callback: Option<LostCallback>,
}

#[cfg(feature = "fault-injection")]
impl LostState {
    /// 毒化していても中身を取り出す。panic しない。
    fn lock(&self) -> MutexGuard<'_, LostInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// コールバックを登録する。登録済みなら上書きし、ロスト済みなら保持するだけで呼ばない。
    fn set_callback(&self, callback: LostCallback) {
        // 上書きされた古いコールバックの `Drop` が再入しても詰まらないよう、錠を放してから捨てる。
        let old = self.lock().callback.replace(callback);
        drop(old);
    }

    /// 模擬のロストが起きたか。device の操作の照らし（`CudaDevice::is_lost`）と単体の試験が読む。
    fn is_lost(&self) -> bool {
        self.lock().reason.is_some()
    }

    /// 1 回目だけ、理由の記録・世代の取り直し・コールバックの取り出しを錠の中で行い、錠を放してから
    /// 指定の理由でコールバックを 1 回呼ぶ。1 回目なら `Ok(true)`、2 回目以降は何もせず `Ok(false)`。
    ///
    /// 世代は原子の値として読まれる。理由とコールバックは錠の中だけで読む。
    ///
    /// 世代は `source` から 1 つ取って `generation` へ書く。`source` が尽きていたら、理由も世代も
    /// コールバックも変えずに `Err` を返す。コールバックの中から同じ状態を呼んでも詰まらない。
    #[cfg(test)]
    fn lose(
        &self,
        reason: DeviceLostReason,
        generation: &AtomicU64,
        source: &AtomicU64,
    ) -> Result<bool, GenerationExhausted> {
        let (first, panic) = self.lose_catching(reason, generation, source)?;
        if let Some(payload) = panic {
            std::panic::resume_unwind(payload);
        }
        Ok(first)
    }

    /// `lose` と同じ手順だが、通知先のコールバックが panic したら再送出せず、その中身を返す。
    ///
    /// 理由と世代は panic の前に記録済みなので、呼ぶ側は返った中身を、後続の処理（古い世代の完了の
    /// callback の通知など）を終えてから再送出すること。
    fn lose_catching(
        &self,
        reason: DeviceLostReason,
        generation: &AtomicU64,
        source: &AtomicU64,
    ) -> Result<(bool, Option<Box<dyn std::any::Any + Send>>), GenerationExhausted> {
        let callback = {
            let mut inner = self.lock();
            if inner.reason.is_some() {
                return Ok((false, None));
            }
            let next = take_next(source).ok_or(GenerationExhausted)?;
            generation.store(next, Ordering::Release);
            inner.reason = Some(reason.clone());
            inner.callback.take()
        };
        let panic =
            callback.and_then(|callback| catch_unwind(AssertUnwindSafe(|| callback(reason))).err());
        Ok((true, panic))
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

/// 16 個の `c_char` をビットを変えずに 16 バイトに写す（`c_char` の符号の有無に依らない）。
#[cfg(feature = "ext-interop")]
fn uuid_bytes(raw: [std::ffi::c_char; 16]) -> [u8; 16] {
    raw.map(|b| b.to_ne_bytes()[0])
}

/// 閉包で UUID を読み、`driver_uuid` を持たない識別子にする。読めなければ `DeviceUnavailable`。
/// 閉包で受けるので、単体試験から device に触らず呼べる。
#[cfg(feature = "ext-interop")]
fn read_physical_gpu_id<E: std::fmt::Debug>(
    uuid: impl FnOnce() -> Result<[std::ffi::c_char; 16], E>,
) -> Result<PhysicalGpuId, ContractError> {
    let raw = uuid()
        .map_err(|e| ContractError::DeviceUnavailable(format!("CudaContext::uuid: {e:?}")))?;
    Ok(PhysicalGpuId {
        device_uuid: uuid_bytes(raw),
        driver_uuid: None,
    })
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
        #[cfg(feature = "ext-interop")]
        let physical_gpu_id = read_physical_gpu_id(|| ctx.uuid().map(|u| u.bytes))?;
        let stream = ctx.new_stream().map_err(|e| {
            ContractError::DeviceUnavailable(format!("CudaContext::new_stream: {e:?}"))
        })?;
        let exhausted =
            || ContractError::DeviceUnavailable("device counter space is exhausted".into());
        let id = __backend_next_device_id().ok_or_else(exhausted)?;
        let generation = take_next(&NEXT_GENERATION).ok_or_else(exhausted)?;
        Ok(CudaDevice {
            id,
            generation: AtomicU64::new(generation),
            #[cfg(feature = "fault-injection")]
            lost: LostState::default(),
            #[cfg(feature = "fault-injection")]
            gate: GateState::default(),
            ownership: DeviceOwnership::ProductIndependent,
            adapter_name: attributes.adapter_name,
            limits: LimitsTable {
                max_buffer_bytes: attributes.total_mem,
                max_dispatch_dims: grid_dims,
                max_texture_size: 0,
            },
            #[cfg(feature = "ext-interop")]
            physical_gpu_id,
            compute_capability: attributes.compute_capability,
            capabilities: CapabilitySnapshot {
                compute_capability: attributes.compute_capability,
                can_map_host_memory,
            },
            stream,
            buffers: Mutex::new(Buffers::default()),
            held: std::sync::atomic::AtomicUsize::new(0),
            pipelines: Mutex::new(Pipelines::default()),
            #[cfg(test)]
            nvrtc_calls: AtomicU64::new(0),
            #[cfg(test)]
            pipeline_driver_calls: AtomicU64::new(0),
            open: Arc::new(OpenEncoders::default()),
            submissions: Mutex::new(Submissions::new()),
            validation: ValidationReporter::new(),
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
        note_request_call();
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
                note_request_call();
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

/// `create_pipeline` が受け付けた入力。`classify_pipeline_input` が返し、`module_input` が読み込む PTX にする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptedInput<'a> {
    /// NVRTC で PTX にする CUDA C のソースと入口名。
    CudaC {
        source: &'a str,
        entry_point: &'a str,
    },
    /// NVRTC に渡さずにそのまま読み込む PTX のテキストと入口名。
    Ptx {
        source: &'a str,
        entry_point: &'a str,
    },
}

/// `create_pipeline` の入力を分類し、受け付ける `CudaC`・`Ptx` の入力を返す。
/// device にも下位の API にも触らない。`ComputeOps::create_pipeline` から最初に呼ばれる。
/// `Msl` 系・`SpirV`・`Dxil` は `ArtifactRejected { kind: "unsupported-input" }` で、理由に受け取った variant と
/// 受け付ける `CudaC`・`Ptx` を載せる（`MslSpecializedWithOptions` は Metal 専用であることも載せる）。
fn classify_pipeline_input<'a>(
    input: &PipelineInput<'a>,
) -> Result<AcceptedInput<'a>, ContractError> {
    let (variant, note) = match input {
        PipelineInput::CudaC {
            source,
            entry_point,
        } => {
            return Ok(AcceptedInput::CudaC {
                source,
                entry_point,
            });
        }
        PipelineInput::Ptx {
            source,
            entry_point,
        } => {
            return Ok(AcceptedInput::Ptx {
                source,
                entry_point,
            });
        }
        PipelineInput::Wgsl { .. } => {
            return Err(ContractError::Unsupported {
                op: "create_pipeline",
                backend: BACKEND,
            });
        }
        PipelineInput::Msl { .. } => ("Msl", ""),
        PipelineInput::SpirV { .. } => ("SpirV", ""),
        PipelineInput::Dxil { .. } => ("Dxil", ""),
        PipelineInput::MslSpecialized { .. } => ("MslSpecialized", ""),
        PipelineInput::MslWithOptions { .. } => ("MslWithOptions", ""),
        PipelineInput::MslSpecializedWithOptions { .. } => {
            ("MslSpecializedWithOptions", " is Metal-only and")
        }
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
            "pipeline input {variant}{note} is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx"
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
/// `module_input` が `CudaC` の入力でコンパイルの前に呼ぶ。ソースを先に見る。
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

/// PTX のテキストと入口名に NUL のバイトが無いことを照らす。下位の API へ文字列を渡す前の検査で、
/// `module_input` が `Ptx` の入力で読み込む PTX を作る前に呼ぶ。テキストを先に見る。
fn check_ptx_no_nul(source: &str, entry_point: &str) -> Result<(), ContractError> {
    if source.contains('\0') {
        return Err(ContractError::ArtifactRejected {
            kind: "module",
            reason: "PTX source contains a NUL byte".to_string(),
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
/// ライブラリが無ければ `Unsupported`。`CudaDevice::compile_with_nvrtc` から呼ばれる。
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

/// 受け付けた入力から、読み込む PTX と入口名を作る。`CudaC` はソースと入口名の NUL のバイトを照らしてから
/// `compile` を 1 回呼ぶ。`Ptx` はテキストと入口名の NUL のバイトを照らし、`compile` を呼ばずにテキストを
/// そのまま PTX にする。`ComputeOps::create_pipeline` が device の NVRTC の呼び出し口を `compile` に渡して呼ぶ。
fn module_input<'a>(
    accepted: AcceptedInput<'a>,
    compile: impl FnOnce(&str) -> Result<Ptx, ContractError>,
) -> Result<(Ptx, &'a str), ContractError> {
    match accepted {
        AcceptedInput::CudaC {
            source,
            entry_point,
        } => {
            check_no_nul(source, entry_point)?;
            Ok((compile(source)?, entry_point))
        }
        AcceptedInput::Ptx {
            source,
            entry_point,
        } => {
            check_ptx_no_nul(source, entry_point)?;
            Ok((Ptx::from_src(source), entry_point))
        }
    }
}

/// 投入済みの表の 1 件の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupState {
    Pending,
    Completed,
    Failed,
}

/// event の完了を待たずに問い合わせる。問合せの失敗と panic は `Failed`。
///
/// 投入済みの表は event を `Arc` で持ち、再利用の一覧は値で持つので、どちらからも呼べる形にする。
fn event_state<E: std::borrow::Borrow<CudaEvent>>(event: &E) -> GroupState {
    let event: &CudaEvent = event.borrow();
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
    /// 終端を待つ登録済みの callback。通し番号と、登録した印の世代と組で持つ。
    callbacks: Vec<(u64, u64, CompletionCallback)>,
    /// 直近の `reclaim` で未完了のまま止まった先頭の番号。同じ錠の間はその判定を使い、再び問い合わせない。
    stalled: Option<u64>,
    /// `wait_blocking` の待ちが失敗した通し番号と、その呼び出しが返した値。
    wait_failures: BTreeMap<u64, ContractError>,
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
            wait_failures: BTreeMap::new(),
        }
    }

    /// 通し番号の完了状態。呼ぶ前に `reclaim` しておく。
    ///
    /// `reclaim` が未完了で止まった先頭は、再び問い合わせずに `Pending` とする
    /// （問合せの再読みで表に残ったまま完了と判定されるのを防ぐ）。
    ///
    /// 表にあれば列の状態、失敗の集合にある番号と払い出していない番号（0 を含む）は `Lost`、
    /// 回収済みの他は `Complete`。
    /// `wait_blocking` の待ちが失敗した番号は、表にあっても `Lost`。
    fn state_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> CompletionState {
        if self.wait_failures.contains_key(&sequence) {
            return CompletionState::Lost;
        }
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
    ///
    /// 登録の世代が `current_generation` より小さいものは列の状態に関わらず `Lost`、大きいものは `Pending` で残す。
    fn take_ready(
        &mut self,
        current_generation: u64,
        classify: impl Fn(&G) -> GroupState,
    ) -> Vec<(CompletionCallback, CompletionState)> {
        let mut ready = Vec::new();
        for (sequence, generation, callback) in std::mem::take(&mut self.callbacks) {
            let state = match generation.cmp(&current_generation) {
                std::cmp::Ordering::Less => CompletionState::Lost,
                std::cmp::Ordering::Greater => CompletionState::Pending,
                std::cmp::Ordering::Equal => self.state_of(sequence, &classify),
            };
            match state {
                CompletionState::Pending => self.callbacks.push((sequence, generation, callback)),
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
            .map(|(_, _, callback)| callback)
            .collect()
    }

    /// 登録の世代が `current_generation` より小さい callback を全部外し、`Lost` を付けて返す。呼ぶのは錠を放してから。
    #[cfg(any(test, feature = "fault-injection"))]
    fn take_stale(
        &mut self,
        current_generation: u64,
    ) -> Vec<(CompletionCallback, CompletionState)> {
        let mut stale = Vec::new();
        for (sequence, generation, callback) in std::mem::take(&mut self.callbacks) {
            if generation < current_generation {
                stale.push((callback, CompletionState::Lost));
            } else {
                self.callbacks.push((sequence, generation, callback));
            }
        }
        stale
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

    /// 通し番号が投入済みの表に残っているか。呼ぶ前に `reclaim` しておく。
    ///
    /// 回収が先頭で止まった後ろの番号は、完了していても真になる。
    /// `wait_blocking` の待ちが失敗した番号は、表から外れても真。
    fn is_unfinished(&self, sequence: u64) -> bool {
        self.pending.contains_key(&sequence) || self.wait_failures.contains_key(&sequence)
    }

    /// `wait_blocking` が待つ前に、通し番号から次の一歩を決める。呼ぶ前に `reclaim` しておく。表を変えない。
    ///
    /// 払い出していない番号（0 を含む）は `Unissued`、待ちが失敗した番号はその値、失敗の集合にある番号は
    /// `BackendFailure`、表にあれば `Wait`、回収済みの他は `Done`。
    #[cfg(any(test, feature = "native-blocking-wait"))]
    fn wait_step(&self, sequence: u64) -> WaitStep {
        if sequence == 0 || sequence >= self.next_sequence {
            return WaitStep::Unissued {
                last: self.next_sequence.saturating_sub(1),
            };
        }
        if let Some(error) = self.wait_failures.get(&sequence) {
            return WaitStep::Failed(error.clone());
        }
        if self.failed.contains(&sequence) {
            return WaitStep::Failed(failed_wait_error());
        }
        if self.pending.contains_key(&sequence) {
            WaitStep::Wait
        } else {
            WaitStep::Done
        }
    }

    /// `wait_blocking` の待ちが失敗した番号に印を付ける。印を付けた番号には、最初に付けた値を持ち続ける。
    #[cfg(any(test, feature = "native-blocking-wait"))]
    fn mark_wait_failure(&mut self, sequence: u64, error: ContractError) {
        self.wait_failures.entry(sequence).or_insert(error);
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

/// `wait_blocking` が待つ前に投入済みの表から決める次の一歩。
#[cfg(any(test, feature = "native-blocking-wait"))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum WaitStep {
    /// 投入の記録の無い番号。`last` は最後に払い出した番号（無ければ 0）。
    Unissued { last: u64 },
    /// 失敗した投入。値はそのまま返す。
    Failed(ContractError),
    /// 投入して回収済み。
    Done,
    /// 投入済みの表にあり、待つ。
    Wait,
}

/// 失敗の集合にある番号に `wait_blocking` が返す値。
#[cfg(any(test, feature = "native-blocking-wait"))]
fn failed_wait_error() -> ContractError {
    ContractError::BackendFailure {
        op: "wait_blocking",
        backend: BACKEND,
        detail: "the submission ended with an error".into(),
    }
}

/// 投入の記録の無い番号に `wait_blocking` が返す値。
#[cfg(any(test, feature = "native-blocking-wait"))]
fn unissued_wait_error(requested: u64, last: u64) -> ContractError {
    ContractError::InvalidArgument(format!(
        "wait_blocking: sequence {requested} was not returned by submit on this device (last returned: {last})"
    ))
}

/// `wait_blocking` の待つ前の照らし。順は、ロスト済み（今の世代の `DeviceLost`）、別 device の印（`DeviceMismatch`）、
/// 世代違いの印（印の世代の `DeviceLost`）。
#[cfg(any(test, feature = "native-blocking-wait"))]
fn check_wait_token(
    lost: bool,
    token: &SubmitToken,
    device_id: u64,
    generation: u64,
) -> Result<(), ContractError> {
    if lost {
        return Err(ContractError::DeviceLost { generation });
    }
    check_token_owner(token, device_id, generation)
}

/// 期限つきの待ちで、event の完了を確かめ直す間に挟む時間の上限。
#[cfg(any(test, feature = "native-blocking-wait"))]
const WAIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_micros(200);

/// 期限つきの待ち。`probe` が `Pending` の間、`sleep` を挟んで確かめ直し、`elapsed` が `limit` に届いたら `Timeout`。
///
/// 1 回目の確かめは期限によらず行う。挟む時間は残りと `WAIT_POLL_INTERVAL` の短い方。
#[cfg(any(test, feature = "native-blocking-wait"))]
fn wait_until_limit(
    limit: std::time::Duration,
    mut probe: impl FnMut() -> CompletionState,
    mut elapsed: impl FnMut() -> std::time::Duration,
    mut sleep: impl FnMut(std::time::Duration),
) -> Result<CompletionState, ContractError> {
    loop {
        match probe() {
            CompletionState::Pending => {}
            done => return Ok(done),
        }
        let spent = elapsed();
        if spent >= limit {
            return Err(ContractError::Timeout);
        }
        sleep((limit - spent).min(WAIT_POLL_INTERVAL));
    }
}

/// コピー・クリアの offset と長さが守る倍数。
const BUFFER_TRANSFER_ALIGN: u64 = 4;

/// 記録したバッファ 1 本。識別子と払い出しの世代で指し、driver の型を持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordedBuffer {
    buffer_id: u64,
    generation: u64,
}

/// 記録先に積んだバッファのコピー・クリア 1 件。`copy_buffer_to_buffer`・`clear_buffer` が照らしの後に積む。
///
/// 長さ 0 は積まないので、`size` は 1 以上。
#[derive(Debug, Clone, PartialEq, Eq)]
enum TransferRecord {
    Copy {
        src: RecordedBuffer,
        src_offset: u64,
        dst: RecordedBuffer,
        dst_offset: u64,
        size: u64,
    },
    Clear {
        buffer: RecordedBuffer,
        offset: u64,
        size: u64,
    },
    /// `dispatch_ex` が照らしの後に積む dispatch 1 件（TASK-805）。
    Dispatch(DispatchRecord),
}

impl TransferRecord {
    /// この記録が触るバッファを並びの順に返す。投入の時の確かめと、最後に使った投入の更新に使う。
    fn buffers(&self) -> Vec<RecordedBuffer> {
        match self {
            TransferRecord::Copy { src, dst, .. } => vec![*src, *dst],
            TransferRecord::Clear { buffer, .. } => vec![*buffer],
            TransferRecord::Dispatch(dispatch) => dispatch
                .args
                .iter()
                .filter_map(|arg| match arg {
                    RecordedArg::Buffer { buffer, .. } => Some(*buffer),
                    RecordedArg::Constant { .. } => None,
                })
                .collect(),
        }
    }
}

/// 記録した dispatch の引数 1 件。カーネルの引数の並びの 1 つに当たり、driver の型を持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordedArg {
    /// 束縛のバッファ。ポインタは投入の時に表から解決し、`offset` を足す。
    Buffer {
        slot: u32,
        buffer: RecordedBuffer,
        offset: u64,
    },
    /// 定数。bytes は記録の時点の写し。
    Constant { slot: u32, bytes: Vec<u8> },
}

/// 記録した dispatch 1 件。引数は束縛と定数をあわせて slot の昇順に並べ、パイプラインは表の識別子で指す。
#[derive(Debug, Clone, PartialEq, Eq)]
struct DispatchRecord {
    pipeline: u64,
    args: Vec<RecordedArg>,
    grid: [u32; 3],
    block: [u32; 3],
    /// `ThreadgroupMemory` の各 `bytes` の合計（動的な共有メモリのバイト数）。
    shared_mem_bytes: u32,
}

/// `check_dispatch_record` に渡す束縛 1 件の事実。表の照らしを通った後の値。
#[derive(Debug, Clone, Copy)]
struct DispatchBufferArg {
    slot: u32,
    buffer: RecordedBuffer,
    offset: u64,
}

/// `dispatch_ex` の記録の時点の、slot・定数・共有メモリの照らし。device に触らない。
///
/// 照らす順は、slot（束縛・定数の順に slot ごとに重なり・配置に無い、その後で配置にあって渡されない）、定数の空、共有メモリの合計の
/// あふれ。最初に当たったものを
/// `dispatch_ex: ` を前置した `InvalidArgument` で返す。通れば、束縛と定数を slot の昇順に並べた記録を返す。
/// `offset` はバッファの長さと比べない。
fn check_dispatch_record(
    pipeline: u64,
    layout_slots: &[u32],
    bindings: &[DispatchBufferArg],
    consts: &[(u32, &[u8])],
    dispatch_dims: [u32; 3],
    threads_per_threadgroup: [u32; 3],
    threadgroup_memory: &[u32],
) -> Result<DispatchRecord, ContractError> {
    let invalid = |msg: String| {
        Err(ContractError::InvalidArgument(format!(
            "dispatch_ex: {msg}"
        )))
    };
    let mut used = BTreeSet::new();
    for slot in bindings
        .iter()
        .map(|b| b.slot)
        .chain(consts.iter().map(|(slot, _)| *slot))
    {
        if !used.insert(slot) {
            return invalid(format!("duplicate slot in dispatch: {slot}"));
        }
        if !layout_slots.contains(&slot) {
            return invalid(format!("slot {slot} is not in the binding layout"));
        }
    }
    if let Some(slot) = layout_slots.iter().find(|slot| !used.contains(slot)) {
        return invalid(format!("slot {slot} of the binding layout is not provided"));
    }
    if let Some((slot, _)) = consts.iter().find(|(_, bytes)| bytes.is_empty()) {
        return invalid(format!("constant slot {slot} is empty"));
    }
    let Some(shared_mem_bytes) = threadgroup_memory
        .iter()
        .try_fold(0_u32, |total, bytes| total.checked_add(*bytes))
    else {
        return invalid("threadgroup memory total overflows".into());
    };
    let mut args: Vec<RecordedArg> = bindings
        .iter()
        .map(|b| RecordedArg::Buffer {
            slot: b.slot,
            buffer: b.buffer,
            offset: b.offset,
        })
        .chain(consts.iter().map(|(slot, bytes)| RecordedArg::Constant {
            slot: *slot,
            bytes: bytes.to_vec(),
        }))
        .collect();
    args.sort_by_key(|arg| match arg {
        RecordedArg::Buffer { slot, .. } | RecordedArg::Constant { slot, .. } => *slot,
    });
    Ok(DispatchRecord {
        pipeline,
        args,
        grid: dispatch_dims,
        block: threads_per_threadgroup,
        shared_mem_bytes,
    })
}

/// `dispatch_ex` の記録の時点の、`dispatch_dims` の照らし。device に触らない。
///
/// どれかの要素が 0 なら `dispatch_dims contains zero`、0 を含まずにどれかの要素が `max_dispatch_dims` の
/// 同じ軸の値を超えれば `dispatch_dims exceeds the limit` の `InvalidArgument` を返す（TASK-806）。
fn check_dispatch_dims(
    dispatch_dims: [u32; 3],
    max_dispatch_dims: [u32; 3],
) -> Result<(), ContractError> {
    if dispatch_dims.contains(&0) {
        return Err(ContractError::InvalidArgument(
            "dispatch_dims contains zero".into(),
        ));
    }
    if dispatch_dims
        .iter()
        .zip(max_dispatch_dims)
        .any(|(dim, max)| *dim > max)
    {
        return Err(ContractError::InvalidArgument(
            "dispatch_dims exceeds the limit".into(),
        ));
    }
    Ok(())
}

/// コピー・クリアの記録の時点で、表から引いたバッファ 1 本の事実。
#[derive(Debug, Clone, Copy)]
struct TransferEntry {
    buffer_id: u64,
    /// ハンドルの世代。
    generation: u64,
    /// 表の項目を払い出したときの世代が、ハンドルの世代と同じか。
    issued_matches: bool,
    /// ハンドルの `size()` が表のバッファの長さと同じか。
    size_matches: bool,
    /// 確保のときに記録した用途。
    usage: BufferUsage,
    /// 表のバッファの長さ。
    allocation_len: u64,
}

/// コピー・クリアの記録の時点で、ハンドル 1 本について引いた事実。
#[derive(Debug, Clone, Copy)]
struct TransferOperand {
    /// ハンドルの払い出し元がこの device か。
    same_device: bool,
    /// ハンドルの世代が今の世代と同じか。
    generation_current: bool,
    /// 表に項目があれば、その事実。
    entry: Option<TransferEntry>,
}

/// ハンドル 1 本の device・世代・表・大きさを、この順に照らして表の事実を返す。device に触らない。
fn check_transfer_operand(operand: TransferOperand) -> Result<TransferEntry, ContractError> {
    if !operand.same_device {
        return Err(ContractError::DeviceMismatch);
    }
    if !operand.generation_current {
        return Err(ContractError::InvalidArgument(
            "buffer handle generation does not match the device".into(),
        ));
    }
    let Some(entry) = operand.entry.filter(|entry| entry.issued_matches) else {
        return Err(ContractError::InvalidArgument(
            "unknown buffer handle".into(),
        ));
    };
    if !entry.size_matches {
        return Err(ContractError::InvalidArgument(
            "buffer handle size does not match the buffer length".into(),
        ));
    }
    Ok(entry)
}

/// 記録した用途が役割を満たすかを照らす。拒否は `op` を前置した `InvalidArgument`。
fn check_transfer_usage(
    op: &str,
    usage: BufferUsage,
    role: UsageRole,
) -> Result<(), ContractError> {
    match validate_usage_for_role(usage, role) {
        Ok(()) => Ok(()),
        Err(ContractError::InvalidArgument(msg)) => {
            Err(ContractError::InvalidArgument(format!("{op}: {msg}")))
        }
        Err(other) => Err(other),
    }
}

/// コピーの元・先とクリアの対象の offset と長さの整列を照らす。
fn check_transfer_alignment(op: &str, offsets: &[u64], size: u64) -> Result<(), ContractError> {
    if offsets
        .iter()
        .chain(std::iter::once(&size))
        .any(|value| value % BUFFER_TRANSFER_ALIGN != 0)
    {
        return Err(ContractError::InvalidArgument(format!(
            "{op}: offset and size must be multiples of {BUFFER_TRANSFER_ALIGN}"
        )));
    }
    Ok(())
}

/// `copy_buffer_to_buffer` の記録の時点の照らし。device に触らない。
///
/// 照らす順は、device（元 → 先）・世代と表と大きさ（元 → 先）・用途（元 → 先）・範囲（元 → 先）・整列・
/// 同じバッファの拒否・長さ 0。各行は両方のバッファに当ててから次の行へ進む。通って長さが 0 なら `Ok(None)`
/// （何も記録しない）。
fn check_copy_record(
    src: TransferOperand,
    src_offset: u64,
    dst: TransferOperand,
    dst_offset: u64,
    size: u64,
) -> Result<Option<TransferRecord>, ContractError> {
    if !src.same_device || !dst.same_device {
        return Err(ContractError::DeviceMismatch);
    }
    let src_entry = check_transfer_operand(src)?;
    let dst_entry = check_transfer_operand(dst)?;
    check_transfer_usage(
        "copy_buffer_to_buffer",
        src_entry.usage,
        UsageRole::CopyBufferSource,
    )?;
    check_transfer_usage(
        "copy_buffer_to_buffer",
        dst_entry.usage,
        UsageRole::CopyBufferDestination,
    )?;
    validate_buffer_range(src_offset, size, src_entry.allocation_len)?;
    validate_buffer_range(dst_offset, size, dst_entry.allocation_len)?;
    check_transfer_alignment("copy_buffer_to_buffer", &[src_offset, dst_offset], size)?;
    if src_entry.buffer_id == dst_entry.buffer_id {
        return Err(ContractError::InvalidArgument(
            "copy_buffer_to_buffer: source and destination must be different buffers".into(),
        ));
    }
    if size == 0 {
        return Ok(None);
    }
    Ok(Some(TransferRecord::Copy {
        src: RecordedBuffer {
            buffer_id: src_entry.buffer_id,
            generation: src_entry.generation,
        },
        src_offset,
        dst: RecordedBuffer {
            buffer_id: dst_entry.buffer_id,
            generation: dst_entry.generation,
        },
        dst_offset,
        size,
    }))
}

/// `clear_buffer` の記録の時点の照らし。device に触らない。
///
/// 照らす順は、device・世代と表と大きさ・用途（`UPLOAD`）・範囲・整列・長さ 0。通って長さが 0 なら `Ok(None)`。
fn check_clear_record(
    buffer: TransferOperand,
    offset: u64,
    size: u64,
) -> Result<Option<TransferRecord>, ContractError> {
    let entry = check_transfer_operand(buffer)?;
    if !entry.usage.contains(BufferUsage::UPLOAD) {
        return Err(ContractError::InvalidArgument(
            "clear_buffer: buffer usage lacks UPLOAD".into(),
        ));
    }
    validate_buffer_range(offset, size, entry.allocation_len)?;
    check_transfer_alignment("clear_buffer", &[offset], size)?;
    if size == 0 {
        return Ok(None);
    }
    Ok(Some(TransferRecord::Clear {
        buffer: RecordedBuffer {
            buffer_id: entry.buffer_id,
            generation: entry.generation,
        },
        offset,
        size,
    }))
}

/// 投入の時に stream へ積む 1 件。バッファは識別子で指し、範囲は表の長さで照らし直した `usize` の `[start, end)`。
#[derive(Debug, Clone, PartialEq, Eq)]
enum TransferJob {
    Copy {
        src: u64,
        src_range: (usize, usize),
        dst: u64,
        dst_range: (usize, usize),
    },
    Clear {
        buffer: u64,
        range: (usize, usize),
    },
    /// 照らしを通った dispatch 1 件。ポインタの解決は積む前に `prepare_launches` が全件まとめて行う。
    Dispatch(DispatchRecord),
}

impl TransferJob {
    /// この件が触るバッファの識別子を並びの順に返す。
    fn buffer_ids(&self) -> Vec<u64> {
        match self {
            TransferJob::Copy { src, dst, .. } => vec![*src, *dst],
            TransferJob::Clear { buffer, .. } => vec![*buffer],
            TransferJob::Dispatch(dispatch) => dispatch
                .args
                .iter()
                .filter_map(|arg| match arg {
                    RecordedArg::Buffer { buffer, .. } => Some(buffer.buffer_id),
                    RecordedArg::Constant { .. } => None,
                })
                .collect(),
        }
    }
}

/// 利用者のカーネルを走らせる門。単体の試験のビルドだけで開き、ほかのビルドでは dispatch の記録を含む投入を `Unsupported` で拒む。
const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);

/// dispatch の記録を含む記録先の列を、カーネルを走らせる門に照らす。門が閉じていれば `Unsupported`（`op` は `submit`）。
///
/// 解放済みの確かめと範囲の照らし（`plan_transfers`）の後に呼ぶ。コピー・クリアだけの列は門に関わらず通す。
fn check_dispatch_gate(jobs: &[TransferJob], enabled: bool) -> Result<(), ContractError> {
    if !enabled
        && jobs
            .iter()
            .any(|job| matches!(job, TransferJob::Dispatch(_)))
    {
        return Err(ContractError::Unsupported {
            op: "submit",
            backend: BACKEND,
        });
    }
    Ok(())
}

/// カーネルの仮引数の数と大きさを、記録した引数のバイト数と照らす。device に触らない。
///
/// `query(i)` は i 番目の仮引数の大きさを返し、仮引数が無ければ `None`。引数の数と同じ番号まで問い、数・大きさの
/// どちらかが食い違えば `submit: ` を前置した `InvalidArgument`。問う回数は引数の数 + 1 以下。
fn check_kernel_params(
    arg_lens: &[usize],
    mut query: impl FnMut(usize) -> Result<Option<usize>, ContractError>,
) -> Result<(), ContractError> {
    let invalid = |msg: String| Err(ContractError::InvalidArgument(format!("submit: {msg}")));
    for (index, len) in arg_lens.iter().enumerate() {
        match query(index)? {
            Some(size) if size == *len => {}
            Some(size) => {
                return invalid(format!(
                    "kernel parameter {index} takes {size} bytes but {len} were recorded"
                ));
            }
            None => {
                return invalid(format!(
                    "the kernel takes {index} parameters but {} were recorded",
                    arg_lens.len()
                ));
            }
        }
    }
    if query(arg_lens.len())?.is_some() {
        return invalid(format!(
            "the kernel takes more than the {} recorded parameters",
            arg_lens.len()
        ));
    }
    Ok(())
}

/// `function` の `index` 番目の仮引数の大きさを driver に問う。仮引数が無ければ（`CUDA_ERROR_INVALID_VALUE`）`None`。
///
/// ほかの driver の失敗と、driver に関数が無いときの panic は `BackendFailure`（`op` は `cuFuncGetParamInfo`）。
fn kernel_param_size(
    function: &CudaFunction,
    index: usize,
) -> Result<Option<usize>, ContractError> {
    guard_memory_panic("cuFuncGetParamInfo", || {
        let mut offset = 0_usize;
        let mut size = 0_usize;
        // SAFETY: `function` は読み込まれたモジュールの関数で、呼ぶ側がモジュールの `Arc` を持つ `CudaFunction` を借りている。
        // 2 つの出力のポインタは、この呼び出しの間生きるこの関数の局所の `usize` を指し、driver はそこへ書くだけで保持しない。
        let result = unsafe {
            cudarc::driver::sys::cuFuncGetParamInfo(
                function.cu_function(),
                index,
                &raw mut offset,
                &raw mut size,
            )
        };
        match result {
            cudarc::driver::sys::CUresult::CUDA_SUCCESS => Ok(Some(size)),
            cudarc::driver::sys::CUresult::CUDA_ERROR_INVALID_VALUE => Ok(None),
            other => Err(backend_failure("cuFuncGetParamInfo", other)),
        }
    })
}

/// 起動 1 件の引数 1 つ。`kernelParams` の各要素はこの値を指す。
enum LaunchArg<'a> {
    /// 束縛のポインタ。解決と `offset` の足し算を終えた値。
    Pointer(cudarc::driver::sys::CUdeviceptr),
    /// 定数。件の bytes を借りる。
    Bytes(&'a [u8]),
}

/// 投入の時に積む前に組み立てた起動 1 件。引数はカーネルの引数の並びの順。
struct PreparedLaunch<'a> {
    /// 関数。モジュールの参照を持つので、起動の間モジュールは読み込まれたまま。
    function: CudaFunction,
    args: Vec<LaunchArg<'a>>,
    grid: [u32; 3],
    block: [u32; 3],
    shared_mem_bytes: u32,
}

/// `[offset, offset + size)` がバッファの長さ `len` に収まるかをあふれなく照らし、`usize` の範囲に直す。
fn transfer_range(offset: u64, size: u64, len: u64) -> Result<(usize, usize), ContractError> {
    let exceeds =
        || ContractError::InvalidArgument("recorded transfer range exceeds the buffer".into());
    let end = offset
        .checked_add(size)
        .filter(|end| *end <= len)
        .ok_or_else(exceeds)?;
    let start = usize::try_from(offset).map_err(|_| exceeds())?;
    let end = usize::try_from(end).map_err(|_| exceeds())?;
    Ok((start, end))
}

/// 投入の時に、記録の列を stream に積む件の列へ組み立てる。device に触らない。
///
/// `lookup` は識別子から（払い出しの世代, 長さ）を引く。先に全記録の全バッファを並びの順に見て、表に無い・表の世代が
/// 記録の世代と違う・記録の世代が今の世代と違うものがあれば、範囲を見ずに `InvalidArgument`。通れば各記録の範囲を
/// 表の長さで照らし直す。dispatch の束縛の解放済みは、文に束縛の slot を載せる。dispatch の記録はそのまま件にする。
fn plan_transfers(
    records: &[TransferRecord],
    lookup: impl Fn(u64) -> Option<(u64, u64)>,
    current_generation: u64,
) -> Result<Vec<TransferJob>, ContractError> {
    let alive = |buffer: &RecordedBuffer| {
        buffer.generation == current_generation
            && lookup(buffer.buffer_id).map(|(generation, _)| generation) == Some(buffer.generation)
    };
    for record in records {
        if let TransferRecord::Dispatch(dispatch) = record {
            for arg in &dispatch.args {
                if let RecordedArg::Buffer { slot, buffer, .. } = arg
                    && !alive(buffer)
                {
                    return Err(ContractError::InvalidArgument(format!(
                        "a bound buffer was freed before submit (binding slot {slot})"
                    )));
                }
            }
        } else if !record.buffers().iter().all(alive) {
            return Err(ContractError::InvalidArgument(
                "a copied or cleared buffer was freed before submit".into(),
            ));
        }
    }
    let length_of = |id: u64| {
        lookup(id)
            .map(|(_, len)| len)
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))
    };
    records
        .iter()
        .map(|record| match record {
            TransferRecord::Copy {
                src,
                src_offset,
                dst,
                dst_offset,
                size,
            } => Ok(TransferJob::Copy {
                src: src.buffer_id,
                src_range: transfer_range(*src_offset, *size, length_of(src.buffer_id)?)?,
                dst: dst.buffer_id,
                dst_range: transfer_range(*dst_offset, *size, length_of(dst.buffer_id)?)?,
            }),
            TransferRecord::Clear {
                buffer,
                offset,
                size,
            } => Ok(TransferJob::Clear {
                buffer: buffer.buffer_id,
                range: transfer_range(*offset, *size, length_of(buffer.buffer_id)?)?,
            }),
            TransferRecord::Dispatch(dispatch) => Ok(TransferJob::Dispatch(dispatch.clone())),
        })
        .collect()
}

/// 表から識別子の異なる 2 項目を同時に借りる。`shared` は共有、`unique` は排他。同じ識別子・どちらかが無ければ `None`。
fn pair_mut<T>(map: &mut BTreeMap<u64, T>, shared: u64, unique: u64) -> Option<(&T, &mut T)> {
    if shared == unique {
        return None;
    }
    let mut first = None;
    let mut second = None;
    for (id, value) in map.iter_mut() {
        if *id == shared {
            first = Some(&*value);
        } else if *id == unique {
            second = Some(value);
        }
    }
    Some((first?, second?))
}

/// 開いている記録先の識別子と、その記録先に積んだコピー・クリアの列。`CommandEncoder` の破棄の hook を兼ねる。
#[derive(Default)]
struct OpenEncoders {
    ids: Mutex<BTreeMap<u64, Vec<TransferRecord>>>,
}

impl OpenEncoders {
    /// 表の錠を取る。poison されていても中身を使う。
    fn lock(&self) -> MutexGuard<'_, BTreeMap<u64, Vec<TransferRecord>>> {
        self.ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 識別子を空の列で表に入れる。0 は入れない。
    fn open(&self, id: u64) {
        if id != 0 {
            self.lock().entry(id).or_default();
        }
    }

    /// 識別子が開いているか。
    fn is_open(&self, id: u64) -> bool {
        self.lock().contains_key(&id)
    }

    /// 開いている記録先の列の末尾に記録を足す。開いていなければ `InvalidArgument` で何も足さない。
    fn record(&self, id: u64, record: TransferRecord) -> Result<(), ContractError> {
        match self.lock().get_mut(&id) {
            Some(records) => {
                records.push(record);
                Ok(())
            }
            None => Err(ContractError::InvalidArgument(
                "encoder is not open on this device".into(),
            )),
        }
    }

    /// 渡された識別子が全部開いていれば表から外し、各記録先の列を渡された順につないで返す。
    ///
    /// 1 つでも無い（0 を含む）・重複していれば `InvalidArgument` で、表は変えない。
    fn claim(&self, ids: &[u64]) -> Result<Vec<TransferRecord>, ContractError> {
        let mut open = self.lock();
        let mut seen = BTreeSet::new();
        for id in ids {
            if !open.contains_key(id) || !seen.insert(*id) {
                return Err(ContractError::InvalidArgument(
                    "encoder is not open on this device".into(),
                ));
            }
        }
        let mut records = Vec::new();
        for id in ids {
            if let Some(mut list) = open.remove(id) {
                records.append(&mut list);
            }
        }
        Ok(records)
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

/// ロスト済みなら、渡した世代を載せた `DeviceLost` を返す。device に触らない。
///
/// `CudaDevice::check_not_lost` が device の状態と呼んだ時点の世代を渡して呼ぶ。
fn lost_result(lost: bool, generation: u64) -> Result<(), ContractError> {
    if lost {
        return Err(ContractError::DeviceLost { generation });
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
    /// 払い出したときの device の世代。
    generation: u64,
    /// コピー・クリアで最後に使った投入の通し番号。使っていなければ `None`。
    last_submission: Option<u64>,
}

/// 表から、識別子が `id` で払い出しの世代が `generation` の項目だけを外して返す。
///
/// 世代が違う・表に無い識別子は何も外さず `None`。
fn remove_issued<T>(
    map: &mut BTreeMap<u64, T>,
    id: u64,
    generation: u64,
    issued_generation: impl Fn(&T) -> u64,
) -> Option<T> {
    match map.get(&id) {
        Some(entry) if issued_generation(entry) == generation => map.remove(&id),
        _ => None,
    }
}

/// 記録した用途が操作の役割を満たすかを範囲より先に照らし、通れば `[start, end)` を返す純粋関数。
///
/// 用途の拒否は `op` を前置した `InvalidArgument`。範囲は `validate_buffer_range` と `usize` への変換で照らす。
fn access_range(
    op: &'static str,
    role: UsageRole,
    recorded: BufferUsage,
    entry_len: u64,
    offset: u64,
    len: u64,
) -> Result<(usize, usize), ContractError> {
    match validate_usage_for_role(recorded, role) {
        Ok(()) => {}
        Err(ContractError::InvalidArgument(msg)) => {
            return Err(ContractError::InvalidArgument(format!("{op}: {msg}")));
        }
        Err(other) => return Err(other),
    }
    validate_buffer_range(offset, len, entry_len)?;
    let start = usize::try_from(offset)
        .map_err(|_| ContractError::InvalidArgument("offset exceeds usize".into()))?;
    let len = usize::try_from(len)
        .map_err(|_| ContractError::InvalidArgument("length exceeds usize".into()))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| ContractError::InvalidArgument("range exceeds usize".into()))?;
    Ok((start, end))
}

/// device が持つバッファの表と再利用の一覧と解放の保留。ハンドルの id から `BufferEntry` を引く。
#[derive(Default)]
struct Buffers {
    next_id: u64,
    next_slot: u64,
    map: BTreeMap<u64, BufferEntry>,
    pool: ReusePool<CudaSlice<u8>, CudaEvent>,
    /// 表から外したが、最後に使った投入が投入済みの表に残っているバッファ。
    pending: PendingReleases<BufferEntry>,
}

/// 表から外したが、最後に使った投入が終わっていないバッファの列。バッファの表の錠の中に置く。
///
/// 項目は通し番号と一緒に積んだ順に持つ。ゼロ化も再利用の一覧への回しもこの部品は行わず、
/// 終わった項目を外して返す（続きは呼び出し側が錠の外で行う）。
struct PendingReleases<B> {
    items: Vec<(u64, B)>,
}

impl<B> Default for PendingReleases<B> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<B> PendingReleases<B> {
    /// 通し番号つきで末尾へ積む。
    fn push(&mut self, sequence: u64, item: B) {
        self.items.push((sequence, item));
    }

    /// `unfinished` が偽の通し番号の項目を積んだ順に外して返す。真の項目は相対の順を保って残す（先頭で止めない）。
    fn take_finished(&mut self, unfinished: impl Fn(u64) -> bool) -> Vec<B> {
        let mut finished = Vec::new();
        let mut kept = Vec::new();
        for (sequence, item) in self.items.drain(..) {
            if unfinished(sequence) {
                kept.push((sequence, item));
            } else {
                finished.push(item);
            }
        }
        self.items = kept;
        finished
    }

    /// 積んである件数。
    fn len(&self) -> usize {
        self.items.len()
    }
}

/// 表から外したバッファを、保留に入れるか解放へ回すかを決める。解放へ回すものを返し、保留に入れたら `None`。
///
/// 最後に使った投入が `Some(sequence)` で `unfinished(sequence)` が真のときだけ、保留に入れる。
fn hold_or_release<B>(
    item: B,
    last_submission: Option<u64>,
    unfinished: impl Fn(u64) -> bool,
    pending: &mut PendingReleases<B>,
) -> Option<B> {
    match last_submission {
        Some(sequence) if unfinished(sequence) => {
            pending.push(sequence, item);
            None
        }
        _ => Some(item),
    }
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
    generation: AtomicU64,
    /// 模擬のロストの状態。
    #[cfg(feature = "fault-injection")]
    lost: LostState,
    /// 試験用の門の状態。
    #[cfg(feature = "fault-injection")]
    gate: GateState,
    ownership: DeviceOwnership,
    adapter_name: String,
    limits: LimitsTable,
    /// device の作成で 1 回読んだ物理 GPU の識別子。`physical_gpu_id` が返す。
    #[cfg(feature = "ext-interop")]
    physical_gpu_id: PhysicalGpuId,
    compute_capability: (i32, i32),
    /// `query`・`query_ex` が読む能力の判定の材料。`open_device` で 1 回だけ捕捉する。
    capabilities: CapabilitySnapshot,
    stream: Arc<CudaStream>,
    buffers: Mutex<Buffers>,
    /// 解放を保留しているバッファの数。バッファの表の錠の中で書き、回収の点は錠を取らずに 0 かを読む。
    held: std::sync::atomic::AtomicUsize,
    pipelines: Mutex<Pipelines>,
    /// `compile_with_nvrtc` に入った回数。試験のビルドだけで数える。
    #[cfg(test)]
    nvrtc_calls: AtomicU64,
    /// `create_pipeline` が `bind_to_thread`・モジュールの読み込み・関数の取り出しを呼ぶ前に 1 ずつ足す回数。
    /// 試験のビルドだけで数える。
    #[cfg(test)]
    pipeline_driver_calls: AtomicU64,
    open: Arc<OpenEncoders>,
    submissions: Mutex<Submissions<Arc<CudaEvent>>>,
    /// 検証の報告の受け口の登録を置く部品。この背後実装は報告を出さない（TASK-607）。
    validation: ValidationReporter,
}

impl Drop for CudaDevice {
    /// stream を同期してから、登録済みの完了の callback を呼び、再利用の一覧・確保済みのバッファ・
    /// パイプライン・投入の表を解放する。同期を確かめられないときは、それらと stream を解放しない。
    /// ロストの通知は呼ばない。`Drop` から panic は外へ出さない。
    /// `fault-injection` のビルドでは、同期の前に門を全部開ける。
    fn drop(&mut self) {
        #[cfg(feature = "fault-injection")]
        self.gate.open_all();
        let synced = sync_confirmed(|| self.stream.synchronize());
        let generation = self.current_generation();
        // `get_mut` は錠を取らない。callback は表から外した後に呼ぶ。
        let ready = teardown_ready(
            self.submissions
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            synced,
            generation,
            event_state,
        );
        if let Some(payload) = run_callbacks(ready) {
            discard_panic(payload);
        }
        let buffers = std::mem::take(
            self.buffers
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let pipelines = std::mem::take(
            self.pipelines
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let submissions = std::mem::replace(
            self.submissions
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            Submissions::new(),
        );
        if !synced {
            std::mem::forget(Arc::clone(&self.stream));
        }
        release_if(synced, (buffers, pipelines, submissions));
    }
}

/// `sync` が `Ok` を返したときだけ `true`。`Err` と panic は `false`。
fn sync_confirmed<E>(sync: impl FnOnce() -> Result<(), E>) -> bool {
    matches!(catch_unwind(AssertUnwindSafe(sync)), Ok(Ok(())))
}

/// `synced` が真なら `held` を落とし、偽なら落とさずに手放す。落とす途中の panic は外へ出さない。
fn release_if<T>(synced: bool, held: T) {
    if synced {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(move || drop(held))) {
            discard_panic(payload);
        }
    } else {
        std::mem::forget(held);
    }
}

/// 破棄で呼ぶ登録済みの callback と値。同期を確かめたら回収して `take_ready` と同じ判定、
/// 判定が `Pending` のまま残ったものと、同期を確かめられないときは `Lost`。
/// どの callback も 1 回だけ返し、表は空になる。
fn teardown_ready<G>(
    subs: &mut Submissions<G>,
    synced: bool,
    generation: u64,
    classify: impl Fn(&G) -> GroupState,
) -> Vec<(CompletionCallback, CompletionState)> {
    let mut ready = Vec::new();
    if synced {
        subs.reclaim(&classify);
        ready = subs.take_ready(generation, &classify);
    }
    ready.extend(
        subs.take_all_callbacks()
            .into_iter()
            .map(|callback| (callback, CompletionState::Lost)),
    );
    ready
}

/// 明示の破棄の順。`flush` → `sync` → `notify` → `release` を 1 回ずつ呼び、最初の panic の中身を返す。
/// `flush` の中身を優先し、そのとき `notify` の中身は捨てる。
fn destroy_in_order<D>(
    device: D,
    flush: impl FnOnce(&D) -> Option<Box<dyn std::any::Any + Send>>,
    sync: impl FnOnce(&D),
    notify: impl FnOnce(&D) -> Option<Box<dyn std::any::Any + Send>>,
    release: impl FnOnce(D),
) -> Option<Box<dyn std::any::Any + Send>> {
    let first = flush(&device);
    sync(&device);
    let notified = notify(&device);
    release(device);
    match first {
        Some(payload) => {
            if let Some(extra) = notified {
                discard_panic(extra);
            }
            Some(payload)
        }
        None => notified,
    }
}

/// 使わない panic の中身を捨てる。中身の `Drop` が panic しても、そこから巻き戻らない。
fn discard_panic(payload: Box<dyn std::any::Any + Send>) {
    if let Err(inner) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
        std::mem::forget(inner);
    }
}

/// 駆動が `judged` の世代で決めた値を、呼ぶ直前の世代 `current` で直す。世代が進んでいれば `Lost`、そうでなければ決めた値。
fn delivered_state(decided: CompletionState, judged: u64, current: u64) -> CompletionState {
    if current > judged {
        CompletionState::Lost
    } else {
        decided
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
    /// 今の世代。
    fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// この device の NVRTC の呼び出し口。試験のビルドでは入った回数を 1 足してから `compile_cuda_c` を呼ぶ。
    /// `create_pipeline` が `CudaC` の入力のときだけ `module_input` を通して呼ぶ。
    fn compile_with_nvrtc(&self, source: &str) -> Result<Ptx, ContractError> {
        #[cfg(test)]
        self.nvrtc_calls.fetch_add(1, Ordering::SeqCst);
        compile_cuda_c(source, self.compute_capability)
    }

    /// 投入済みの表の錠を取る。poison されていても中身を使う。
    fn lock_submissions(&self) -> MutexGuard<'_, Submissions<Arc<CudaEvent>>> {
        self.submissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 模擬のロスト済みか。`fault-injection` の無いビルドでは常に偽。
    fn is_lost(&self) -> bool {
        #[cfg(feature = "fault-injection")]
        let lost = self.lost.is_lost();
        #[cfg(not(feature = "fault-injection"))]
        let lost = false;
        lost
    }

    /// 模擬のロスト済みなら、呼んだ時点の世代を載せた `DeviceLost` を返す。`fault-injection` の無いビルドでは常に通る。
    ///
    /// 世代はロストの照らしの後に読む。照らしの前に読んだ世代は、照らしが通ればロストの前の値である
    /// （模擬のロストは、ロストの状態の錠の中で世代を書いてから理由を置く）。
    fn check_not_lost(&self) -> Result<(), ContractError> {
        let lost = self.is_lost();
        lost_result(lost, self.current_generation())
    }

    /// 投入済みの表にある通し番号の完了を待つ。錠を持たずに呼ぶ。`wait_blocking` から呼ばれる。
    ///
    /// `None` は `event` を同期し、失敗は `BackendFailure`（`op` は `event_synchronize`）で番号に印を付ける。
    /// `Some(limit)` は表の錠を確かめのたびに取り直して完了を確かめ、届かなければ `Timeout`（印は付けない）。
    /// 待った後の状態が `Lost` なら番号に返す値、`Pending` なら `BackendFailure`。
    #[cfg(feature = "native-blocking-wait")]
    fn wait_issued(
        &self,
        sequence: u64,
        event: Option<Arc<CudaEvent>>,
        timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        let state = match timeout {
            None => {
                if let Some(event) = event {
                    let synced = guard_memory_panic("event_synchronize", || {
                        event
                            .synchronize()
                            .map_err(|e| backend_failure("event_synchronize", e))
                    });
                    if let Err(error) = synced {
                        self.lock_submissions()
                            .mark_wait_failure(sequence, error.clone());
                        return Err(error);
                    }
                }
                self.lock_submissions()
                    .reclaim_and_state_of(sequence, event_state)
            }
            Some(limit) => {
                let started = std::time::Instant::now();
                wait_until_limit(
                    limit,
                    || {
                        self.lock_submissions()
                            .reclaim_and_state_of(sequence, event_state)
                    },
                    || started.elapsed(),
                    std::thread::sleep,
                )?
            }
        };
        match state {
            CompletionState::Complete => Ok(()),
            CompletionState::Lost => Err(match self.lock_submissions().wait_step(sequence) {
                WaitStep::Failed(error) => error,
                _ => failed_wait_error(),
            }),
            CompletionState::Pending => Err(ContractError::BackendFailure {
                op: "wait_blocking",
                backend: BACKEND,
                detail: "submission is still pending after the wait".into(),
            }),
        }
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

    /// 記録先の列を stream に積んでから event を記録し、通し番号を払い出して印を返す。呼ぶ側がバッファの表と投入済みの表の錠を持つ。
    ///
    /// `jobs` を記録の順に積み、通れば各バッファの最後に使った投入をこの番号にする。積む途中と event の記録の失敗は
    /// `BackendFailure` で、番号を消費せず最後に使った投入を変えない。完了は待たない。
    /// `fault-injection` のビルドで門を仕込んであれば、event の前に門を積む。積めなければ `BackendFailure`
    /// （`op` は `launch_host_function`）で仕込みを残す。門を積んだ後に event の記録が失敗したら、門は積んだまま残り `open_gate` で開く。
    fn issue_event(
        &self,
        subs: &mut Submissions<Arc<CudaEvent>>,
        generation: u64,
        map: &mut BTreeMap<u64, BufferEntry>,
        jobs: &[TransferJob],
    ) -> Result<SubmitToken, ContractError> {
        let mut launches = self.prepare_launches(map, jobs)?;
        let sequence = subs.issue(|| {
            #[cfg(feature = "fault-injection")]
            self.gate.attach_with(|latch| {
                guard_memory_panic("launch_host_function", || self.launch_gate(latch))
            })?;
            self.launch_transfers(map, jobs, &mut launches)?;
            guard_memory_panic("record_event", || {
                self.stream
                    .record_event(None)
                    .map(Arc::new)
                    .map_err(|e| backend_failure("record_event", e))
            })
        })?;
        for id in jobs.iter().flat_map(TransferJob::buffer_ids) {
            if let Some(entry) = map.get_mut(&id) {
                entry.last_submission = Some(sequence);
            }
        }
        Ok(SubmitToken::__backend_new(generation, sequence, self.id))
    }

    /// 記録の順に、コピー・クリアを stream へ積む。host は待たない。
    ///
    /// 範囲は件ごとに積む前に `try_slice`・`try_slice_mut` で切り、外れれば `InvalidArgument`。
    /// driver の失敗と panic は `BackendFailure`（`op` は `memcpy_dtod`・`memset_zeros`・`cuLaunchKernel`）で、積んだ分は戻さない。
    /// dispatch の件は `launches` の先頭から順に 1 件ずつ取って起動する。
    fn launch_transfers(
        &self,
        map: &mut BTreeMap<u64, BufferEntry>,
        jobs: &[TransferJob],
        launches: &mut std::vec::IntoIter<PreparedLaunch<'_>>,
    ) -> Result<(), ContractError> {
        let unknown = || ContractError::InvalidArgument("unknown buffer handle".into());
        for job in jobs {
            match *job {
                TransferJob::Copy {
                    src,
                    src_range,
                    dst,
                    dst_range,
                } => {
                    let (src_entry, dst_entry) = pair_mut(map, src, dst).ok_or_else(unknown)?;
                    let view = src_entry
                        .slice
                        .try_slice(src_range.0..src_range.1)
                        .ok_or_else(unknown)?;
                    let mut target = dst_entry
                        .slice
                        .try_slice_mut(dst_range.0..dst_range.1)
                        .ok_or_else(unknown)?;
                    guard_memory_panic("memcpy_dtod", || {
                        self.stream
                            .memcpy_dtod(&view, &mut target)
                            .map_err(|e| backend_failure("memcpy_dtod", e))
                    })?;
                }
                TransferJob::Clear { buffer, range } => {
                    let entry = map.get_mut(&buffer).ok_or_else(unknown)?;
                    let mut target = entry
                        .slice
                        .try_slice_mut(range.0..range.1)
                        .ok_or_else(unknown)?;
                    guard_memory_panic("memset_zeros", || {
                        self.stream
                            .memset_zeros(&mut target)
                            .map_err(|e| backend_failure("memset_zeros", e))
                    })?;
                }
                TransferJob::Dispatch(_) => {
                    let mut launch =
                        launches
                            .next()
                            .ok_or_else(|| ContractError::BackendFailure {
                                op: "cuLaunchKernel",
                                backend: BACKEND,
                                detail: "the dispatch has no prepared launch".into(),
                            })?;
                    guard_memory_panic("cuLaunchKernel", || self.launch_kernel(&mut launch))?;
                }
            }
        }
        Ok(())
    }

    /// 投入の時に、dispatch の全件について、パイプラインの関数を引いてカーネルの仮引数と照らし、束縛のポインタを解決する。
    ///
    /// コピー・クリア・カーネルを stream に積む前に呼ぶ。1 件でも引けない・食い違う・解決できなければ、コピー・クリア・カーネルを
    /// 何も積まずに `InvalidArgument` を返す（all-or-nothing）。先に全件の関数と仮引数を照らし（ポインタは 8 バイトの引数として数える）、
    /// その後で全件のポインタを解決する。ポインタと `offset` の和があふれる場合も `InvalidArgument`。和がバッファの長さを超えるかは
    /// 照らさない。ポインタの解決は cudarc の event の待ち・記録を stream に積むことがあり、その失敗は解決の後に `BackendFailure`
    /// （`op` は `device_ptr`）で返す。錠は呼ぶ側が持つ「バッファの表 → 投入済みの表」の後に、パイプラインの表を取る。
    fn prepare_launches<'a>(
        &self,
        map: &BTreeMap<u64, BufferEntry>,
        jobs: &'a [TransferJob],
    ) -> Result<std::vec::IntoIter<PreparedLaunch<'a>>, ContractError> {
        let dispatches: Vec<&DispatchRecord> = jobs
            .iter()
            .filter_map(|job| match job {
                TransferJob::Dispatch(dispatch) => Some(dispatch),
                _ => None,
            })
            .collect();
        if dispatches.is_empty() {
            return Ok(Vec::new().into_iter());
        }
        guard_memory_panic("bind_to_thread", || {
            self.stream
                .context()
                .bind_to_thread()
                .map_err(|e| backend_failure("bind_to_thread", e))
        })?;
        let functions = {
            let pipelines = self
                .pipelines
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut functions = Vec::with_capacity(dispatches.len());
            for dispatch in &dispatches {
                let function = pipelines
                    .map
                    .get(&dispatch.pipeline)
                    .map(|entry| entry._function.clone())
                    .ok_or_else(|| {
                        ContractError::InvalidArgument("submit: unknown pipeline".into())
                    })?;
                let lens: Vec<usize> = dispatch
                    .args
                    .iter()
                    .map(|arg| match arg {
                        RecordedArg::Buffer { .. } => {
                            std::mem::size_of::<cudarc::driver::sys::CUdeviceptr>()
                        }
                        RecordedArg::Constant { bytes, .. } => bytes.len(),
                    })
                    .collect();
                check_kernel_params(&lens, |index| kernel_param_size(&function, index))?;
                functions.push(function);
            }
            functions
        };
        let mut launches = Vec::with_capacity(dispatches.len());
        for (dispatch, function) in dispatches.into_iter().zip(functions) {
            let mut args = Vec::with_capacity(dispatch.args.len());
            for arg in &dispatch.args {
                match arg {
                    RecordedArg::Buffer {
                        slot,
                        buffer,
                        offset,
                    } => {
                        let entry = map.get(&buffer.buffer_id).ok_or_else(|| {
                            ContractError::InvalidArgument(format!(
                                "submit: bound buffer of slot {slot} is not resolvable"
                            ))
                        })?;
                        let base = guard_memory_panic("device_ptr", || {
                            let (base, _record) = entry.slice.device_ptr(&self.stream);
                            Ok(base)
                        })?;
                        let pointer = base.checked_add(*offset).ok_or_else(|| {
                            ContractError::InvalidArgument(format!(
                                "submit: pointer of slot {slot} plus offset overflows"
                            ))
                        })?;
                        args.push(LaunchArg::Pointer(pointer));
                    }
                    RecordedArg::Constant { bytes, .. } => args.push(LaunchArg::Bytes(bytes)),
                }
            }
            launches.push(PreparedLaunch {
                function,
                args,
                grid: dispatch.grid,
                block: dispatch.block,
                shared_mem_bytes: dispatch.shared_mem_bytes,
            });
        }
        self.stream
            .context()
            .check_err()
            .map_err(|e| backend_failure("device_ptr", e))?;
        Ok(launches.into_iter())
    }

    /// 組み立てた起動 1 件を stream に積む。host は待たない。driver の失敗は `BackendFailure`（`op` は `cuLaunchKernel`）。
    fn launch_kernel(&self, launch: &mut PreparedLaunch<'_>) -> Result<(), ContractError> {
        let mut params: Vec<*mut c_void> = launch
            .args
            .iter_mut()
            .map(|arg| match arg {
                LaunchArg::Pointer(pointer) => std::ptr::from_mut(pointer).cast::<c_void>(),
                LaunchArg::Bytes(bytes) => bytes.as_ptr().cast_mut().cast::<c_void>(),
            })
            .collect();
        let [gx, gy, gz] = launch.grid;
        let [bx, by, bz] = launch.block;
        // SAFETY: `function` は `PreparedLaunch` が持つ `CudaFunction` の clone で、そのモジュールの `Arc` を持つ。
        // パイプラインの表もモジュールの `Arc` を持ち、表から外すのは device の `Drop` が stream の同期を確かめた後だけ
        // （確かめられなければ解放しない）なので、積んだカーネルが走り終えるまで読み込みは外されない。context は `prepare_launches` でこのスレッドに結び、stream は `self` が持つ。
        // `params` の要素数は `args` と同じで、`prepare_launches` の `check_kernel_params` が、関数の仮引数の数と各仮引数の
        // 大きさが `args` の数と各要素のバイト数（ポインタは 8 バイト、定数は件の bytes の長さ）にちょうど等しいことを確かめた
        // （driver が host 側で読む範囲は各要素が指す値の中に収まる）。`args`・件の bytes・`params` はこの呼び出しが戻るまで生き、
        // driver は呼び出しの中で値を写して書かない（定数の要素は共有の借用から作ったポインタ）。束縛のポインタは同じ
        // `submit` の錠の中で解放済みでないと確かめたバッファの先頭に `offset` を足した値で、そのバッファは `issue_event` が
        // 最後に使った投入をこの投入にするので、この投入が終わるまで `free` は解放を保留する。後の件を積むのに失敗して
        // この投入が番号を持たずに終わるときは保留しないが、`free` の後のゼロ化と解放（`free_async` か stream の同期の後の解放）も
        // 同じ stream に積むか stream を待つので、積んだカーネルより先には走らない。この device のバッファに触る命令は
        // device を作るときに作った stream 1 本だけに積むので、前後のコピー・クリア・読み戻しとの順は stream の順で決まる。カーネルが範囲の外に触れるかは
        // `offset` を照らさないので保証しない。その経路は `DISPATCH_EXECUTION_ENABLED` の門が単体の試験のビルドの外では閉じる。
        // ポインタの解決で cudarc が記録した event は起動より前の点を指すが、命令は stream 1 本だけに積むので順は崩れない。
        unsafe {
            cudarc::driver::result::launch_kernel(
                launch.function.cu_function(),
                (gx, gy, gz),
                (bx, by, bz),
                launch.shared_mem_bytes,
                self.stream.cu_stream(),
                &mut params,
            )
        }
        .map_err(|e| backend_failure("cuLaunchKernel", e))
    }

    /// ハンドル 1 本について、記録の時点の照らしに渡す事実を表から引く。driver を呼ばない。
    fn transfer_operand(&self, buffers: &Buffers, handle: &BufferHandle) -> TransferOperand {
        let generation = handle.__backend_generation();
        let entry = buffers
            .map
            .get(&handle.__backend_id())
            .map(|entry| TransferEntry {
                buffer_id: handle.__backend_id(),
                generation,
                issued_matches: entry.generation == generation,
                size_matches: handle.size() == entry.slice.len() as u64,
                usage: entry.usage,
                allocation_len: entry.slice.len() as u64,
            });
        TransferOperand {
            same_device: handle.__backend_device_id() == self.id,
            generation_current: generation == self.current_generation(),
            entry,
        }
    }

    /// 照らしを通った記録を、開いている記録先の列の末尾に足して記録の件数を 1 つ増やす。`Ok(None)` は何もしない。
    fn record_transfer(
        &self,
        encoder: &mut CommandEncoder,
        checked: Result<Option<TransferRecord>, ContractError>,
    ) -> Result<(), ContractError> {
        if let Some(record) = checked? {
            self.open.record(encoder.__backend_id(), record)?;
            encoder.__backend_record();
        }
        Ok(())
    }

    /// 記録先がこの device の開いている集合に無ければ `InvalidArgument`。
    fn ensure_encoder_open(&self, encoder: &CommandEncoder) -> Result<(), ContractError> {
        if self.open.is_open(encoder.__backend_id()) {
            Ok(())
        } else {
            Err(ContractError::InvalidArgument(
                "encoder is not open on this device".into(),
            ))
        }
    }

    /// 記録の列を、表の（世代・長さ）と今の世代に照らして stream に積む件の列へ組み立てる。呼ぶ側がバッファの表の錠を持つ。
    fn plan(
        &self,
        map: &BTreeMap<u64, BufferEntry>,
        records: &[TransferRecord],
    ) -> Result<Vec<TransferJob>, ContractError> {
        let jobs = plan_transfers(
            records,
            |id| {
                map.get(&id)
                    .map(|entry| (entry.generation, entry.slice.len() as u64))
            },
            self.current_generation(),
        )?;
        check_dispatch_gate(&jobs, DISPATCH_EXECUTION_ENABLED)?;
        Ok(jobs)
    }

    /// 解放を保留したバッファのうち、最後に使った投入が投入済みの表から外れたものを再利用の一覧へ回す。
    ///
    /// 保留が無ければ錠を取らずに戻る。錠は「バッファの表 → 投入済みの表」の順に取り、解放と一覧への回しは錠を放してから行う。
    fn release_finished(&self) {
        if self.held.load(Ordering::Acquire) == 0 {
            return;
        }
        let finished = catch_unwind(AssertUnwindSafe(|| {
            let mut buffers = self.lock();
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            let finished = buffers
                .pending
                .take_finished(|sequence| subs.is_unfinished(sequence));
            self.held.store(buffers.pending.len(), Ordering::Release);
            finished
        }))
        .unwrap_or_default();
        for entry in finished {
            self.retire_entry(entry);
        }
    }

    /// 表から外したバッファを、ゼロ化を積んで再利用の一覧へ回す。一覧に入れられなければ解放する。錠を持たずに呼ぶ。
    ///
    /// ゼロ化は stream に積むだけで host は待たない。ドライバへの解放は錠の外で呼び、panic も外へ出さない。
    fn retire_entry(&self, entry: BufferEntry) {
        // 鍵は表の項目から決める。
        let BufferEntry {
            slice, slot, usage, ..
        } = entry;
        let byte_len = slice.len() as u64;
        let key = (byte_len, usage.bits());
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

    /// 門を待つ host の関数を stream に積む。門の参照を 1 つ `Arc::into_raw` で渡し、取り戻さない。
    #[cfg(feature = "fault-injection")]
    fn launch_gate(&self, latch: &Arc<GateLatch>) -> Result<(), ContractError> {
        self.stream
            .context()
            .bind_to_thread()
            .map_err(|e| backend_failure("launch_host_function", e))?;
        let arg = Arc::into_raw(Arc::clone(latch)).cast_mut().cast::<c_void>();
        // SAFETY: stream は `self` が `Arc<CudaStream>` で持ち、ここでは破棄しない。context は直前でこのスレッドに結んだ。
        // `gate_host_fn` の署名は `launch_host_function` が受ける関数の型と同じで、CUDA の API を呼ばない。
        // `arg` は `Arc::into_raw` で作り、どこでも取り戻さないので、`gate_host_fn` が呼ばれる回数と、
        // 積めなかったときの driver の動きに依らず、指す先は有効なまま（`gate_host_fn` の `# Safety` を満たす）。
        unsafe {
            cudarc::driver::result::stream::launch_host_function(
                self.stream.cu_stream(),
                gate_host_fn,
                arg,
            )
        }
        .map_err(|e| backend_failure("launch_host_function", e))
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
        let (judged, ready) = {
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            let judged = self.current_generation();
            (judged, subs.take_ready(judged, event_state))
        };
        // 呼ぶ直前に世代を読み直す。
        let ready = ready
            .into_iter()
            .map(|(callback, state)| {
                let deliver = move |decided| {
                    callback(delivered_state(decided, judged, self.current_generation()));
                };
                (deliver, state)
            })
            .collect();
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

    /// ハンドルの所属・表に記録した用途・範囲を順に検査して `[start, end)` を返す。`cudarc` の境界の panic に届く前に拒む。
    ///
    /// 用途は表の項目の記録で判定し、ハンドルの申告は使わない。
    fn range(
        &self,
        op: &'static str,
        role: UsageRole,
        table: &BTreeMap<u64, BufferEntry>,
        handle: &BufferHandle,
        offset: u64,
        len: u64,
    ) -> Result<(usize, usize), ContractError> {
        if handle.__backend_device_id() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        if handle.__backend_generation() != self.current_generation() {
            return Err(ContractError::InvalidArgument(
                "buffer handle generation does not match the device".into(),
            ));
        }
        let entry = table
            .get(&handle.__backend_id())
            .ok_or_else(|| ContractError::InvalidArgument("unknown buffer handle".into()))?;
        access_range(op, role, entry.usage, entry.slice.len() as u64, offset, len)
    }

    /// バッファの範囲を host へ読む。同期まで同じ関数の中で終える。
    fn read(
        &self,
        op: &'static str,
        src: &BufferHandle,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        self.check_not_lost()?;
        let buffers = self.lock();
        let (start, end) = self.range(
            op,
            UsageRole::ReadbackSource,
            &buffers.map,
            src,
            offset,
            len,
        )?;
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
        self.current_generation()
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
    /// `create_pipeline` を持つ `ComputeOps` の実装（この型自身）を返す（TASK-166 のポインタ）。
    fn compute(&self) -> Option<&dyn ComputeOps> {
        Some(self)
    }
    /// 実行時の判定なしに、この device の相互運用の操作を返す。
    #[cfg(feature = "ext-interop")]
    fn external_interop(&self) -> Option<&dyn ExternalInterop> {
        Some(self)
    }
    /// device の作成で 1 回読んだ識別子を返す。`driver_uuid` は `None`。
    #[cfg(feature = "ext-interop")]
    fn physical_gpu_id(&self) -> Option<PhysicalGpuId> {
        Some(self.physical_gpu_id)
    }
    /// 実行時の判定なしに、この device の故障の差し込みの口を返す。
    #[cfg(feature = "fault-injection")]
    fn fault_hooks(&self) -> Option<&dyn FaultHooks> {
        Some(self)
    }
    /// 登録済みの完了の callback を待たずに `Lost` で呼び切り、stream を同期し、`fault-injection` のビルドでは
    /// ロストの通知を `Destroyed` で 1 回呼んでから解放する。最初の panic は解放の後に再送出する。
    /// `fault-injection` のビルドでは、同期の前に門を全部開ける。
    fn destroy(self: Box<Self>) {
        let flush = |d: &Self| {
            let callbacks = d.lock_submissions().take_all_callbacks();
            run_callbacks(
                callbacks
                    .into_iter()
                    .map(|callback| (callback, CompletionState::Lost))
                    .collect(),
            )
        };
        let sync = |d: &Self| {
            #[cfg(feature = "fault-injection")]
            d.gate.open_all();
            let _ = sync_confirmed(|| d.stream.synchronize());
        };
        #[cfg(feature = "fault-injection")]
        let notify = |d: &Self| {
            d.lost
                .lose_catching(DeviceLostReason::Destroyed, &d.generation, &NEXT_GENERATION)
                .ok()
                .and_then(|(_, panic)| panic)
        };
        #[cfg(not(feature = "fault-injection"))]
        let notify = |_: &Self| None;
        if let Some(payload) = destroy_in_order(*self, flush, sync, notify, drop) {
            std::panic::resume_unwind(payload);
        }
    }
}

/// 相互運用の操作。必須の 4 操作は `Unsupported { op, backend: Cuda }`（`op` は呼んだ操作の名前）を返す。
#[cfg(feature = "ext-interop")]
impl ExternalInterop for CudaDevice {
    fn export_memory(&self, _buffer: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> {
        Err(ContractError::Unsupported {
            op: "export_memory",
            backend: BACKEND,
        })
    }
    fn import_memory(
        &self,
        _external: ExternalMemoryHandle,
        _byte_len: u64,
        _usage: BufferUsage,
    ) -> Result<BufferHandle, ContractError> {
        Err(ContractError::Unsupported {
            op: "import_memory",
            backend: BACKEND,
        })
    }
    fn export_semaphore(
        &self,
        _token: SubmitToken,
    ) -> Result<ExternalSemaphoreHandle, ContractError> {
        Err(ContractError::Unsupported {
            op: "export_semaphore",
            backend: BACKEND,
        })
    }
    fn import_semaphore(
        &self,
        _external: ExternalSemaphoreHandle,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        Err(ContractError::Unsupported {
            op: "import_semaphore",
            backend: BACKEND,
        })
    }
}

/// 模擬のロストと試験用の門を持つ故障の差し込みの口。実際の故障ではない。
///
/// `simulate_runtime_error`・`simulate_validation_report` は既定の本体のまま
/// （`UnsupportedExtensionDefault`）。ロストの後の操作の拒否はここでは持たない。
#[cfg(feature = "fault-injection")]
impl FaultHooks for CudaDevice {
    /// 1 回目は理由を記録し、世代を取り直し、登録されたコールバックを錠の外で 1 回呼び、その後に登録の世代が古い
    /// 完了の callback を表から外して `Lost` で 1 回ずつ呼ぶ。2 回目以降は何もせず `Ok`。
    /// 世代の源が尽きていたら、状態を変えずに `BackendFailure` を返す。
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        let (first, lost_panic) = self
            .lost
            .lose_catching(reason, &self.generation, &NEXT_GENERATION)
            .map_err(|_| ContractError::BackendFailure {
                op: "simulate_device_lost",
                backend: BACKEND,
                detail: "device generation space is exhausted".into(),
            })?;
        if first {
            // 投入済みの表の錠はこの文の終わりで放してから、利用者の callback を呼ぶ。
            // ロスト通知が panic していても、古い世代の完了の callback は先に `Lost` で呼び切る。
            let stale = self
                .lock_submissions()
                .take_stale(self.current_generation());
            let stale_panic = run_callbacks(stale);
            if let Some(payload) = lost_panic {
                if let Some(extra) = stale_panic {
                    discard_panic(extra);
                }
                std::panic::resume_unwind(payload);
            }
            if let Some(payload) = stale_panic {
                std::panic::resume_unwind(payload);
            }
        }
        Ok(())
    }

    /// 次に成功する投入の event の前に積む門を仕込む。仕込み済みなら何もしない。常に `Ok`。
    fn arm_gate(&self) -> Result<(), ContractError> {
        self.gate.arm();
        Ok(())
    }

    /// 仕込んだ門と積んだ門を全部開ける。何度呼んでもよい。常に `Ok`。
    fn open_gate(&self) -> Result<(), ContractError> {
        self.gate.open_all();
        Ok(())
    }
}

/// ロストの通知先の登録。登録済みなら上書きし、ロストの後の登録は保持するだけで呼ばない。
#[cfg(feature = "fault-injection")]
impl DeviceLostNotify for CudaDevice {
    fn set_lost_callback(&self, callback: LostCallback) {
        self.lost.set_callback(callback);
    }
}

/// 検証の報告の受け口の登録を受け付けて保持し、登録済みなら置き換える。
/// この背後実装は報告を出さないので、登録した関数は呼ばれない（TASK-607）。
impl ValidationIntake for CudaDevice {
    fn set_validation_callback(&self, callback: ValidationCallback) {
        self.validation.set_callback(callback);
    }
}

impl MemoryOps for CudaDevice {
    /// バッファの範囲を別のバッファへコピーする命令を、記録の時点に照らしてから記録先に積む。
    ///
    /// 開いていない記録先・別の device・世代・表・大きさ・用途・範囲・整列・同じバッファは `InvalidArgument`
    /// （別の device は `DeviceMismatch`）で、記録先を変えない。長さ 0 は何も記録せず `Ok(())`。stream へ積むのは投入の時（TASK-624）。
    fn copy_buffer_to_buffer(
        &self,
        encoder: &mut CommandEncoder,
        src: &BufferHandle,
        src_offset: u64,
        dst: &BufferHandle,
        dst_offset: u64,
        size: u64,
    ) -> Result<(), ContractError> {
        self.check_not_lost()?;
        self.ensure_encoder_open(encoder)?;
        let (src_operand, dst_operand) = {
            let buffers = self.lock();
            (
                self.transfer_operand(&buffers, src),
                self.transfer_operand(&buffers, dst),
            )
        };
        self.record_transfer(
            encoder,
            check_copy_record(src_operand, src_offset, dst_operand, dst_offset, size),
        )
    }

    /// バッファの範囲を 0 で埋める命令を、記録の時点に照らしてから記録先に積む。
    ///
    /// 照らしと拒否の扱いは `copy_buffer_to_buffer` と同じで、用途は `UPLOAD` を要る。stream へ積むのは投入の時（TASK-624）。
    fn clear_buffer(
        &self,
        encoder: &mut CommandEncoder,
        buffer: &BufferHandle,
        offset: u64,
        size: u64,
    ) -> Result<(), ContractError> {
        self.check_not_lost()?;
        self.ensure_encoder_open(encoder)?;
        let operand = {
            let buffers = self.lock();
            self.transfer_operand(&buffers, buffer)
        };
        self.record_transfer(encoder, check_clear_record(operand, offset, size))
    }

    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        // 世代はロストの照らしの前に読み、照らしが通ればハンドルと表の項目にその値を使う。
        let generation = self.current_generation();
        self.check_not_lost()?;
        if byte_len == 0 {
            return Err(ContractError::InvalidArgument(
                "allocation length must not be zero".into(),
            ));
        }
        validate_allocation_size(byte_len, self.limits.max_buffer_bytes)?;
        let len = usize::try_from(byte_len)
            .map_err(|_| ContractError::InvalidArgument("length exceeds usize".into()))?;
        self.release_finished();
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
        self.lock().map.insert(
            id,
            BufferEntry {
                slice,
                slot,
                usage,
                generation,
                last_submission: None,
            },
        );
        Ok(BufferHandle::__backend_new(
            generation, id, self.id, byte_len, usage,
        ))
    }

    fn upload(
        &self,
        dst: &BufferHandle,
        dst_offset: u64,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        self.check_not_lost()?;
        let mut buffers = self.lock();
        let (start, end) = self.range(
            "upload",
            UsageRole::UploadDestination,
            &buffers.map,
            dst,
            dst_offset,
            bytes.len() as u64,
        )?;
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
        self.read("download_blocking", src, src_offset, byte_len)
    }

    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        // ロックは `read` の中で離れているので、コールバックはロックの外で呼ばれる。
        callback(self.read("download_async", src, src_offset, byte_len));
    }

    /// 表から外したバッファを、再利用の一覧へ回すか、解放を保留する。
    ///
    /// コピー・クリアで最後に使った投入が投入済みの表に残っていれば、stream に何も積まずに保留して戻る。
    /// 保留したバッファは `submit`・`submit_many`・`poll`・`allocate`・`wait_blocking`（feature `native-blocking-wait` の下）の中で、
    /// 投入済みの表から外れた後に一覧へ回る（TASK-624・TASK-606）。
    fn free(&self, buffer: BufferHandle) {
        if buffer.__backend_device_id() != self.id {
            return;
        }
        let release = {
            let mut buffers = self.lock();
            let removed = remove_issued(
                &mut buffers.map,
                buffer.__backend_id(),
                buffer.__backend_generation(),
                |entry| entry.generation,
            );
            let Some(entry) = removed else {
                return;
            };
            match entry.last_submission {
                // コピー・クリアで使っていないバッファは、投入済みの表に触らない。
                None => Some(entry),
                Some(sequence) => {
                    let mut subs = self.lock_submissions();
                    subs.reclaim(event_state);
                    let released = hold_or_release(
                        entry,
                        Some(sequence),
                        |sequence| subs.is_unfinished(sequence),
                        &mut buffers.pending,
                    );
                    self.held.store(buffers.pending.len(), Ordering::Release);
                    released
                }
            }
        };
        if let Some(entry) = release {
            self.retire_entry(entry);
        }
    }

    /// 解放を保留しているバッファの数を返す。
    ///
    /// 数えるのは `free` が保留したバッファだけで、再利用の一覧で 0 にし終えるのを待つバッファは数えない。
    /// 読むだけで回収はしない（TASK-345）。
    fn pending_release_count(&self) -> Option<u64> {
        Some(u64::try_from(self.held.load(Ordering::Acquire)).unwrap_or(u64::MAX))
    }
}

impl ComputeOps for CudaDevice {
    /// 入力の分類、`slots` の重複、NUL のバイトの順に照らし、最初に当たったものを返す。
    /// 通った `CudaC` は NVRTC で PTX にし、モジュールの読み込みと関数の取得を経て、
    /// 関数と配置の写しを device のパイプラインの表に登録してハンドルを返す。
    /// NVRTC のライブラリが無ければ `Unsupported`。
    /// 通った `Ptx` はテキストをそのまま読み込み、NVRTC の呼び出し口を通らない。
    /// feature `fault-injection` のビルドで模擬のロストの後は、入力を照らす前に、呼んだ時点の世代を載せた `DeviceLost` を返す。
    fn create_pipeline(
        &self,
        input: PipelineInput<'_>,
        layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        self.check_not_lost()?;
        let accepted = classify_pipeline_input(&input)?;
        check_unique_slots(&layout.slots)?;
        let (ptx, entry_point) = module_input(accepted, |source| self.compile_with_nvrtc(source))?;
        let ctx = self.stream.context();
        let (module, function) = guard_pipeline_panic(|| {
            #[cfg(test)]
            self.pipeline_driver_calls.fetch_add(1, Ordering::SeqCst);
            ctx.bind_to_thread()
                .map_err(|e| backend_failure("bind_to_thread", e))?;
            #[cfg(test)]
            self.pipeline_driver_calls.fetch_add(1, Ordering::SeqCst);
            let module = ctx.load_module(ptx).map_err(|e| {
                artifact_failure(
                    "module",
                    "cuModuleLoadData",
                    driver_error_is_artifact_cause(&e),
                    e,
                )
            })?;
            #[cfg(test)]
            self.pipeline_driver_calls.fetch_add(1, Ordering::SeqCst);
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
    /// 開いている集合に無ければ `InvalidArgument` で、どちらも何も投入しない。記録したコピー・クリアのバッファが
    /// 解放済みなら `InvalidArgument` で、何も積まず番号を消費しない。通れば記録の順に stream へ積んでから event を 1 つ記録する。
    fn submit(
        &self,
        mut commands: CommandEncoder,
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        let result = {
            let mut buffers = self.lock();
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            let generation = self.current_generation();
            self.check_not_lost()
                .and_then(|()| reject_wait("submit(wait)", wait))
                .and_then(|()| self.open.claim(&[commands.__backend_id()]))
                .and_then(|records| self.plan(&buffers.map, &records))
                .and_then(|jobs| {
                    commands.__backend_disarm_closer();
                    self.issue_event(&mut subs, generation, &mut buffers.map, &jobs)
                })
        };
        self.release_finished();
        self.drive_callbacks();
        result
    }
    /// 複数の記録先を 1 回で投入し、印を 1 つ返す。
    ///
    /// 最初に投入済みの表を回収する。検査の順は、`after` の各印（別 device は `DeviceMismatch`、
    /// 世代違いは `DeviceLost`）、空の列（`InvalidArgument`）、`wait`（`Unsupported`）、全記録先の所属
    /// （`InvalidArgument`）、記録したコピー・クリアのバッファの解放済みの確かめ（`InvalidArgument`）。検査に当たると何も投入しない。
    /// 1 つの stream に順に積むので、`after` は照合だけで追加の待ちは入れない。通れば、渡した記録先の順に各記録先の記録を積んでから
    /// event を 1 つ記録し、錠を放してから終端になった登録済みの callback を呼ぶ（`submit` と同じ）。
    fn submit_many(
        &self,
        mut commands: Vec<CommandEncoder>,
        after: &[SubmitToken],
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        let result = {
            let mut buffers = self.lock();
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            let generation = self.current_generation();
            self.check_not_lost()
                .and_then(|()| {
                    precheck_submit_many(commands.len(), after, wait, self.id, generation)
                })
                .and_then(|()| {
                    let ids: Vec<u64> = commands.iter().map(CommandEncoder::__backend_id).collect();
                    self.open.claim(&ids)
                })
                .and_then(|records| self.plan(&buffers.map, &records))
                .and_then(|jobs| {
                    for command in &mut commands {
                        command.__backend_disarm_closer();
                    }
                    self.issue_event(&mut subs, generation, &mut buffers.map, &jobs)
                })
        };
        self.release_finished();
        self.drive_callbacks();
        result
    }
    /// 完了の問合せ。待たない。
    ///
    /// 別 device・世代違いの印、払い出していない番号、失敗した列は `Lost`。
    /// 回収のあとで、終端になった登録済みの callback を錠の外で呼ぶ。
    fn poll(&self, token: SubmitToken) -> CompletionState {
        self.release_finished();
        if self.is_lost() {
            return CompletionState::Lost;
        }
        if check_token_owner(&token, self.id, self.current_generation()).is_err() {
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
    /// 終端ならこの呼び出しの中で呼ぶ。未完了なら登録して戻り、`submit`・`submit_many`・`poll`・`poll_ex`・`on_complete`・
    /// `wait_blocking`（feature `native-blocking-wait` の下）のどれかの中で終端と分かった時に、その呼び出しを行ったスレッドで呼ぶ。
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        if check_token_owner(&token, self.id, self.current_generation()).is_err() {
            callback(CompletionState::Lost);
            return;
        }
        let terminal = {
            let mut subs = self.lock_submissions();
            // 世代は表の錠の中で読み直す。
            if token.generation != self.current_generation() {
                Some((callback, CompletionState::Lost))
            } else {
                match subs.reclaim_and_state_of(token.sequence, event_state) {
                    CompletionState::Pending => {
                        subs.callbacks
                            .push((token.sequence, token.generation, callback));
                        None
                    }
                    CompletionState::Complete => Some((callback, CompletionState::Complete)),
                    CompletionState::Lost => Some((callback, CompletionState::Lost)),
                }
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
    /// 投入の完了まで呼び出し元のスレッドで待つ。待つ間は錠を持たない。
    ///
    /// 最初に `poll` と同じく解放を保留したバッファを回す。照らす順は、ロスト済み（`DeviceLost`）、別 device の印（`DeviceMismatch`）、世代違いの印（`DeviceLost`）、
    /// 投入の記録の無い番号（0 を含む。`InvalidArgument`。`timeout` によらず待たない）、
    /// 失敗した投入（`BackendFailure`）、回収済み（`Ok(())`）、待つ、の順。`None` は event を同期し、`Some` は期限まで
    /// 完了を確かめ直して届かなければ `Timeout`。失敗した投入・回収済み・待った後は、戻る前に解放を保留した
    /// バッファを回し、終端になった登録済みの callback を錠の外で呼ぶ。
    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        token: SubmitToken,
        timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        self.release_finished();
        check_wait_token(self.is_lost(), &token, self.id, self.current_generation())?;
        let (step, event) = {
            let mut subs = self.lock_submissions();
            subs.reclaim(event_state);
            let step = subs.wait_step(token.sequence);
            let event = match step {
                WaitStep::Wait => subs.pending.get(&token.sequence).cloned(),
                _ => None,
            };
            (step, event)
        };
        let result = match step {
            WaitStep::Unissued { last } => {
                return Err(unissued_wait_error(token.sequence, last));
            }
            WaitStep::Failed(error) => Err(error),
            WaitStep::Done => Ok(()),
            WaitStep::Wait => self.wait_issued(token.sequence, event, timeout),
        };
        let result = self.check_not_lost().and(result);
        self.release_finished();
        self.drive_callbacks();
        result
    }
    /// dispatch 1 件を、記録の時点に照らしてから記録先に積む。stream へ積むのは投入の時。
    ///
    /// 照らす順は、記録先（開いていなければ `InvalidArgument`）、パイプライン（別の device は `DeviceMismatch`、
    /// 表に無ければ `InvalidArgument`）、束縛の各バッファ（`copy_buffer_to_buffer` と同じ device・世代・表・大きさ）、
    /// `dispatch_dims`（`check_dispatch_dims`。0 と `limits().max_dispatch_dims` の超過）、
    /// slot・定数・共有メモリ（`check_dispatch_record`）。拒否は記録先を変えない。定数の bytes は記録の時点に写す。
    fn dispatch_ex(
        &self,
        encoder: &mut CommandEncoder,
        pipeline: &PipelineHandle,
        bindings: &[Binding<'_>],
        consts: &[ConstantBinding<'_>],
        dispatch_dims: [u32; 3],
        threads_per_threadgroup: [u32; 3],
        threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        self.check_not_lost()?;
        self.ensure_encoder_open(encoder)?;
        if pipeline.__backend_device_id() != self.id {
            return Err(ContractError::DeviceMismatch);
        }
        let layout_slots = {
            let pipelines = self
                .pipelines
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pipelines
                .map
                .get(&pipeline.__backend_id())
                .map(|entry| entry._layout.slots.clone())
                .ok_or_else(|| {
                    ContractError::InvalidArgument("dispatch_ex: unknown pipeline".into())
                })?
        };
        let buffer_args = {
            let buffers = self.lock();
            bindings
                .iter()
                .map(|binding| {
                    check_transfer_operand(self.transfer_operand(&buffers, binding.buffer))
                        .map(|entry| DispatchBufferArg {
                            slot: binding.slot,
                            buffer: RecordedBuffer {
                                buffer_id: entry.buffer_id,
                                generation: entry.generation,
                            },
                            offset: binding.offset,
                        })
                        .map_err(|e| match e {
                            ContractError::InvalidArgument(msg) => {
                                ContractError::InvalidArgument(format!("dispatch_ex: {msg}"))
                            }
                            other => other,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        check_dispatch_dims(dispatch_dims, self.limits.max_dispatch_dims)?;
        let const_args: Vec<(u32, &[u8])> = consts.iter().map(|c| (c.slot, c.bytes)).collect();
        let memory: Vec<u32> = threadgroup_memory.iter().map(|m| m.bytes).collect();
        let record = check_dispatch_record(
            pipeline.__backend_id(),
            &layout_slots,
            &buffer_args,
            &const_args,
            dispatch_dims,
            threads_per_threadgroup,
            &memory,
        )?;
        self.record_transfer(encoder, Ok(Some(TransferRecord::Dispatch(record))))
    }
    /// `limits()` の `max_buffer_bytes` と同じ値を返す。
    fn max_buffer_bytes(&self) -> u64 {
        self.limits.max_buffer_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_range_checks_the_usage_before_the_range() {
        // 用途が足りず範囲も外れる入力は、用途の拒否になる。
        let err = access_range(
            "upload",
            UsageRole::UploadDestination,
            BufferUsage::DOWNLOAD,
            16,
            17,
            1,
        )
        .unwrap_err();
        match err {
            ContractError::InvalidArgument(msg) => {
                assert!(msg.starts_with("upload: "), "{msg}");
                assert!(msg.contains("UPLOAD"), "{msg}");
            }
            other => panic!("unexpected: {other:?}"),
        }
        // 空の用途は、長さ 0 の読み戻しでも拒む。
        assert!(matches!(
            access_range(
                "download_blocking",
                UsageRole::ReadbackSource,
                BufferUsage::empty(),
                16,
                0,
                0
            ),
            Err(ContractError::InvalidArgument(_))
        ));
        // 過申告の用途で範囲内なら通る。
        let all = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
        assert!(matches!(
            access_range("upload", UsageRole::UploadDestination, all, 16, 4, 8),
            Ok((4, 12))
        ));
        // 用途が足りていて u64 があふれる入力は拒む。
        assert!(matches!(
            access_range(
                "download_blocking",
                UsageRole::ReadbackSource,
                BufferUsage::DOWNLOAD,
                16,
                u64::MAX,
                1
            ),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    /// 共通の型の用途の 13 値。1 ビットずつ試すための一覧。
    const ALL_USAGES: [BufferUsage; 13] = [
        BufferUsage::STORAGE,
        BufferUsage::UPLOAD,
        BufferUsage::DOWNLOAD,
        BufferUsage::VERTEX,
        BufferUsage::INDEX,
        BufferUsage::UNIFORM,
        BufferUsage::EXPORTABLE,
        BufferUsage::MAP_READ,
        BufferUsage::MAP_WRITE,
        BufferUsage::INDIRECT,
        BufferUsage::QUERY_RESOLVE,
        BufferUsage::BLAS_INPUT,
        BufferUsage::TLAS_INPUT,
    ];

    /// `access_range` を呼ぶ操作の組。操作の名前・役割・要る用途・文字列に含まれる用途の名前。
    const ACCESS_OPS: [(&str, UsageRole, BufferUsage, &str); 2] = [
        (
            "upload",
            UsageRole::UploadDestination,
            BufferUsage::UPLOAD,
            "UPLOAD",
        ),
        (
            "download_blocking",
            UsageRole::ReadbackSource,
            BufferUsage::DOWNLOAD,
            "DOWNLOAD",
        ),
    ];

    fn all_usages_union() -> BufferUsage {
        ALL_USAGES
            .iter()
            .fold(BufferUsage::empty(), |acc, &u| acc | u)
    }

    /// 用途の拒否であること（操作の名前で始まり、要る用途の名前を含む）を確かめる。
    fn assert_usage_rejection(
        result: Result<(usize, usize), ContractError>,
        op: &str,
        usage_name: &str,
    ) {
        match result {
            Err(ContractError::InvalidArgument(msg)) => {
                assert!(msg.starts_with(&format!("{op}: ")), "{msg}");
                assert!(msg.contains(usage_name), "{msg}");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    /// 要る 1 ビットだけで通り、それ以外の 12 値の 1 ビットだけと空は用途の拒否になる。
    #[test]
    fn access_range_accepts_the_required_bit_and_rejects_every_other_single_bit() {
        let all = all_usages_union();
        assert_eq!(all.bits().count_ones(), 13);
        for (op, role, required, name) in ACCESS_OPS {
            assert!(matches!(
                access_range(op, role, required, 16, 0, 16),
                Ok((0, 16))
            ));
            assert!(matches!(
                access_range(op, role, all, 16, 0, 16),
                Ok((0, 16))
            ));
            assert_usage_rejection(
                access_range(op, role, BufferUsage::empty(), 16, 0, 16),
                op,
                name,
            );
            let mut rejected = 0;
            for &single in ALL_USAGES.iter().filter(|&&u| u != required) {
                assert_usage_rejection(access_range(op, role, single, 16, 0, 16), op, name);
                rejected += 1;
            }
            assert_eq!(rejected, 12);
        }
    }

    /// 用途が足りるとき、範囲は終端の境界と `u64` のあふれで決まる。
    #[test]
    fn access_range_range_boundaries_with_a_sufficient_usage() {
        let accepted: [(u64, u64, (usize, usize)); 3] =
            [(8, 8, (8, 16)), (0, 16, (0, 16)), (16, 0, (16, 16))];
        let rejected: [(u64, u64); 5] = [
            (8, 9),
            (17, 0),
            (u64::MAX, 1),
            (1, u64::MAX),
            (u64::MAX - 7, 16),
        ];
        for (op, role, required, _) in ACCESS_OPS {
            for (offset, len, expected) in accepted {
                assert_eq!(
                    access_range(op, role, required, 16, offset, len).unwrap(),
                    expected,
                    "{op} {offset} {len}"
                );
            }
            for (offset, len) in rejected {
                assert!(
                    matches!(
                        access_range(op, role, required, 16, offset, len),
                        Err(ContractError::InvalidArgument(_))
                    ),
                    "{op} {offset} {len}"
                );
            }
        }
    }

    /// 用途が足りないときは範囲が外れていても用途の拒否が返る。
    #[test]
    fn access_range_reports_the_usage_rejection_before_the_range() {
        for (op, role, required, name) in ACCESS_OPS {
            for (offset, len) in [(17u64, 1u64), (u64::MAX, 1)] {
                assert_usage_rejection(
                    access_range(op, role, BufferUsage::STORAGE, 16, offset, len),
                    op,
                    name,
                );
                assert!(matches!(
                    access_range(op, role, required, 16, offset, len),
                    Err(ContractError::InvalidArgument(_))
                ));
            }
        }
    }

    /// 同じ引数でも、判定は表に記録した用途だけで替わる。
    #[test]
    fn access_range_decides_by_the_recorded_usage_only() {
        let up = UsageRole::UploadDestination;
        let down = UsageRole::ReadbackSource;
        assert_usage_rejection(
            access_range("upload", up, BufferUsage::STORAGE, 16, 4, 8),
            "upload",
            "UPLOAD",
        );
        assert!(matches!(
            access_range(
                "upload",
                up,
                BufferUsage::STORAGE | BufferUsage::UPLOAD,
                16,
                4,
                8
            ),
            Ok((4, 12))
        ));
        assert_usage_rejection(
            access_range("download_blocking", down, BufferUsage::UPLOAD, 16, 4, 8),
            "download_blocking",
            "DOWNLOAD",
        );
        assert!(matches!(
            access_range(
                "download_blocking",
                down,
                BufferUsage::UPLOAD | BufferUsage::DOWNLOAD,
                16,
                4,
                8
            ),
            Ok((4, 12))
        ));
    }

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

    /// 鍵の一覧の外の ID は、どの能力の値でも core の `UNLISTED_FEATURE_QUERY` と等しい値を返す。
    #[test]
    fn ids_outside_the_key_list_are_unlisted() {
        for id in [
            FeatureId("feature:shader_f16"),
            FeatureId("feature:SHADER_F16 "),
            FeatureId("feature:SUBGRUOP"),
            FeatureId("feature:not-a-feature"),
            FeatureId(""),
        ] {
            assert!(
                !fandhe_silicon_core::feature_key::FEATURE_QUERY_KEYS.contains(&id),
                "{id:?}"
            );
            for cc in [(5, 3), (7, 4), (7, 5), (9, 0)] {
                for map in [None, Some(false), Some(true)] {
                    assert_eq!(
                        cuda_capability_of(&snapshot(cc, map), id),
                        fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY,
                        "{id:?}"
                    );
                }
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
        t.callbacks.push((1, 1, Box::new(|_| {})));
        t.callbacks.push((2, 1, Box::new(|_| {})));
        let ready = t.take_ready(1, |g| *g);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].1, CompletionState::Complete);
        assert_eq!(t.callbacks.len(), 1);
    }

    #[test]
    fn a_stalled_head_stays_pending_even_if_the_group_reads_complete_afterwards() {
        let mut t = Submissions::<GroupState>::new();
        let _ = t.issue(|| Ok(GroupState::Completed));
        let _ = t.issue(|| Ok(GroupState::Completed));
        t.callbacks.push((2, 1, Box::new(|_| {})));
        // 回収の時点では 1 件目が未完了だったと見なす。
        let calls = std::cell::Cell::new(0);
        let first = t.reclaim(|_| {
            calls.set(calls.get() + 1);
            GroupState::Pending
        });
        assert_eq!(first, Some(1));
        // 再読みでは完了と答えるが、止まった先頭は Pending のまま配信もされない。
        assert_eq!(t.state_of(1, |g| *g), CompletionState::Pending);
        t.callbacks.push((1, 1, Box::new(|_| {})));
        let ready = t.take_ready(1, |g| *g);
        assert_eq!(ready.len(), 1);
        assert_eq!(t.callbacks.len(), 1);
        assert_eq!(t.callbacks[0].0, 1);
        assert!(t.pending.contains_key(&1));
    }

    #[test]
    fn take_all_callbacks_returns_every_registered_callback_once() {
        let mut t = Submissions::<GroupState>::new();
        t.callbacks.push((1, 1, Box::new(|_| {})));
        t.callbacks.push((2, 1, Box::new(|_| {})));
        assert_eq!(t.take_all_callbacks().len(), 2);
        assert!(t.callbacks.is_empty());
        assert!(t.take_all_callbacks().is_empty());
    }

    /// 払い出しの世代が違う項目と表に無い識別子は外さず、一致した項目だけを 1 回外す。
    #[test]
    fn remove_issued_matches_the_issue_generation_not_the_current_one() {
        let mut map: BTreeMap<u64, (u64, &str)> = BTreeMap::new();
        map.insert(1, (1, "a"));
        map.insert(2, (2, "b"));
        assert!(remove_issued(&mut map, 1, 2, |e| e.0).is_none());
        assert!(remove_issued(&mut map, 3, 1, |e| e.0).is_none());
        assert_eq!(map.len(), 2);
        assert_eq!(remove_issued(&mut map, 1, 1, |e| e.0), Some((1, "a")));
        assert!(remove_issued(&mut map, 1, 1, |e| e.0).is_none());
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&2));
    }

    /// 空の表と偽の識別子は何も外さず、別の項目を巻き込まない。
    #[test]
    fn remove_issued_ignores_forged_ids_and_an_empty_table() {
        let mut map: BTreeMap<u64, (u64, &str)> = BTreeMap::new();
        assert!(remove_issued(&mut map, 0, 0, |e| e.0).is_none());
        assert!(remove_issued(&mut map, u64::MAX, u64::MAX, |e| e.0).is_none());
        map.insert(1, (1, "a"));
        assert!(remove_issued(&mut map, u64::MAX, 1, |e| e.0).is_none());
        assert!(remove_issued(&mut map, u64::MAX, u64::MAX, |e| e.0).is_none());
        assert_eq!(map.len(), 1);
    }

    /// 登録の世代が小さければ列の状態に関わらず `Lost`、大きければ表に残す。
    #[test]
    fn take_ready_answers_lost_for_an_older_generation_and_keeps_a_newer_one() {
        let mut t = Submissions::<GroupState>::new();
        for _ in 0..3 {
            let _ = t.issue(|| Ok(GroupState::Completed));
        }
        t.callbacks.push((1, 1, Box::new(|_| {})));
        t.callbacks.push((2, 2, Box::new(|_| {})));
        t.callbacks.push((3, 3, Box::new(|_| {})));
        let ready = t.take_ready(2, |g| *g);
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].1, CompletionState::Lost);
        assert_eq!(ready[1].1, CompletionState::Complete);
        assert_eq!(t.callbacks.len(), 1);
        assert_eq!(t.callbacks[0].0, 3);
        assert_eq!(t.callbacks[0].1, 3);
    }

    /// 駆動が決めた後に世代が進んでいれば、呼ぶ値は `Lost` になる。
    #[test]
    fn a_complete_judged_before_the_generation_moved_is_delivered_as_lost() {
        assert_eq!(
            delivered_state(CompletionState::Complete, 1, 1),
            CompletionState::Complete
        );
        assert_eq!(
            delivered_state(CompletionState::Complete, 1, 2),
            CompletionState::Lost
        );
        assert_eq!(
            delivered_state(CompletionState::Lost, 1, 1),
            CompletionState::Lost
        );
    }

    /// 登録の世代が古いものだけを `Lost` で外し、新しいものは表に残す。
    #[test]
    fn take_stale_returns_older_registrations_as_lost_once() {
        let mut t = Submissions::<GroupState>::new();
        let _ = t.issue(|| Ok(GroupState::Pending));
        t.callbacks.push((1, 1, Box::new(|_| {})));
        t.callbacks.push((1, 2, Box::new(|_| {})));
        let stale = t.take_stale(2);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].1, CompletionState::Lost);
        assert_eq!(t.callbacks.len(), 1);
        assert_eq!(t.callbacks[0].1, 2);
        assert!(t.take_stale(2).is_empty());
        assert_eq!(t.callbacks.len(), 1);
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

    /// 入力の分類: `CudaC`・`Ptx` が通り、ほかは `Unsupported` か `ArtifactRejected` になる。
    #[test]
    fn cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified() {
        use fandhe_silicon_core::execution::{MslFloatFunctions, MslMathMode};
        assert_eq!(
            classify_pipeline_input(&PipelineInput::CudaC {
                source: "s",
                entry_point: "e",
            }),
            Ok(AcceptedInput::CudaC {
                source: "s",
                entry_point: "e",
            })
        );
        assert_eq!(
            classify_pipeline_input(&PipelineInput::Ptx {
                source: "p",
                entry_point: "e",
            }),
            Ok(AcceptedInput::Ptx {
                source: "p",
                entry_point: "e",
            })
        );
        assert!(matches!(
            classify_pipeline_input(&PipelineInput::Wgsl {
                source: "w",
                entry_point: "e",
            }),
            Err(ContractError::Unsupported {
                op: "create_pipeline",
                backend: BackendKind::Cuda
            })
        ));
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
            "pipeline input Msl is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx"
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
            "pipeline input MslSpecializedWithOptions is Metal-only and is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx"
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

    #[cfg(feature = "ext-interop")]
    #[test]
    fn uuid_bytes_keep_every_bit() {
        let bytes: [u8; 16] =
            std::array::from_fn(|i| (i as u8).wrapping_mul(17).wrapping_add(0x80));
        let raw = bytes.map(|b| std::ffi::c_char::from_ne_bytes([b]));
        assert_eq!(uuid_bytes(raw), bytes);
    }

    #[cfg(feature = "ext-interop")]
    #[test]
    fn uuid_read_maps_to_an_id_without_driver_uuid_or_to_device_unavailable() {
        let ok =
            read_physical_gpu_id(|| Ok::<_, &str>([std::ffi::c_char::from_ne_bytes([0xff]); 16]))
                .expect("uuid read succeeds");
        assert_eq!(ok.device_uuid, [0xff; 16]);
        assert!(ok.driver_uuid.is_none());
        assert!(matches!(
            read_physical_gpu_id(|| Err::<[std::ffi::c_char; 16], _>("injected")),
            Err(ContractError::DeviceUnavailable(_))
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

    /// ロスト通知が panic しても、登録済みの完了 callback は全部 `Lost` で呼ばれ、panic は呼び出し側へ届く。
    #[cfg(feature = "fault-injection")]
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_panicking_lost_callback_still_delivers_lost_to_completion_callbacks() {
        let d = device();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let generation = d.current_generation();
        {
            let mut table = d.lock_submissions();
            for sequence in 1..=2_u64 {
                let seen = Arc::clone(&seen);
                table.callbacks.push((
                    sequence,
                    generation,
                    Box::new(move |state| seen.lock().unwrap().push(state)),
                ));
            }
        }
        d.set_lost_callback(Box::new(|_| panic!("lost callback panic")));

        let caught = catch_unwind(AssertUnwindSafe(|| {
            d.simulate_device_lost(DeviceLostReason::Unknown)
        }));
        assert!(caught.is_err());
        assert_eq!(
            *seen.lock().unwrap(),
            vec![CompletionState::Lost, CompletionState::Lost]
        );
        assert!(d.simulate_device_lost(DeviceLostReason::Destroyed).is_ok());
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

    /// WGSL は `Unsupported`、MSL は `ArtifactRejected { kind: "unsupported-input" }`。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn wgsl_is_unsupported_and_msl_is_unsupported_input() {
        let d = device();
        let layout = BindingLayout::new(vec![0]);
        let wgsl = d.create_pipeline(
            PipelineInput::Wgsl {
                source: "@compute @workgroup_size(1) fn k() {}",
                entry_point: "k",
            },
            &layout,
        );
        assert!(matches!(
            wgsl,
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

    /// 模擬のロストを device で起こす試験（TASK-2934）。
    #[cfg(feature = "fault-injection")]
    mod lost {
        use super::*;
        use fandhe_silicon_core::fault::{HookFault, inject};

        /// 理由ごとに、コールバックは 1 回だけ呼ばれ、世代は 1 回だけ進む。2 回目のロストは何も変えない。
        #[test]
        #[ignore = "requires a CUDA device"]
        fn each_reason_reaches_the_callback_once_and_the_generation_moves_once() {
            for reason in [
                DeviceLostReason::Destroyed,
                DeviceLostReason::Unknown,
                DeviceLostReason::ValidationError("v".into()),
            ] {
                let d = device();
                let seen = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&seen);
                d.set_lost_callback(Box::new(move |r| sink.lock().unwrap().push(r)));
                let before = d.generation();
                inject(d.fault_hooks(), HookFault::DeviceLost(reason.clone())).expect("first loss");
                let after = d.generation();
                assert!(after > before);
                assert!(d.lost.is_lost());
                assert_eq!(*seen.lock().unwrap(), vec![reason]);

                inject(
                    d.fault_hooks(),
                    HookFault::DeviceLost(DeviceLostReason::Unknown),
                )
                .expect("second loss");
                assert_eq!(d.generation(), after);
                assert_eq!(seen.lock().unwrap().len(), 1);
            }
        }

        /// ロストの前のハンドルは使えないが、`free` で表から 1 回だけ外れ、写しの `free` は何も変えない。
        #[test]
        #[ignore = "requires a CUDA device"]
        fn a_handle_from_before_the_loss_can_still_be_freed_once() {
            let d = device();
            let h = d.allocate(64, BufferUsage::UPLOAD).expect("allocate");
            inject(
                d.fault_hooks(),
                HookFault::DeviceLost(DeviceLostReason::Unknown),
            )
            .expect("loss");
            assert_eq!(
                d.upload(&h, 0, &[1]),
                Err(ContractError::DeviceLost {
                    generation: d.generation()
                })
            );
            assert_eq!(d.lock().map.len(), 1);

            let again = BufferHandle::__backend_new(
                h.__backend_generation(),
                h.__backend_id(),
                h.__backend_device_id(),
                64,
                BufferUsage::empty(),
            );
            d.free(h);
            assert_eq!(d.lock().map.len(), 0);
            d.free(again);
            assert_eq!(d.lock().map.len(), 0);
        }

        /// ロストの前の印は、`poll` で `Lost` になり、`on_complete` は `Lost` を 1 回渡す。
        #[test]
        #[ignore = "requires a CUDA device"]
        fn a_token_from_before_the_loss_polls_and_completes_as_lost() {
            let d = device();
            let token = d.submit(d.create_encoder(), &[]).expect("submit");
            inject(
                d.fault_hooks(),
                HookFault::DeviceLost(DeviceLostReason::Unknown),
            )
            .expect("loss");
            assert_eq!(d.poll(token), CompletionState::Lost);
            let seen = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&seen);
            d.on_complete(token, Box::new(move |s| sink.lock().unwrap().push(s)));
            assert_eq!(*seen.lock().unwrap(), vec![CompletionState::Lost]);
        }
    }
}

/// CUDA の device に触る、バッファの再利用とゼロ化の試験。device の無い環境では流さない。
#[cfg(all(test, feature = "native-blocking-wait"))]
mod pool_device_tests {
    use super::*;

    /// 読み書きの両方を許す用途。確保の用途に `UPLOAD` と `DOWNLOAD` が要る試験で使う。
    fn rw() -> BufferUsage {
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

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
            let h = d.allocate(len, rw()).expect("allocate");
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
            let h = d.allocate(len, rw()).expect("allocate");
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
        let mut h = d.allocate(len, rw()).expect("allocate");
        for round in 0..1000u32 {
            let fill = u8::try_from(round % 255).unwrap_or(0) + 1;
            d.upload(&h, 0, &vec![fill; 4097]).expect("upload");
            d.free(h);
            h = d.allocate(len, rw()).expect("allocate");
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
        let a = d.allocate(len, rw()).expect("allocate");
        let slot_a = d.test_slot_of(&a).expect("slot");
        d.free(a);
        d.stream.synchronize().expect("synchronize");
        let reused_before = d.test_pool_reuse_count();
        let b = d
            .allocate(len, BufferUsage::UPLOAD | BufferUsage::DOWNLOAD)
            .expect("allocate");
        assert_eq!(d.test_pool_reuse_count(), reused_before);
        assert_eq!(b.usage(), BufferUsage::UPLOAD | BufferUsage::DOWNLOAD);
        assert_eq!(nonzero_bytes(&d, &b, len), 0);
        let again = d.allocate(len, rw()).expect("allocate");
        assert_eq!(d.test_pool_reuse_count(), reused_before + 1);
        assert_eq!(d.test_slot_of(&again), Some(slot_a));
    }

    /// 解放済みハンドルの写しは、再利用されたバッファに当たらない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_freed_handle_does_not_alias_the_reused_buffer() {
        let d = device();
        let h = d.allocate(64, rw()).expect("allocate");
        let stale =
            BufferHandle::__backend_new(d.current_generation(), h.__backend_id(), d.id, 64, rw());
        let slot = d.test_slot_of(&h).expect("slot");
        d.free(h);
        d.stream.synchronize().expect("synchronize");
        let fresh = d.allocate(64, rw()).expect("allocate");
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
            .map(|_| d.allocate(32, rw()).expect("allocate"))
            .collect();
        for h in handles {
            d.free(h);
        }
        assert_eq!(d.test_pool_len(), POOL_MAX_ENTRIES);
        let other = d.allocate(33, rw()).expect("allocate");
        assert_eq!(d.test_pool_reuse_count(), 0);
        assert_eq!(d.test_pool_len(), POOL_MAX_ENTRIES);
        d.free(other);
    }
}

/// CUDA の device に触る、upload・download_blocking・download_async の用途と範囲の試験。device の無い環境では流さない。
#[cfg(all(test, feature = "native-blocking-wait"))]
mod transfer_device_tests {
    use super::*;

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 読み書きの両方を許す用途。
    fn rw() -> BufferUsage {
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    /// 0 でない値が並ぶ長さ `len` の列。
    fn pattern(len: u64) -> Vec<u8> {
        (0..len)
            .map(|i| u8::try_from(i % 251).unwrap_or(0) + 1)
            .collect()
    }

    /// 任意の位置から書いた内容が、全体の読み戻しの該当の範囲に現れる（MEM-4）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn upload_at_offsets_reads_back_through_download_blocking() {
        let d = device();
        let max = d.limits().max_buffer_bytes;
        let mib = 1024 * 1024u64;
        for (len, offset, write_len) in [
            (1u64, 0u64, 1u64),
            (4097, 0, 300),
            (4097, 5, 300),
            (4097, 3797, 300),
            (mib, 0, 4096),
            (mib, 4099, 4096),
        ] {
            if len > max {
                println!("not-run: len={len} exceeds max_buffer_bytes={max}");
                continue;
            }
            let h = d.allocate(len, rw()).expect("allocate");
            let src = pattern(write_len);
            d.upload(&h, offset, &src).expect("upload");
            let mut expected = vec![0u8; usize::try_from(len).expect("len")];
            let start = usize::try_from(offset).expect("offset");
            expected[start..start + src.len()].copy_from_slice(&src);
            assert_eq!(
                d.download_blocking(&h, 0, len).expect("download"),
                expected,
                "len={len} offset={offset}"
            );
            d.free(h);
        }
    }

    /// 読み戻しの長さは `byte_len` に等しく、中身は書いた内容の該当の範囲（MEM-5）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn download_blocking_returns_byte_len_bytes_of_the_written_contents() {
        let d = device();
        let h = d.allocate(4097, rw()).expect("allocate");
        let src = pattern(4097);
        d.upload(&h, 0, &src).expect("upload");
        for (offset, byte_len) in [(0u64, 0u64), (0, 1), (5, 7), (0, 4097)] {
            let got = d.download_blocking(&h, offset, byte_len).expect("download");
            let start = usize::try_from(offset).expect("offset");
            let end = start + usize::try_from(byte_len).expect("byte_len");
            assert_eq!(got.len(), end - start);
            assert_eq!(got, src[start..end], "offset={offset} byte_len={byte_len}");
        }
        d.free(h);
    }

    /// 終端を越える・あふれる upload は拒まれ、内容は変わらない（MEM-70）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn upload_past_the_end_or_overflowing_is_rejected_and_leaves_the_contents_unchanged() {
        let d = device();
        let h = d.allocate(16, rw()).expect("allocate");
        d.upload(&h, 0, &[0x11; 16]).expect("upload");
        d.upload(&h, 8, &[0x22; 8]).expect("upload to the end");
        let mut expected = vec![0x11u8; 8];
        expected.extend_from_slice(&[0x22; 8]);
        for (offset, data) in [
            (8u64, vec![0x33u8; 9]),
            (17, vec![]),
            (u64::MAX, vec![0x33; 1]),
            (u64::MAX - 7, vec![0x33; 16]),
        ] {
            assert!(
                matches!(
                    d.upload(&h, offset, &data),
                    Err(ContractError::InvalidArgument(_))
                ),
                "offset={offset} len={}",
                data.len()
            );
        }
        assert_eq!(d.download_blocking(&h, 0, 16).expect("download"), expected);
        d.free(h);
    }

    /// 終端を越える読み戻しは拒まれ、範囲内の窓は書いた内容と一致する（MEM-73）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn download_blocking_past_the_end_is_rejected_and_in_range_windows_match() {
        let d = device();
        let h = d.allocate(4097, rw()).expect("allocate");
        let src = pattern(4097);
        d.upload(&h, 0, &src).expect("upload");
        for (offset, byte_len) in [(4090u64, 8u64), (0, 4098), (4098, 0)] {
            assert!(
                matches!(
                    d.download_blocking(&h, offset, byte_len),
                    Err(ContractError::InvalidArgument(_))
                ),
                "offset={offset} byte_len={byte_len}"
            );
        }
        for (offset, byte_len) in [(0u64, 4097u64), (1, 4096), (4096, 1), (2048, 0)] {
            let start = usize::try_from(offset).expect("offset");
            let end = start + usize::try_from(byte_len).expect("byte_len");
            assert_eq!(
                d.download_blocking(&h, offset, byte_len).expect("download"),
                src[start..end],
                "offset={offset} byte_len={byte_len}"
            );
        }
        d.free(h);
    }

    /// `offset + len` が `u64` であふれる入力は、upload も download_blocking も拒む（MEM-76）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn overflowing_offset_plus_len_is_rejected_by_upload_and_download_blocking() {
        let d = device();
        let h = d.allocate(16, rw()).expect("allocate");
        let src = pattern(16);
        d.upload(&h, 0, &src).expect("upload");
        for (offset, data) in [(u64::MAX, vec![0x44u8; 1]), (u64::MAX - 7, vec![0x44; 16])] {
            assert!(matches!(
                d.upload(&h, offset, &data),
                Err(ContractError::InvalidArgument(_))
            ));
        }
        for (offset, byte_len) in [(u64::MAX, 1u64), (1, u64::MAX), (u64::MAX - 7, 16)] {
            assert!(matches!(
                d.download_blocking(&h, offset, byte_len),
                Err(ContractError::InvalidArgument(_))
            ));
        }
        assert_eq!(d.download_blocking(&h, 0, 16).expect("download"), src);
        d.free(h);
    }

    /// 用途が足りない upload と読み戻しは拒まれ、内容は変わらない（MEM-109）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn missing_usage_is_rejected_and_leaves_the_contents_unchanged() {
        let d = device();
        let download_only = d
            .allocate(16, BufferUsage::STORAGE | BufferUsage::DOWNLOAD)
            .expect("allocate");
        assert!(matches!(
            d.upload(&download_only, 0, &[0x55]),
            Err(ContractError::InvalidArgument(_))
        ));
        assert!(matches!(
            d.upload(&download_only, 0, &[]),
            Err(ContractError::InvalidArgument(_))
        ));
        assert_eq!(
            d.download_blocking(&download_only, 0, 16)
                .expect("download"),
            vec![0u8; 16]
        );
        d.free(download_only);

        let upload_only = d
            .allocate(16, BufferUsage::STORAGE | BufferUsage::UPLOAD)
            .expect("allocate");
        d.upload(&upload_only, 0, &[0x55; 16]).expect("upload");
        for byte_len in [1u64, 0] {
            assert!(matches!(
                d.download_blocking(&upload_only, 0, byte_len),
                Err(ContractError::InvalidArgument(_))
            ));
        }
        d.free(upload_only);

        let none = d.allocate(16, BufferUsage::empty()).expect("allocate");
        assert!(matches!(
            d.upload(&none, 0, &[0x55]),
            Err(ContractError::InvalidArgument(_))
        ));
        assert!(matches!(
            d.download_blocking(&none, 0, 1),
            Err(ContractError::InvalidArgument(_))
        ));
        d.free(none);
    }

    /// 用途の判定は表に記録した値で行い、ハンドルの申告が多くても通らない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn the_recorded_usage_decides_even_if_the_handle_claims_more() {
        let d = device();
        let h = d.allocate(16, BufferUsage::STORAGE).expect("allocate");
        let claimed =
            BufferHandle::__backend_new(d.current_generation(), h.__backend_id(), d.id, 16, rw());
        for handle in [&claimed, &h] {
            assert!(matches!(
                d.upload(handle, 0, &[1u8]),
                Err(ContractError::InvalidArgument(_))
            ));
            assert!(matches!(
                d.download_blocking(handle, 0, 1),
                Err(ContractError::InvalidArgument(_))
            ));
        }
        d.free(h);
    }

    /// `DOWNLOAD` の無い確保への download_async は、コールバックを 1 回だけ `InvalidArgument` で呼ぶ。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn download_async_without_download_usage_calls_back_once_with_invalid_argument() {
        let d = device();
        let h = d
            .allocate(16, BufferUsage::STORAGE | BufferUsage::UPLOAD)
            .expect("allocate");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        d.download_async(
            &h,
            0,
            16,
            Box::new(move |r| {
                sink.lock()
                    .unwrap()
                    .push(matches!(r, Err(ContractError::InvalidArgument(_))));
            }),
        );
        assert_eq!(*seen.lock().unwrap(), vec![true]);
        d.free(h);
    }
}

#[cfg(all(test, feature = "fault-injection"))]
mod lost_state_tests {
    //! 模擬のロストの状態 `LostState` の固定（device に触らない。TASK-2934）。
    use super::*;

    type Seen = Arc<Mutex<Vec<DeviceLostReason>>>;

    fn recorder(seen: &Seen) -> LostCallback {
        let seen = Arc::clone(seen);
        Box::new(move |reason| seen.lock().unwrap().push(reason))
    }

    fn all_reasons() -> [DeviceLostReason; 4] {
        [
            DeviceLostReason::Destroyed,
            DeviceLostReason::Unknown,
            DeviceLostReason::ValidationError("v".into()),
            DeviceLostReason::Unrecoverable("u".into()),
        ]
    }

    #[test]
    fn the_callback_runs_once_and_a_second_lose_is_ignored() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(5));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        assert!(!state.is_lost());

        assert_eq!(
            state.lose(DeviceLostReason::Unknown, &generation, &source),
            Ok(true)
        );
        assert_eq!(
            state.lose(DeviceLostReason::Destroyed, &generation, &source),
            Ok(false)
        );
        assert!(state.is_lost());
        assert_eq!(*seen.lock().unwrap(), [DeviceLostReason::Unknown]);
    }

    #[test]
    fn every_reason_is_passed_through_unchanged() {
        for reason in all_reasons() {
            let state = LostState::default();
            let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
            let seen = Seen::default();
            state.set_callback(recorder(&seen));
            state.lose(reason.clone(), &generation, &source).unwrap();
            assert_eq!(*seen.lock().unwrap(), [reason]);
        }
    }

    #[test]
    fn the_generation_is_taken_once() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(7));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        assert_eq!(generation.load(Ordering::Relaxed), 7);
        assert_eq!(source.load(Ordering::Relaxed), 8);
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert_eq!(generation.load(Ordering::Relaxed), 7);
        assert_eq!(source.load(Ordering::Relaxed), 8);
    }

    #[test]
    fn an_exhausted_source_leaves_the_state_unchanged() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(u64::MAX));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        assert_eq!(
            state.lose(DeviceLostReason::Unknown, &generation, &source),
            Err(GenerationExhausted)
        );
        assert!(!state.is_lost());
        assert_eq!(generation.load(Ordering::Relaxed), 1);
        assert_eq!(source.load(Ordering::Relaxed), u64::MAX);
        assert!(seen.lock().unwrap().is_empty());

        // 源が戻れば、保持されたままのコールバックでロストできる。
        source.store(3, Ordering::Relaxed);
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), [DeviceLostReason::Destroyed]);
        assert_eq!(generation.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn a_callback_registered_after_the_loss_is_kept_but_not_called() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert!(seen.lock().unwrap().is_empty());
        assert!(state.lock().callback.is_some());
    }

    /// 通知先が panic しても、理由と世代は記録済みで、`lose_catching` は中身を返し、2 回目は何もしない。
    #[test]
    fn a_panicking_callback_is_returned_after_the_loss_is_recorded() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(5));
        state.set_callback(Box::new(|_| panic!("lost callback panic")));

        let (first, panic) = state
            .lose_catching(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        assert!(first);
        assert!(panic.is_some());
        assert!(state.is_lost());
        assert_eq!(generation.load(Ordering::Acquire), 5);
        let (again, panic) = state
            .lose_catching(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert!(!again);
        assert!(panic.is_none());
    }

    #[test]
    fn a_loss_without_a_callback_then_a_registration_never_calls_it() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert!(seen.lock().unwrap().is_empty());
    }

    /// 捨てられるときに、同じ状態の `set_callback` と `lose` を呼ぶ値。
    struct ReentrantDrop {
        state: Arc<LostState>,
        generation: Arc<AtomicU64>,
        source: Arc<AtomicU64>,
        done: Arc<Mutex<bool>>,
    }

    impl Drop for ReentrantDrop {
        fn drop(&mut self) {
            self.state.set_callback(Box::new(|_| {}));
            let _ = self
                .state
                .lose(DeviceLostReason::Unknown, &self.generation, &self.source);
            *self.done.lock().unwrap() = true;
        }
    }

    #[test]
    fn dropping_an_overwritten_callback_may_reenter_without_deadlock() {
        let state = Arc::new(LostState::default());
        let generation = Arc::new(AtomicU64::new(1));
        let source = Arc::new(AtomicU64::new(2));
        let done = Arc::new(Mutex::new(false));
        let guard = ReentrantDrop {
            state: Arc::clone(&state),
            generation: Arc::clone(&generation),
            source: Arc::clone(&source),
            done: Arc::clone(&done),
        };
        state.set_callback(Box::new(move |_| {
            let _keep = &guard;
        }));
        // 上書きで古いコールバックが捨てられ、その `Drop` が再入する。錠を握ったままなら詰まる。
        state.set_callback(Box::new(|_| {}));
        assert!(*done.lock().unwrap());
        assert!(state.is_lost());
    }

    #[test]
    fn concurrent_loses_call_the_callback_once_and_advance_the_source_once() {
        const THREADS: usize = 8;
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(10));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        let barrier = std::sync::Barrier::new(THREADS);
        std::thread::scope(|scope| {
            for _ in 0..THREADS {
                scope.spawn(|| {
                    barrier.wait();
                    state
                        .lose(DeviceLostReason::Unknown, &generation, &source)
                        .unwrap();
                });
            }
        });
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert_eq!(source.load(Ordering::Relaxed), 11);
        assert_eq!(generation.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn a_later_registration_replaces_the_earlier_one() {
        let state = LostState::default();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        let (first, second) = (Seen::default(), Seen::default());
        state.set_callback(recorder(&first));
        state.set_callback(recorder(&second));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        assert!(first.lock().unwrap().is_empty());
        assert_eq!(second.lock().unwrap().len(), 1);
    }

    #[test]
    fn the_callback_runs_outside_the_lock() {
        let state = Arc::new(LostState::default());
        let (generation, source) = (Arc::new(AtomicU64::new(1)), Arc::new(AtomicU64::new(2)));
        let reentered = Arc::new(Mutex::new(None));
        {
            let (state2, generation2, source2, reentered) = (
                Arc::clone(&state),
                Arc::clone(&generation),
                Arc::clone(&source),
                Arc::clone(&reentered),
            );
            state.set_callback(Box::new(move |_| {
                // 錠を持ったまま呼ばれていれば、ここで詰まる。
                state2.set_callback(Box::new(|_| {}));
                let again = state2.lose(DeviceLostReason::Unknown, &generation2, &source2);
                *reentered.lock().unwrap() = Some((state2.is_lost(), again));
            }));
        }
        state
            .lose(DeviceLostReason::Destroyed, &generation, &source)
            .unwrap();
        assert_eq!(*reentered.lock().unwrap(), Some((true, Ok(false))));
    }

    #[test]
    fn a_poisoned_lock_does_not_panic() {
        let state = Arc::new(LostState::default());
        let poisoner = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.inner.lock().unwrap();
            panic!("poison the lock");
        })
        .join();
        let (generation, source) = (AtomicU64::new(1), AtomicU64::new(2));
        let seen = Seen::default();
        state.set_callback(recorder(&seen));
        state
            .lose(DeviceLostReason::Unknown, &generation, &source)
            .unwrap();
        assert_eq!(seen.lock().unwrap().len(), 1);
    }
}
/// 検証の報告の受け口の登録の試験（device に触らない。TASK-607 のポインタ）。
#[cfg(test)]
mod validation_intake_tests {
    use super::*;

    #[test]
    fn the_cuda_device_takes_a_validation_callback() {
        fn takes<T: ValidationIntake + Send + Sync>() {}
        takes::<CudaDevice>();
    }
}
/// 破棄の部品の試験（device に触らない。TASK-191 のポインタ）。
#[cfg(test)]
mod teardown_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// 落とされた回数を数える値。
    struct Counted(Arc<AtomicUsize>);

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// 落とすと panic する値。
    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("drop panics");
        }
    }

    /// 同期を確かめられたときだけ落とす。失敗と panic では落とさず、panic は外へ出ない。
    #[test]
    fn release_if_drops_only_after_a_confirmed_sync() {
        let count = Arc::new(AtomicUsize::new(0));
        release_if(
            sync_confirmed(|| Ok::<(), ()>(())),
            Counted(Arc::clone(&count)),
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
        release_if(
            sync_confirmed(|| Err::<(), ()>(())),
            Counted(Arc::clone(&count)),
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
        release_if(
            sync_confirmed(|| -> Result<(), ()> { panic!("sync panics") }),
            Counted(Arc::clone(&count)),
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    /// 落とす途中の panic は外へ出ない。
    #[test]
    fn release_if_does_not_unwind_when_a_drop_panics() {
        release_if(true, PanicOnDrop);
    }

    /// 同期を確かめたら `take_ready` と同じ判定で値を決め、表は空になる。
    #[test]
    fn teardown_ready_judges_like_poll_after_a_confirmed_sync() {
        let mut t = Submissions::<GroupState>::new();
        let _ = t.issue(|| Ok(GroupState::Completed));
        let _ = t.issue(|| Ok(GroupState::Failed));
        let _ = t.issue(|| Ok(GroupState::Pending));
        for sequence in 1..=3 {
            t.callbacks.push((sequence, 1, Box::new(|_| {})));
        }
        let ready = teardown_ready(&mut t, true, 1, |g| *g);
        let states: Vec<CompletionState> = ready.iter().map(|(_, s)| *s).collect();
        assert_eq!(
            states,
            [
                CompletionState::Complete,
                CompletionState::Lost,
                CompletionState::Lost
            ]
        );
        assert!(t.callbacks.is_empty());
    }

    /// 同期を確かめられなければ、完了していた投入の callback にも `Lost` を渡す。
    #[test]
    fn teardown_ready_delivers_lost_without_a_confirmed_sync() {
        let mut t = Submissions::<GroupState>::new();
        let _ = t.issue(|| Ok(GroupState::Completed));
        t.callbacks.push((1, 1, Box::new(|_| {})));
        let ready = teardown_ready(&mut t, false, 1, |g| *g);
        let states: Vec<CompletionState> = ready.iter().map(|(_, s)| *s).collect();
        assert_eq!(states, [CompletionState::Lost]);
        assert!(t.callbacks.is_empty());
    }

    /// 明示の破棄の 4 つの段は、この順で 1 回ずつ呼ばれる。
    #[test]
    fn destroy_steps_run_in_order() {
        use std::cell::RefCell;
        let log = RefCell::new(Vec::new());
        let out = destroy_in_order(
            (),
            |_| {
                log.borrow_mut().push("flush");
                None
            },
            |_| log.borrow_mut().push("sync"),
            |_| {
                log.borrow_mut().push("notify");
                None
            },
            |_| log.borrow_mut().push("release"),
        );
        assert!(out.is_none());
        assert_eq!(*log.borrow(), ["flush", "sync", "notify", "release"]);
    }

    /// 2 つの段が panic の中身を返すと、解放まで終えてから最初の段のものを返す。
    #[test]
    fn destroy_steps_return_the_first_panic_after_release() {
        use std::cell::RefCell;
        let log = RefCell::new(Vec::new());
        let out = destroy_in_order(
            (),
            |_| {
                log.borrow_mut().push("flush");
                Some(Box::new("flush") as Box<dyn std::any::Any + Send>)
            },
            |_| log.borrow_mut().push("sync"),
            |_| {
                log.borrow_mut().push("notify");
                Some(Box::new("notify") as Box<dyn std::any::Any + Send>)
            },
            |_| log.borrow_mut().push("release"),
        );
        assert_eq!(log.borrow().last(), Some(&"release"));
        let payload = out.expect("a panic payload");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"flush"));
    }

    /// 完了の callback の段に panic が無ければ、通知の段のものを返す。
    #[test]
    fn destroy_steps_return_the_notify_panic_when_flush_has_none() {
        let out = destroy_in_order(
            (),
            |_| None,
            |_| {},
            |_| Some(Box::new("notify") as Box<dyn std::any::Any + Send>),
            |_| {},
        );
        let payload = out.expect("a panic payload");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"notify"));
    }
}

/// CUDA の device に触る、破棄の試験（TASK-191 のポインタ）。device の無い環境では流さない。
#[cfg(test)]
mod teardown_device_tests {
    use super::*;

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 3 本を確保して書き込み、1 本を `free` し、空の記録先を投入した device と印を返す。
    fn prepared() -> (CudaDevice, SubmitToken) {
        let d = device();
        let handles: Vec<BufferHandle> = (0..3)
            .map(|_| {
                let h = d
                    .allocate(4097, BufferUsage::STORAGE | BufferUsage::UPLOAD)
                    .expect("allocate");
                d.upload(&h, 0, &[7u8; 4097]).expect("upload");
                h
            })
            .collect();
        let mut handles = handles.into_iter();
        if let Some(h) = handles.next() {
            d.free(h);
        }
        let token = d.submit(d.create_encoder(), &[]).expect("submit");
        (d, token)
    }

    // DEV-150
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dropping_right_after_submit_records_no_driver_error() {
        let (d, _token) = prepared();
        let ctx = Arc::clone(d.stream.context());
        drop(d);
        assert!(ctx.check_err().is_ok());
    }

    // DEV-150
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dropping_right_after_on_complete_calls_it_once() {
        let (d, token) = prepared();
        let ctx = Arc::clone(d.stream.context());
        let seen: Arc<Mutex<Vec<CompletionState>>> = Arc::default();
        let sink = Arc::clone(&seen);
        d.on_complete(token, Box::new(move |s| sink.lock().unwrap().push(s)));
        drop(d);
        assert_eq!(*seen.lock().unwrap(), vec![CompletionState::Complete]);
        assert!(ctx.check_err().is_ok());
    }

    // DEV-151
    #[test]
    #[ignore = "requires a CUDA device"]
    fn destroying_right_after_submit_records_no_driver_error() {
        let (d, _token) = prepared();
        let ctx = Arc::clone(d.stream.context());
        (Box::new(d) as Box<dyn Device>).destroy();
        assert!(ctx.check_err().is_ok());
    }

    // DEV-151
    #[test]
    #[ignore = "requires a CUDA device"]
    fn destroying_right_after_on_complete_calls_it_once() {
        let (d, token) = prepared();
        let ctx = Arc::clone(d.stream.context());
        let seen: Arc<Mutex<Vec<CompletionState>>> = Arc::default();
        let sink = Arc::clone(&seen);
        d.on_complete(token, Box::new(move |s| sink.lock().unwrap().push(s)));
        (Box::new(d) as Box<dyn Device>).destroy();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert!(!matches!(seen[0], CompletionState::Pending));
        assert!(ctx.check_err().is_ok());
    }

    #[cfg(feature = "fault-injection")]
    mod lost_notify {
        use super::*;
        use fandhe_silicon_core::fault::{HookFault, inject};

        type Seen = Arc<Mutex<Vec<DeviceLostReason>>>;

        fn register(d: &CudaDevice) -> Seen {
            let seen = Seen::default();
            let sink = Arc::clone(&seen);
            d.set_lost_callback(Box::new(move |r| sink.lock().unwrap().push(r)));
            seen
        }

        #[test]
        #[ignore = "requires a CUDA device"]
        fn destroy_notifies_destroyed_once() {
            let d = device();
            let seen = register(&d);
            (Box::new(d) as Box<dyn Device>).destroy();
            assert_eq!(*seen.lock().unwrap(), vec![DeviceLostReason::Destroyed]);
        }

        #[test]
        #[ignore = "requires a CUDA device"]
        fn destroy_after_a_simulated_loss_does_not_notify_again() {
            let d = device();
            let seen = register(&d);
            inject(
                d.fault_hooks(),
                HookFault::DeviceLost(DeviceLostReason::Unknown),
            )
            .expect("loss");
            (Box::new(d) as Box<dyn Device>).destroy();
            assert_eq!(*seen.lock().unwrap(), vec![DeviceLostReason::Unknown]);
        }

        #[test]
        #[ignore = "requires a CUDA device"]
        fn drop_does_not_notify() {
            let d = device();
            let seen = register(&d);
            drop(d);
            assert!(seen.lock().unwrap().is_empty());
        }
    }
}

/// CUDA の device に触る、宣言を照らしてから作る口の試験（TASK-457 のポインタ）。device の無い環境では流さない。
#[cfg(test)]
mod checked_pipeline_device_tests {
    use super::*;
    use fandhe_silicon_core::execution::{DType, OperandSpec};

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

    fn add_one() -> PipelineInput<'static> {
        PipelineInput::CudaC {
            source: ADD_ONE,
            entry_point: "add_one",
        }
    }

    fn table_len(d: &CudaDevice) -> usize {
        d.pipelines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map
            .len()
    }

    fn spec<'a>(slot: u32, dtype: DType, shape: &'a [u64], stride: &'a [i64]) -> OperandSpec<'a> {
        OperandSpec {
            slot,
            dtype,
            shape,
            stride,
            alignment: 4,
        }
    }

    // BIND-60
    /// 件数が違う宣言は `C1` で返り、パイプラインの表に何も足さない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn declared_count_mismatch_is_rejected_and_adds_no_pipeline() {
        let d = device();
        let before = table_len(&d);
        let declared = [spec(0, DType::F32, &[4], &[1])];
        let r = d.create_pipeline_checked(add_one(), &BindingLayout::new(vec![0, 1, 2]), &declared);
        assert!(matches!(&r, Err(ContractError::InvalidArgument(m)) if m.starts_with("C1")));
        assert_eq!(table_len(&d), before);
    }

    // BIND-60
    /// 次元数が違う宣言は `C3` で返り、パイプラインの表に何も足さない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dimension_mismatch_is_rejected_and_adds_no_pipeline() {
        let d = device();
        let before = table_len(&d);
        let declared = [
            spec(0, DType::F32, &[4, 4], &[1]),
            spec(1, DType::F32, &[4], &[1]),
            spec(2, DType::U32, &[1], &[1]),
        ];
        let r = d.create_pipeline_checked(add_one(), &BindingLayout::new(vec![0, 1, 2]), &declared);
        assert!(matches!(&r, Err(ContractError::InvalidArgument(m)) if m.starts_with("C3")));
        assert_eq!(table_len(&d), before);
    }

    // BIND-60
    /// 照らしは入力の分類より先に走る。`Wgsl` の入力でも先頭が `C1` で返る。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn rejection_comes_before_the_input_is_classified() {
        let d = device();
        let before = table_len(&d);
        let declared = [spec(0, DType::F32, &[4], &[1])];
        let input = PipelineInput::Wgsl {
            source: "",
            entry_point: "k",
        };
        let r = d.create_pipeline_checked(input, &BindingLayout::new(vec![0, 1]), &declared);
        assert!(matches!(&r, Err(ContractError::InvalidArgument(m)) if m.starts_with("C1")));
        assert_eq!(table_len(&d), before);
    }

    // BIND-15
    /// 照らしを通る宣言は、この device のパイプラインを 1 つ作って表に足す。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn matching_declaration_creates_a_pipeline_of_this_device() {
        let d = device();
        let before = table_len(&d);
        let declared = [
            spec(0, DType::F32, &[4], &[1]),
            spec(1, DType::F32, &[4], &[1]),
            spec(2, DType::U32, &[1], &[1]),
        ];
        let h = d
            .create_pipeline_checked(add_one(), &BindingLayout::new(vec![0, 1, 2]), &declared)
            .expect("pipeline");
        assert_eq!(h.__backend_device_id(), d.id);
        assert_eq!(table_len(&d), before + 1);
    }

    /// `within` の上限。
    const LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    /// `ADD_ONE` を `compile_cuda_c` で PTX のテキストにする。device の NVRTC の呼び出し口を通らない。
    fn add_one_ptx(d: &CudaDevice) -> String {
        compile_cuda_c(ADD_ONE, d.compute_capability)
            .expect("NVRTC")
            .to_src()
    }

    // BIND-15
    /// 照らしを通る宣言と `Ptx` の入力は、この device のパイプラインを 1 つ作って表に足す。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn matching_declaration_with_ptx_input_creates_a_pipeline_of_this_device() {
        within(|| {
            let d = device();
            let text = add_one_ptx(&d);
            assert!(text.contains(".entry add_one"));
            let before = table_len(&d);
            let declared = [
                spec(0, DType::F32, &[4], &[1]),
                spec(1, DType::F32, &[4], &[1]),
                spec(2, DType::U32, &[1], &[1]),
            ];
            let input = PipelineInput::Ptx {
                source: &text,
                entry_point: "add_one",
            };
            let h = d
                .create_pipeline_checked(input, &BindingLayout::new(vec![0, 1, 2]), &declared)
                .expect("pipeline");
            assert_eq!(h.__backend_device_id(), d.id);
            assert_eq!(table_len(&d), before + 1);
        });
    }
}

/// CUDA の device に触る、`on_complete` と検証の報告の受け口の試験（TASK-607 のポインタ）。
#[cfg(test)]
mod on_complete_device_tests {
    use super::*;
    use std::thread::ThreadId;
    use std::time::{Duration, Instant};

    /// callback が受けた状態と、呼ばれたスレッドの記録。
    struct Recorded {
        states: Arc<Mutex<Vec<CompletionState>>>,
        threads: Arc<Mutex<Vec<ThreadId>>>,
    }

    impl Recorded {
        fn states(&self) -> Vec<CompletionState> {
            self.states.lock().unwrap().clone()
        }

        fn threads(&self) -> Vec<ThreadId> {
            self.threads.lock().unwrap().clone()
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 印の写しを返す。
    fn copy_of(t: &SubmitToken) -> SubmitToken {
        SubmitToken::__backend_new(t.generation, t.sequence, t.__backend_device_id())
    }

    /// 空の記録先を投入して印を返す。
    fn submit_empty(d: &CudaDevice) -> SubmitToken {
        d.submit(d.create_encoder(), &[]).expect("submit")
    }

    /// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
    fn poll_until_complete(d: &CudaDevice, t: &SubmitToken) {
        let start = Instant::now();
        loop {
            match d.poll(copy_of(t)) {
                CompletionState::Complete => return,
                CompletionState::Pending => {}
                other => panic!("unexpected state: {other:?}"),
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the token did not complete in time"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// 受けた状態と呼ばれたスレッドを積む callback と、その記録を返す。
    fn recorder() -> (Recorded, Box<dyn FnOnce(CompletionState) + Send>) {
        let states = Arc::new(Mutex::new(Vec::new()));
        let threads = Arc::new(Mutex::new(Vec::new()));
        let (s, t) = (Arc::clone(&states), Arc::clone(&threads));
        let callback = Box::new(move |state| {
            s.lock().unwrap().push(state);
            t.lock().unwrap().push(std::thread::current().id());
        });
        (Recorded { states, threads }, callback)
    }

    /// 登録の表の件数。
    fn registered(d: &CudaDevice) -> usize {
        d.lock_submissions().callbacks.len()
    }

    // CMD-33
    /// 回収済みの印は、登録の呼び出しの中で `Complete` を 1 回渡し、登録しない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_reclaimed_token_calls_back_complete_once_inside_the_call() {
        let d = device();
        let t = submit_empty(&d);
        poll_until_complete(&d, &t);
        let (rec, cb) = recorder();
        d.on_complete(copy_of(&t), cb);
        assert_eq!(rec.states(), vec![CompletionState::Complete]);
        assert_eq!(rec.threads(), vec![std::thread::current().id()]);
        assert_eq!(registered(&d), 0);
        d.poll(copy_of(&t));
        d.poll(copy_of(&t));
        assert_eq!(rec.states().len(), 1);
    }

    // CMD-33
    /// 別 device の印は、登録の呼び出しの中で `Lost` を 1 回渡し、登録しない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_foreign_token_calls_back_lost_once_without_registering() {
        let a = device();
        let b = device();
        let ta = submit_empty(&a);
        let before = b.test_submitted_count();
        let (rec, cb) = recorder();
        b.on_complete(copy_of(&ta), cb);
        assert_eq!(rec.states(), vec![CompletionState::Lost]);
        assert_eq!(registered(&b), 0);
        assert_eq!(b.test_submitted_count(), before);
        let tb = submit_empty(&b);
        poll_until_complete(&b, &tb);
        assert_eq!(rec.states().len(), 1);
        poll_until_complete(&a, &ta);
    }

    // CMD-33
    /// 払い出していない番号の印は、登録の呼び出しの中で `Lost` を 1 回渡し、登録しない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn an_unissued_sequence_calls_back_lost_once_without_registering() {
        let d = device();
        let t = submit_empty(&d);
        let unissued =
            SubmitToken::__backend_new(t.generation, t.sequence + 1, t.__backend_device_id());
        let (rec, cb) = recorder();
        d.on_complete(unissued, cb);
        assert_eq!(rec.states(), vec![CompletionState::Lost]);
        assert_eq!(registered(&d), 0);
        let t2 = submit_empty(&d);
        poll_until_complete(&d, &t2);
        assert_eq!(rec.states().len(), 1);
        poll_until_complete(&d, &t);
    }

    // CMD-32
    /// 投入した印の callback は `Complete` をちょうど 1 回受け、`Pending` を受けない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_submitted_token_calls_back_complete_exactly_once_and_never_pending() {
        let d = device();
        let t1 = submit_empty(&d);
        let (rec1, cb1) = recorder();
        d.on_complete(copy_of(&t1), cb1);
        poll_until_complete(&d, &t1);
        d.poll(copy_of(&t1));
        let t2 = submit_empty(&d);
        let (_rec2, cb2) = recorder();
        d.on_complete(copy_of(&t2), cb2);
        poll_until_complete(&d, &t2);
        assert_eq!(rec1.states(), vec![CompletionState::Complete]);
        assert_eq!(rec1.threads(), vec![std::thread::current().id()]);
        assert_eq!(registered(&d), 0);
    }

    // CMD-85
    /// 通常の操作では、登録した検証の報告の関数は呼ばれない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn the_validation_callback_is_not_called_by_normal_operations() {
        let d = device();
        let mut counts: Vec<Arc<AtomicU64>> = Vec::new();
        for _ in 0..2 {
            let count = Arc::new(AtomicU64::new(0));
            counts.push(Arc::clone(&count));
            let c = Arc::clone(&count);
            let callback: ValidationCallback = Box::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            });
            ValidationIntake::set_validation_callback(&d, callback);
            let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
            let h = d.allocate(16, usage).expect("allocate");
            d.upload(&h, 0, &[0u8; 16]).expect("upload");
            let t = submit_empty(&d);
            poll_until_complete(&d, &t);
            let (_rec, cb) = recorder();
            d.on_complete(copy_of(&t), cb);
            d.free(h);
            for c in &counts {
                assert_eq!(c.load(Ordering::SeqCst), 0);
            }
            assert_eq!(d.validation.delivered_count(), 0);
            assert_eq!(d.validation.undelivered_count(), 0);
        }
    }
}

/// 門の部品（`GateLatch`・`GateState`）の device に触らない試験（TASK-345）。
#[cfg(all(test, feature = "fault-injection"))]
mod gate_tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    /// 積む手段の代わり。呼ばれた回数を数え、渡された門を写す。
    fn recording<'a>(
        calls: &'a Cell<u32>,
        seen: &'a RefCell<Vec<Arc<GateLatch>>>,
    ) -> impl FnOnce(&Arc<GateLatch>) -> Result<(), ()> + 'a {
        move |latch| {
            calls.set(calls.get() + 1);
            seen.borrow_mut().push(Arc::clone(latch));
            Ok(())
        }
    }

    #[test]
    fn attaching_without_an_armed_gate_launches_nothing() {
        let state = GateState::default();
        let calls = Cell::new(0);
        let seen = RefCell::new(Vec::new());
        assert_eq!(state.attach_with(recording(&calls, &seen)), Ok(()));
        assert_eq!(calls.get(), 0);
        assert_eq!(state.issued_len(), 0);
    }

    #[test]
    fn arming_twice_launches_one_gate_for_the_next_submission_only() {
        let state = GateState::default();
        let calls = Cell::new(0);
        let seen = RefCell::new(Vec::new());
        state.arm();
        state.arm();
        assert_eq!(state.attach_with(recording(&calls, &seen)), Ok(()));
        assert_eq!(state.attach_with(recording(&calls, &seen)), Ok(()));
        assert_eq!(calls.get(), 1);
        assert!(!state.is_armed());
        assert_eq!(state.issued_len(), 1);
        assert!(!seen.borrow()[0].is_open());
    }

    #[test]
    fn opening_before_the_submission_clears_the_armed_gate() {
        let state = GateState::default();
        let calls = Cell::new(0);
        let seen = RefCell::new(Vec::new());
        state.arm();
        state.open_all();
        assert_eq!(state.attach_with(recording(&calls, &seen)), Ok(()));
        assert_eq!(calls.get(), 0);
        assert!(!state.is_armed());
    }

    #[test]
    fn a_failed_launch_keeps_the_armed_gate() {
        let state = GateState::default();
        let failed: RefCell<Vec<Arc<GateLatch>>> = RefCell::new(Vec::new());
        state.arm();
        let result = state.attach_with(|latch| {
            failed.borrow_mut().push(Arc::clone(latch));
            Err("launch failed")
        });
        assert_eq!(result, Err("launch failed"));
        assert!(state.is_armed());
        assert_eq!(state.issued_len(), 0);

        let calls = Cell::new(0);
        let seen = RefCell::new(Vec::new());
        let retried: Result<(), &str> = state.attach_with(|latch| {
            calls.set(calls.get() + 1);
            seen.borrow_mut().push(Arc::clone(latch));
            Ok(())
        });
        assert_eq!(retried, Ok(()));
        assert_eq!(calls.get(), 1);
        assert!(Arc::ptr_eq(&failed.borrow()[0], &seen.borrow()[0]));

        state.open_all();
        assert!(failed.borrow()[0].is_open());
    }

    #[test]
    fn opening_opens_every_issued_gate_and_is_idempotent() {
        let state = GateState::default();
        let calls = Cell::new(0);
        let seen = RefCell::new(Vec::new());
        for _ in 0..2 {
            state.arm();
            assert_eq!(state.attach_with(recording(&calls, &seen)), Ok(()));
        }
        assert_eq!(state.issued_len(), 2);
        state.open_all();
        assert!(seen.borrow().iter().all(|latch| latch.is_open()));
        assert_eq!(state.issued_len(), 0);
        state.open_all();
        assert!(seen.borrow().iter().all(|latch| latch.is_open()));
    }

    #[test]
    fn an_opened_gate_is_dropped_from_the_issued_list_on_the_next_attach() {
        let state = GateState::default();
        let calls = Cell::new(0);
        let seen = RefCell::new(Vec::new());
        state.arm();
        assert_eq!(state.attach_with(recording(&calls, &seen)), Ok(()));
        seen.borrow()[0].open();
        state.arm();
        assert_eq!(state.attach_with(recording(&calls, &seen)), Ok(()));
        assert_eq!(state.issued_len(), 1);
    }

    #[test]
    fn waiting_on_a_closed_gate_returns_false_after_the_limit() {
        let latch = GateLatch::default();
        assert!(!latch.wait_open(Duration::from_millis(10)));
        assert!(!latch.is_open());
    }

    #[test]
    fn waiting_returns_true_once_another_thread_opens_the_gate() {
        let latch = Arc::new(GateLatch::default());
        let opener = {
            let latch = Arc::clone(&latch);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                latch.open();
            })
        };
        assert!(latch.wait_open(Duration::from_secs(10)));
        opener.join().expect("opener thread");
    }
}

/// 門で止めた投入の、CUDA の device に触る試験（TASK-345 のポインタ）。
#[cfg(all(test, feature = "fault-injection"))]
mod gate_device_tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// 門を仕込んだ後、`poll` が `Pending` を返す側へ寄せるための待ち。
    const SETTLE: Duration = Duration::from_millis(50);

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 印の写しを返す。
    fn copy_of(t: &SubmitToken) -> SubmitToken {
        SubmitToken::__backend_new(t.generation, t.sequence, t.__backend_device_id())
    }

    /// 空の記録先を投入して印を返す。
    fn submit_empty(d: &CudaDevice) -> SubmitToken {
        d.submit(d.create_encoder(), &[]).expect("submit")
    }

    /// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
    fn poll_until_complete(d: &CudaDevice, t: &SubmitToken) {
        let start = Instant::now();
        loop {
            match d.poll(copy_of(t)) {
                CompletionState::Complete => return,
                CompletionState::Pending => {}
                other => panic!("unexpected state: {other:?}"),
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the token did not complete in time"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// 門で止めた投入は、開けるまで `Pending` のまま。開けると上限より早く完了する。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_gated_submission_stays_pending_until_the_gate_opens() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        let t = submit_empty(&d);
        std::thread::sleep(SETTLE);
        assert_eq!(d.poll(copy_of(&t)), CompletionState::Pending);
        let start = Instant::now();
        assert_eq!(d.open_gate(), Ok(()));
        poll_until_complete(&d, &t);
        assert!(start.elapsed() < GATE_EXPIRY / 2);
        assert_eq!(d.open_gate(), Ok(()));
    }

    /// 門で止めた投入の後ろの投入も、門が開くまで `Pending` のまま。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_later_submission_waits_behind_the_gate() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        let first = submit_empty(&d);
        let second = submit_empty(&d);
        std::thread::sleep(SETTLE);
        assert_eq!(d.poll(copy_of(&second)), CompletionState::Pending);
        assert_eq!(d.open_gate(), Ok(()));
        poll_until_complete(&d, &first);
        poll_until_complete(&d, &second);
    }

    /// 開けない門は上限が過ぎると戻り、投入は完了する。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_closed_gate_expires_and_the_submission_completes() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        let t = submit_empty(&d);
        std::thread::sleep(SETTLE);
        assert_eq!(d.poll(copy_of(&t)), CompletionState::Pending);
        poll_until_complete(&d, &t);
    }

    /// 投入の前に開けた門は積まれず、投入は上限を待たずに完了する。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn opening_before_the_submission_leaves_it_ungated() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        assert_eq!(d.open_gate(), Ok(()));
        let start = Instant::now();
        let t = submit_empty(&d);
        poll_until_complete(&d, &t);
        assert!(start.elapsed() < GATE_EXPIRY / 2);
    }

    /// 閉じた門を積んだままの `Drop` は、上限を待たずに戻る。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dropping_with_a_closed_gate_returns_before_the_expiry() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        let _token = submit_empty(&d);
        let start = Instant::now();
        drop(d);
        assert!(start.elapsed() < GATE_EXPIRY / 2);
    }

    /// 閉じた門を積んだままの `destroy` は、上限を待たずに戻る。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn destroying_with_a_closed_gate_returns_before_the_expiry() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        let _token = submit_empty(&d);
        let start = Instant::now();
        Box::new(d).destroy();
        assert!(start.elapsed() < GATE_EXPIRY / 2);
    }
}

/// 門で止めた投入に登録した完了の callback の、CUDA の device に触る試験。device の無い環境では流さない（TASK-607 のポインタ）。
#[cfg(all(test, feature = "fault-injection"))]
mod on_complete_gate_device_tests {
    use super::*;
    use std::thread::ThreadId;
    use std::time::{Duration, Instant};

    /// 門を仕込んだ後、`poll` が `Pending` を返す側へ寄せるための待ち。
    const SETTLE: Duration = Duration::from_millis(50);

    /// callback が受けた状態と、呼ばれたスレッドの記録。
    struct Recorded {
        states: Arc<Mutex<Vec<CompletionState>>>,
        threads: Arc<Mutex<Vec<ThreadId>>>,
    }

    impl Recorded {
        fn states(&self) -> Vec<CompletionState> {
            self.states.lock().unwrap().clone()
        }

        fn threads(&self) -> Vec<ThreadId> {
            self.threads.lock().unwrap().clone()
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 印の写しを返す。
    fn copy_of(t: &SubmitToken) -> SubmitToken {
        SubmitToken::__backend_new(t.generation, t.sequence, t.__backend_device_id())
    }

    /// 空の記録先を投入して印を返す。
    fn submit_empty(d: &CudaDevice) -> SubmitToken {
        d.submit(d.create_encoder(), &[]).expect("submit")
    }

    /// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
    fn poll_until_complete(d: &CudaDevice, t: &SubmitToken) {
        let start = Instant::now();
        loop {
            match d.poll(copy_of(t)) {
                CompletionState::Complete => return,
                CompletionState::Pending => {}
                other => panic!("unexpected state: {other:?}"),
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the token did not complete in time"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// 受けた状態と呼ばれたスレッドを積む callback と、その記録を返す。
    fn recorder() -> (Recorded, Box<dyn FnOnce(CompletionState) + Send>) {
        let states = Arc::new(Mutex::new(Vec::new()));
        let threads = Arc::new(Mutex::new(Vec::new()));
        let (s, t) = (Arc::clone(&states), Arc::clone(&threads));
        let callback = Box::new(move |state| {
            s.lock().unwrap().push(state);
            t.lock().unwrap().push(std::thread::current().id());
        });
        (Recorded { states, threads }, callback)
    }

    /// 登録の表の件数。
    fn registered(d: &CudaDevice) -> usize {
        d.lock_submissions().callbacks.len()
    }

    // CMD-32
    /// 門で止めた投入に登録した callback は、登録・`poll`・門の開けでは呼ばれず、完了の後の `poll` の中で `Complete` を 1 回受ける。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_callback_registered_on_a_gated_token_is_called_complete_once_by_a_later_poll() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        let t = submit_empty(&d);
        std::thread::sleep(SETTLE);
        assert_eq!(d.poll(copy_of(&t)), CompletionState::Pending);
        let (rec, cb) = recorder();
        d.on_complete(copy_of(&t), cb);
        assert_eq!(rec.states(), Vec::<CompletionState>::new());
        assert_eq!(registered(&d), 1);
        assert_eq!(d.poll(copy_of(&t)), CompletionState::Pending);
        assert_eq!(rec.states(), Vec::<CompletionState>::new());
        assert_eq!(d.open_gate(), Ok(()));
        assert_eq!(rec.states(), Vec::<CompletionState>::new());
        poll_until_complete(&d, &t);
        assert_eq!(rec.states(), vec![CompletionState::Complete]);
        assert_eq!(rec.threads(), vec![std::thread::current().id()]);
        assert_eq!(registered(&d), 0);
        d.poll(copy_of(&t));
        assert_eq!(rec.states().len(), 1);
    }

    // CMD-34
    /// 門で止めた投入に登録した callback は、模擬のロストの呼び出しの中で `Lost` を 1 回受け、門を開けた後も破棄の後も再び呼ばれない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_callback_registered_on_a_gated_token_is_called_lost_once_by_a_simulated_loss() {
        let d = device();
        assert_eq!(d.arm_gate(), Ok(()));
        let t = submit_empty(&d);
        std::thread::sleep(SETTLE);
        assert_eq!(d.poll(copy_of(&t)), CompletionState::Pending);
        let (rec, cb) = recorder();
        d.on_complete(copy_of(&t), cb);
        assert_eq!(rec.states(), Vec::<CompletionState>::new());
        assert_eq!(registered(&d), 1);
        assert_eq!(d.simulate_device_lost(DeviceLostReason::Unknown), Ok(()));
        assert_eq!(rec.states(), vec![CompletionState::Lost]);
        assert_eq!(rec.threads(), vec![std::thread::current().id()]);
        assert_eq!(registered(&d), 0);
        assert_eq!(d.open_gate(), Ok(()));
        std::thread::sleep(SETTLE);
        assert_eq!(d.poll(copy_of(&t)), CompletionState::Lost);
        drop(d);
        assert_eq!(rec.states(), vec![CompletionState::Lost]);
    }
}

/// バッファのコピー・クリアの記録の時点の照らし・投入の時点の組み立て・解放の保留の部品の、device に触らない試験（TASK-624 のポインタ）。
#[cfg(test)]
mod copy_clear_tests {
    use super::*;

    const LEN: u64 = 64;
    const GEN: u64 = 1;

    /// 表に項目があり、世代・払い出しの世代・大きさが合う、同じ device のハンドル。
    fn operand(buffer_id: u64, usage: BufferUsage) -> TransferOperand {
        TransferOperand {
            same_device: true,
            generation_current: true,
            entry: Some(TransferEntry {
                buffer_id,
                generation: GEN,
                issued_matches: true,
                size_matches: true,
                usage,
                allocation_len: LEN,
            }),
        }
    }

    fn src() -> TransferOperand {
        operand(1, BufferUsage::DOWNLOAD)
    }

    fn dst() -> TransferOperand {
        operand(2, BufferUsage::UPLOAD)
    }

    fn other_device(mut o: TransferOperand) -> TransferOperand {
        o.same_device = false;
        o
    }

    fn stale_generation(mut o: TransferOperand) -> TransferOperand {
        o.generation_current = false;
        o
    }

    fn without_entry(mut o: TransferOperand) -> TransferOperand {
        o.entry = None;
        o
    }

    fn other_issue(mut o: TransferOperand) -> TransferOperand {
        if let Some(entry) = o.entry.as_mut() {
            entry.issued_matches = false;
        }
        o
    }

    fn wrong_size(mut o: TransferOperand) -> TransferOperand {
        if let Some(entry) = o.entry.as_mut() {
            entry.size_matches = false;
        }
        o
    }

    fn with_usage(mut o: TransferOperand, usage: BufferUsage) -> TransferOperand {
        if let Some(entry) = o.entry.as_mut() {
            entry.usage = usage;
        }
        o
    }

    fn assert_invalid<T: std::fmt::Debug>(result: Result<T, ContractError>, needle: &str) {
        match result {
            Err(ContractError::InvalidArgument(msg)) => {
                assert!(msg.contains(needle), "{msg:?} lacks {needle:?}");
            }
            other => panic!("expected InvalidArgument containing {needle:?}, got {other:?}"),
        }
    }

    fn assert_mismatch<T: std::fmt::Debug>(result: Result<T, ContractError>) {
        assert!(
            matches!(result, Err(ContractError::DeviceMismatch)),
            "{result:?}"
        );
    }

    fn recorded(buffer_id: u64) -> RecordedBuffer {
        RecordedBuffer {
            buffer_id,
            generation: GEN,
        }
    }

    #[test]
    fn copy_passes_and_records_the_operands() {
        let record = check_copy_record(src(), 4, dst(), 8, 16).expect("copy");
        assert_eq!(
            record,
            Some(TransferRecord::Copy {
                src: recorded(1),
                src_offset: 4,
                dst: recorded(2),
                dst_offset: 8,
                size: 16,
            })
        );
    }

    #[test]
    fn clear_passes_and_records_the_operand() {
        let record = check_clear_record(dst(), 4, 16).expect("clear");
        assert_eq!(
            record,
            Some(TransferRecord::Clear {
                buffer: recorded(2),
                offset: 4,
                size: 16,
            })
        );
    }

    #[test]
    fn copy_rejects_each_row() {
        assert_mismatch(check_copy_record(other_device(src()), 0, dst(), 0, 4));
        assert_mismatch(check_copy_record(src(), 0, other_device(dst()), 0, 4));
        assert_invalid(
            check_copy_record(stale_generation(src()), 0, dst(), 0, 4),
            "generation",
        );
        assert_invalid(
            check_copy_record(src(), 0, stale_generation(dst()), 0, 4),
            "generation",
        );
        assert_invalid(
            check_copy_record(without_entry(src()), 0, dst(), 0, 4),
            "unknown buffer handle",
        );
        assert_invalid(
            check_copy_record(src(), 0, without_entry(dst()), 0, 4),
            "unknown buffer handle",
        );
        assert_invalid(
            check_copy_record(other_issue(src()), 0, dst(), 0, 4),
            "unknown buffer handle",
        );
        assert_invalid(
            check_copy_record(src(), 0, other_issue(dst()), 0, 4),
            "unknown buffer handle",
        );
        assert_invalid(
            check_copy_record(wrong_size(src()), 0, dst(), 0, 4),
            "size does not match",
        );
        assert_invalid(
            check_copy_record(src(), 0, wrong_size(dst()), 0, 4),
            "size does not match",
        );
        assert_invalid(
            check_copy_record(with_usage(src(), BufferUsage::UPLOAD), 0, dst(), 0, 4),
            "DOWNLOAD",
        );
        assert_invalid(
            check_copy_record(src(), 0, with_usage(dst(), BufferUsage::DOWNLOAD), 0, 4),
            "UPLOAD",
        );
        assert_invalid(
            check_copy_record(src(), LEN, dst(), 0, 4),
            "buffer range end",
        );
        assert_invalid(
            check_copy_record(src(), 0, dst(), LEN - 4, 8),
            "buffer range end",
        );
        assert_invalid(check_copy_record(src(), 2, dst(), 0, 4), "multiples");
        assert_invalid(check_copy_record(src(), 0, dst(), 6, 4), "multiples");
        assert_invalid(check_copy_record(src(), 0, dst(), 0, 6), "multiples");
        let same = operand(1, BufferUsage::DOWNLOAD | BufferUsage::UPLOAD);
        assert_invalid(check_copy_record(same, 0, same, 32, 4), "different buffers");
    }

    #[test]
    fn clear_rejects_each_row() {
        assert_mismatch(check_clear_record(other_device(dst()), 0, 4));
        assert_invalid(
            check_clear_record(stale_generation(dst()), 0, 4),
            "generation",
        );
        assert_invalid(
            check_clear_record(without_entry(dst()), 0, 4),
            "unknown buffer handle",
        );
        assert_invalid(
            check_clear_record(other_issue(dst()), 0, 4),
            "unknown buffer handle",
        );
        assert_invalid(
            check_clear_record(wrong_size(dst()), 0, 4),
            "size does not match",
        );
        assert_invalid(
            check_clear_record(with_usage(dst(), BufferUsage::DOWNLOAD), 0, 4),
            "UPLOAD",
        );
        assert_invalid(check_clear_record(dst(), LEN, 4), "buffer range end");
        assert_invalid(check_clear_record(dst(), 2, 4), "multiples");
        assert_invalid(check_clear_record(dst(), 0, 6), "multiples");
    }

    #[test]
    fn copy_reports_the_earlier_row_when_two_adjacent_rows_apply() {
        // device と世代。
        assert_mismatch(check_copy_record(
            other_device(stale_generation(src())),
            0,
            dst(),
            0,
            4,
        ));
        // 世代と表。
        assert_invalid(
            check_copy_record(stale_generation(without_entry(src())), 0, dst(), 0, 4),
            "generation",
        );
        // 表と大きさ。
        assert_invalid(
            check_copy_record(other_issue(wrong_size(src())), 0, dst(), 0, 4),
            "unknown buffer handle",
        );
        // 大きさと用途。
        assert_invalid(
            check_copy_record(
                wrong_size(with_usage(src(), BufferUsage::UPLOAD)),
                0,
                dst(),
                0,
                4,
            ),
            "size does not match",
        );
        // 用途と範囲。
        assert_invalid(
            check_copy_record(with_usage(src(), BufferUsage::UPLOAD), LEN, dst(), 0, 4),
            "DOWNLOAD",
        );
        // 範囲と整列。
        assert_invalid(
            check_copy_record(src(), LEN + 1, dst(), 0, 4),
            "buffer range end",
        );
        // 整列と同じバッファ。
        let same = operand(1, BufferUsage::DOWNLOAD | BufferUsage::UPLOAD);
        assert_invalid(check_copy_record(same, 2, same, 2, 4), "multiples");
        // 同じバッファと長さ 0。
        assert_invalid(check_copy_record(same, 0, same, 0, 0), "different buffers");
        // 行の中は元が先: 元の表に無く、先の世代が古い。
        assert_invalid(
            check_copy_record(without_entry(src()), 0, stale_generation(dst()), 0, 4),
            "unknown buffer handle",
        );
        // 行の中は元が先: 元の用途が足りず、先の用途も足りない。
        assert_invalid(
            check_copy_record(
                with_usage(src(), BufferUsage::UPLOAD),
                0,
                with_usage(dst(), BufferUsage::DOWNLOAD),
                0,
                4,
            ),
            "DOWNLOAD",
        );
    }

    #[test]
    fn clear_reports_the_earlier_row_when_two_adjacent_rows_apply() {
        assert_mismatch(check_clear_record(
            other_device(stale_generation(dst())),
            0,
            4,
        ));
        assert_invalid(
            check_clear_record(stale_generation(without_entry(dst())), 0, 4),
            "generation",
        );
        assert_invalid(
            check_clear_record(other_issue(wrong_size(dst())), 0, 4),
            "unknown buffer handle",
        );
        assert_invalid(
            check_clear_record(wrong_size(with_usage(dst(), BufferUsage::DOWNLOAD)), 0, 4),
            "size does not match",
        );
        assert_invalid(
            check_clear_record(with_usage(dst(), BufferUsage::DOWNLOAD), LEN, 4),
            "UPLOAD",
        );
        assert_invalid(check_clear_record(dst(), LEN + 1, 4), "buffer range end");
        assert_invalid(check_clear_record(dst(), LEN - 2, 0), "multiples");
    }

    #[test]
    fn u64_overflow_is_rejected() {
        assert_invalid(
            check_copy_record(src(), u64::MAX - 3, dst(), 0, 8),
            "overflows",
        );
        assert_invalid(
            check_copy_record(src(), 0, dst(), u64::MAX - 3, 8),
            "overflows",
        );
        assert_invalid(check_clear_record(dst(), u64::MAX - 3, 8), "overflows");
    }

    #[test]
    fn zero_length_records_nothing_unless_an_earlier_row_applies() {
        assert_eq!(check_copy_record(src(), 0, dst(), 0, 0).unwrap(), None);
        assert_eq!(check_copy_record(src(), LEN, dst(), 0, 0).unwrap(), None);
        assert_eq!(check_clear_record(dst(), 0, 0).unwrap(), None);
        assert_mismatch(check_copy_record(other_device(src()), 0, dst(), 0, 0));
        assert_invalid(
            check_copy_record(without_entry(src()), 0, dst(), 0, 0),
            "unknown buffer handle",
        );
        assert_invalid(
            check_copy_record(with_usage(src(), BufferUsage::UPLOAD), 0, dst(), 0, 0),
            "DOWNLOAD",
        );
        assert_invalid(
            check_copy_record(src(), LEN + 4, dst(), 0, 0),
            "buffer range end",
        );
        assert_invalid(check_copy_record(src(), 2, dst(), 0, 0), "multiples");
        assert_invalid(
            check_clear_record(with_usage(dst(), BufferUsage::DOWNLOAD), 0, 0),
            "UPLOAD",
        );
        assert_invalid(check_clear_record(dst(), LEN + 4, 0), "buffer range end");
        assert_invalid(check_clear_record(dst(), 2, 0), "multiples");
    }

    #[test]
    fn usage_rejections_name_the_operation_and_the_missing_usage() {
        for (result, usage) in [
            (
                check_copy_record(with_usage(src(), BufferUsage::UPLOAD), 0, dst(), 0, 4),
                "DOWNLOAD",
            ),
            (
                check_copy_record(src(), 0, with_usage(dst(), BufferUsage::DOWNLOAD), 0, 4),
                "UPLOAD",
            ),
        ] {
            assert_invalid(result.clone(), "copy_buffer_to_buffer");
            assert_invalid(result, usage);
        }
        let cleared = check_clear_record(with_usage(dst(), BufferUsage::DOWNLOAD), 0, 4);
        assert_invalid(cleared.clone(), "clear_buffer");
        assert_invalid(cleared, "UPLOAD");
    }

    fn copy_record(src: u64, dst: u64) -> TransferRecord {
        TransferRecord::Copy {
            src: recorded(src),
            src_offset: 4,
            dst: recorded(dst),
            dst_offset: 8,
            size: 16,
        }
    }

    fn clear_record(buffer: u64, offset: u64, size: u64) -> TransferRecord {
        TransferRecord::Clear {
            buffer: recorded(buffer),
            offset,
            size,
        }
    }

    /// 識別子 1〜3 が世代 `GEN`・長さ `LEN` で表にある。
    fn table(id: u64) -> Option<(u64, u64)> {
        (1..=3).contains(&id).then_some((GEN, LEN))
    }

    #[test]
    fn plan_keeps_the_record_order_and_converts_the_ranges() {
        let records = [copy_record(1, 2), clear_record(3, 0, 4), copy_record(2, 1)];
        let jobs = plan_transfers(&records, table, GEN).expect("plan");
        assert_eq!(
            jobs,
            vec![
                TransferJob::Copy {
                    src: 1,
                    src_range: (4, 20),
                    dst: 2,
                    dst_range: (8, 24),
                },
                TransferJob::Clear {
                    buffer: 3,
                    range: (0, 4),
                },
                TransferJob::Copy {
                    src: 2,
                    src_range: (4, 20),
                    dst: 1,
                    dst_range: (8, 24),
                },
            ]
        );
        assert_eq!(plan_transfers(&[], table, GEN).expect("empty"), vec![]);
    }

    #[test]
    fn plan_rejects_a_freed_or_stale_buffer_before_any_range() {
        let freed = "freed before submit";
        for records in [
            vec![copy_record(9, 2)],
            vec![copy_record(1, 9)],
            vec![clear_record(9, 0, 4)],
        ] {
            assert_invalid(plan_transfers(&records, table, GEN), freed);
        }
        // 表の項目の払い出しの世代が記録の世代と違う。
        let reissued = |id: u64| (1..=3).contains(&id).then_some((GEN + 1, LEN));
        assert_invalid(plan_transfers(&[copy_record(1, 2)], reissued, GEN), freed);
        // 記録の世代が今の世代と違う。
        assert_invalid(plan_transfers(&[copy_record(1, 2)], table, GEN + 1), freed);
        // 前の記録の範囲が外れていても、後ろの解放済みを先に返す。
        assert_invalid(
            plan_transfers(
                &[clear_record(1, LEN, 4), clear_record(9, 0, 4)],
                table,
                GEN,
            ),
            freed,
        );
        // 解放済みが無ければ、表の長さで範囲を照らし直す。
        assert_invalid(
            plan_transfers(&[clear_record(1, LEN, 4)], table, GEN),
            "exceeds the buffer",
        );
        let short = |id: u64| (1..=3).contains(&id).then_some((GEN, 8));
        assert_invalid(
            plan_transfers(&[copy_record(1, 2)], short, GEN),
            "exceeds the buffer",
        );
        // あふれる範囲は拒む。
        assert_invalid(
            plan_transfers(&[clear_record(1, u64::MAX - 3, 8)], table, GEN),
            "exceeds the buffer",
        );
    }

    #[test]
    fn jobs_name_the_buffers_they_touch() {
        let copy = TransferJob::Copy {
            src: 1,
            src_range: (0, 4),
            dst: 2,
            dst_range: (0, 4),
        };
        let clear = TransferJob::Clear {
            buffer: 3,
            range: (0, 4),
        };
        assert_eq!(copy.buffer_ids(), vec![1, 2]);
        assert_eq!(clear.buffer_ids(), vec![3]);
        assert_eq!(copy_record(1, 2).buffers(), vec![recorded(1), recorded(2)]);
        assert_eq!(clear_record(3, 0, 4).buffers(), vec![recorded(3)]);
    }

    #[test]
    fn open_encoders_record_only_to_open_ids_and_claim_returns_the_records_in_order() {
        let open = OpenEncoders::default();
        open.open(0);
        assert!(!open.is_open(0));
        assert!(matches!(
            open.record(0, clear_record(1, 0, 4)),
            Err(ContractError::InvalidArgument(_))
        ));
        assert!(matches!(
            open.record(7, clear_record(1, 0, 4)),
            Err(ContractError::InvalidArgument(_))
        ));
        assert!(open.lock().is_empty());
        open.open(5);
        open.open(6);
        assert_eq!(open.record(5, clear_record(1, 0, 4)), Ok(()));
        assert_eq!(open.record(6, clear_record(2, 0, 4)), Ok(()));
        assert_eq!(open.record(5, clear_record(3, 0, 4)), Ok(()));
        for ids in [&[0_u64][..], &[7], &[5, 5], &[5, 7]] {
            assert!(matches!(
                open.claim(ids),
                Err(ContractError::InvalidArgument(_))
            ));
            assert_eq!(open.lock().len(), 2);
            assert!(open.is_open(5) && open.is_open(6));
        }
        assert_eq!(
            open.claim(&[6, 5]),
            Ok(vec![
                clear_record(2, 0, 4),
                clear_record(1, 0, 4),
                clear_record(3, 0, 4),
            ])
        );
        assert!(open.lock().is_empty());
        // 破棄された記録先は、記録ごと表から外れる。
        open.open(9);
        assert_eq!(open.record(9, clear_record(1, 0, 4)), Ok(()));
        open.close(9);
        assert!(!open.is_open(9));
        assert!(matches!(
            open.record(9, clear_record(1, 0, 4)),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[test]
    fn pair_mut_borrows_two_distinct_entries() {
        let mut map: BTreeMap<u64, i32> = BTreeMap::from([(1, 10), (2, 20), (3, 30)]);
        {
            let (shared, unique) = pair_mut(&mut map, 3, 1).expect("pair");
            assert_eq!(*shared, 30);
            *unique += *shared;
        }
        assert_eq!(map[&1], 40);
        assert_eq!(map[&3], 30);
        assert!(pair_mut(&mut map, 2, 2).is_none());
        assert!(pair_mut(&mut map, 9, 1).is_none());
        assert!(pair_mut(&mut map, 1, 9).is_none());
    }

    #[test]
    fn hold_or_release_holds_only_while_the_last_submission_is_unfinished() {
        let mut pending = PendingReleases::<u32>::default();
        let unfinished = |sequence: u64| sequence == 5;
        assert_eq!(hold_or_release(1, None, unfinished, &mut pending), Some(1));
        assert_eq!(pending.len(), 0);
        assert_eq!(hold_or_release(2, Some(5), unfinished, &mut pending), None);
        assert_eq!(pending.len(), 1);
        assert_eq!(
            hold_or_release(3, Some(6), unfinished, &mut pending),
            Some(3)
        );
        assert_eq!(pending.len(), 1);
        assert_eq!(
            hold_or_release(4, Some(5), |_| false, &mut pending),
            Some(4)
        );
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn pending_releases_take_finished_keeps_the_unfinished_in_order() {
        let mut pending = PendingReleases::<char>::default();
        pending.push(3, 'a');
        pending.push(1, 'b');
        pending.push(2, 'c');
        pending.push(4, 'd');
        assert_eq!(pending.len(), 4);
        let finished = pending.take_finished(|sequence| sequence == 3 || sequence == 4);
        assert_eq!(finished, vec!['b', 'c']);
        assert_eq!(pending.len(), 2);
        assert!(pending.take_finished(|_| true).is_empty());
        assert_eq!(pending.len(), 2);
        assert_eq!(pending.take_finished(|_| false), vec!['a', 'd']);
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn a_sequence_stays_unfinished_until_reclaim_removes_it() {
        let mut subs = Submissions::<GroupState>::new();
        for state in [
            GroupState::Completed,
            GroupState::Failed,
            GroupState::Pending,
            GroupState::Completed,
        ] {
            subs.issue(|| Ok(state)).expect("issue");
        }
        assert!((1..=4).all(|sequence| subs.is_unfinished(sequence)));
        subs.reclaim(|state| *state);
        assert!(!subs.is_unfinished(1));
        assert!(!subs.is_unfinished(2));
        assert!(subs.is_unfinished(3));
        // 先頭が未完了で止まっているので、完了していても残る。
        assert!(subs.is_unfinished(4));
        assert!(!subs.is_unfinished(0));
        assert!(!subs.is_unfinished(5));
    }
}

/// バッファのコピーとクリアの記録と投入の、CUDA の device に触る試験。device の無い環境では流さない（TASK-624 のポインタ）。
#[cfg(all(test, feature = "native-blocking-wait"))]
mod copy_clear_device_tests {
    use super::*;
    use std::time::{Duration, Instant};

    const LEN: u64 = 64;

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 印の写しを返す。
    fn copy_of(t: &SubmitToken) -> SubmitToken {
        SubmitToken::__backend_new(t.generation, t.sequence, t.__backend_device_id())
    }

    /// 読み書きの両方を許す用途。
    fn both() -> BufferUsage {
        BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    /// 0 でない値が並ぶ長さ `LEN` の列。`seed` で並びを変える。
    fn pattern(seed: u8) -> Vec<u8> {
        (0..LEN)
            .map(|i| u8::try_from(i % 200).unwrap_or(0).wrapping_add(seed) | 1)
            .collect()
    }

    /// 確保して `bytes` を先頭から書く。
    fn filled(d: &CudaDevice, bytes: &[u8]) -> BufferHandle {
        let h = d.allocate(LEN, both()).expect("allocate");
        d.upload(&h, 0, bytes).expect("upload");
        h
    }

    /// バッファ全体の読み戻し。
    fn read(d: &CudaDevice, h: &BufferHandle) -> Vec<u8> {
        d.download_blocking(h, 0, LEN).expect("download")
    }

    /// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
    fn poll_until_complete(d: &CudaDevice, t: &SubmitToken) {
        let start = Instant::now();
        loop {
            match d.poll(copy_of(t)) {
                CompletionState::Complete => return,
                CompletionState::Pending => {}
                other => panic!("unexpected state: {other:?}"),
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the token did not complete in time"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// CPU の参照。`dst` の `dst_offset` へ、`src` の `src_offset` からの `size` バイトを写した列。
    fn copied(
        dst: &[u8],
        src: &[u8],
        src_offset: usize,
        dst_offset: usize,
        size: usize,
    ) -> Vec<u8> {
        let mut out = dst.to_vec();
        out[dst_offset..dst_offset + size].copy_from_slice(&src[src_offset..src_offset + size]);
        out
    }

    /// CPU の参照。`bytes` の `offset` からの `size` バイトを 0 にした列。
    fn cleared(bytes: &[u8], offset: usize, size: usize) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out[offset..offset + size].fill(0);
        out
    }

    /// 記録の後・投入の前の読み戻しは元の値のまま。投入の完了の後に写しが現れる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_recorded_copy_takes_effect_only_after_submit() {
        let d = device();
        let src_bytes = pattern(1);
        let zeros = vec![0u8; LEN as usize];
        let src = filled(&d, &src_bytes);
        let dst = d.allocate(LEN, both()).expect("allocate");
        let mut enc = d.create_encoder();
        assert_eq!(
            d.copy_buffer_to_buffer(&mut enc, &src, 8, &dst, 16, 32),
            Ok(())
        );
        assert_eq!(read(&d, &dst), zeros);
        let t = d.submit(enc, &[]).expect("submit");
        poll_until_complete(&d, &t);
        assert_eq!(read(&d, &dst), copied(&zeros, &src_bytes, 8, 16, 32));
        assert_eq!(read(&d, &src), src_bytes);
    }

    /// クリアは範囲だけを 0 にし、効くのは投入の完了の後。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_recorded_clear_zeroes_only_the_range() {
        let d = device();
        let bytes = pattern(2);
        let buf = filled(&d, &bytes);
        let mut enc = d.create_encoder();
        assert_eq!(d.clear_buffer(&mut enc, &buf, 4, 32), Ok(()));
        assert_eq!(read(&d, &buf), bytes);
        let t = d.submit(enc, &[]).expect("submit");
        poll_until_complete(&d, &t);
        assert_eq!(read(&d, &buf), cleared(&bytes, 4, 32));
    }

    /// 1 つの記録先の記録は、記録の順に効く。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn records_take_effect_in_record_order() {
        let d = device();
        let bytes = pattern(3);
        let a = filled(&d, &bytes);
        let b = d.allocate(LEN, both()).expect("allocate");
        let c = d.allocate(LEN, both()).expect("allocate");
        let mut enc = d.create_encoder();
        assert_eq!(d.copy_buffer_to_buffer(&mut enc, &a, 0, &b, 0, LEN), Ok(()));
        assert_eq!(d.clear_buffer(&mut enc, &b, 0, 16), Ok(()));
        assert_eq!(d.copy_buffer_to_buffer(&mut enc, &b, 0, &c, 0, LEN), Ok(()));
        let t = d.submit(enc, &[]).expect("submit");
        poll_until_complete(&d, &t);
        let expected = cleared(&bytes, 0, 16);
        assert_eq!(read(&d, &b), expected);
        assert_eq!(read(&d, &c), expected);
    }

    /// `submit_many` は渡した記録先の順に効く。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn submit_many_runs_each_encoder_in_the_given_order() {
        let d = device();
        let bytes = pattern(4);
        let a = filled(&d, &bytes);
        let b = d.allocate(LEN, both()).expect("allocate");
        let c = d.allocate(LEN, both()).expect("allocate");
        let mut first = d.create_encoder();
        let mut second = d.create_encoder();
        assert_eq!(
            d.copy_buffer_to_buffer(&mut first, &a, 0, &b, 0, LEN),
            Ok(())
        );
        assert_eq!(
            d.copy_buffer_to_buffer(&mut second, &b, 0, &c, 0, LEN),
            Ok(())
        );
        let t = d
            .submit_many(vec![first, second], &[], &[])
            .expect("submit_many");
        poll_until_complete(&d, &t);
        assert_eq!(read(&d, &c), bytes);
    }

    /// 拒まれた記録は記録先を変えず、続く正しい記録は投入で効く。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_rejected_record_leaves_the_encoder_usable() {
        let d = device();
        let bytes = pattern(5);
        let a = filled(&d, &bytes);
        let b = d.allocate(LEN, both()).expect("allocate");
        let download_only = d.allocate(LEN, BufferUsage::DOWNLOAD).expect("allocate");
        let mut enc = d.create_encoder();
        let rejected = [
            // コピーの用途: 先に `UPLOAD` が無い。
            d.copy_buffer_to_buffer(&mut enc, &a, 0, &download_only, 0, 4),
            // コピーの範囲: 元の範囲が終わりを越える。
            d.copy_buffer_to_buffer(&mut enc, &a, 4, &b, 0, LEN),
            // コピーの整列: 長さが 4 の倍数でない。
            d.copy_buffer_to_buffer(&mut enc, &a, 0, &b, 0, 3),
            // コピーの元と先が同じバッファ。
            d.copy_buffer_to_buffer(&mut enc, &a, 0, &a, 0, 4),
            // クリアの用途: `UPLOAD` が無い。
            d.clear_buffer(&mut enc, &download_only, 0, 4),
            // クリアの範囲。
            d.clear_buffer(&mut enc, &a, 0, LEN + 4),
            // クリアの整列: offset が 4 の倍数でない。
            d.clear_buffer(&mut enc, &a, 2, 4),
        ];
        for (index, result) in rejected.iter().enumerate() {
            assert!(
                matches!(result, Err(ContractError::InvalidArgument(_))),
                "record {index}: {result:?}"
            );
        }
        assert_eq!(d.copy_buffer_to_buffer(&mut enc, &a, 0, &b, 0, LEN), Ok(()));
        let t = d.submit(enc, &[]).expect("submit");
        poll_until_complete(&d, &t);
        assert_eq!(read(&d, &b), bytes);
        assert_eq!(read(&d, &download_only), vec![0u8; LEN as usize]);
    }

    /// 長さ 0 のコピーとクリアは `Ok(())` で、何も変えない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_zero_length_record_changes_nothing() {
        let d = device();
        let bytes = pattern(6);
        let a = filled(&d, &bytes);
        let b = d.allocate(LEN, both()).expect("allocate");
        let mut enc = d.create_encoder();
        assert_eq!(d.copy_buffer_to_buffer(&mut enc, &a, 0, &b, 0, 0), Ok(()));
        assert_eq!(d.clear_buffer(&mut enc, &a, 0, 0), Ok(()));
        let t = d.submit(enc, &[]).expect("submit");
        poll_until_complete(&d, &t);
        assert_eq!(read(&d, &a), bytes);
        assert_eq!(read(&d, &b), vec![0u8; LEN as usize]);
    }

    /// 記録の後に元を `free` すると `submit` は拒まれ、投入の数も次の通し番号も進まない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn freeing_a_recorded_buffer_before_submit_rejects_the_submit() {
        let d = device();
        let a = filled(&d, &pattern(7));
        let b = d.allocate(LEN, both()).expect("allocate");
        let mut enc = d.create_encoder();
        assert_eq!(d.copy_buffer_to_buffer(&mut enc, &a, 0, &b, 0, LEN), Ok(()));
        d.free(a);
        let before = d.test_submitted_count();
        let rejected = d.submit(enc, &[]);
        assert!(
            matches!(rejected, Err(ContractError::InvalidArgument(_))),
            "{rejected:?}"
        );
        assert_eq!(d.test_submitted_count(), before);
        let next = d.submit(d.create_encoder(), &[]).expect("submit");
        assert_eq!(next.sequence, before + 1);
        poll_until_complete(&d, &next);
        assert_eq!(read(&d, &b), vec![0u8; LEN as usize]);
    }

    /// 投入の直後に元を `free` しても結果は保たれ、同じ長さの次の確保は全 0。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn freeing_a_buffer_right_after_submit_keeps_the_result() {
        let d = device();
        let bytes = pattern(8);
        let a = filled(&d, &bytes);
        let b = d.allocate(LEN, both()).expect("allocate");
        let mut enc = d.create_encoder();
        assert_eq!(d.copy_buffer_to_buffer(&mut enc, &a, 0, &b, 0, LEN), Ok(()));
        let t = d.submit(enc, &[]).expect("submit");
        d.free(a);
        poll_until_complete(&d, &t);
        assert_eq!(read(&d, &b), bytes);
        let next = d.allocate(LEN, both()).expect("allocate");
        assert_eq!(read(&d, &next), vec![0u8; LEN as usize]);
    }
}

/// 門で止めたコピーに使ったバッファの解放の保留と回収の、CUDA の device に触る試験。device の無い環境では流さない（TASK-624 のポインタ）。
#[cfg(all(test, feature = "fault-injection"))]
mod copy_clear_gate_device_tests {
    use super::*;
    use std::time::{Duration, Instant};

    const LEN: u64 = 64;

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 印の写しを返す。
    fn copy_of(t: &SubmitToken) -> SubmitToken {
        SubmitToken::__backend_new(t.generation, t.sequence, t.__backend_device_id())
    }

    /// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
    fn poll_until_complete(d: &CudaDevice, t: &SubmitToken) {
        let start = Instant::now();
        loop {
            match d.poll(copy_of(t)) {
                CompletionState::Complete => return,
                CompletionState::Pending => {}
                other => panic!("unexpected state: {other:?}"),
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the token did not complete in time"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// 解放を保留しているバッファの数。
    fn held(d: &CudaDevice) -> usize {
        d.lock().pending.len()
    }

    fn usage() -> BufferUsage {
        BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    /// コピーに使ったバッファの `free` だけが投入の完了まで保留され、完了の後に解かれる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn buffers_freed_while_their_copy_is_gated_are_held_until_completion() {
        let d = device();
        let src = d.allocate(LEN, usage()).expect("allocate");
        let dst = d.allocate(LEN, usage()).expect("allocate");
        let unused = d.allocate(LEN, usage()).expect("allocate");
        d.upload(&src, 0, &[1u8; 64]).expect("upload");
        assert_eq!(d.arm_gate(), Ok(()));
        let mut enc = d.create_encoder();
        assert_eq!(
            d.copy_buffer_to_buffer(&mut enc, &src, 0, &dst, 0, LEN),
            Ok(())
        );
        let t = d.submit(enc, &[]).expect("submit");
        d.free(unused);
        assert_eq!(held(&d), 0);
        d.free(src);
        assert_eq!(held(&d), 1);
        d.free(dst);
        assert_eq!(held(&d), 2);
        assert_eq!(d.open_gate(), Ok(()));
        poll_until_complete(&d, &t);
        assert_eq!(d.poll(copy_of(&t)), CompletionState::Complete);
        assert_eq!(held(&d), 0);
    }

    /// 保留したバッファは、`allocate`・`submit`・`submit_many`・`poll` のどれを呼んでも、投入が終わっていれば解かれる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn held_buffers_are_released_at_each_recovery_point() {
        let d = device();
        for point in ["allocate", "submit", "submit_many", "poll"] {
            let src = d.allocate(LEN, usage()).expect("allocate");
            let dst = d.allocate(LEN, usage()).expect("allocate");
            let sync = d.allocate(LEN, usage()).expect("allocate");
            d.upload(&src, 0, &[1u8; 64]).expect("upload");
            assert_eq!(d.arm_gate(), Ok(()));
            let mut enc = d.create_encoder();
            assert_eq!(
                d.copy_buffer_to_buffer(&mut enc, &src, 0, &dst, 0, LEN),
                Ok(())
            );
            let t = d.submit(enc, &[]).expect("submit");
            d.free(src);
            assert_eq!(held(&d), 1, "{point}: before opening");
            assert_eq!(d.open_gate(), Ok(()));
            // 4 バイトの書き込みは stream を同期するが、保留の回収はしない。
            d.upload(&sync, 0, &[2u8; 4]).expect("upload");
            assert_eq!(held(&d), 1, "{point}: after syncing");
            match point {
                "allocate" => {
                    let h = d.allocate(LEN, usage()).expect("allocate");
                    d.free(h);
                }
                "submit" => {
                    d.submit(d.create_encoder(), &[]).expect("submit");
                }
                "submit_many" => {
                    d.submit_many(vec![d.create_encoder()], &[], &[])
                        .expect("submit_many");
                }
                _ => assert_eq!(d.poll(copy_of(&t)), CompletionState::Complete),
            }
            assert_eq!(held(&d), 0, "{point}: after the recovery point");
            d.free(dst);
            d.free(sync);
        }
    }
}

/// 解放の保留の数の数え方の、device に触らない試験（TASK-345 のポインタ）。
#[cfg(test)]
mod pending_release_count_tests {
    use super::*;

    // MEM-12
    /// 同じ投入に使ったバッファも 1 つずつ数え、終わった投入の分だけ減る。
    #[test]
    fn each_held_buffer_counts_once_even_for_the_same_submission() {
        let mut pending = PendingReleases::<char>::default();
        pending.push(7, 'a');
        pending.push(7, 'b');
        pending.push(8, 'c');
        assert_eq!(pending.len(), 3);
        assert_eq!(
            pending.take_finished(|sequence| sequence == 8),
            vec!['a', 'b']
        );
        assert_eq!(pending.len(), 1);
    }
}

/// `wait_blocking` の照らしの順と待ちの部品の、device に触らない試験（TASK-606 のポインタ）。
#[cfg(test)]
mod wait_blocking_tests {
    use super::*;
    use std::cell::Cell;
    use std::time::Duration;

    fn token(sequence: u64, device_id: u64, generation: u64) -> SubmitToken {
        SubmitToken::__backend_new(generation, sequence, device_id)
    }

    /// 完了・失敗・未完了・完了の 4 つを払い出し、回収した表。1 は回収済み、2 は失敗、3・4 は表に残る。
    fn table() -> Submissions<GroupState> {
        let mut t = Submissions::<GroupState>::new();
        for g in [
            GroupState::Completed,
            GroupState::Failed,
            GroupState::Pending,
            GroupState::Completed,
        ] {
            let _ = t.issue(|| Ok(g));
        }
        t.reclaim(|g| *g);
        t
    }

    // CMD-78
    /// ロスト済みは別 device の印より先に今の世代の `DeviceLost`、次に別 device、次に世代違い。
    #[test]
    fn lost_is_checked_before_the_owner_and_the_generation() {
        assert_eq!(
            check_wait_token(true, &token(1, 9, 1), 7, 2),
            Err(ContractError::DeviceLost { generation: 2 })
        );
        assert_eq!(
            check_wait_token(false, &token(1, 9, 1), 7, 2),
            Err(ContractError::DeviceMismatch)
        );
        assert_eq!(
            check_wait_token(false, &token(1, 7, 1), 7, 2),
            Err(ContractError::DeviceLost { generation: 1 })
        );
        assert_eq!(check_wait_token(false, &token(1, 7, 2), 7, 2), Ok(()));
    }

    // CMD-82
    /// 払い出していない番号（0 と最後の番号より大きいもの）は待たずに `Unissued`。表は変わらない。
    #[test]
    fn unissued_sequences_are_rejected_without_changing_the_table() {
        let t = table();
        let before = (
            t.next_sequence,
            t.watermark,
            t.pending.len(),
            t.failed.len(),
        );
        assert_eq!(t.wait_step(0), WaitStep::Unissued { last: 4 });
        assert_eq!(t.wait_step(5), WaitStep::Unissued { last: 4 });
        assert_eq!(t.wait_step(u64::MAX), WaitStep::Unissued { last: 4 });
        assert_eq!(
            (
                t.next_sequence,
                t.watermark,
                t.pending.len(),
                t.failed.len()
            ),
            before
        );
        let empty = Submissions::<GroupState>::new();
        assert_eq!(empty.wait_step(1), WaitStep::Unissued { last: 0 });
    }

    // CMD-82
    /// 払い出していない番号の値は `InvalidArgument` で、操作名・要求した番号・最後の番号を載せる。
    #[test]
    fn the_unissued_error_names_the_operation_and_both_sequences() {
        let ContractError::InvalidArgument(text) = unissued_wait_error(9, 4) else {
            panic!("expected InvalidArgument");
        };
        assert!(text.contains("wait_blocking"), "{text}");
        assert!(text.contains('9'), "{text}");
        assert!(text.contains('4'), "{text}");
    }

    /// 回収済みは `Done`、表に残る番号は `Wait`、失敗の集合の番号は `BackendFailure`。
    #[test]
    fn issued_sequences_map_to_done_wait_or_failed() {
        let t = table();
        assert_eq!(t.wait_step(1), WaitStep::Done);
        assert_eq!(t.wait_step(2), WaitStep::Failed(failed_wait_error()));
        assert_eq!(t.wait_step(3), WaitStep::Wait);
        assert_eq!(t.wait_step(4), WaitStep::Wait);
        assert!(matches!(
            failed_wait_error(),
            ContractError::BackendFailure {
                op: "wait_blocking",
                backend: BackendKind::Cuda,
                ..
            }
        ));
    }

    // CMD-84
    /// 待ちの失敗の印は最初の値を持ち続け、失敗した投入の値より先に返る。`state_of` は `Lost`、表から外れても完了とみなさない。
    #[test]
    fn a_marked_sequence_keeps_its_first_error_and_stays_unfinished() {
        let mut t = table();
        let first = backend_failure("event_synchronize", "first");
        t.mark_wait_failure(3, first.clone());
        t.mark_wait_failure(3, backend_failure("event_synchronize", "second"));
        assert_eq!(t.wait_step(3), WaitStep::Failed(first));
        let failed = backend_failure("event_synchronize", "failed");
        t.mark_wait_failure(2, failed.clone());
        assert_eq!(t.wait_step(2), WaitStep::Failed(failed));
        assert_eq!(t.state_of(3, |g| *g), CompletionState::Lost);
        t.pending.remove(&3);
        assert!(t.is_unfinished(3));
        assert!(!t.is_unfinished(1));
    }

    // CMD-40
    /// 期限つきの待ちは、確かめが終端を返せば止まり、期限に届けば `Timeout`。挟む時間は残りと上限の短い方。
    #[test]
    fn the_limited_wait_stops_on_a_terminal_state_or_at_the_limit() {
        let now = Cell::new(Duration::ZERO);
        let slept = std::cell::RefCell::new(Vec::new());
        let probes = Cell::new(0);
        let r = wait_until_limit(
            Duration::from_micros(500),
            || {
                probes.set(probes.get() + 1);
                CompletionState::Pending
            },
            || now.get(),
            |d| {
                slept.borrow_mut().push(d);
                now.set(now.get() + d);
            },
        );
        assert_eq!(r, Err(ContractError::Timeout));
        assert_eq!(
            *slept.borrow(),
            vec![
                Duration::from_micros(200),
                Duration::from_micros(200),
                Duration::from_micros(100)
            ]
        );
        assert_eq!(probes.get(), 4);

        let calls = Cell::new(0);
        let r = wait_until_limit(
            Duration::from_secs(1),
            || {
                calls.set(calls.get() + 1);
                if calls.get() < 3 {
                    CompletionState::Pending
                } else {
                    CompletionState::Complete
                }
            },
            || Duration::ZERO,
            |_| {},
        );
        assert_eq!(r, Ok(CompletionState::Complete));
        assert_eq!(calls.get(), 3);
    }

    // CMD-40
    /// 期限 0 でも 1 回は確かめ、終端なら値を返し、未完了なら眠らずに `Timeout`。
    #[test]
    fn a_zero_limit_probes_once_and_never_sleeps() {
        let slept = Cell::new(false);
        assert_eq!(
            wait_until_limit(
                Duration::ZERO,
                || CompletionState::Lost,
                || Duration::ZERO,
                |_| slept.set(true)
            ),
            Ok(CompletionState::Lost)
        );
        assert_eq!(
            wait_until_limit(
                Duration::ZERO,
                || CompletionState::Pending,
                || Duration::ZERO,
                |_| slept.set(true)
            ),
            Err(ContractError::Timeout)
        );
        assert!(!slept.get());
    }
}

/// 解放の保留の数・投入済みの表の件数・ゼロ化の完了待ちの、CUDA の device に触る試験。device の無い環境では流さない（TASK-346 のポインタ）。
#[cfg(all(test, feature = "fault-injection"))]
mod release_backlog_device_tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    const LEN: u64 = 64;

    /// 同時に未完了にしておく投入の数。
    const WINDOW: usize = 8;

    /// 各回の後の 2 つの数の上限。
    const BOUND: u64 = WINDOW as u64 + 1;

    /// くり返しの回数。
    const ROUNDS: usize = 10_000;

    /// くり返しの試験の上限。本体も各回の初めにこの上限を照らす。
    const ROUNDS_LIMIT: Duration = Duration::from_secs(120);

    /// ほかの試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// `body` を別のスレッドで流し、`limit` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(limit: Duration, body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(limit) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {limit:?}")
            }
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    /// 印の写しを返す。
    fn copy_of(t: &SubmitToken) -> SubmitToken {
        SubmitToken::__backend_new(t.generation, t.sequence, t.__backend_device_id())
    }

    fn usage() -> BufferUsage {
        BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    /// `pending_release_count` の値。`None` なら失敗とする。
    fn held(d: &CudaDevice) -> u64 {
        match d.pending_release_count() {
            Some(n) => n,
            None => panic!("pending_release_count returned None"),
        }
    }

    /// 投入済みの表の件数。
    fn submitted(d: &CudaDevice) -> u64 {
        u64::try_from(d.lock_submissions().pending.len()).unwrap_or(u64::MAX)
    }

    /// 再利用の一覧でゼロ化の完了を待つバッファの数。
    fn zeroing(d: &CudaDevice) -> usize {
        d.lock().pool.zeroing.len()
    }

    /// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
    fn poll_until_complete(d: &CudaDevice, t: &SubmitToken) {
        let start = Instant::now();
        loop {
            match d.poll(copy_of(t)) {
                CompletionState::Complete => return,
                CompletionState::Pending => {}
                other => panic!("unexpected state: {other:?}"),
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "the token did not complete in time"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// `buffer` 全体の 0 埋めを記録して投入する。
    fn submit_clear(d: &CudaDevice, buffer: &BufferHandle) -> SubmitToken {
        let mut enc = d.create_encoder();
        assert_eq!(d.clear_buffer(&mut enc, buffer, 0, LEN), Ok(()));
        d.submit(enc, &[]).expect("submit")
    }

    // MEM-11
    /// 未完了の投入を 8 件までに保ち、確保・0 埋めの投入・`free` を 10,000 回くり返すと、各回の後の解放の保留の数と
    /// 投入済みの表の件数はどちらも 9 以下。全部の印を `poll` で `Complete` まで待ち、もう 1 回 `poll` を呼ぶと、どちらも 0。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn the_backlog_stays_within_nine_over_ten_thousand_rounds_and_ends_at_zero() {
        within(ROUNDS_LIMIT, || {
            let start = Instant::now();
            let d = device();
            let mut window = VecDeque::with_capacity(WINDOW + 1);
            let mut max_held = 0;
            let mut max_submitted = 0;
            for round in 0..ROUNDS {
                assert!(
                    start.elapsed() < ROUNDS_LIMIT,
                    "round {round} started after {ROUNDS_LIMIT:?}"
                );
                if window.len() == WINDOW
                    && let Some(oldest) = window.pop_front()
                {
                    poll_until_complete(&d, &oldest);
                }
                let buffer = d.allocate(LEN, usage()).expect("allocate");
                let token = submit_clear(&d, &buffer);
                d.free(buffer);
                window.push_back(token);
                let now_held = held(&d);
                let now_submitted = submitted(&d);
                assert!(
                    now_held <= BOUND,
                    "round {round}: pending_release_count={now_held}"
                );
                assert!(
                    now_submitted <= BOUND,
                    "round {round}: submissions={now_submitted}"
                );
                max_held = max_held.max(now_held);
                max_submitted = max_submitted.max(now_submitted);
            }
            println!("max_pending_release_count={max_held} max_submissions={max_submitted}");
            let last = match window.back() {
                Some(t) => copy_of(t),
                None => panic!("no token is left in the window"),
            };
            for token in window.drain(..) {
                poll_until_complete(&d, &token);
            }
            assert_eq!(d.poll(last), CompletionState::Complete);
            assert_eq!(d.pending_release_count(), Some(0));
            assert_eq!(submitted(&d), 0);
        });
    }

    // MEM-13
    /// 解放の保留から外れてゼロ化の完了を待つバッファは、`pending_release_count` に数えない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn buffers_waiting_for_zeroing_are_not_counted() {
        within(LIMIT, || {
            let d = device();
            let buffer = d.allocate(LEN, usage()).expect("allocate");
            let sync = d.allocate(LEN, usage()).expect("allocate");
            assert_eq!(d.arm_gate(), Ok(()));
            let first = submit_clear(&d, &buffer);
            d.free(buffer);
            assert_eq!(d.pending_release_count(), Some(1));
            assert_eq!(zeroing(&d), 0);
            assert_eq!(d.open_gate(), Ok(()));
            // 4 バイトの書き込みは stream を同期するが、保留の回収はしない。
            assert_eq!(d.upload(&sync, 0, &[1u8; 4]), Ok(()));
            assert_eq!(d.pending_release_count(), Some(1));
            assert_eq!(d.arm_gate(), Ok(()));
            let start = Instant::now();
            // 空の投入の中で保留が外れ、そのバッファの 0 埋めは門の後ろに積まれる。
            let second = d.submit(d.create_encoder(), &[]).expect("submit");
            assert_eq!(d.pending_release_count(), Some(0));
            assert_eq!(zeroing(&d), 1);
            assert_eq!(d.poll(copy_of(&second)), CompletionState::Pending);
            assert_eq!(d.poll(copy_of(&first)), CompletionState::Complete);
            assert_eq!(d.pending_release_count(), Some(0));
            assert!(start.elapsed() < GATE_EXPIRY / 2);
            assert_eq!(d.open_gate(), Ok(()));
            poll_until_complete(&d, &second);
            assert_eq!(d.upload(&sync, 0, &[2u8; 4]), Ok(()));
            let again = d.allocate(LEN, usage()).expect("allocate");
            assert_eq!(zeroing(&d), 0);
            assert_eq!(d.pending_release_count(), Some(0));
            d.free(again);
            d.free(sync);
        });
    }
}

/// `Ptx` の入力の分類と、読み込む PTX を作るときに NVRTC の呼び出し口を呼ぶかの、device に触らない試験（TASK-804 のポインタ）。
#[cfg(test)]
mod ptx_input_tests {
    use super::*;
    use std::cell::RefCell;

    /// `module_input` に渡す呼び出し口の代わり。受けたソースを `seen` に積み、`result` を返す。
    fn recording<'s>(
        seen: &'s RefCell<Vec<String>>,
        result: Result<&'static str, ContractError>,
    ) -> impl FnOnce(&str) -> Result<Ptx, ContractError> + 's {
        move |source| {
            seen.borrow_mut().push(source.to_string());
            result.map(Ptx::from_src)
        }
    }

    // COMP-91
    /// `Ptx` の入力は呼び出し口を呼ばずに、テキストがそのまま読み込む PTX になる。
    #[test]
    fn ptx_input_becomes_the_module_text_without_calling_the_compiler() {
        let seen = RefCell::new(Vec::new());
        let text =
            ".version 8.0\n.target sm_80\n.address_size 64\n.visible .entry k()\n{\n\tret;\n}\n";
        let (ptx, entry_point) = module_input(
            AcceptedInput::Ptx {
                source: text,
                entry_point: "k",
            },
            recording(&seen, Ok("compiled")),
        )
        .expect("ptx");
        assert_eq!(ptx.to_src(), text);
        assert_eq!(entry_point, "k");
        assert_eq!(seen.borrow().len(), 0);
    }

    /// `CudaC` の入力は呼び出し口を 1 回だけ呼び、その結果を読み込む PTX にする。呼び出し口の失敗はそのまま返る。
    #[test]
    fn cuda_c_input_calls_the_compiler_once_with_the_source() {
        let seen = RefCell::new(Vec::new());
        let (ptx, entry_point) = module_input(
            AcceptedInput::CudaC {
                source: "s",
                entry_point: "e",
            },
            recording(&seen, Ok("compiled")),
        )
        .expect("cuda c");
        assert_eq!(ptx.to_src(), "compiled");
        assert_eq!(entry_point, "e");
        assert_eq!(*seen.borrow(), vec!["s".to_string()]);

        let seen = RefCell::new(Vec::new());
        let failure = ContractError::ArtifactRejected {
            kind: "cuda-c",
            reason: "x".into(),
        };
        let r = module_input(
            AcceptedInput::CudaC {
                source: "s",
                entry_point: "e",
            },
            recording(&seen, Err(failure.clone())),
        );
        assert_eq!(r.map(|(ptx, e)| (ptx.to_src(), e)), Err(failure));
        assert_eq!(seen.borrow().len(), 1);
    }

    /// NUL のバイトは呼び出し口を呼ぶ前に拒む。`Ptx` のテキストは `module`、`CudaC` のソースは `cuda-c`、
    /// 入口名はどちらも `function`。
    #[test]
    fn nul_bytes_are_rejected_before_the_module_text_is_made() {
        let cases = [
            (
                AcceptedInput::Ptx {
                    source: "a\0b",
                    entry_point: "k",
                },
                "module",
            ),
            (
                AcceptedInput::Ptx {
                    source: "p",
                    entry_point: "k\0",
                },
                "function",
            ),
            (
                AcceptedInput::Ptx {
                    source: "a\0b",
                    entry_point: "k\0",
                },
                "module",
            ),
            (
                AcceptedInput::CudaC {
                    source: "a\0b",
                    entry_point: "k",
                },
                "cuda-c",
            ),
            (
                AcceptedInput::CudaC {
                    source: "s",
                    entry_point: "k\0",
                },
                "function",
            ),
        ];
        for (input, expected) in cases {
            let seen = RefCell::new(Vec::new());
            let r = module_input(input, recording(&seen, Ok("compiled")));
            assert!(
                matches!(&r, Err(ContractError::ArtifactRejected { kind, .. }) if *kind == expected),
                "{input:?}"
            );
            assert_eq!(seen.borrow().len(), 0, "{input:?}");
        }
    }
}

/// `Ptx` の入力のパイプラインの生成と、device の NVRTC の呼び出し口に入った回数の、CUDA の device に触る試験。
/// device の無い環境では流さない（TASK-804 のポインタ）。
#[cfg(test)]
mod ptx_pipeline_device_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// 各試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// 要素に 1 を足す小さなカーネル。
    const ADD_ONE: &str = "extern \"C\" __global__ void add_one(float* out, const float* inp, unsigned int n) { unsigned int i = blockIdx.x * blockDim.x + threadIdx.x; if (i < n) { out[i] = inp[i] + 1.0f; } }";

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    fn table_len(d: &CudaDevice) -> usize {
        d.pipelines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map
            .len()
    }

    /// device の NVRTC の呼び出し口に入った回数。
    fn nvrtc_calls(d: &CudaDevice) -> u64 {
        d.nvrtc_calls.load(Ordering::SeqCst)
    }

    /// `ADD_ONE` を、`compile_options` の `--gpu-architecture` に `--frandom-seed=0x5eed` を足して NVRTC で PTX のテキストにする。
    /// device の NVRTC の呼び出し口を通らない。
    fn nvrtc_ptx(capability: (i32, i32)) -> String {
        let mut options = compile_options(capability).expect("options");
        options.options.push("--frandom-seed=0x5eed".to_string());
        cudarc::nvrtc::compile_ptx_with_opts(ADD_ONE, options)
            .expect("NVRTC")
            .to_src()
    }

    fn add_one_cuda_c() -> PipelineInput<'static> {
        PipelineInput::CudaC {
            source: ADD_ONE,
            entry_point: "add_one",
        }
    }

    // COMP-23
    /// NVRTC で作った PTX のテキストを `Ptx` で渡すと、この device のパイプラインが作られて表に 1 件足される。
    /// 2 回作ると識別子が異なる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn nvrtc_ptx_text_creates_a_pipeline_of_this_device() {
        within(|| {
            let d = device();
            let text = nvrtc_ptx(d.compute_capability);
            assert!(text.contains(".entry add_one"));
            let layout = BindingLayout::new(vec![0, 1, 2]);
            let before = table_len(&d);
            let a = d
                .create_pipeline(
                    PipelineInput::Ptx {
                        source: &text,
                        entry_point: "add_one",
                    },
                    &layout,
                )
                .expect("first pipeline");
            assert_eq!(a.__backend_device_id(), d.id);
            assert_eq!(table_len(&d), before + 1);
            let b = d
                .create_pipeline(
                    PipelineInput::Ptx {
                        source: &text,
                        entry_point: "add_one",
                    },
                    &layout,
                )
                .expect("second pipeline");
            assert_eq!(b.__backend_device_id(), d.id);
            assert_ne!(a.__backend_id(), b.__backend_id());
            assert_eq!(table_len(&d), before + 2);
        });
    }

    // COMP-91
    /// `Ptx` の生成は NVRTC の呼び出し口に入らない。同じ device の `CudaC` の生成は 1 回だけ入る。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn ptx_creation_does_not_enter_the_nvrtc_call_site() {
        within(|| {
            let d = device();
            let text = nvrtc_ptx(d.compute_capability);
            let layout = BindingLayout::new(vec![0, 1, 2]);
            assert_eq!(nvrtc_calls(&d), 0);
            for _ in 0..2 {
                d.create_pipeline(
                    PipelineInput::Ptx {
                        source: &text,
                        entry_point: "add_one",
                    },
                    &layout,
                )
                .expect("ptx pipeline");
            }
            assert_eq!(nvrtc_calls(&d), 0);
            d.create_pipeline(add_one_cuda_c(), &layout)
                .expect("cuda c pipeline");
            assert_eq!(nvrtc_calls(&d), 1);
        });
    }

    // COMP-91
    /// 読み込めない `Ptx` は `ArtifactRejected` で、表に足さず、NVRTC の呼び出し口に入らない。
    /// 読めないテキストと NUL のバイトを含むテキストは `module`、無い入口名と NUL のバイトを含む入口名は `function`。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn ptx_load_failures_are_artifact_rejected_without_entering_the_nvrtc_call_site() {
        within(|| {
            let d = device();
            let text = nvrtc_ptx(d.compute_capability);
            let with_nul = format!("{text}\0");
            let layout = BindingLayout::new(vec![0, 1, 2]);
            let before = table_len(&d);
            let cases: [(&str, &str, &str); 4] = [
                ("this is not PTX", "add_one", "module"),
                (&with_nul, "add_one", "module"),
                (&text, "absent", "function"),
                (&text, "add\0one", "function"),
            ];
            for (source, entry_point, expected) in cases {
                let r = d.create_pipeline(
                    PipelineInput::Ptx {
                        source,
                        entry_point,
                    },
                    &layout,
                );
                assert!(
                    matches!(&r, Err(ContractError::ArtifactRejected { kind, .. }) if *kind == expected),
                    "{expected}: {entry_point:?}"
                );
                assert_eq!(table_len(&d), before);
            }
            assert_eq!(nvrtc_calls(&d), 0);
            d.create_pipeline(add_one_cuda_c(), &layout)
                .expect("cuda c pipeline");
            assert_eq!(nvrtc_calls(&d), 1);
        });
    }
}

/// `create_pipeline` の受け付けない入力の理由と、失敗の写しの関数が返す値の、device に触らない試験（TASK-291 のポインタ）。
#[cfg(test)]
mod pipeline_error_tests {
    use super::*;
    use fandhe_silicon_core::execution::{
        FunctionConstant, FunctionConstantValue, MslFloatFunctions, MslMathMode,
    };

    /// 入力のソースの字句。理由の文に載らないことを照らす。
    const SOURCE: &str = "SOURCE-TEXT-OF-THE-INPUT";

    // ERR-7
    /// 受け付けない 6 つの入力は `unsupported-input` で、理由に受け取った variant と受け付ける `CudaC`・`Ptx` を載せる。
    /// `MslSpecializedWithOptions` の理由だけが Metal 専用であることを載せる。入力のソースは理由に載らない。
    #[test]
    fn unsupported_inputs_name_the_variant_and_the_accepted_inputs() {
        let constants = [FunctionConstant {
            index: 0,
            value: FunctionConstantValue::U32(4),
        }];
        let words = [0x0723_0203_u32];
        let bytes = [0x44_u8];
        let cases = [
            (
                PipelineInput::Msl {
                    source: SOURCE,
                    entry_point: "k",
                },
                "pipeline input Msl is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx",
            ),
            (
                PipelineInput::MslSpecialized {
                    source: SOURCE,
                    entry_point: "k",
                    function_constants: &constants,
                },
                "pipeline input MslSpecialized is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx",
            ),
            (
                PipelineInput::MslWithOptions {
                    source: SOURCE,
                    entry_point: "k",
                    math_mode: MslMathMode::Safe,
                },
                "pipeline input MslWithOptions is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx",
            ),
            (
                PipelineInput::MslSpecializedWithOptions {
                    source: SOURCE,
                    entry_point: "k",
                    function_constants: &constants,
                    math_mode: MslMathMode::Safe,
                    float_functions: MslFloatFunctions::Precise,
                },
                "pipeline input MslSpecializedWithOptions is Metal-only and is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx",
            ),
            (
                PipelineInput::SpirV {
                    words: &words,
                    entry_point: "k",
                },
                "pipeline input SpirV is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx",
            ),
            (
                PipelineInput::Dxil {
                    bytes: &bytes,
                    entry_point: "k",
                },
                "pipeline input Dxil is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx",
            ),
        ];
        for (input, expected) in &cases {
            let Err(ContractError::ArtifactRejected { kind, reason }) =
                classify_pipeline_input(input)
            else {
                panic!("{input:?} is not ArtifactRejected");
            };
            assert_eq!(kind, "unsupported-input", "{input:?}");
            assert_eq!(reason, *expected, "{input:?}");
            assert!(!reason.contains(SOURCE), "{input:?}: {reason}");
        }
    }

    // ERR-11
    /// ドライバの呼び出しの失敗は、呼び出しの名前と `Debug` の文字列を載せた `BackendFailure` になる。
    /// `allocate` のメモリ不足は `AllocationFailed` で、要求したバイト数を載せる。
    #[test]
    fn driver_failures_carry_the_call_name_and_the_debug_text() {
        assert_eq!(
            backend_failure("memcpy_htod", "detail"),
            ContractError::BackendFailure {
                op: "memcpy_htod",
                backend: BackendKind::Cuda,
                detail: "\"detail\"".to_string(),
            }
        );
        assert_eq!(
            allocation_failure(
                "alloc_zeros",
                64,
                DriverError(cudarc::driver::sys::CUresult::CUDA_ERROR_OUT_OF_MEMORY)
            ),
            ContractError::AllocationFailed {
                requested_bytes: 64
            }
        );
    }

    // ERR-63
    /// 成果物の読み込みの失敗は、入力の成果物に原因があれば `ArtifactRejected` で理由に呼び出しの名前と `Debug` の文字列を載せ、
    /// ほかは `BackendFailure` で `op` に呼び出しの名前を載せる。NVRTC のコンパイルの失敗は `cuda-c` で `nvrtcCompileProgram` を載せる。
    #[test]
    fn artifact_failures_carry_the_call_name_and_the_debug_text() {
        for (kind, call) in [
            ("module", "cuModuleLoadData"),
            ("function", "cuModuleGetFunction"),
        ] {
            assert_eq!(
                artifact_failure(kind, call, true, "detail"),
                ContractError::ArtifactRejected {
                    kind,
                    reason: format!("{call}: \"detail\""),
                }
            );
            assert_eq!(
                artifact_failure(kind, call, false, "detail"),
                ContractError::BackendFailure {
                    op: call,
                    backend: BackendKind::Cuda,
                    detail: "\"detail\"".to_string(),
                }
            );
        }
        use cudarc::nvrtc::result::NvrtcError;
        use cudarc::nvrtc::sys::nvrtcResult;
        let compile = CompileError::CompileError {
            nvrtc: NvrtcError(nvrtcResult::NVRTC_ERROR_COMPILATION),
            options: vec![],
            log: std::ffi::CString::new("log").unwrap(),
        };
        let expected = format!("nvrtcCompileProgram: {compile:?}");
        assert_eq!(
            nvrtc_failure(compile),
            ContractError::ArtifactRejected {
                kind: "cuda-c",
                reason: expected,
            }
        );
        let out_of_memory =
            CompileError::CreationError(NvrtcError(nvrtcResult::NVRTC_ERROR_OUT_OF_MEMORY));
        let detail = format!("{out_of_memory:?}");
        assert_eq!(
            nvrtc_failure(out_of_memory),
            ContractError::BackendFailure {
                op: "nvrtcCreateProgram",
                backend: BackendKind::Cuda,
                detail,
            }
        );
    }
}

/// `create_pipeline` の受け付けない入力と成果物の生成の失敗の、CUDA の device に触る試験。
/// device の無い環境では流さない（TASK-291 のポインタ）。
#[cfg(test)]
mod pipeline_error_device_tests {
    use super::*;
    use fandhe_silicon_core::execution::{
        FunctionConstant, FunctionConstantValue, MslFloatFunctions, MslMathMode,
    };
    use std::sync::mpsc;
    use std::time::Duration;

    /// 各試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// 要素に 1 を足す小さなカーネル。
    const ADD_ONE: &str = "extern \"C\" __global__ void add_one(float* out, const float* inp, unsigned int n) { unsigned int i = blockIdx.x * blockDim.x + threadIdx.x; if (i < n) { out[i] = inp[i] + 1.0f; } }";

    /// 入口 `k` の MSL。Metal の背後実装の試験の `KERNEL` と同じ文字列。
    const MSL_KERNEL: &str = "#include <metal_stdlib>\n\
        using namespace metal;\n\
        kernel void k(device float* out [[buffer(0)]], uint tid [[thread_position_in_grid]]) {\n\
            out[tid] = 1.0;\n\
        }\n";

    /// function constant の 0 番（`uint`）と 1 番（`bool`）を持つ入口 `k` の MSL。Metal の背後実装の試験の `SPECIALIZED_KERNEL` と同じ文字列。
    const MSL_SPECIALIZED_KERNEL: &str = "#include <metal_stdlib>\n\
        using namespace metal;\n\
        constant uint N [[function_constant(0)]];\n\
        constant bool B [[function_constant(1)]];\n\
        kernel void k(device uint* out [[buffer(0)]]) {\n\
            out[0] = N;\n\
            if (B) { out[1] = 1; }\n\
        }\n";

    /// 公開の SPIR-V の仕様のバイナリ形式で組んだ、何もしない compute の入口 `k` の語の列
    /// （`Shader`・`Logical GLSL450`・`GLCompute`・`LocalSize 1 1 1`）。
    const SPIRV_WORDS: [u32; 34] = [
        0x0723_0203,
        0x0001_0000,
        0,
        5,
        0,
        0x0002_0011,
        1,
        0x0003_000E,
        0,
        1,
        0x0004_000F,
        5,
        3,
        0x0000_006B,
        0x0006_0010,
        3,
        17,
        1,
        1,
        1,
        0x0002_0013,
        1,
        0x0003_0021,
        2,
        1,
        0x0005_0036,
        1,
        3,
        0,
        2,
        0x0002_00F8,
        4,
        0x0001_00FD,
        0x0001_0038,
    ];

    /// DXBC のコンテナの見出しだけの 32 バイト（`DXBC`・0 の digest・版 1.0・全体の長さ 32・部の数 0）。
    const DXBC_HEADER: [u8; 32] = [
        0x44, 0x58, 0x42, 0x43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x00, 0x00,
        0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    fn table_len(d: &CudaDevice) -> usize {
        d.pipelines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map
            .len()
    }

    /// device の NVRTC の呼び出し口に入った回数。
    fn nvrtc_calls(d: &CudaDevice) -> u64 {
        d.nvrtc_calls.load(Ordering::SeqCst)
    }

    /// `create_pipeline` が `bind_to_thread`・モジュールの読み込み・関数の取り出しの前に数えた回数。
    fn driver_calls(d: &CudaDevice) -> u64 {
        d.pipeline_driver_calls.load(Ordering::SeqCst)
    }

    // ERR-7
    /// 受け付けない 6 つの入力は `unsupported-input` で、理由に受け取った variant と `CudaC`・`Ptx` を載せ、
    /// `Metal` の語は `MslSpecializedWithOptions` の理由だけに載る。表に足さず、NVRTC の呼び出し口にも
    /// 下位の API の呼び出しにも入らない。続く `CudaC` の生成は NVRTC に 1 回、下位の API に 3 回入る。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn unsupported_inputs_are_rejected_without_entering_nvrtc_or_the_driver() {
        within(|| {
            let d = device();
            let layout = BindingLayout::new(vec![0]);
            let constants = [
                FunctionConstant {
                    index: 0,
                    value: FunctionConstantValue::U32(4),
                },
                FunctionConstant {
                    index: 1,
                    value: FunctionConstantValue::Bool(true),
                },
            ];
            let before = table_len(&d);
            let cases = [
                (
                    PipelineInput::Msl {
                        source: MSL_KERNEL,
                        entry_point: "k",
                    },
                    "Msl",
                    false,
                ),
                (
                    PipelineInput::MslSpecialized {
                        source: MSL_SPECIALIZED_KERNEL,
                        entry_point: "k",
                        function_constants: &constants,
                    },
                    "MslSpecialized",
                    false,
                ),
                (
                    PipelineInput::MslWithOptions {
                        source: MSL_KERNEL,
                        entry_point: "k",
                        math_mode: MslMathMode::Safe,
                    },
                    "MslWithOptions",
                    false,
                ),
                (
                    PipelineInput::MslSpecializedWithOptions {
                        source: MSL_SPECIALIZED_KERNEL,
                        entry_point: "k",
                        function_constants: &constants,
                        math_mode: MslMathMode::Safe,
                        float_functions: MslFloatFunctions::Precise,
                    },
                    "MslSpecializedWithOptions",
                    true,
                ),
                (
                    PipelineInput::SpirV {
                        words: &SPIRV_WORDS,
                        entry_point: "k",
                    },
                    "SpirV",
                    false,
                ),
                (
                    PipelineInput::Dxil {
                        bytes: &DXBC_HEADER,
                        entry_point: "k",
                    },
                    "Dxil",
                    false,
                ),
            ];
            for (input, variant, metal_only) in cases {
                match d.create_pipeline(input, &layout) {
                    Err(ContractError::ArtifactRejected { kind, reason }) => {
                        assert_eq!(kind, "unsupported-input", "{variant}");
                        assert!(
                            reason.starts_with(&format!("pipeline input {variant} ")),
                            "{variant}: {reason}"
                        );
                        assert!(reason.contains("CudaC"), "{variant}: {reason}");
                        assert!(reason.contains("Ptx"), "{variant}: {reason}");
                        assert_eq!(reason.contains("Metal"), metal_only, "{variant}: {reason}");
                        assert!(!reason.contains("metal_stdlib"), "{variant}: {reason}");
                    }
                    other => panic!("{variant}: {other:?}"),
                }
                assert_eq!(table_len(&d), before, "{variant}");
                assert_eq!(nvrtc_calls(&d), 0, "{variant}");
                assert_eq!(driver_calls(&d), 0, "{variant}");
            }
            d.create_pipeline(
                PipelineInput::CudaC {
                    source: ADD_ONE,
                    entry_point: "add_one",
                },
                &BindingLayout::new(vec![0, 1, 2]),
            )
            .expect("cuda c pipeline");
            assert_eq!(nvrtc_calls(&d), 1);
            assert_eq!(driver_calls(&d), 3);
            assert_eq!(table_len(&d), before + 1);
        });
    }

    // ERR-63
    /// コンパイルできない `CudaC` は `cuda-c`、無い入口名は `function`、読み込めない `Ptx` は `module` で、
    /// 理由は失敗した呼び出しの名前で始まる。どれも表に足さない。NVRTC の呼び出し口と下位の API に入った回数は、
    /// 失敗した呼び出しまでの分だけ増える。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn artifact_failures_name_the_failed_call_and_add_no_pipeline() {
        within(|| {
            let d = device();
            let layout = BindingLayout::new(vec![0, 1, 2]);
            let before = table_len(&d);
            let cases = [
                (
                    PipelineInput::CudaC {
                        source: "this is not CUDA C",
                        entry_point: "add_one",
                    },
                    "cuda-c",
                    "nvrtcCompileProgram: ",
                    1,
                    0,
                ),
                (
                    PipelineInput::CudaC {
                        source: ADD_ONE,
                        entry_point: "absent",
                    },
                    "function",
                    "cuModuleGetFunction: ",
                    2,
                    3,
                ),
                (
                    PipelineInput::Ptx {
                        source: "this is not PTX",
                        entry_point: "add_one",
                    },
                    "module",
                    "cuModuleLoadData: ",
                    2,
                    5,
                ),
            ];
            for (input, expected_kind, call, nvrtc, driver) in cases {
                match d.create_pipeline(input, &layout) {
                    Err(ContractError::ArtifactRejected { kind, reason }) => {
                        assert_eq!(kind, expected_kind, "{call}");
                        assert!(reason.starts_with(call), "{call}: {reason}");
                    }
                    other => panic!("{call}: {other:?}"),
                }
                assert_eq!(table_len(&d), before, "{call}");
                assert_eq!(nvrtc_calls(&d), nvrtc, "{call}");
                assert_eq!(driver_calls(&d), driver, "{call}");
            }
        });
    }
}

#[cfg(test)]
mod dispatch_record_tests {
    use super::*;

    /// 試験の記録が使う世代。
    const GEN: u64 = 7;

    fn recorded(id: u64) -> RecordedBuffer {
        RecordedBuffer {
            buffer_id: id,
            generation: GEN,
        }
    }

    fn arg(slot: u32, id: u64, offset: u64) -> DispatchBufferArg {
        DispatchBufferArg {
            slot,
            buffer: recorded(id),
            offset,
        }
    }

    /// `InvalidArgument` で、理由の文が `expected` と全文で同じことを照らす。
    fn assert_invalid<T: std::fmt::Debug>(result: Result<T, ContractError>, expected: &str) {
        match result {
            Err(ContractError::InvalidArgument(msg)) => assert_eq!(msg, expected),
            other => panic!("expected InvalidArgument {expected:?}, got {other:?}"),
        }
    }

    // COMP-3
    // COMP-4
    // COMP-5
    // BIND-9
    /// 束縛と定数を slot の昇順に並べ、定数の bytes を写し、共有メモリの合計を持つ。`index` は使わない。
    #[test]
    fn record_orders_buffers_and_constants_by_slot_and_sums_shared_memory() {
        let bytes = [1_u8, 2, 3, 4];
        let record = check_dispatch_record(
            5,
            &[0, 1, 2],
            &[arg(2, 3, 16), arg(0, 1, 0)],
            &[(1, &bytes[..])],
            [4, 2, 1],
            [32, 1, 1],
            &[64, 128],
        )
        .expect("record");
        assert_eq!(
            record,
            DispatchRecord {
                pipeline: 5,
                args: vec![
                    RecordedArg::Buffer {
                        slot: 0,
                        buffer: recorded(1),
                        offset: 0,
                    },
                    RecordedArg::Constant {
                        slot: 1,
                        bytes: vec![1, 2, 3, 4],
                    },
                    RecordedArg::Buffer {
                        slot: 2,
                        buffer: recorded(3),
                        offset: 16,
                    },
                ],
                grid: [4, 2, 1],
                block: [32, 1, 1],
                shared_mem_bytes: 192,
            }
        );
        assert_eq!(
            TransferRecord::Dispatch(record).buffers(),
            vec![recorded(1), recorded(3)]
        );
    }

    // COMP-88
    /// `offset` はバッファの長さと比べず、そのまま記録する。
    #[test]
    fn record_keeps_the_offset_without_an_upper_check() {
        let record = check_dispatch_record(
            1,
            &[0],
            &[arg(0, 1, u64::MAX)],
            &[],
            [1, 1, 1],
            [1, 1, 1],
            &[],
        )
        .expect("record");
        assert_eq!(
            record.args,
            vec![RecordedArg::Buffer {
                slot: 0,
                buffer: recorded(1),
                offset: u64::MAX,
            }]
        );
    }

    // COMP-29
    /// 空の定数は、slot を含む `InvalidArgument`。配置にあって渡されない slot は、空の定数より先に当たる。
    #[test]
    fn record_rejects_an_empty_constant_with_its_slot() {
        let empty: &[u8] = &[];
        assert_invalid(
            check_dispatch_record(
                1,
                &[0, 3],
                &[arg(0, 1, 0)],
                &[(3, empty)],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            ),
            "dispatch_ex: constant slot 3 is empty",
        );
        assert_invalid(
            check_dispatch_record(
                1,
                &[0, 3, 5],
                &[arg(0, 1, 0)],
                &[(3, empty)],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            ),
            "dispatch_ex: slot 5 of the binding layout is not provided",
        );
    }

    // BIND-3
    // COMP-35
    /// slot ごとの重なり・配置に無い slot と、その後の配置にあって渡されない slot を拒む。
    #[test]
    fn record_rejects_duplicate_unknown_and_missing_slots_in_this_order() {
        let four = [0_u8; 4];
        assert_invalid(
            check_dispatch_record(
                1,
                &[0],
                &[arg(0, 1, 0)],
                &[(0, &four[..])],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            ),
            "dispatch_ex: duplicate slot in dispatch: 0",
        );
        assert_invalid(
            check_dispatch_record(
                1,
                &[0],
                &[arg(0, 1, 0), arg(9, 2, 0)],
                &[],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            ),
            "dispatch_ex: slot 9 is not in the binding layout",
        );
        assert_invalid(
            check_dispatch_record(1, &[0, 1], &[arg(0, 1, 0)], &[], [1, 1, 1], [1, 1, 1], &[]),
            "dispatch_ex: slot 1 of the binding layout is not provided",
        );
        // 束縛を定数より先に見るので、束縛の配置に無い slot が、定数の配置に無い slot より先に当たる。
        assert_invalid(
            check_dispatch_record(
                1,
                &[0],
                &[arg(9, 1, 0)],
                &[(8, &four[..])],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            ),
            "dispatch_ex: slot 9 is not in the binding layout",
        );
        // slot ごとに重なり・配置に無いの順で見るので、配置に無い slot の 2 回目より先に、1 回目の配置に無いが当たる。
        assert_invalid(
            check_dispatch_record(
                1,
                &[0],
                &[arg(9, 1, 0), arg(9, 2, 0)],
                &[],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            ),
            "dispatch_ex: slot 9 is not in the binding layout",
        );
    }

    /// 共有メモリの合計が `u32` をあふれると `InvalidArgument`。空の定数は、合計のあふれより先に当たる。
    #[test]
    fn record_rejects_a_shared_memory_total_that_overflows() {
        assert_invalid(
            check_dispatch_record(1, &[], &[], &[], [1, 1, 1], [1, 1, 1], &[u32::MAX, 1]),
            "dispatch_ex: threadgroup memory total overflows",
        );
        let empty: &[u8] = &[];
        assert_invalid(
            check_dispatch_record(
                1,
                &[0],
                &[],
                &[(0, empty)],
                [1, 1, 1],
                [1, 1, 1],
                &[u32::MAX, 1],
            ),
            "dispatch_ex: constant slot 0 is empty",
        );
        let record = check_dispatch_record(1, &[], &[], &[], [1, 1, 1], [1, 1, 1], &[u32::MAX, 0])
            .expect("record");
        assert_eq!(record.shared_mem_bytes, u32::MAX);
    }

    fn dispatch_record(slot: u32, id: u64) -> TransferRecord {
        TransferRecord::Dispatch(
            check_dispatch_record(
                1,
                &[slot],
                &[arg(slot, id, 0)],
                &[],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            )
            .expect("record"),
        )
    }

    fn table(id: u64) -> Option<(u64, u64)> {
        (1..=3).contains(&id).then_some((GEN, 64))
    }

    // COMP-39
    /// 束縛のバッファの解放済みは、範囲を見ずに slot を載せた `InvalidArgument`。前のコピー・クリアの記録より先に、全記録を見る。
    #[test]
    fn plan_rejects_a_freed_bound_buffer_with_its_slot() {
        assert_invalid(
            plan_transfers(&[dispatch_record(4, 9)], table, GEN),
            "a bound buffer was freed before submit (binding slot 4)",
        );
        assert_invalid(
            plan_transfers(&[dispatch_record(4, 1)], table, GEN + 1),
            "a bound buffer was freed before submit (binding slot 4)",
        );
        let clear = TransferRecord::Clear {
            buffer: recorded(1),
            offset: 64,
            size: 4,
        };
        assert_invalid(
            plan_transfers(&[clear, dispatch_record(2, 9)], table, GEN),
            "a bound buffer was freed before submit (binding slot 2)",
        );
    }

    /// dispatch の記録はそのまま件になり、門が閉じていれば dispatch の件を含む列は `Unsupported`（`op` は `submit`）。
    /// コピー・クリアだけの列と、開いた門は通す。
    #[test]
    fn gate_rejects_lists_with_a_dispatch_only_when_closed() {
        let dispatch = dispatch_record(0, 1);
        let clear = TransferRecord::Clear {
            buffer: recorded(2),
            offset: 0,
            size: 4,
        };
        let jobs = plan_transfers(&[clear.clone(), dispatch.clone()], table, GEN).expect("plan");
        let TransferRecord::Dispatch(record) = dispatch else {
            unreachable!()
        };
        assert_eq!(
            jobs,
            vec![
                TransferJob::Clear {
                    buffer: 2,
                    range: (0, 4),
                },
                TransferJob::Dispatch(record),
            ]
        );
        assert_eq!(jobs[1].buffer_ids(), vec![1]);
        assert_eq!(
            check_dispatch_gate(&jobs, false),
            Err(ContractError::Unsupported {
                op: "submit",
                backend: BACKEND,
            })
        );
        assert_eq!(check_dispatch_gate(&jobs, true), Ok(()));
        let clear_only = plan_transfers(&[clear], table, GEN).expect("plan");
        assert_eq!(check_dispatch_gate(&clear_only, false), Ok(()));
    }

    /// 仮引数の数と大きさが記録した引数とちょうど同じなら通り、問う回数は引数の数 + 1。
    #[test]
    fn kernel_params_pass_when_count_and_sizes_match() {
        let sizes = [8_usize, 4, 8];
        let mut asked = Vec::new();
        let result = check_kernel_params(&[8, 4, 8], |index| {
            asked.push(index);
            Ok(sizes.get(index).copied())
        });
        assert_eq!(result, Ok(()));
        assert_eq!(asked, vec![0, 1, 2, 3]);
        assert_eq!(check_kernel_params(&[], |_| Ok(None)), Ok(()));
    }

    /// 大きさの食い違い・仮引数が少ない・仮引数が多いは `InvalidArgument` で、問い合わせの失敗はそのまま返す。
    #[test]
    fn kernel_params_reject_size_and_count_mismatches() {
        let sizes = [8_usize, 4];
        let query = |index: usize| Ok(sizes.get(index).copied());
        assert_invalid(
            check_kernel_params(&[8, 8], query),
            "submit: kernel parameter 1 takes 4 bytes but 8 were recorded",
        );
        assert_invalid(
            check_kernel_params(&[8, 4, 4], query),
            "submit: the kernel takes 2 parameters but 3 were recorded",
        );
        assert_invalid(
            check_kernel_params(&[8], query),
            "submit: the kernel takes more than the 1 recorded parameters",
        );
        let failure = ContractError::BackendFailure {
            op: "cuFuncGetParamInfo",
            backend: BACKEND,
            detail: "x".into(),
        };
        assert_eq!(
            check_kernel_params(&[8], |_| Err(failure.clone())),
            Err(failure)
        );
    }
}

#[cfg(all(test, feature = "native-blocking-wait"))]
mod dispatch_device_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// 各試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// `out[i] = inp[i] * k + bias` を n 要素で書く。引数の順は slot 0〜4。
    const SCALE: &str = "extern \"C\" __global__ void scale(float* out, const float* inp, float k, float bias, unsigned int n) { unsigned int i = blockIdx.x * blockDim.x + threadIdx.x; if (i < n) { out[i] = inp[i] * k + bias; } }";

    /// ブロックごとに動的な共有メモリへ写し、ブロックの中で逆順に書き戻す。
    const REVERSE: &str = "extern \"C\" __global__ void reverse(float* out, const float* inp) { extern __shared__ float tile[]; unsigned int t = threadIdx.x; unsigned int base = blockIdx.x * blockDim.x; tile[t] = inp[base + t]; __syncthreads(); out[base + t] = tile[blockDim.x - 1 - t]; }";

    /// 各スレッドが、ブロックの番号とブロックの大きさを書く。
    const SHAPE: &str = "extern \"C\" __global__ void shape(unsigned int* out) { unsigned int i = blockIdx.x * blockDim.x + threadIdx.x; out[2 * i] = blockIdx.x; out[2 * i + 1] = blockDim.x; }";

    /// 2 つのポインタのアドレスを整数にしてから差を書く。ポインタの先は読まない。
    const DIFF: &str = "extern \"C\" __global__ void diff(unsigned long long* out, const char* a, const char* b) { out[0] = (unsigned long long)b - (unsigned long long)a; }";

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    fn usage() -> BufferUsage {
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    fn pipeline(
        d: &CudaDevice,
        source: &'static str,
        entry: &'static str,
        slots: Vec<u32>,
    ) -> PipelineHandle {
        d.create_pipeline(
            PipelineInput::CudaC {
                source,
                entry_point: entry,
            },
            &BindingLayout::new(slots),
        )
        .expect("pipeline")
    }

    fn floats(d: &CudaDevice, values: &[f32]) -> BufferHandle {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let h = d.allocate(bytes.len() as u64, usage()).expect("allocate");
        d.upload(&h, 0, &bytes).expect("upload");
        h
    }

    fn read_floats(d: &CudaDevice, h: &BufferHandle) -> Vec<f32> {
        let bytes = d.download_blocking(h, 0, h.size()).expect("download");
        let (words, rest) = bytes.as_chunks::<4>();
        assert!(rest.is_empty());
        words.iter().map(|w| f32::from_ne_bytes(*w)).collect()
    }

    fn records_of(d: &CudaDevice, encoder: &CommandEncoder) -> usize {
        d.open
            .lock()
            .get(&encoder.__backend_id())
            .map_or(0, Vec::len)
    }

    // COMP-3
    // COMP-4
    // COMP-26
    // COMP-27
    // COMP-32
    // COMP-6
    /// 束縛と定数を slot の順にカーネル引数として渡し、グリッドとブロックの大きさで起動した結果が CPU 参照と一致する。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_ex_passes_bindings_and_constants_and_launches_the_grid() {
        within(|| {
            let d = device();
            let p = pipeline(&d, SCALE, "scale", vec![0, 1, 2, 3, 4]);
            let input: Vec<f32> = (0..50).map(|i| i as f32).collect();
            let inp = floats(&d, &input);
            let out = floats(&d, &[0.0; 64]);
            let (k, bias, n) = (
                3.0_f32.to_ne_bytes(),
                0.5_f32.to_ne_bytes(),
                50_u32.to_ne_bytes(),
            );
            let mut e = d.create_encoder();
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(1, &inp, 0), Binding::new(0, &out, 0)],
                &[
                    ConstantBinding::new(4, &n),
                    ConstantBinding::new(2, &k),
                    ConstantBinding::new(3, &bias),
                ],
                [2, 1, 1],
                [32, 1, 1],
                &[],
            )
            .expect("dispatch_ex");
            d.submit(e, &[]).expect("submit");
            let got = read_floats(&d, &out);
            for (i, value) in got.iter().enumerate() {
                let expected = if i < 50 { input[i] * 3.0 + 0.5 } else { 0.0 };
                assert_eq!(value.to_bits(), expected.to_bits(), "element {i}");
            }
        });
    }

    // COMP-33
    /// `ThreadgroupMemory` の `bytes` の合計を動的な共有メモリとして起動し、`index` は使わない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_ex_launches_with_the_sum_of_threadgroup_memory_as_dynamic_shared_memory() {
        within(|| {
            let d = device();
            let p = pipeline(&d, REVERSE, "reverse", vec![0, 1]);
            let input: Vec<f32> = (0..128).map(|i| i as f32).collect();
            let inp = floats(&d, &input);
            let out = floats(&d, &[0.0; 128]);
            let mut e = d.create_encoder();
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &out, 0), Binding::new(1, &inp, 0)],
                &[],
                [2, 1, 1],
                [64, 1, 1],
                &[
                    ThreadgroupMemory::new(7, 128),
                    ThreadgroupMemory::new(7, 128),
                ],
            )
            .expect("dispatch_ex");
            d.submit(e, &[]).expect("submit");
            let got = read_floats(&d, &out);
            let expected: Vec<f32> = (0..2)
                .flat_map(|block| (0..64).rev().map(move |t| (block * 64 + t) as f32))
                .collect();
            assert_eq!(got, expected);
        });
    }

    // COMP-34
    /// 簡易版の `dispatch` は、ブロックの大きさ `[1, 1, 1]` で起動する。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_launches_blocks_of_one_thread() {
        within(|| {
            let d = device();
            let p = pipeline(&d, SHAPE, "shape", vec![0]);
            let out = d.allocate(32, usage()).expect("allocate");
            d.upload(&out, 0, &[0xff; 32]).expect("upload");
            let mut e = d.create_encoder();
            d.dispatch(&mut e, &p, &[Binding::new(0, &out, 0)], [4, 1, 1])
                .expect("dispatch");
            d.submit(e, &[]).expect("submit");
            let bytes = d.download_blocking(&out, 0, 32).expect("download");
            let got: Vec<u32> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|w| u32::from_ne_bytes(*w))
                .collect();
            assert_eq!(got, vec![0, 1, 1, 1, 2, 1, 3, 1]);
        });
    }

    // COMP-36
    /// 同じ記録先に記録した 2 つの dispatch とクリアは記録の順に走り、後の dispatch は前の書き込みを読む。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatches_run_in_record_order_and_see_earlier_writes() {
        within(|| {
            let d = device();
            let p = pipeline(&d, SCALE, "scale", vec![0, 1, 2, 3, 4]);
            let input: Vec<f32> = (0..32).map(|i| i as f32).collect();
            let a = floats(&d, &input);
            let b = floats(&d, &[0.0; 32]);
            let c = floats(&d, &[0.0; 32]);
            let n = 32_u32.to_ne_bytes();
            let (two, one, zero) = (
                2.0_f32.to_ne_bytes(),
                1.0_f32.to_ne_bytes(),
                0.0_f32.to_ne_bytes(),
            );
            let mut e = d.create_encoder();
            // b = a * 2 + 1
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &b, 0), Binding::new(1, &a, 0)],
                &[
                    ConstantBinding::new(2, &two),
                    ConstantBinding::new(3, &one),
                    ConstantBinding::new(4, &n),
                ],
                [1, 1, 1],
                [32, 1, 1],
                &[],
            )
            .expect("first");
            // a の先頭 16 バイトを 0 にする。
            d.clear_buffer(&mut e, &a, 0, 16).expect("clear");
            // c = b * 2 + 0
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &c, 0), Binding::new(1, &b, 0)],
                &[
                    ConstantBinding::new(2, &two),
                    ConstantBinding::new(3, &zero),
                    ConstantBinding::new(4, &n),
                ],
                [1, 1, 1],
                [32, 1, 1],
                &[],
            )
            .expect("second");
            d.submit(e, &[]).expect("submit");
            let expected: Vec<f32> = input.iter().map(|x| (x * 2.0 + 1.0) * 2.0).collect();
            assert_eq!(read_floats(&d, &c), expected);
            let mut cleared = input.clone();
            cleared[..4].fill(0.0);
            assert_eq!(read_floats(&d, &a), cleared);
        });
    }

    // COMP-88
    /// 束縛のポインタには `offset` を足し、`offset` はバッファの長さと比べない（ポインタの先は読まない）。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_ex_adds_the_offset_to_the_pointer_without_an_upper_check() {
        within(|| {
            let d = device();
            let p = pipeline(&d, DIFF, "diff", vec![0, 1, 2]);
            let out = d.allocate(8, usage()).expect("allocate");
            let target = d.allocate(64, usage()).expect("allocate");
            for offset in [16_u64, 4096] {
                let mut e = d.create_encoder();
                d.dispatch_ex(
                    &mut e,
                    &p,
                    &[
                        Binding::new(0, &out, 0),
                        Binding::new(1, &target, 0),
                        Binding::new(2, &target, offset),
                    ],
                    &[],
                    [1, 1, 1],
                    [1, 1, 1],
                    &[],
                )
                .expect("dispatch_ex");
                d.submit(e, &[]).expect("submit");
                let got = d.download_blocking(&out, 0, 8).expect("download");
                assert_eq!(u64::from_ne_bytes(got.try_into().expect("8 bytes")), offset);
            }
        });
    }

    // COMP-29
    /// 空の定数は記録の時点で slot を含む `InvalidArgument` で、記録先に何も足さない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_ex_rejects_an_empty_constant_before_recording() {
        within(|| {
            let d = device();
            let p = pipeline(&d, SCALE, "scale", vec![0, 1, 2, 3, 4]);
            let a = floats(&d, &[0.0; 4]);
            let four = [0_u8; 4];
            let mut e = d.create_encoder();
            let result = d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &a, 0), Binding::new(1, &a, 0)],
                &[
                    ConstantBinding::new(2, &four),
                    ConstantBinding::new(3, &[]),
                    ConstantBinding::new(4, &four),
                ],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            );
            match result {
                Err(ContractError::InvalidArgument(msg)) => {
                    assert_eq!(msg, "dispatch_ex: constant slot 3 is empty")
                }
                other => panic!("unexpected: {other:?}"),
            }
            assert_eq!(records_of(&d, &e), 0);
        });
    }

    // COMP-35
    /// 配置にある slot を省いた dispatch は、記録の時点で欠けた slot を含む `InvalidArgument` で、記録先に何も足さない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_ex_rejects_an_omitted_slot_before_recording() {
        within(|| {
            let d = device();
            let p = pipeline(&d, REVERSE, "reverse", vec![0, 1]);
            let a = floats(&d, &[0.0; 4]);
            let mut e = d.create_encoder();
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &a, 0), Binding::new(1, &a, 0)],
                &[],
                [1, 1, 1],
                [4, 1, 1],
                &[ThreadgroupMemory::new(0, 16)],
            )
            .expect("first");
            let result = d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &a, 0)],
                &[],
                [1, 1, 1],
                [4, 1, 1],
                &[ThreadgroupMemory::new(0, 16)],
            );
            match result {
                Err(ContractError::InvalidArgument(msg)) => {
                    assert_eq!(
                        msg,
                        "dispatch_ex: slot 1 of the binding layout is not provided"
                    )
                }
                other => panic!("unexpected: {other:?}"),
            }
            assert_eq!(records_of(&d, &e), 1);
        });
    }

    // COMP-39
    /// 束縛したバッファを投入の前に `free` すると、投入は slot を含む `InvalidArgument` で、同じ記録先の前のクリアも積まない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn submit_rejects_a_bound_buffer_freed_before_submit_and_enqueues_nothing() {
        within(|| {
            let d = device();
            let p = pipeline(&d, REVERSE, "reverse", vec![0, 1]);
            let kept = floats(&d, &[5.0; 4]);
            let freed = floats(&d, &[1.0; 4]);
            let mut e = d.create_encoder();
            d.clear_buffer(&mut e, &kept, 0, 16).expect("clear");
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &kept, 0), Binding::new(1, &freed, 0)],
                &[],
                [1, 1, 1],
                [4, 1, 1],
                &[ThreadgroupMemory::new(0, 16)],
            )
            .expect("dispatch_ex");
            d.free(freed);
            match d.submit(e, &[]) {
                Err(ContractError::InvalidArgument(msg)) => {
                    assert_eq!(
                        msg,
                        "a bound buffer was freed before submit (binding slot 1)"
                    )
                }
                other => panic!("unexpected: {other:?}"),
            }
            assert_eq!(read_floats(&d, &kept), vec![5.0; 4]);
        });
    }

    /// カーネルの仮引数と記録した引数の数が食い違うと（記録が多い・少ないのどちらも）、投入は `InvalidArgument` で、
    /// 同じ記録先の前のクリアも積まない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn submit_rejects_a_parameter_count_mismatch_and_enqueues_nothing() {
        within(|| {
            let d = device();
            // カーネルの仮引数は 2 つで、配置は 3 つの slot を宣言する。
            let p = pipeline(&d, REVERSE, "reverse", vec![0, 1, 2]);
            let kept = floats(&d, &[5.0; 4]);
            let four = 4_u32.to_ne_bytes();
            let mut e = d.create_encoder();
            d.clear_buffer(&mut e, &kept, 0, 16).expect("clear");
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &kept, 0), Binding::new(1, &kept, 0)],
                &[ConstantBinding::new(2, &four)],
                [1, 1, 1],
                [4, 1, 1],
                &[ThreadgroupMemory::new(0, 16)],
            )
            .expect("dispatch_ex");
            match d.submit(e, &[]) {
                Err(ContractError::InvalidArgument(msg)) => {
                    assert_eq!(
                        msg,
                        "submit: the kernel takes 2 parameters but 3 were recorded"
                    )
                }
                other => panic!("unexpected: {other:?}"),
            }
            assert_eq!(read_floats(&d, &kept), vec![5.0; 4]);
            // カーネルの仮引数は 3 つで、配置は 2 つの slot を宣言する。
            let short = pipeline(&d, DIFF, "diff", vec![0, 1]);
            let mut e = d.create_encoder();
            d.clear_buffer(&mut e, &kept, 0, 16).expect("clear");
            d.dispatch_ex(
                &mut e,
                &short,
                &[Binding::new(0, &kept, 0), Binding::new(1, &kept, 0)],
                &[],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            )
            .expect("dispatch_ex");
            match d.submit(e, &[]) {
                Err(ContractError::InvalidArgument(msg)) => {
                    assert_eq!(
                        msg,
                        "submit: the kernel takes more than the 2 recorded parameters"
                    )
                }
                other => panic!("unexpected: {other:?}"),
            }
            assert_eq!(read_floats(&d, &kept), vec![5.0; 4]);
        });
    }

    // COMP-3
    /// 別の device のパイプラインを渡した `dispatch` と `dispatch_ex` は `DeviceMismatch` で、記録先に何も足さない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_before_recording() {
        within(|| {
            let d = device();
            let other = device();
            let p = pipeline(&other, SHAPE, "shape", vec![0]);
            let out = d.allocate(32, usage()).expect("allocate");
            let mut e = d.create_encoder();
            assert_eq!(
                d.dispatch(&mut e, &p, &[Binding::new(0, &out, 0)], [4, 1, 1]),
                Err(ContractError::DeviceMismatch)
            );
            assert_eq!(
                d.dispatch_ex(
                    &mut e,
                    &p,
                    &[Binding::new(0, &out, 0)],
                    &[],
                    [4, 1, 1],
                    [1, 1, 1],
                    &[],
                ),
                Err(ContractError::DeviceMismatch)
            );
            assert_eq!(records_of(&d, &e), 0);
        });
    }
}

#[cfg(test)]
mod dispatch_check_tests {
    use super::*;

    /// 試験の上限。x は 2^31 - 1、y・z は 65,535。
    const MAX: [u32; 3] = [2_147_483_647, 65_535, 65_535];

    /// `InvalidArgument` で、理由の文が `expected` と全文で同じことを照らす。
    fn assert_invalid(result: Result<(), ContractError>, expected: &str) {
        match result {
            Err(ContractError::InvalidArgument(msg)) => assert_eq!(msg, expected),
            other => panic!("expected InvalidArgument {expected:?}, got {other:?}"),
        }
    }

    // COMP-86
    /// 各軸が上限ちょうどまでの `dispatch_dims` は通る。
    #[test]
    fn dispatch_dims_pass_up_to_the_limit_on_each_axis() {
        for dims in [
            [1, 1, 1],
            [2_147_483_647, 1, 1],
            [1, 65_535, 1],
            [1, 1, 65_535],
            [2_147_483_647, 65_535, 65_535],
        ] {
            assert_eq!(check_dispatch_dims(dims, MAX), Ok(()), "{dims:?}");
        }
        assert_eq!(check_dispatch_dims([u32::MAX; 3], [u32::MAX; 3]), Ok(()));
    }

    // COMP-82
    /// どれかの軸が 0 なら、上限を超える軸があっても 0 の文を返す。
    #[test]
    fn dispatch_dims_reject_a_zero_on_any_axis_before_the_limit() {
        for dims in [
            [0, 1, 1],
            [1, 0, 1],
            [1, 1, 0],
            [0, 0, 0],
            [0, 65_536, 1],
            [2_147_483_648, 1, 0],
        ] {
            assert_invalid(
                check_dispatch_dims(dims, MAX),
                "dispatch_dims contains zero",
            );
        }
    }

    // COMP-86
    /// どれかの軸が上限を 1 でも超えれば、上限の文を返す。
    #[test]
    fn dispatch_dims_reject_one_more_than_the_limit_on_each_axis() {
        for (dims, max) in [
            ([2_147_483_648, 1, 1], MAX),
            ([1, 65_536, 1], MAX),
            ([1, 1, 65_536], MAX),
            ([u32::MAX, u32::MAX, u32::MAX], MAX),
            ([5, 1, 1], [4, 4, 4]),
        ] {
            assert_invalid(
                check_dispatch_dims(dims, max),
                "dispatch_dims exceeds the limit",
            );
        }
    }
}

/// dispatch の記録の時点の拒否の、CUDA の device に触る試験。拒んだ呼び出しが記録先の記録を変えないことを照らし、
/// 通った記録は投入して結果を読む。device の無い環境では流さない（TASK-806 のポインタ）。
#[cfg(all(test, feature = "native-blocking-wait"))]
mod dispatch_check_device_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// 各試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// 各スレッドが `out[0]` に 1 を足す。`out` の先頭の 1 語だけを使う。
    const COUNT: &str =
        "extern \"C\" __global__ void count(unsigned int* out) { atomicAdd(out, 1u); }";

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    fn usage() -> BufferUsage {
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    /// `COUNT` のパイプライン。配置は slot 0 だけ。
    fn count_pipeline(d: &CudaDevice) -> PipelineHandle {
        d.create_pipeline(
            PipelineInput::CudaC {
                source: COUNT,
                entry_point: "count",
            },
            &BindingLayout::new(vec![0]),
        )
        .expect("pipeline")
    }

    /// 0 で始まる 4 バイトの数え先。
    fn counter(d: &CudaDevice) -> BufferHandle {
        let h = d.allocate(4, usage()).expect("allocate");
        d.upload(&h, 0, &[0; 4]).expect("upload");
        h
    }

    fn read_count(d: &CudaDevice, h: &BufferHandle) -> u32 {
        let bytes = d.download_blocking(h, 0, 4).expect("download");
        let (words, rest) = bytes.as_chunks::<4>();
        assert!(rest.is_empty());
        assert_eq!(words.len(), 1);
        u32::from_ne_bytes(words[0])
    }

    fn records_of(d: &CudaDevice, encoder: &CommandEncoder) -> usize {
        d.open
            .lock()
            .get(&encoder.__backend_id())
            .map_or(0, Vec::len)
    }

    /// `InvalidArgument` で、理由の文が `expected` と全文で同じことを照らす。
    fn assert_invalid(result: Result<(), ContractError>, expected: &str) {
        match result {
            Err(ContractError::InvalidArgument(msg)) => assert_eq!(msg, expected),
            other => panic!("expected InvalidArgument {expected:?}, got {other:?}"),
        }
    }

    // COMP-77
    /// 内部 id 999,999 の偽造したパイプラインを渡す `dispatch` と `dispatch_ex` は `dispatch_ex: unknown pipeline` で、
    /// 記録先の前の記録を変えない。偽造したパイプラインは、0 を含む `dispatch_dims` より先に当たる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_and_dispatch_ex_reject_a_forged_pipeline_before_recording() {
        within(|| {
            let d = device();
            let p = count_pipeline(&d);
            assert_ne!(p.__backend_id(), 999_999);
            let forged = PipelineHandle::__backend_new(999_999, d.id);
            let out = counter(&d);
            let mut e = d.create_encoder();
            d.dispatch(&mut e, &p, &[Binding::new(0, &out, 0)], [4, 1, 1])
                .expect("dispatch");
            assert_invalid(
                d.dispatch(&mut e, &forged, &[Binding::new(0, &out, 0)], [4, 1, 1]),
                "dispatch_ex: unknown pipeline",
            );
            assert_invalid(
                d.dispatch_ex(
                    &mut e,
                    &forged,
                    &[Binding::new(0, &out, 0)],
                    &[],
                    [0, 1, 1],
                    [1, 1, 1],
                    &[],
                ),
                "dispatch_ex: unknown pipeline",
            );
            assert_eq!(records_of(&d, &e), 1);
            d.submit(e, &[]).expect("submit");
            assert_eq!(read_count(&d, &out), 4);
        });
    }

    // COMP-78
    /// 別の device で作った、自 device のパイプラインと同じ内部 id のパイプラインを渡す `dispatch` と `dispatch_ex` は
    /// `DeviceMismatch` で、記録先の前の記録を変えない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_with_the_same_id() {
        within(|| {
            let d = device();
            let other = device();
            let p = count_pipeline(&d);
            let q = count_pipeline(&other);
            assert_eq!(q.__backend_id(), p.__backend_id());
            assert_ne!(q.__backend_device_id(), p.__backend_device_id());
            let out = counter(&d);
            let mut e = d.create_encoder();
            d.dispatch(&mut e, &p, &[Binding::new(0, &out, 0)], [4, 1, 1])
                .expect("dispatch");
            assert_eq!(
                d.dispatch(&mut e, &q, &[Binding::new(0, &out, 0)], [4, 1, 1]),
                Err(ContractError::DeviceMismatch)
            );
            assert_eq!(
                d.dispatch_ex(
                    &mut e,
                    &q,
                    &[Binding::new(0, &out, 0)],
                    &[],
                    [4, 1, 1],
                    [1, 1, 1],
                    &[],
                ),
                Err(ContractError::DeviceMismatch)
            );
            assert_eq!(records_of(&d, &e), 1);
            d.submit(e, &[]).expect("submit");
            assert_eq!(read_count(&d, &out), 4);
        });
    }

    // COMP-89
    /// 作っていない記録先・投入した記録先と同じ id の値・別の device の記録先を渡す `dispatch` と `dispatch_ex` は
    /// `encoder is not open on this device` で、どの記録先にも記録しない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_and_dispatch_ex_reject_an_encoder_that_is_not_open() {
        within(|| {
            let d = device();
            let other = device();
            let p = count_pipeline(&d);
            let out = counter(&d);
            let mut done = d.create_encoder();
            let done_id = done.__backend_id();
            d.dispatch(&mut done, &p, &[Binding::new(0, &out, 0)], [4, 1, 1])
                .expect("dispatch");
            d.submit(done, &[]).expect("submit");
            let mut forged = CommandEncoder::__backend_new(u64::MAX, None);
            let mut stale = CommandEncoder::__backend_new(done_id, None);
            let mut foreign = other.create_encoder();
            for e in [&mut forged, &mut stale, &mut foreign] {
                assert_invalid(
                    d.dispatch(e, &p, &[Binding::new(0, &out, 0)], [4, 1, 1]),
                    "encoder is not open on this device",
                );
                assert_invalid(
                    d.dispatch_ex(
                        e,
                        &p,
                        &[Binding::new(0, &out, 0)],
                        &[],
                        [4, 1, 1],
                        [1, 1, 1],
                        &[],
                    ),
                    "encoder is not open on this device",
                );
                assert_eq!(records_of(&d, e), 0);
            }
            assert_eq!(records_of(&other, &foreign), 0);
            assert_eq!(read_count(&d, &out), 4);
        });
    }

    // COMP-82
    /// どれかの軸が 0 の `dispatch_dims` を渡す `dispatch` と `dispatch_ex` は `dispatch_dims contains zero` で、
    /// 記録先の前の記録を変えない。未知のバッファは 0 より先に、0 は配置にあって渡されない slot より先に当たる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_and_dispatch_ex_reject_dispatch_dims_with_a_zero_before_recording() {
        within(|| {
            let d = device();
            let p = count_pipeline(&d);
            let out = counter(&d);
            let unknown =
                BufferHandle::__backend_new(d.current_generation(), 999_999, d.id, 4, usage());
            let mut e = d.create_encoder();
            d.dispatch(&mut e, &p, &[Binding::new(0, &out, 0)], [4, 1, 1])
                .expect("dispatch");
            for dims in [[0, 1, 1], [1, 0, 1], [1, 1, 0]] {
                assert_invalid(
                    d.dispatch(&mut e, &p, &[Binding::new(0, &out, 0)], dims),
                    "dispatch_dims contains zero",
                );
                assert_invalid(
                    d.dispatch_ex(
                        &mut e,
                        &p,
                        &[Binding::new(0, &out, 0)],
                        &[],
                        dims,
                        [1, 1, 1],
                        &[],
                    ),
                    "dispatch_dims contains zero",
                );
            }
            assert_invalid(
                d.dispatch_ex(
                    &mut e,
                    &p,
                    &[Binding::new(0, &unknown, 0)],
                    &[],
                    [0, 1, 1],
                    [1, 1, 1],
                    &[],
                ),
                "dispatch_ex: unknown buffer handle",
            );
            assert_invalid(
                d.dispatch_ex(&mut e, &p, &[], &[], [0, 1, 1], [1, 1, 1], &[]),
                "dispatch_dims contains zero",
            );
            assert_eq!(records_of(&d, &e), 1);
            d.submit(e, &[]).expect("submit");
            assert_eq!(read_count(&d, &out), 4);
        });
    }

    // COMP-86
    /// y・z が 65,535 の `dispatch_dims` は記録して起動し、y・z の 65,536 と、x が `limits().max_dispatch_dims` の x を
    /// 1 超える値は `dispatch_dims exceeds the limit` で記録しない。上限の超過は、配置にあって渡されない slot より先に当たる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dispatch_and_dispatch_ex_record_the_y_and_z_limit_and_reject_one_more() {
        within(|| {
            let d = device();
            assert_eq!(d.limits().max_dispatch_dims[1], 65_535);
            assert_eq!(d.limits().max_dispatch_dims[2], 65_535);
            let p = count_pipeline(&d);
            let y = counter(&d);
            let z = counter(&d);
            let mut e = d.create_encoder();
            d.dispatch(&mut e, &p, &[Binding::new(0, &y, 0)], [1, 65_535, 1])
                .expect("dispatch");
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &z, 0)],
                &[],
                [1, 1, 65_535],
                [1, 1, 1],
                &[],
            )
            .expect("dispatch_ex");
            let x = d.limits().max_dispatch_dims[0] + 1;
            for dims in [[x, 1, 1], [1, 65_536, 1], [1, 1, 65_536]] {
                assert_invalid(
                    d.dispatch(&mut e, &p, &[Binding::new(0, &y, 0)], dims),
                    "dispatch_dims exceeds the limit",
                );
                assert_invalid(
                    d.dispatch_ex(
                        &mut e,
                        &p,
                        &[Binding::new(0, &z, 0)],
                        &[],
                        dims,
                        [1, 1, 1],
                        &[],
                    ),
                    "dispatch_dims exceeds the limit",
                );
            }
            assert_invalid(
                d.dispatch_ex(&mut e, &p, &[], &[], [1, 65_536, 1], [1, 1, 1], &[]),
                "dispatch_dims exceeds the limit",
            );
            assert_eq!(records_of(&d, &e), 2);
            d.submit(e, &[]).expect("submit");
            assert_eq!(read_count(&d, &y), 65_535);
            assert_eq!(read_count(&d, &z), 65_535);
        });
    }
}

/// ロストの後の操作の照らし `lost_result` の、device に触らない試験（TASK-188）。
#[cfg(test)]
mod lost_check_tests {
    use super::*;

    #[test]
    fn lost_result_passes_when_not_lost() {
        for g in [1_u64, 7, u64::MAX] {
            assert_eq!(lost_result(false, g), Ok(()));
        }
    }

    #[test]
    fn lost_result_carries_the_given_generation_when_lost() {
        for g in [1_u64, 7, u64::MAX] {
            assert_eq!(
                lost_result(true, g),
                Err(ContractError::DeviceLost { generation: g })
            );
        }
    }
}

/// CUDA の device に触る、模擬のロストの後の操作の拒否と作り直した device の試験（TASK-188）。device の無い環境では流さない。
#[cfg(all(test, feature = "native-blocking-wait", feature = "fault-injection"))]
mod lost_invalidation_device_tests {
    use super::*;
    use fandhe_silicon_core::error::CompletionStateEx;
    use fandhe_silicon_core::execution::{DType, OperandSpec};
    use fandhe_silicon_core::fault::{HookFault, inject};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// 各試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// 各スレッドが `out[0]` に 1 を足す。`out` の先頭の 1 語だけを使う。
    const COUNT: &str =
        "extern \"C\" __global__ void count(unsigned int* out) { atomicAdd(out, 1u); }";

    /// 確保の長さ。
    const LEN: u64 = 64;

    /// `download_async` の callback が受けた結果を溜める入れ物。
    type Received = Arc<Mutex<Vec<Result<Vec<u8>, ContractError>>>>;

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    fn request() -> DeviceRequest {
        DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        }
    }

    fn device() -> CudaDevice {
        open_device(request()).expect("CUDA device")
    }

    fn usage() -> BufferUsage {
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    fn lose(d: &dyn Device) {
        inject(
            d.fault_hooks(),
            HookFault::DeviceLost(DeviceLostReason::Unknown),
        )
        .expect("loss");
    }

    /// `COUNT` のパイプライン。配置は slot 0 だけ。
    fn count_pipeline(compute: &dyn ComputeOps) -> PipelineHandle {
        compute
            .create_pipeline(
                PipelineInput::CudaC {
                    source: COUNT,
                    entry_point: "count",
                },
                &BindingLayout::new(vec![0]),
            )
            .expect("pipeline")
    }

    fn pattern() -> Vec<u8> {
        (0..LEN)
            .map(|i| (i as u8).wrapping_mul(7).wrapping_add(3))
            .collect()
    }

    /// 開いている記録先の記録の数。集合に無い記録先は `None`。
    fn records_of(d: &CudaDevice, encoder: &CommandEncoder) -> Option<usize> {
        d.open.lock().get(&encoder.__backend_id()).map(Vec::len)
    }

    /// 投入済みの表にある通し番号の完了の印の持ち手の数。表に無い番号は `None`。
    fn event_holders(d: &CudaDevice, sequence: u64) -> Option<usize> {
        d.lock_submissions()
            .pending
            .get(&sequence)
            .map(Arc::strong_count)
    }

    // DEV-139
    // DEV-140
    /// 2 回の模擬のロストでも通知は 1 回で、世代は 1 回目にだけ進む。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn two_losses_call_the_callback_once_and_move_the_generation_once() {
        within(|| {
            let d = device();
            let seen = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&seen);
            d.set_lost_callback(Box::new(move |r| sink.lock().unwrap().push(r)));
            let before = d.generation();
            lose(&d);
            let after = d.generation();
            lose(&d);
            assert_eq!(*seen.lock().unwrap(), vec![DeviceLostReason::Unknown]);
            assert!(after > before);
            assert_eq!(d.generation(), after);
        });
    }

    // DEV-141
    /// ロストの後は、並べた操作が呼んだ時点の世代の `DeviceLost`（`poll`・`poll_ex` は `Lost`）を返し、何も記録・投入・払い出しをしない。
    /// 0 の長さ・表に無いハンドル・開いていない記録先・0 の要素・空でない `wait`・空の `submit_many` も `DeviceLost` になる。
    /// 2 回繰り返しても同じで、世代は変わらない。ロスト前のバッファは `free` で表から外れる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn the_listed_operations_issue_nothing_after_the_loss() {
        within(|| {
            let d = device();
            let a = d.allocate(LEN, usage()).expect("allocate a");
            let b = d.allocate(LEN, usage()).expect("allocate b");
            let c = d.allocate(4, usage()).expect("allocate c");
            d.upload(&a, 0, &pattern()).expect("upload");
            let issued = d.submit(d.create_encoder(), &[]).expect("submit");
            d.wait_blocking(issued, None).expect("wait_blocking");
            let p = count_pipeline(&d);
            let recorded = |count: usize| -> Vec<CommandEncoder> {
                (0..count)
                    .map(|_| {
                        let mut e = d.create_encoder();
                        d.dispatch(&mut e, &p, &[Binding::new(0, &c, 0)], [4, 1, 1])
                            .expect("dispatch");
                        e
                    })
                    .collect()
            };
            let mut for_submit = recorded(2);
            let mut for_many = recorded(2);
            let mut empties = [d.create_encoder(), d.create_encoder()];
            let buffers_before = {
                let guard = d.lock();
                (guard.map.len(), guard.next_id)
            };
            let sequence_before = d.lock_submissions().next_sequence;

            lose(&d);
            let generation = d.generation();
            let lost = || Err(ContractError::DeviceLost { generation });
            let forged = BufferHandle::__backend_new(generation, 999_999, d.id, LEN, usage());
            let shape = [1_u64];
            let stride = [1_i64];
            let declared = [OperandSpec {
                slot: 0,
                dtype: DType::U32,
                shape: &shape,
                stride: &stride,
                alignment: 4,
            }];

            for empty in &mut empties {
                assert_eq!(d.allocate(LEN, usage()).map(|_| ()), lost());
                assert_eq!(d.allocate(0, usage()).map(|_| ()), lost());
                assert_eq!(d.upload(&a, 0, &pattern()), lost());
                assert_eq!(d.upload(&forged, 0, &[1]), lost());
                assert_eq!(d.download_blocking(&a, 0, LEN).map(|_| ()), lost());
                let got: Received = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&got);
                d.download_async(&a, 0, LEN, Box::new(move |r| sink.lock().unwrap().push(r)));
                assert_eq!(*got.lock().unwrap(), vec![lost().map(|()| Vec::new())]);
                let started = Instant::now();
                assert_eq!(
                    d.wait_blocking(issued, Some(Duration::from_secs(5))),
                    lost()
                );
                assert!(started.elapsed() < Duration::from_secs(5));
                assert_eq!(
                    d.dispatch(empty, &p, &[Binding::new(0, &c, 0)], [4, 1, 1]),
                    lost()
                );
                assert_eq!(
                    d.dispatch_ex(
                        &mut CommandEncoder::__backend_new(u64::MAX, None),
                        &p,
                        &[Binding::new(0, &c, 0)],
                        &[],
                        [0, 1, 1],
                        [1, 1, 1],
                        &[],
                    ),
                    lost()
                );
                assert_eq!(
                    d.dispatch_checked(
                        empty,
                        &p,
                        &[Binding::new(0, &c, 0)],
                        &[],
                        &declared,
                        [4, 1, 1],
                        [1, 1, 1],
                        &[],
                    ),
                    lost()
                );
                assert_eq!(d.copy_buffer_to_buffer(empty, &a, 0, &b, 0, LEN), lost());
                assert_eq!(d.clear_buffer(empty, &a, 0, LEN), lost());
                assert_eq!(
                    d.copy_buffer_to_buffer(
                        &mut CommandEncoder::__backend_new(u64::MAX, None),
                        &a,
                        0,
                        &b,
                        0,
                        LEN
                    ),
                    lost()
                );
                assert_eq!(
                    d.clear_buffer(
                        &mut CommandEncoder::__backend_new(u64::MAX, None),
                        &a,
                        0,
                        LEN
                    ),
                    lost()
                );
                assert_eq!(d.submit(for_submit.remove(0), &[]).map(|_| ()), lost());
                assert_eq!(
                    d.submit(
                        d.create_encoder(),
                        &[ImportedSemaphoreHandle::__backend_new(1)]
                    )
                    .map(|_| ()),
                    lost()
                );
                assert_eq!(
                    d.submit_many(vec![for_many.remove(0)], &[], &[])
                        .map(|_| ()),
                    lost()
                );
                assert_eq!(d.submit_many(Vec::new(), &[], &[]).map(|_| ()), lost());
                assert_eq!(d.poll(issued), CompletionState::Lost);
                assert_eq!(d.poll_ex(issued), CompletionStateEx::Lost);
                assert_eq!(
                    d.poll(SubmitToken::__backend_new(
                        generation,
                        issued.sequence,
                        d.id
                    )),
                    CompletionState::Lost
                );
                assert_eq!(records_of(&d, empty), Some(0));
                assert_eq!(d.generation(), generation);
            }

            {
                let guard = d.lock();
                assert_eq!((guard.map.len(), guard.next_id), buffers_before);
            }
            assert_eq!(d.lock_submissions().next_sequence, sequence_before);
            d.free(a);
            assert_eq!(d.lock().map.len(), buffers_before.0 - 1);
        });
    }

    // DEV-141
    /// 門で止めた投入の待ちに入ったこと（完了の印の持ち手が 2 つになること）を見てから模擬のロストを起こし、門を開けて
    /// 投入が終わると、待ちは呼んだ後の世代の `DeviceLost` を返す。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_wait_that_spans_the_loss_returns_device_lost() {
        within(|| {
            let d = device();
            let started = Instant::now();
            assert_eq!(d.arm_gate(), Ok(()));
            let token = d.submit(d.create_encoder(), &[]).expect("submit");
            assert_eq!(event_holders(&d, token.sequence), Some(1));
            std::thread::scope(|s| {
                let waiter = s.spawn(|| d.wait_blocking(token, Some(Duration::from_secs(10))));
                while event_holders(&d, token.sequence) != Some(2) {
                    assert!(!waiter.is_finished(), "the wait ended before the loss");
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(
                    started.elapsed() < GATE_EXPIRY / 2,
                    "the wait was reached too late"
                );
                lose(&d);
                assert_eq!(d.open_gate(), Ok(()));
                assert_eq!(
                    waiter.join().expect("waiter"),
                    Err(ContractError::DeviceLost {
                        generation: d.generation()
                    })
                );
            });
            assert_eq!(d.poll(token), CompletionState::Lost);
        });
    }

    // DEV-147
    /// ロストの後に `request` で得た新しい device は、確保・upload・コピーと dispatch の投入・読み戻しが通る。
    /// ロストした device は作り直されず、`allocate` は `DeviceLost` のまま。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_new_request_after_the_loss_runs_a_dispatch_and_reads_back() {
        within(|| {
            let lost_device = device();
            let before = lost_device.generation();
            lose(&lost_device);
            let after = lost_device.generation();

            let fresh = CudaDeviceProvider::request(request()).expect("request");
            assert!(fresh.generation() > before);
            assert!(fresh.generation() > after);
            let memory = fresh.memory();
            let queue = fresh.queue();
            let src = memory.allocate(LEN, usage()).expect("allocate src");
            let dst = memory.allocate(LEN, usage()).expect("allocate dst");
            let counter = memory.allocate(4, usage()).expect("allocate counter");
            memory.upload(&src, 0, &pattern()).expect("upload");
            memory.upload(&counter, 0, &[0; 4]).expect("upload counter");
            let p = count_pipeline(fresh.compute().expect("compute"));
            let mut e = queue.create_encoder();
            memory
                .copy_buffer_to_buffer(&mut e, &src, 0, &dst, 0, LEN)
                .expect("copy");
            queue
                .dispatch(&mut e, &p, &[Binding::new(0, &counter, 0)], [16, 1, 1])
                .expect("dispatch");
            let token = queue.submit(e, &[]).expect("submit");
            queue
                .wait_blocking(token, Some(Duration::from_secs(10)))
                .expect("wait_blocking");
            assert_eq!(
                memory.download_blocking(&dst, 0, LEN).expect("download"),
                pattern()
            );
            assert_eq!(
                memory
                    .download_blocking(&counter, 0, 4)
                    .expect("download counter"),
                16_u32.to_ne_bytes()
            );
            assert_eq!(
                lost_device.allocate(LEN, usage()).map(|_| ()),
                Err(ContractError::DeviceLost {
                    generation: lost_device.generation()
                })
            );
        });
    }
}

/// CUDA の device に触る、模擬のロストの後のパイプラインの生成とロストの理由の試験（TASK-561）。device の無い環境では流さない。
#[cfg(all(test, feature = "fault-injection"))]
mod lost_pipeline_device_tests {
    use super::*;
    use fandhe_silicon_core::execution::{DType, OperandSpec};
    use fandhe_silicon_core::fault::{HookFault, inject};
    use std::sync::mpsc;
    use std::time::Duration;

    /// 各試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// 各スレッドが `out[0]` に 1 を足す。`out` の先頭の 1 語だけを使う。
    const COUNT: &str =
        "extern \"C\" __global__ void count(unsigned int* out) { atomicAdd(out, 1u); }";

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    fn lose(d: &dyn Device, reason: DeviceLostReason) {
        inject(d.fault_hooks(), HookFault::DeviceLost(reason)).expect("loss");
    }

    /// `COUNT` の入力。
    fn count() -> PipelineInput<'static> {
        PipelineInput::CudaC {
            source: COUNT,
            entry_point: "count",
        }
    }

    /// パイプラインの表の件数と次の識別子。
    fn pipelines_of(d: &CudaDevice) -> (usize, u64) {
        let pipelines = d
            .pipelines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (pipelines.map.len(), pipelines.next_id)
    }

    /// NVRTC の呼び出し口に入った回数と、パイプラインの生成が下位の呼び出しの前に数えた回数。
    fn calls_of(d: &CudaDevice) -> (u64, u64) {
        (
            d.nvrtc_calls.load(Ordering::SeqCst),
            d.pipeline_driver_calls.load(Ordering::SeqCst),
        )
    }

    // ERR-55
    /// ロストの後は、`create_pipeline` が `CudaC`・`Ptx`・`Msl`・`Wgsl`・ソースに NUL のバイトを持つ `CudaC`・重複した slot の
    /// どれにも呼んだ時点の世代の `DeviceLost` を返し、NVRTC の呼び出し口と下位の呼び出しに入らず、パイプラインの表を変えない。
    /// 引数の宣言が合う `create_pipeline_checked` と、`Device::compute` から呼んでも同じ。2 回繰り返しても同じで、世代は変わらない。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn create_pipeline_after_the_loss_returns_device_lost_without_compiling() {
        within(|| {
            let d = device();
            let layout = BindingLayout::new(vec![0]);
            let calls_at_open = calls_of(&d);
            d.create_pipeline(count(), &layout)
                .expect("pipeline before the loss");
            let pipelines_before = pipelines_of(&d);
            let calls_before = calls_of(&d);
            assert!(calls_before.0 > calls_at_open.0);
            assert!(calls_before.1 > calls_at_open.1);

            lose(&d, DeviceLostReason::Unknown);
            let generation = d.generation();
            let lost = || Err(ContractError::DeviceLost { generation });
            let shape = [1_u64];
            let stride = [1_i64];
            let declared = [OperandSpec {
                slot: 0,
                dtype: DType::U32,
                shape: &shape,
                stride: &stride,
                alignment: 4,
            }];

            for _ in 0..2 {
                assert_eq!(d.create_pipeline(count(), &layout).map(|_| ()), lost());
                assert_eq!(
                    d.create_pipeline(
                        PipelineInput::Ptx {
                            source: "",
                            entry_point: "count",
                        },
                        &layout,
                    )
                    .map(|_| ()),
                    lost()
                );
                assert_eq!(
                    d.create_pipeline(
                        PipelineInput::Msl {
                            source: "",
                            entry_point: "count",
                        },
                        &layout,
                    )
                    .map(|_| ()),
                    lost()
                );
                assert_eq!(
                    d.create_pipeline(
                        PipelineInput::Wgsl {
                            source: "",
                            entry_point: "count",
                        },
                        &layout,
                    )
                    .map(|_| ()),
                    lost()
                );
                assert_eq!(
                    d.create_pipeline(
                        PipelineInput::CudaC {
                            source: "\0",
                            entry_point: "count",
                        },
                        &layout,
                    )
                    .map(|_| ()),
                    lost()
                );
                assert_eq!(
                    d.create_pipeline(count(), &BindingLayout::new(vec![0, 0]))
                        .map(|_| ()),
                    lost()
                );
                assert_eq!(
                    d.create_pipeline_checked(count(), &layout, &declared)
                        .map(|_| ()),
                    lost()
                );
                let compute = (&d as &dyn Device).compute().expect("compute");
                assert_eq!(
                    compute.create_pipeline(count(), &layout).map(|_| ()),
                    lost()
                );
                assert_eq!(d.generation(), generation);
            }

            assert_eq!(pipelines_of(&d), pipelines_before);
            assert_eq!(calls_of(&d), calls_before);
        });
    }

    // ERR-34
    /// 理由に `Unknown`・`Unrecoverable` を指定した模擬のロストは、通知にその理由で 1 回届く。検証の報告の受け口に登録した
    /// 関数は、報告を模す差し込み（`UnsupportedExtensionDefault` を返し、ロストもしない）でも、模擬のロストでも呼ばれない。
    /// ロストの後の `create_pipeline` は、呼んだ時点の世代の `DeviceLost` を返す。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback() {
        within(|| {
            for reason in [
                DeviceLostReason::Unknown,
                DeviceLostReason::Unrecoverable("injected".into()),
            ] {
                let d = device();
                let notified = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&notified);
                d.set_lost_callback(Box::new(move |r| sink.lock().unwrap().push(r)));
                let reports = Arc::new(Mutex::new(Vec::new()));
                let sink = Arc::clone(&reports);
                ValidationIntake::set_validation_callback(
                    &d,
                    Box::new(move |e| sink.lock().unwrap().push(e)),
                );
                let before = d.generation();

                assert_eq!(
                    d.simulate_validation_report("injected".into()),
                    Err(ContractError::UnsupportedExtensionDefault {
                        op: "simulate_validation_report"
                    })
                );
                assert!(!d.lost.is_lost());
                assert_eq!(d.generation(), before);

                lose(&d, reason.clone());
                assert_eq!(*notified.lock().unwrap(), vec![reason]);
                assert!(d.generation() > before);
                assert!(reports.lock().unwrap().is_empty());
                assert_eq!(d.validation.delivered_count(), 0);
                assert_eq!(d.validation.undelivered_count(), 0);
                assert_eq!(
                    d.create_pipeline(count(), &BindingLayout::new(vec![0]))
                        .map(|_| ()),
                    Err(ContractError::DeviceLost {
                        generation: d.generation()
                    })
                );
            }
        });
    }
}

/// CUDA の device に触る、dispatch を記録した記録先の破棄と、門で止めた dispatch に束縛したバッファの解放の保留の試験（TASK-347）。
/// device の無い環境では流さない。
#[cfg(all(test, feature = "native-blocking-wait", feature = "fault-injection"))]
mod submission_state_device_tests {
    use super::*;
    use fandhe_silicon_core::error::CompletionStateEx;
    use std::sync::mpsc;
    use std::time::Duration;

    /// 各試験の上限。
    const LIMIT: Duration = Duration::from_secs(30);

    /// 各スレッドが `out[threadIdx.x]` に `value` を書く。
    const FILL: &str = "extern \"C\" __global__ void fill(unsigned int* out, unsigned int value) { out[threadIdx.x] = value; }";

    /// 各スレッドが `inp[threadIdx.x]` を `out[threadIdx.x]` に写す。
    const COPY: &str = "extern \"C\" __global__ void copy_words(unsigned int* out, const unsigned int* inp) { out[threadIdx.x] = inp[threadIdx.x]; }";

    /// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
    fn within(body: impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            body();
            let _ = sender.send(());
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(()) => {
                let _ = handle.join();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
                panic!("the test body ended without reporting");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the test body did not end within {LIMIT:?}")
            }
        }
    }

    fn device() -> CudaDevice {
        open_device(DeviceRequest {
            backend: BACKEND,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .expect("CUDA device")
    }

    fn usage() -> BufferUsage {
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    }

    fn pipeline(d: &CudaDevice, source: &'static str, entry: &'static str) -> PipelineHandle {
        d.create_pipeline(
            PipelineInput::CudaC {
                source,
                entry_point: entry,
            },
            &BindingLayout::new(vec![0, 1]),
        )
        .expect("pipeline")
    }

    fn words(d: &CudaDevice, values: &[u32]) -> BufferHandle {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let h = d.allocate(bytes.len() as u64, usage()).expect("allocate");
        d.upload(&h, 0, &bytes).expect("upload");
        h
    }

    fn read_words(d: &CudaDevice, h: &BufferHandle) -> Vec<u32> {
        let bytes = d.download_blocking(h, 0, h.size()).expect("download");
        let (chunks, rest) = bytes.as_chunks::<4>();
        assert!(rest.is_empty());
        chunks.iter().map(|w| u32::from_ne_bytes(*w)).collect()
    }

    fn records_of(d: &CudaDevice, encoder: &CommandEncoder) -> usize {
        d.open
            .lock()
            .get(&encoder.__backend_id())
            .map_or(0, Vec::len)
    }

    /// 印の写しを返す。
    fn copy_of(t: &SubmitToken) -> SubmitToken {
        SubmitToken::__backend_new(t.generation, t.sequence, t.__backend_device_id())
    }

    /// `out` の 4 語に `value` を書く dispatch を `encoder` に記録する。
    fn record_fill(
        d: &CudaDevice,
        encoder: &mut CommandEncoder,
        p: &PipelineHandle,
        out: &BufferHandle,
        value: &[u8],
    ) {
        d.dispatch_ex(
            encoder,
            p,
            &[Binding::new(0, out, 0)],
            &[ConstantBinding::new(1, value)],
            [1, 1, 1],
            [4, 1, 1],
            &[],
        )
        .expect("dispatch_ex");
    }

    // MEM-26
    /// dispatch を記録した記録先を投入せずに捨てると、開いている数は 0 → 1 → 0 で、投入の回数と解放の保留の数は変わらない。
    /// 記録の後も捨てた後の空の投入の後も出力は書かれず、同じ dispatch を記録した別の記録先を投入すると書かれる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing() {
        within(|| {
            let d = device();
            let p = pipeline(&d, FILL, "fill");
            let out = words(&d, &[0; 4]);
            let value = 7_u32.to_ne_bytes();
            let submitted = d.test_submitted_count();
            assert_eq!(d.test_open_encoder_count(), 0);
            let mut dropped = d.create_encoder();
            record_fill(&d, &mut dropped, &p, &out, &value);
            assert_eq!(records_of(&d, &dropped), 1);
            assert_eq!(d.test_open_encoder_count(), 1);
            assert_eq!(read_words(&d, &out), vec![0; 4]);
            drop(dropped);
            assert_eq!(d.test_open_encoder_count(), 0);
            assert_eq!(d.test_submitted_count(), submitted);
            assert_eq!(d.pending_release_count(), Some(0));
            let t = d.submit(d.create_encoder(), &[]).expect("submit");
            assert_eq!(d.wait_blocking(t, None), Ok(()));
            assert_eq!(read_words(&d, &out), vec![0; 4]);
            assert_eq!(d.pending_release_count(), Some(0));
            let mut kept = d.create_encoder();
            record_fill(&d, &mut kept, &p, &out, &value);
            let t = d.submit(kept, &[]).expect("submit");
            assert_eq!(d.wait_blocking(t, None), Ok(()));
            assert_eq!(read_words(&d, &out), vec![7; 4]);
            assert_eq!(d.test_submitted_count(), submitted + 2);
            assert_eq!(d.test_open_encoder_count(), 0);
            d.free(out);
        });
    }

    // MEM-21
    /// 門で止めた dispatch に束縛した入力を `free` すると数が 1 になり、期限 100 ms の待ちが `Timeout` を返した後も 1 のままで、
    /// 状態は `Pending`。門を開けて待ち終えると 0 で、出力は入力の写しになる。
    #[test]
    #[ignore = "requires a CUDA device"]
    fn a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held() {
        within(|| {
            let d = device();
            let p = pipeline(&d, COPY, "copy_words");
            let inp = words(&d, &[1, 2, 3, 4]);
            let out = words(&d, &[0; 4]);
            assert_eq!(d.pending_release_count(), Some(0));
            assert_eq!(d.arm_gate(), Ok(()));
            let mut e = d.create_encoder();
            d.dispatch_ex(
                &mut e,
                &p,
                &[Binding::new(0, &out, 0), Binding::new(1, &inp, 0)],
                &[],
                [1, 1, 1],
                [4, 1, 1],
                &[],
            )
            .expect("dispatch_ex");
            let t = d.submit(e, &[]).expect("submit");
            d.free(inp);
            assert_eq!(d.pending_release_count(), Some(1));
            assert_eq!(
                d.wait_blocking(copy_of(&t), Some(Duration::from_millis(100))),
                Err(ContractError::Timeout)
            );
            assert_eq!(d.pending_release_count(), Some(1));
            assert_eq!(d.poll_ex(copy_of(&t)), CompletionStateEx::Pending);
            assert_eq!(d.pending_release_count(), Some(1));
            assert_eq!(d.open_gate(), Ok(()));
            assert_eq!(d.wait_blocking(copy_of(&t), None), Ok(()));
            assert_eq!(d.pending_release_count(), Some(0));
            assert_eq!(read_words(&d, &out), vec![1, 2, 3, 4]);
            d.free(out);
        });
    }
}
