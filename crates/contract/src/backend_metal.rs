//! Metal の背後実装（下-1）。`backend-metal` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! macOS のときだけ、既定の device の取得（`DeviceProvider::request`）・診断・バッファの確保と読み書き・
//! 投入の窓口（`Device::queue`）を持つ
//! （TASK-158、DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265、TASK-592 のポインタ）。
//! macOS 以外（Linux・wasm32）では、feature を付けても実装は空のままで、公開の項目を持たない
//! （device に触らない純粋な検査の関数だけが、この段の試験のために置かれている）。
//! Objective-C のバインディングは `objc2` 系の 4 crate（macOS 向けにだけ宣言。承認の記録は `deps-approved.toml`）。
//!
//! 公開するのは `MetalDeviceProvider` だけ。device の型 `MetalDevice` は非公開で、上・下-2 は
//! `request` が返す `Box<dyn Device>` 越しに使う。
//!
//! 投入は、記録先ごとに空のコマンドバッファを 1 つ作って確定する（`submit`・`submit_many`）。
//! 記録の操作（コピー・クリア・dispatch）・待ちセマフォ・ロストの通知はこの段では持たない。
//! したがって投入するコマンドバッファはエンコーダを開かず、どのバッファも参照せず、
//! device のメモリへの CPU からの読み書きと GPU の読み書きが重なることはない。
//! 読み書きの `unsafe` ブロックの同期の根拠は、この前提に頼る。
//! 記録の操作（コピー・クリア・dispatch）を足す変更では、これらのブロックの同期の根拠を必ず書き直す。
//!
//! `MetalDevice` は Objective-C のオブジェクトを持つので `Send`・`Sync` ではない（`unsafe impl` は足さない）。
//! 投入済みの表は device の錠の中に置き、記録先の後始末の hook は識別子の集合だけを握る。

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    __BackendSharedEncoderCloser, ImportedSemaphoreHandle, SubmitToken,
};

/// 源から次の値を取る。あふれたら `None` を返し、源は動かさない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn take_next(counter: &AtomicU64) -> Option<u64> {
    // toolchain の版に依らず警告の出ない、比較交換のループで払い出す。
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1)?;
        match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some(current),
            Err(actual) => current = actual,
        }
    }
}

/// コマンドバッファ 1 つの状態。`MTLCommandBufferStatus` を写す。試験では偽の値を使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum BufState {
    /// 完了した。
    Done,
    /// `Error` で終わった。
    Failed,
    /// 未完了。
    Running,
}

/// 1 回の投入（コマンドバッファの列）の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum GroupState {
    Pending,
    Completed,
    Failed,
}

/// 列のどれかが `Failed` なら失敗、全部が `Done` なら完了、他は未完了とする。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn group_state_of(states: impl IntoIterator<Item = BufState>) -> GroupState {
    let mut all_done = true;
    for state in states {
        match state {
            BufState::Failed => return GroupState::Failed,
            BufState::Running => all_done = false,
            BufState::Done => {}
        }
    }
    if all_done {
        GroupState::Completed
    } else {
        GroupState::Pending
    }
}

/// 投入済みの表。投入の通し番号は device ごとに 1 から払い出す。`G` は 1 回の投入が持つ列の型。
///
/// 本物では `G` が Objective-C のコマンドバッファの列で、試験では偽の状態の列を使う。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct Submissions<G> {
    /// 次に払い出す通し番号。
    next_sequence: u64,
    /// この番号以下は回収済み（失敗の集合にあるものを除き完了）。
    watermark: u64,
    /// 通し番号から列への表。回収されるまで保持する。
    pending: BTreeMap<u64, G>,
    /// 失敗として回収した通し番号。watermark 以下の失敗の番号だけを持つので、増えるのは失敗の件数だけ。
    failed: HashSet<u64>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl<G> Submissions<G> {
    /// 何も投入していない表。
    fn new() -> Self {
        Self {
            next_sequence: 1,
            watermark: 0,
            pending: BTreeMap::new(),
            failed: HashSet::new(),
        }
    }

    /// 通し番号の昇順に、決着した先頭から表を外して watermark を進める。未完了で止まる。
    ///
    /// 先頭が未完了なら、後ろが完了していても回収しない。失敗は失敗の集合に移す（完了として数えない）。
    fn reclaim(&mut self, classify: impl Fn(&G) -> GroupState) {
        while let Some(entry) = self.pending.first_entry() {
            let sequence = *entry.key();
            match classify(entry.get()) {
                GroupState::Pending => break,
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
    }

    /// 通し番号の完了状態。呼ぶ前に `reclaim` しておく。
    ///
    /// 表にあれば列の状態、無ければ失敗の集合にあるものと未払い出しの番号（0 を含む）は `Lost`、
    /// watermark 以下の他は `Complete`。
    fn state_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> CompletionState {
        if let Some(group) = self.pending.get(&sequence) {
            return match classify(group) {
                GroupState::Pending => CompletionState::Pending,
                GroupState::Completed => CompletionState::Complete,
                GroupState::Failed => CompletionState::Lost,
            };
        }
        if self.failed.contains(&sequence) {
            return CompletionState::Lost;
        }
        if sequence >= 1 && sequence <= self.watermark {
            CompletionState::Complete
        } else {
            CompletionState::Lost
        }
    }
}

/// 開いている記録先の識別子の集合。`CommandEncoder` の後始末の hook を兼ねる。
///
/// Objective-C の型を持たないので `Send + Sync` で、記録先が device より長く生きても安全に使える。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct OpenEncoders {
    ids: Mutex<HashSet<u64>>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl OpenEncoders {
    fn new() -> Self {
        Self {
            ids: Mutex::new(HashSet::new()),
        }
    }

    /// 集合の錠を取る。更新は挿入と削除だけなので、汚れていても中身を使う。
    fn lock(&self) -> MutexGuard<'_, HashSet<u64>> {
        self.ids.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 渡された識別子が全部開いている集合にあることを確かめ、あれば集合から外す。
    ///
    /// 1 つでも無い（0 を含む）・同じ識別子が重複していれば `InvalidArgument` で、集合は変えない。
    fn claim(&self, ids: &[u64]) -> Result<(), ContractError> {
        let mut open = self.lock();
        let mut seen = HashSet::with_capacity(ids.len());
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

/// `wait_blocking` が待ったあとの `poll` の値を結果に写す。
///
/// 失敗した投入は `DeviceLost` ではなく `BackendFailure`（この背後実装は device のロストを返さない）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn wait_result(state: CompletionState) -> Result<(), ContractError> {
    match state {
        CompletionState::Complete => Ok(()),
        CompletionState::Lost => Err(ContractError::BackendFailure {
            op: "wait_blocking",
            backend: BackendKind::Metal,
            detail: "the submission ended with an error".into(),
        }),
        CompletionState::Pending => Err(ContractError::BackendFailure {
            op: "wait_blocking",
            backend: BackendKind::Metal,
            detail: "submission is still pending after the wait".into(),
        }),
    }
}

/// 空でない `wait` を、何も投入せずに `Unsupported` で拒む。device に触らない。
///
/// この段の Metal の背後実装は取り込み済みセマフォを待てない。`submit`・`submit_ex`・`submit_many` が共有する。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn reject_wait(op: &'static str, wait: &[ImportedSemaphoreHandle]) -> Result<(), ContractError> {
    if wait.is_empty() {
        Ok(())
    } else {
        Err(ContractError::Unsupported {
            op,
            backend: BackendKind::Metal,
        })
    }
}

/// 投入の印が、この device・この世代のものか照らす。device に触らない。
///
/// 別の device の印は `DeviceMismatch`、同じ device で世代が違う印は `DeviceLost`。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
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

/// `submit` が device に触る前に通す検査。`wait` だけを見る。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn precheck_submit(wait: &[ImportedSemaphoreHandle]) -> Result<(), ContractError> {
    reject_wait("submit(wait)", wait)
}

/// `submit_many` が device に触る前に通す検査。順は、空の列、`wait`、`after` の各印の照合。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn precheck_submit_many(
    command_count: usize,
    after: &[SubmitToken],
    wait: &[ImportedSemaphoreHandle],
    device_id: u64,
    generation: u64,
) -> Result<(), ContractError> {
    if command_count == 0 {
        return Err(ContractError::InvalidArgument(
            "submit_many requires at least one encoder".into(),
        ));
    }
    reject_wait("submit_many(wait)", wait)?;
    for token in after {
        check_token_owner(token, device_id, generation)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;
    use std::sync::{Arc, Mutex, MutexGuard};

    use super::{
        BufState, GroupState, OpenEncoders, Submissions, check_token_owner, group_state_of,
        precheck_submit, precheck_submit_many, take_next, wait_result,
    };

    use fandhe_silicon_core::capability::{
        BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
    };
    use fandhe_silicon_core::device::{
        Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
    };
    use fandhe_silicon_core::error::{CompletionState, ContractError, UnsupportedReason};
    use fandhe_silicon_core::execution::{
        __BackendSharedEncoderCloser, __backend_next_encoder_id, CommandEncoder, CommandQueue,
        ImportedSemaphoreHandle, SubmitToken,
    };
    use fandhe_silicon_core::memory::{
        BufferHandle, BufferUsage, MemoryOps, validate_allocation_size, validate_buffer_range,
    };
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    // コマンドラインのプログラムで `MTLCreateSystemDefaultDevice` が既定の device を返すには、
    // Core Graphics がリンクされている必要がある。参照を置いてリンクを確実にする（シンボルは使わない）。
    use objc2_core_graphics as _;
    use objc2_metal::{
        MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandQueue,
        MTLCreateSystemDefaultDevice, MTLDevice, MTLResourceOptions,
    };

    /// この背後実装の種類。エラーの `backend` と `Device::backend` の値。
    const BACKEND: BackendKind = BackendKind::Metal;

    /// 診断の `backend_name`。
    const BACKEND_NAME: &str = "metal";

    /// Metal が扱うテクスチャの 1 辺の上限として `LimitsTable` に載せる値。
    const MAX_TEXTURE_SIZE: u32 = 16384;

    /// プロセスの中で device ごとに別の値を払い出す識別子の源。0 は払い出さない。
    static NEXT_DEVICE_ID: AtomicU64 = AtomicU64::new(1);

    /// この背後実装のプロセスの中で、device を作るごとに 1 つ増える世代の源。1 から始まる。
    static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

    /// `request` の中の呼び出しの結果を `None` に差し替える指示。
    ///
    /// 本物の `request` は `Faults::default()`（何も差し替えない）で同じ関数を通る。
    /// feature `fault-injection` の提供型だけが、どちらかを `true` にして呼ぶ。
    #[derive(Clone, Copy, Default)]
    struct Faults {
        /// `MTLCreateSystemDefaultDevice` を呼ばずに、結果を `None` として扱う。
        default_device_none: bool,
        /// `newCommandQueue` の結果を `None` として扱う（device は本物を取る）。
        command_queue_none: bool,
    }

    /// 要求の組み合わせを照らし、device を作って返す。本物の `request` と差し込みの型が共有する。
    ///
    /// 照らす順は、背後実装、プロファイル（`Graphics` の feature、`CpuOnly`、未知の値）、所有形態。
    /// 最初に当たったものを返し、そのあいだは Metal の呼び出しをしない。
    fn request_with(req: DeviceRequest, faults: Faults) -> Result<Box<dyn Device>, ContractError> {
        if req.backend != BACKEND {
            return Err(ContractError::InvalidCombination(format!(
                "Metal provider cannot serve backend {:?}",
                req.backend
            )));
        }
        match req.profile {
            Profile::ComputeOnly => {}
            Profile::Graphics => {
                #[cfg(not(feature = "graphics"))]
                return Err(ContractError::InvalidCombination(
                    "profile Graphics requires the `graphics` feature of the contract crate".into(),
                ));
            }
            Profile::CpuOnly => {
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
            DeviceOwnership::ProductIndependent => {}
            DeviceOwnership::Shared => {
                return Err(ContractError::Unsupported {
                    op: "request(DeviceOwnership::Shared)",
                    backend: BACKEND,
                });
            }
            DeviceOwnership::Imported { .. } => {
                return Err(ContractError::Unsupported {
                    op: "request(DeviceOwnership::Imported)",
                    backend: BACKEND,
                });
            }
            _ => {
                return Err(ContractError::InvalidCombination(
                    "unknown device ownership".into(),
                ));
            }
        }

        let injected = if faults.default_device_none {
            " (injected)"
        } else {
            ""
        };
        let device = if faults.default_device_none {
            None
        } else {
            MTLCreateSystemDefaultDevice()
        }
        .ok_or_else(|| {
            ContractError::DeviceUnavailable(format!(
                "MTLCreateSystemDefaultDevice returned None{injected}"
            ))
        })?;

        let queue_injected = if faults.command_queue_none {
            " (injected)"
        } else {
            ""
        };
        let queue = if faults.command_queue_none {
            None
        } else {
            device.newCommandQueue()
        }
        .ok_or_else(|| {
            ContractError::DeviceUnavailable(format!(
                "newCommandQueue returned None{queue_injected}"
            ))
        })?;

        let exhausted =
            || ContractError::DeviceUnavailable("device counter space is exhausted".into());
        let id = take_next(&NEXT_DEVICE_ID).ok_or_else(exhausted)?;
        let generation = take_next(&NEXT_GENERATION).ok_or_else(exhausted)?;

        let limits = LimitsTable {
            max_buffer_bytes: u64::try_from(device.maxBufferLength()).unwrap_or(u64::MAX),
            max_dispatch_dims: [u32::MAX; 3],
            max_texture_size: MAX_TEXTURE_SIZE,
        };
        let adapter_name = device.name().to_string();

        Ok(Box::new(MetalDevice::new(
            id,
            generation,
            adapter_name,
            limits,
            device,
            queue,
        )))
    }

    /// Metal の背後実装で device を作る入口。
    ///
    /// `request` は `Metal` の背後実装・`ComputeOnly`（`graphics` feature が有効なら `Graphics` も）・
    /// `ProductIndependent` の組だけを受ける。`ProductIndependent` では、呼ぶたびに新しい device と
    /// コマンドキューを作り、共有しない。
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalDeviceProvider;

    impl DeviceProvider for MetalDeviceProvider {
        /// 要求を照らしてから Metal の既定の device を取り、device を作る。
        ///
        /// - 背後実装が Metal でない、`CpuOnly`、`graphics` が無効での `Graphics`:
        ///   `InvalidCombination`（device を作る前）。
        /// - `Shared`・`Imported`: `Unsupported`（device を作る前）。
        /// - 既定の device またはコマンドキューが得られない: `DeviceUnavailable`。
        ///
        /// 別の背後実装へは切り替えない。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(req, Faults::default())
        }
    }

    /// 既定の device の取得が `None` を返した場合を作る差し込み用の入口。実際の故障ではない。
    ///
    /// 要求の検査は `MetalDeviceProvider` と同じで、検査を通ったら `MTLCreateSystemDefaultDevice` を
    /// 呼ばずに、その結果を `None` として扱い、`DeviceUnavailable` を返す（device に触らない）。
    /// 初期化失敗の経路を、実機の故障なしで通すための試験・診断用で、feature `fault-injection` の裏にある。
    #[cfg(feature = "fault-injection")]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalDefaultDeviceNoneProvider;

    #[cfg(feature = "fault-injection")]
    impl DeviceProvider for MetalDefaultDeviceNoneProvider {
        /// 検査を通ったら、既定の device の取得の結果を `None` に差し替えて `DeviceUnavailable` を返す。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(
                req,
                Faults {
                    default_device_none: true,
                    ..Faults::default()
                },
            )
        }
    }

    /// コマンドキューの作成が `None` を返した場合を作る差し込み用の入口。実際の故障ではない。
    ///
    /// 要求の検査は `MetalDeviceProvider` と同じで、検査を通ったら本物の既定の device を取り、
    /// `newCommandQueue` の結果を `None` として扱い、`DeviceUnavailable` を返す（device に触る）。
    /// 初期化失敗の経路を通すための試験・診断用で、feature `fault-injection` の裏にある。
    #[cfg(feature = "fault-injection")]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct MetalCommandQueueNoneProvider;

    #[cfg(feature = "fault-injection")]
    impl DeviceProvider for MetalCommandQueueNoneProvider {
        /// 検査を通ったら、コマンドキューの作成の結果を `None` に差し替えて `DeviceUnavailable` を返す。
        fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
            request_with(
                req,
                Faults {
                    command_queue_none: true,
                    ..Faults::default()
                },
            )
        }
    }

    /// device が持つバッファの 1 本。
    struct BufferEntry {
        /// Metal のバッファ。`free` で表から外すまで device が保持する。
        buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    }

    /// device のバッファの表。識別子は device ごとに 1 から払い出す。
    struct Buffers {
        /// 次に払い出すバッファの識別子。
        next_id: u64,
        /// 識別子からバッファへの表。
        map: HashMap<u64, BufferEntry>,
    }

    /// Metal の device 1 つ。`DeviceProvider::request` が作る。
    ///
    /// device・コマンドキュー・バッファは、この値が持つ（Drop で解放される）。
    /// バッファは shared の storage で確保し、読み書きは CPU からの `contents()` 越しのコピーで行う。
    struct MetalDevice {
        /// この背後実装の中で device ごとに別の識別子。ハンドルの払い出し元の照合に使う。
        id: u64,
        /// device を作るときに取った世代。
        generation: u64,
        /// 作るときに指定された所有形態。
        ownership: DeviceOwnership,
        /// `MTLDevice::name` の文字列。
        adapter_name: String,
        /// 作るときに計算した上限の表。
        limits: LimitsTable,
        /// 既定の device。
        device: Retained<ProtocolObject<dyn MTLDevice>>,
        /// コマンドキュー。`submit`・`submit_many` が空のコマンドバッファをここから作る。
        queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
        /// バッファの表。
        buffers: Mutex<Buffers>,
        /// 開いている（まだ投入も破棄もされていない）記録先の識別子の集合。hook と共有する。
        open: Arc<OpenEncoders>,
        /// 投入済みの表。Objective-C の型を持つので、この錠の中だけに置く。
        submissions: Mutex<Submissions<Vec<CommandBuffer>>>,
    }

    /// 投入した 1 つのコマンドバッファ。
    type CommandBuffer = Retained<ProtocolObject<dyn MTLCommandBuffer>>;

    /// 列の各コマンドバッファの `status()` を写して、列の状態を求める。
    fn group_state(group: &Vec<CommandBuffer>) -> GroupState {
        group_state_of(group.iter().map(|buffer| {
            let status = buffer.status();
            if status == MTLCommandBufferStatus::Error {
                BufState::Failed
            } else if status == MTLCommandBufferStatus::Completed {
                BufState::Done
            } else {
                BufState::Running
            }
        }))
    }

    impl MetalDevice {
        /// 表の錠を取る。他のスレッドが panic して錠が汚れていても、表は壊れない更新だけなので中身を使う。
        fn lock_buffers(&self) -> MutexGuard<'_, Buffers> {
            self.buffers.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// 投入済みの表の錠を取る。表は壊れない更新だけなので、汚れていても中身を使う。
        fn lock_submissions(&self) -> MutexGuard<'_, Submissions<Vec<CommandBuffer>>> {
            self.submissions.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// 空のコマンドバッファを 1 つ作って確定し、通し番号で表に登録する。
        ///
        /// 一括投入も 1 つだけ作る（記録の操作が無いので 1 つで足りる）。作成・確定・採番・登録を表の錠の中で行う。
        /// 錠の中の `commandBuffer()` は、キューの未完了の上限に当たると確定済みの仕事の進みを待つだけで戻る。
        /// 確定していないコマンドバッファは錠の中で溜めない（溜めると戻らなくなる）。
        fn commit_buffers(&self) -> Result<SubmitToken, ContractError> {
            let mut subs = self.lock_submissions();
            subs.reclaim(group_state);
            let sequence = subs.next_sequence;
            let next = sequence
                .checked_add(1)
                .ok_or_else(|| ContractError::BackendFailure {
                    op: "submit",
                    backend: BACKEND,
                    detail: "submission sequence space is exhausted".into(),
                })?;
            let buffer =
                self.queue
                    .commandBuffer()
                    .ok_or_else(|| ContractError::BackendFailure {
                        op: "submit",
                        backend: BACKEND,
                        detail: "commandBuffer returned None".into(),
                    })?;
            buffer.commit();
            subs.next_sequence = next;
            subs.pending.insert(sequence, vec![buffer]);
            Ok(SubmitToken::__backend_new(
                self.generation,
                sequence,
                self.id,
            ))
        }

        /// device を組み立てる。`request_with` と試験が共有する。
        fn new(
            id: u64,
            generation: u64,
            adapter_name: String,
            limits: LimitsTable,
            device: Retained<ProtocolObject<dyn MTLDevice>>,
            queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
        ) -> Self {
            Self {
                id,
                generation,
                ownership: DeviceOwnership::ProductIndependent,
                adapter_name,
                limits,
                device,
                queue,
                buffers: Mutex::new(Buffers {
                    next_id: 1,
                    map: HashMap::new(),
                }),
                open: Arc::new(OpenEncoders::new()),
                submissions: Mutex::new(Submissions::new()),
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

        /// ハンドルを照合し、この device のバッファに対して `f` を呼ぶ。`f` の間は表の錠を持つ。
        ///
        /// 照合の順は、払い出し元の device の識別子（違えば `DeviceMismatch`）、表にある識別子、
        /// 世代の一致（表に無い・世代が違うものは偽造として `InvalidArgument`）。
        fn with_buffer<R>(
            &self,
            handle: &BufferHandle,
            f: impl FnOnce(&ProtocolObject<dyn MTLBuffer>) -> Result<R, ContractError>,
        ) -> Result<R, ContractError> {
            if handle.__backend_device_id() != self.id {
                return Err(ContractError::DeviceMismatch);
            }
            let buffers = self.lock_buffers();
            let Some(entry) = buffers.map.get(&handle.__backend_id()) else {
                return Err(ContractError::InvalidArgument(
                    "buffer handle is not in this device's table".into(),
                ));
            };
            if handle.__backend_generation() != self.generation {
                return Err(ContractError::InvalidArgument(
                    "buffer handle generation does not match this device".into(),
                ));
            }
            f(&entry.buffer)
        }

        /// 範囲を検査してから `[offset, offset + len)` を読み出す。`download_*` が共有する。
        fn read_range(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            self.with_buffer(src, |buffer| {
                let allocation_len = u64::try_from(buffer.length()).map_err(|_| {
                    ContractError::InvalidArgument("buffer length does not fit u64".into())
                })?;
                validate_buffer_range(src_offset, byte_len, allocation_len)?;
                // 検査を通ったので、offset と len は確保長以下で usize に収まる。
                let (Ok(offset), Ok(len)) =
                    (usize::try_from(src_offset), usize::try_from(byte_len))
                else {
                    return Err(ContractError::InvalidArgument(
                        "buffer range does not fit usize".into(),
                    ));
                };
                // 確保できなければ abort せず `AllocationFailed` で返す。
                let mut out: Vec<u8> = Vec::new();
                out.try_reserve_exact(len)
                    .map_err(|_| ContractError::AllocationFailed {
                        requested_bytes: byte_len,
                    })?;
                out.resize(len, 0);
                if len > 0 {
                    let base = buffer.contents().as_ptr().cast::<u8>().cast_const();
                    // SAFETY: バッファは shared の storage なので `contents()` は CPU から読める
                    // `buffer.length()` バイトの領域を指す。`validate_buffer_range` が
                    // `offset + len <= length()` を保証している。GPU の書き込みと重ならないことは、
                    // この背後実装が投入するコマンドバッファがエンコーダを開かず、どのバッファも参照しない
                    // （記録の操作を持たない）ことに根拠を置く（記録の操作を足す変更では、
                    // このブロックの同期の根拠を必ず書き直す）。`with_buffer` が表の錠を持ったまま
                    // `f` を呼ぶため、コピーの間は `free` でバッファが落ちない。`out` は新しく確保した
                    // `len` バイトの領域で、Metal の領域とは重ならない。
                    unsafe {
                        core::ptr::copy_nonoverlapping(base.add(offset), out.as_mut_ptr(), len);
                    }
                }
                Ok(out)
            })
        }
    }

    impl Capabilities for MetalDevice {
        /// この段の `query` は、どの機能にも `Unsupported` を返す。
        fn query(&self, _feature: FeatureId) -> CapabilityLevel {
            CapabilityLevel::Unsupported
        }

        fn limits(&self) -> &LimitsTable {
            &self.limits
        }

        /// どの機能も未知として、理由 `UnknownFeature` を添えて `Unsupported` を返す。
        fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
            CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: Some(UnsupportedReason::UnknownFeature),
                evidence: None,
            }
        }
    }

    impl Device for MetalDevice {
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

    impl CommandQueue for MetalDevice {
        /// 記録先の識別子と空の記録だけを作る。コマンドバッファは作らず、何も投入しない。
        ///
        /// 識別子はプロセス全体の通し番号で、この device の開いている集合に入れる。
        /// 投入せずに破棄すると hook が集合から外す。
        fn create_encoder(&self) -> CommandEncoder {
            let closer: Arc<dyn __BackendSharedEncoderCloser> = self.open.clone();
            match __backend_next_encoder_id() {
                Some(id) => {
                    self.open.lock().insert(id);
                    CommandEncoder::__backend_new_shared(id, closer)
                }
                // 通し番号が尽きた場合の id 0 は集合に入れない。投入は `InvalidArgument` で拒まれる。
                None => CommandEncoder::__backend_new_shared(0, closer),
            }
        }

        /// 空のコマンドバッファを 1 つ作って確定する。
        ///
        /// 空でない `wait` は何も投入せず `Unsupported`。記録先がこの device の開いている集合に無ければ
        /// `InvalidArgument`（別の device の記録先を含む）で、何も投入しない。
        fn submit(
            &self,
            mut commands: CommandEncoder,
            wait: &[ImportedSemaphoreHandle],
        ) -> Result<SubmitToken, ContractError> {
            precheck_submit(wait)?;
            self.open.claim(&[commands.__backend_id()])?;
            commands.__backend_disarm_closer();
            self.commit_buffers()
        }

        /// 複数の記録先を 1 回で投入し、1 つの印を返す。
        ///
        /// 順は、空の列（`InvalidArgument`）、`wait`（`Unsupported`）、`after` の照合
        /// （別 device は `DeviceMismatch`、世代違いは `DeviceLost`）、記録先の所属（`InvalidArgument`）。
        /// 検査に当たると何も投入しない。単一の列でコマンドバッファは投入順に完了するので、
        /// `after` は照合だけで追加の待ちは入れない。
        fn submit_many(
            &self,
            mut commands: Vec<CommandEncoder>,
            after: &[SubmitToken],
            wait: &[ImportedSemaphoreHandle],
        ) -> Result<SubmitToken, ContractError> {
            precheck_submit_many(commands.len(), after, wait, self.id, self.generation)?;
            let ids: Vec<u64> = commands.iter().map(CommandEncoder::__backend_id).collect();
            self.open.claim(&ids)?;
            for command in &mut commands {
                command.__backend_disarm_closer();
            }
            self.commit_buffers()
        }

        /// 列の全部が完了なら `Complete`、どれかが `Error` なら `Lost`、他は `Pending`。
        ///
        /// 別 device の印・世代違いの印は `Lost`。回収済みの番号は watermark と失敗の集合で答える。
        fn poll(&self, token: SubmitToken) -> CompletionState {
            if check_token_owner(&token, self.id, self.generation).is_err() {
                return CompletionState::Lost;
            }
            let mut subs = self.lock_submissions();
            subs.reclaim(group_state);
            subs.state_of(token.sequence, group_state)
        }

        /// その場で `poll` の値を `callback` に 1 回渡す。完了は待たないので、`Pending` を渡すことがある。
        fn on_complete(
            &self,
            token: SubmitToken,
            callback: Box<dyn FnOnce(CompletionState) + Send>,
        ) {
            callback(self.poll(token));
        }

        /// 完了まで待つ。`None` はコマンドバッファの完了を待ち、`Some` は 1 ms 間隔で `poll` を確かめる。
        ///
        /// 待つ間は表の錠を持たない。失敗した投入は `BackendFailure`。別 device の印は `DeviceMismatch`、世代違いは `DeviceLost`。
        #[cfg(feature = "native-blocking-wait")]
        fn wait_blocking(
            &self,
            token: SubmitToken,
            timeout: Option<std::time::Duration>,
        ) -> Result<(), ContractError> {
            check_token_owner(&token, self.id, self.generation)?;
            match timeout {
                None => {
                    // 列を clone して錠を放してから待つ。待つ間も同じ錠を取る `poll`・`submit` を妨げない（錠を握ったまま待つと再入で詰まる）。
                    let group = {
                        let mut subs = self.lock_submissions();
                        subs.reclaim(group_state);
                        subs.pending.get(&token.sequence).cloned()
                    };
                    if let Some(group) = group {
                        for buffer in &group {
                            buffer.waitUntilCompleted();
                        }
                    }
                    wait_result(self.poll(token))
                }
                Some(limit) => {
                    let started = std::time::Instant::now();
                    loop {
                        match self.poll(token) {
                            CompletionState::Pending => {}
                            done => return wait_result(done),
                        }
                        let elapsed = started.elapsed();
                        if elapsed >= limit {
                            return Err(ContractError::Timeout);
                        }
                        std::thread::sleep(
                            (limit - elapsed).min(std::time::Duration::from_millis(1)),
                        );
                    }
                }
            }
        }
    }

    /// `allocate` が Metal のバッファを作る前に通す検査。
    ///
    /// 共通の `validate_allocation_size` は上限だけを見て 0 を通す。長さ 0 を Metal に渡したときの
    /// 振る舞いには頼らず、呼び出し側の引数の誤りとして、この背後実装では 0 を先に
    /// `InvalidArgument` で拒む。
    fn validate_allocation_request(
        byte_len: u64,
        max_buffer_bytes: u64,
    ) -> Result<(), ContractError> {
        if byte_len == 0 {
            return Err(ContractError::InvalidArgument(
                "allocation size must be greater than zero".into(),
            ));
        }
        validate_allocation_size(byte_len, max_buffer_bytes)
    }

    impl MemoryOps for MetalDevice {
        /// shared の storage でバッファを確保する。
        ///
        /// 長さ 0 と上限（`limits().max_buffer_bytes`）超えは `InvalidArgument`。
        /// 返すハンドルの `size`・`usage` は、受けた `byte_len`・`usage`。
        fn allocate(
            &self,
            byte_len: u64,
            usage: BufferUsage,
        ) -> Result<BufferHandle, ContractError> {
            validate_allocation_request(byte_len, self.limits.max_buffer_bytes)?;
            let failed = || ContractError::AllocationFailed {
                requested_bytes: byte_len,
            };
            let length = usize::try_from(byte_len).map_err(|_| failed())?;
            let buffer = self
                .device
                .newBufferWithLength_options(length, MTLResourceOptions::StorageModeShared)
                .ok_or_else(failed)?;
            let mut buffers = self.lock_buffers();
            let id = buffers.next_id;
            buffers.next_id = id.checked_add(1).ok_or_else(failed)?;
            buffers.map.insert(id, BufferEntry { buffer });
            Ok(BufferHandle::__backend_new(
                self.generation,
                id,
                self.id,
                byte_len,
                usage,
            ))
        }

        /// `bytes` を `dst` の `dst_offset` から書き込む。範囲の検査に当たると、バッファの内容を変えずに返す。
        fn upload(
            &self,
            dst: &BufferHandle,
            dst_offset: u64,
            bytes: &[u8],
        ) -> Result<(), ContractError> {
            self.with_buffer(dst, |buffer| {
                let len = u64::try_from(bytes.len()).map_err(|_| {
                    ContractError::InvalidArgument("data length does not fit u64".into())
                })?;
                let allocation_len = u64::try_from(buffer.length()).map_err(|_| {
                    ContractError::InvalidArgument("buffer length does not fit u64".into())
                })?;
                validate_buffer_range(dst_offset, len, allocation_len)?;
                let Ok(offset) = usize::try_from(dst_offset) else {
                    return Err(ContractError::InvalidArgument(
                        "buffer offset does not fit usize".into(),
                    ));
                };
                if !bytes.is_empty() {
                    let base = buffer.contents().as_ptr().cast::<u8>();
                    // SAFETY: バッファは shared の storage なので `contents()` は CPU から書ける
                    // `buffer.length()` バイトの領域を指す。`validate_buffer_range` が
                    // `offset + bytes.len() <= length()` を保証している。GPU の読み書きと重ならないことは、
                    // この背後実装が投入するコマンドバッファがエンコーダを開かず、どのバッファも参照しない
                    // （記録の操作を持たない）ことに根拠を置く（記録の操作を足す変更では、
                    // このブロックの同期の根拠を必ず書き直す）。`with_buffer` が表の錠を持ったまま
                    // `f` を呼ぶため、コピーの間は `free` でバッファが落ちない。`bytes` は呼び出し側の
                    // 借用した領域で、Metal の領域とは重ならない。
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            base.add(offset),
                            bytes.len(),
                        );
                    }
                }
                Ok(())
            })
        }

        /// `src` の `src_offset` から `byte_len` バイトを読み出す。記録の操作が無く GPU はバッファに触れないので、待たずに返る。
        #[cfg(feature = "native-blocking-wait")]
        fn download_blocking(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            self.read_range(src, src_offset, byte_len)
        }

        /// 読み出してから、その場で `callback` を 1 回だけ呼ぶ。失敗も `callback` に渡す。
        fn download_async(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
            callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
        ) {
            callback(self.read_range(src, src_offset, byte_len));
        }

        /// この device のハンドルなら、表から外してバッファを解放する。
        ///
        /// 別の device のハンドルは何も解放しない（同じ識別子を持つ自 device のバッファを巻き込まない）。
        fn free(&self, buffer: BufferHandle) {
            if buffer.__backend_device_id() != self.id
                || buffer.__backend_generation() != self.generation
            {
                return;
            }
            let removed = self.lock_buffers().map.remove(&buffer.__backend_id());
            // 錠を放してから落とす。Metal のバッファの解放を錠の中で行わない。
            drop(removed);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// `allocate` の前段の検査の境界。device には触らない。
        const LIMIT: u64 = 1024;

        #[test]
        fn zero_length_is_invalid_argument() {
            let r = validate_allocation_request(0, LIMIT);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }

        #[test]
        fn zero_length_is_rejected_even_when_the_limit_is_zero() {
            let r = validate_allocation_request(0, 0);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }

        #[test]
        fn one_byte_and_the_limit_itself_pass() {
            assert!(validate_allocation_request(1, LIMIT).is_ok());
            assert!(validate_allocation_request(LIMIT, LIMIT).is_ok());
        }

        #[test]
        fn one_past_the_limit_is_invalid_argument() {
            let r = validate_allocation_request(LIMIT + 1, LIMIT);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }
    }

    /// device に触る試験専用の数の口の確認。実機の回でだけ流す。
    #[cfg(test)]
    mod device_tests {
        use super::*;

        fn new_device() -> MetalDevice {
            // 具体型で数の口を読むため、`request_with` を通さず同じ手順で作る。
            let device = MTLCreateSystemDefaultDevice().expect("default device");
            let queue = device.newCommandQueue().expect("queue");
            MetalDevice::new(
                take_next(&NEXT_DEVICE_ID).expect("id"),
                take_next(&NEXT_GENERATION).expect("generation"),
                String::new(),
                LimitsTable {
                    max_buffer_bytes: 1 << 20,
                    max_dispatch_dims: [u32::MAX; 3],
                    max_texture_size: MAX_TEXTURE_SIZE,
                },
                device,
                queue,
            )
        }

        /// CMD-1 のポインタ。記録先を作る・破棄するだけでは投入の件数が増えず、破棄で開いている件数が戻る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn creating_and_dropping_encoders_does_not_submit() {
            let device = new_device();
            let a = device.create_encoder();
            let b = device.create_encoder();
            assert_eq!(device.test_open_encoder_count(), 2);
            assert_eq!(device.test_submitted_count(), 0);
            drop(a);
            assert_eq!(device.test_open_encoder_count(), 1);
            drop(b);
            assert_eq!(device.test_open_encoder_count(), 0);
            assert_eq!(device.test_submitted_count(), 0);
        }

        /// CMD-12 のポインタ。別 device の記録先は投入されず、元の device の件数も変わらない。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn foreign_encoder_is_rejected_without_submitting() {
            let first = new_device();
            let second = new_device();
            let foreign = first.create_encoder();
            let r = second.submit(foreign, &[]);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            assert_eq!(second.test_submitted_count(), 0);
            assert_eq!(first.test_submitted_count(), 0);
            // 記録先は破棄で元の device の集合から外れる。
            assert_eq!(first.test_open_encoder_count(), 0);
        }

        /// CMD-11 のポインタ。空でない `wait` は投入せず `Unsupported` で、記録先は破棄で閉じる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn non_empty_wait_is_unsupported_without_submitting() {
            let device = new_device();
            let encoder = device.create_encoder();
            let r = device.submit(encoder, &[ImportedSemaphoreHandle::__backend_new(1)]);
            assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
            assert_eq!(device.test_submitted_count(), 0);
            assert_eq!(device.test_open_encoder_count(), 0);
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::MetalDeviceProvider;

#[cfg(all(target_os = "macos", feature = "fault-injection"))]
pub use imp::{MetalCommandQueueNoneProvider, MetalDefaultDeviceNoneProvider};

#[cfg(test)]
mod precheck_tests {
    //! device に触らない検査の固定（どの OS でも流れる）。
    use super::*;

    const DEVICE_ID: u64 = 7;
    const GENERATION: u64 = 3;

    fn semaphore() -> ImportedSemaphoreHandle {
        ImportedSemaphoreHandle::__backend_new(1)
    }

    fn token(generation: u64, device_id: u64) -> SubmitToken {
        SubmitToken::__backend_new(generation, 1, device_id)
    }

    #[test]
    fn empty_wait_passes_and_non_empty_wait_is_unsupported() {
        assert!(precheck_submit(&[]).is_ok());
        let r = precheck_submit(&[semaphore()]);
        assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
    }

    #[test]
    fn submit_many_rejects_an_empty_list_before_looking_at_wait() {
        let r = precheck_submit_many(0, &[], &[semaphore()], DEVICE_ID, GENERATION);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
    }

    #[test]
    fn submit_many_checks_wait_before_after() {
        let foreign = token(GENERATION, DEVICE_ID + 1);
        let r = precheck_submit_many(1, &[foreign], &[semaphore()], DEVICE_ID, GENERATION);
        assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
    }

    #[test]
    fn after_tokens_are_matched_by_device_then_generation() {
        let own = token(GENERATION, DEVICE_ID);
        assert!(precheck_submit_many(1, &[own], &[], DEVICE_ID, GENERATION).is_ok());

        let foreign = token(GENERATION, DEVICE_ID + 1);
        let r = precheck_submit_many(1, &[own, foreign], &[], DEVICE_ID, GENERATION);
        assert_eq!(r, Err(ContractError::DeviceMismatch));

        let stale = token(GENERATION + 1, DEVICE_ID);
        let r = precheck_submit_many(1, &[stale], &[], DEVICE_ID, GENERATION);
        assert_eq!(
            r,
            Err(ContractError::DeviceLost {
                generation: GENERATION + 1
            })
        );

        // device の識別子が違えば、世代も違う印は `DeviceMismatch` が先に出る。
        let both = token(GENERATION + 1, DEVICE_ID + 1);
        assert_eq!(
            check_token_owner(&both, DEVICE_ID, GENERATION),
            Err(ContractError::DeviceMismatch)
        );
    }

    use BufState::{Done, Failed, Running};

    #[allow(clippy::ptr_arg)] // `Submissions<Vec<_>>` の分類関数は `&Vec` を受ける
    fn classify(group: &Vec<BufState>) -> GroupState {
        group_state_of(group.iter().copied())
    }

    fn table(groups: &[Vec<BufState>]) -> Submissions<Vec<BufState>> {
        let mut subs = Submissions::new();
        for group in groups {
            subs.pending.insert(subs.next_sequence, group.clone());
            subs.next_sequence += 1;
        }
        subs
    }

    #[test]
    fn a_failed_group_is_lost_and_moves_to_the_failed_set() {
        let mut subs = table(&[vec![Done, Failed], vec![Done]]);
        assert_eq!(subs.state_of(1, classify), CompletionState::Lost);
        subs.reclaim(classify);
        assert!(subs.failed.contains(&1));
        assert_eq!(subs.watermark, 2);
        assert_eq!(subs.state_of(1, classify), CompletionState::Lost);
        assert_eq!(subs.state_of(2, classify), CompletionState::Complete);
    }

    #[test]
    fn zero_and_unissued_sequences_are_lost() {
        let mut subs = table(&[vec![Done]]);
        subs.reclaim(classify);
        assert_eq!(subs.state_of(0, classify), CompletionState::Lost);
        assert_eq!(subs.state_of(2, classify), CompletionState::Lost);
        let fresh: Submissions<Vec<BufState>> = Submissions::new();
        assert_eq!(fresh.state_of(0, classify), CompletionState::Lost);
    }

    #[test]
    fn an_unfinished_head_blocks_reclaiming_later_groups() {
        let mut subs = table(&[vec![Running], vec![Done]]);
        subs.reclaim(classify);
        assert_eq!(subs.watermark, 0);
        assert_eq!(subs.pending.len(), 2);
        assert_eq!(subs.state_of(1, classify), CompletionState::Pending);
        assert_eq!(subs.state_of(2, classify), CompletionState::Complete);
    }

    #[test]
    fn a_reclaimed_group_that_did_not_fail_is_complete() {
        let mut subs = table(&[vec![Done], vec![Done, Done]]);
        subs.reclaim(classify);
        assert!(subs.pending.is_empty());
        assert!(subs.failed.is_empty());
        assert_eq!(subs.state_of(1, classify), CompletionState::Complete);
        assert_eq!(subs.state_of(2, classify), CompletionState::Complete);
    }

    #[test]
    fn group_state_prefers_failure_over_running() {
        assert_eq!(group_state_of([Running, Failed]), GroupState::Failed);
        assert_eq!(group_state_of([Done, Running]), GroupState::Pending);
        assert_eq!(group_state_of([]), GroupState::Completed);
    }

    #[test]
    fn wait_result_does_not_report_a_failure_as_device_lost() {
        assert!(wait_result(CompletionState::Complete).is_ok());
        let lost = wait_result(CompletionState::Lost);
        assert!(
            matches!(lost, Err(ContractError::BackendFailure { .. })),
            "{lost:?}"
        );
        let pending = wait_result(CompletionState::Pending);
        assert!(
            matches!(pending, Err(ContractError::BackendFailure { .. })),
            "{pending:?}"
        );
    }

    #[test]
    fn claim_rejects_id_zero_unknown_ids_and_duplicates_without_changing_the_set() {
        let open = OpenEncoders::new();
        open.lock().extend([5, 6]);
        for ids in [&[0_u64][..], &[5, 7][..], &[5, 5][..]] {
            let r = open.claim(ids);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            assert_eq!(open.lock().len(), 2);
        }
        assert!(open.claim(&[5, 6]).is_ok());
        assert!(open.lock().is_empty());
    }

    #[test]
    fn closing_removes_the_id_and_is_harmless_twice() {
        let open = OpenEncoders::new();
        open.lock().insert(9);
        open.close(9);
        open.close(9);
        assert!(open.lock().is_empty());
    }

    #[test]
    fn take_next_does_not_move_an_exhausted_counter() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(take_next(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}
