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
//! `free` したバッファは、同じ `byte_len` の再利用の一覧（pool）へ回す（上限を超える分は解放する）。
//! 今の投入はどのバッファも参照しないので、`free` したバッファを無条件に pool へ回す（回す前に、
//! そのバッファを参照する未完了の投入が無いかを確かめる実行時の検査は無い）。
//! 記録の操作を足す変更では、遅延解放（バッファを参照する未完了の投入が終わるまで解放しない）と合わせて
//! pool への回し方を見直す（TASK-610・TASK-319 のポインタ）。pool に入れる前に全バイトを 0 にするので、
//! 再利用された `allocate` の利用者に以前の内容は見えない。pool に入ったバッファは、device の Drop まで
//! 保持される（合計の上限は 64 MiB）。
//!
//! `MetalDevice` は Objective-C のオブジェクトを持つので `Send`・`Sync` ではない（`unsafe impl` は足さない）。
//! 投入済みの表は device の錠の中に置き、記録先の後始末の hook は識別子の集合だけを握る。

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, ContractError};
use fandhe_silicon_core::execution::{
    __BackendSharedEncoderCloser, ImportedSemaphoreHandle, SubmitToken,
};
use fandhe_silicon_core::memory::validate_buffer_range;

/// 部分範囲 `[offset, offset + len)` を確保長 `allocation_len` に照らし、ホストの `usize` の組にして返す。
///
/// `upload` の宛先と `download_*` の元が共有する。終端の `u64` のあふれと確保長の超過は共通の
/// `validate_buffer_range` が `InvalidArgument` で拒む。検査を通った後でも `usize` に収まらない値は、
/// 切り詰めずに `InvalidArgument` で返す。device に触らず、バッファの内容も変えない。
/// `usize` が 64 bit 以上の環境では `u64` の値は必ず収まるので、この分岐は試験では通らない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn host_range(offset: u64, len: u64, allocation_len: u64) -> Result<(usize, usize), ContractError> {
    validate_buffer_range(offset, len, allocation_len)?;
    match (usize::try_from(offset), usize::try_from(len)) {
        (Ok(offset), Ok(len)) => Ok((offset, len)),
        _ => Err(ContractError::InvalidArgument(
            "buffer range does not fit usize".into(),
        )),
    }
}

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

    /// 通し番号の拡張した完了状態。呼ぶ前に `reclaim` しておく。
    ///
    /// 表にあれば列の状態（失敗は `RuntimeError`）、失敗の集合にあるものは `RuntimeError`、
    /// watermark 以下の他は `Complete`。表にも集合にも無く watermark を超える番号（0 と未払い出しを含む）は
    /// `Unknown` で、`Complete` とは返さない。
    fn state_ex_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> CompletionStateEx {
        if let Some(group) = self.pending.get(&sequence) {
            return match classify(group) {
                GroupState::Pending => CompletionStateEx::Pending,
                GroupState::Completed => CompletionStateEx::Complete,
                GroupState::Failed => CompletionStateEx::RuntimeError,
            };
        }
        if self.failed.contains(&sequence) {
            return CompletionStateEx::RuntimeError;
        }
        if sequence >= 1 && sequence <= self.watermark {
            CompletionStateEx::Complete
        } else {
            CompletionStateEx::Unknown
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

/// `on_complete` が callback に渡す終端の状態。待ったあとでも `Pending` なら（起きないはずだが）失敗側の `Lost` にする。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn terminal_state(state: CompletionState) -> CompletionState {
    match state {
        CompletionState::Pending => CompletionState::Lost,
        done => done,
    }
}

/// `wait_blocking` が待ったあとの `poll` の値を結果に写す。
///
/// 失敗した投入は `DeviceLost` ではなく `BackendFailure`（この背後実装は device のロストを返さない）。
#[cfg(any(test, feature = "native-blocking-wait"))]
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

/// 再利用の一覧に溜めるバッファの件数の上限（全 `byte_len` の合計）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const POOL_MAX_ENTRIES: usize = 16;

/// 再利用の一覧に溜めるバッファの大きさの合計（バイト）の上限。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const POOL_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// 再利用の一覧に入るバッファ 1 本。`slot` は Metal のバッファごとに 1 度だけ払い出す識別子で、
/// 一覧を出入りしても変わらない（ハンドルの識別子とは別で、再利用のたびに新しいハンドルの識別子を払い出す）。
/// `G` は本物では Metal のバッファで、試験では偽の値を使う。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct PooledSlot<G> {
    slot: u64,
    buffer: G,
}

/// 同じ `byte_len` ごとの再利用の一覧。`device` の表の錠の中に置く。
///
/// 入れるバッファは呼び出し側が先に全バイトを 0 にしたものに限る（この部品は中身を見ない）。
/// 件数と大きさの合計に上限があり、超える分は入れずに呼び出し側へ返す（呼び出し側が解放する）。
/// 再利用の回数と、取り出した `slot` が直前に入れた `slot` と一致した回数を数える（試験が読む）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct SizePool<G> {
    by_len: BTreeMap<u64, Vec<PooledSlot<G>>>,
    entries: usize,
    bytes: u64,
    last_put_slot: Option<u64>,
    reuse_count: u64,
    slot_match_count: u64,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl<G> SizePool<G> {
    fn new() -> Self {
        Self {
            by_len: BTreeMap::new(),
            entries: 0,
            bytes: 0,
            last_put_slot: None,
            reuse_count: 0,
            slot_match_count: 0,
        }
    }

    /// 入れた後の件数と大きさの合計。上限を超える・計算があふれるときは `None`。
    fn next_totals(&self, byte_len: u64) -> Option<(usize, u64)> {
        let entries = self.entries.checked_add(1)?;
        let bytes = self.bytes.checked_add(byte_len)?;
        (entries <= POOL_MAX_ENTRIES && bytes <= POOL_MAX_BYTES).then_some((entries, bytes))
    }

    /// いま `byte_len` のバッファを入れられるか。呼び出し側がゼロ化の前に確かめる。
    /// 錠の外でゼロ化する間に状況が変わりうるので、`put` が改めて検査する。
    fn would_accept(&self, byte_len: u64) -> bool {
        self.next_totals(byte_len).is_some()
    }

    /// 入れる。上限に当たる（件数・大きさの合計・計算のあふれ）と、一覧を変えずに渡された値を返す。
    fn put(&mut self, byte_len: u64, slot: u64, buffer: G) -> Option<G> {
        let Some((next_entries, next_bytes)) = self.next_totals(byte_len) else {
            return Some(buffer);
        };
        self.entries = next_entries;
        self.bytes = next_bytes;
        self.last_put_slot = Some(slot);
        self.by_len
            .entry(byte_len)
            .or_default()
            .push(PooledSlot { slot, buffer });
        None
    }

    /// 同じ `byte_len` の一覧から 1 本取り出す。別の大きさは見ない。取り出せたら再利用の回数を数える。
    fn take(&mut self, byte_len: u64) -> Option<(u64, G)> {
        let list = self.by_len.get_mut(&byte_len)?;
        let PooledSlot { slot, buffer } = list.pop()?;
        if list.is_empty() {
            self.by_len.remove(&byte_len);
        }
        // 入れた分だけ引くので、あふれ・負にはならない。
        self.entries = self.entries.saturating_sub(1);
        self.bytes = self.bytes.saturating_sub(byte_len);
        self.reuse_count = self.reuse_count.saturating_add(1);
        if self.last_put_slot == Some(slot) {
            self.slot_match_count = self.slot_match_count.saturating_add(1);
        }
        Some((slot, buffer))
    }
}

/// 解放されたバッファを、ゼロ化してから再利用の一覧へ回す流れ。受け入れられなかったバッファを返す
/// （呼び出し側が解放する）。device に触らず、錠の取り方は呼び出し側の閉包に任せる。
///
/// 順は、`accepts(byte_len)` で一覧に入れられるか確かめる（入れられないならゼロ化せずに返す）、
/// `zero` でゼロ化する（失敗したら入れずに返す）、`put` で入れる（検査に当たれば返す）。
/// 一覧に入るのはゼロ化に成功したバッファだけになる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn retire<B, E>(
    byte_len: u64,
    mut buffer: B,
    accepts: impl FnOnce(u64) -> bool,
    zero: impl FnOnce(&mut B) -> Result<(), E>,
    put: impl FnOnce(B) -> Option<B>,
) -> Option<B> {
    if !accepts(byte_len) {
        return Some(buffer);
    }
    if zero(&mut buffer).is_err() {
        return Some(buffer);
    }
    put(buffer)
}

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;
    use std::sync::{Arc, Mutex, MutexGuard};

    use super::host_range;
    #[cfg(feature = "native-blocking-wait")]
    use super::wait_result;
    use super::{
        BufState, GroupState, OpenEncoders, SizePool, Submissions, check_token_owner,
        group_state_of, precheck_submit, precheck_submit_many, retire, take_next, terminal_state,
    };

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
        __BackendSharedEncoderCloser, __backend_next_encoder_id, CommandEncoder, CommandQueue,
        ImportedSemaphoreHandle, SubmitToken,
    };
    use fandhe_silicon_core::memory::{
        BufferHandle, BufferUsage, MemoryOps, validate_allocation_size,
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
        /// このバッファの識別子（slot）。Metal のバッファを作るときに 1 度だけ払い出し、再利用の一覧を出入りしても変わらない。
        slot: u64,
    }

    /// device のバッファの表。識別子は device ごとに 1 から払い出す。
    struct Buffers {
        /// 次に払い出すバッファの識別子。
        next_id: u64,
        /// 識別子からバッファへの表。
        map: HashMap<u64, BufferEntry>,
        /// 次に払い出す slot。
        next_slot: u64,
        /// 解放されたバッファの再利用の一覧。入っているのは全バイトが 0 のバッファだけ。
        pool: SizePool<Retained<ProtocolObject<dyn MTLBuffer>>>,
    }

    /// ゼロ化に使う 0 の並び。`free` の中で大きな領域を確保しないよう、固定長を繰り返し書く。
    const ZEROS: [u8; 4096] = [0; 4096];

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
                    next_slot: 1,
                    pool: SizePool::new(),
                }),
                open: Arc::new(OpenEncoders::new()),
                submissions: Mutex::new(Submissions::new()),
            }
        }

        /// 対象の列の完了を呼び出し元のスレッドで待ち、その後の `poll` の値を返す。`on_complete` と `wait_blocking(None)` が共有する。
        ///
        /// 列を clone して表の錠を放してから待つ。待つ間も同じ錠を取る `poll`・`submit` を妨げない（錠を握ったまま待たない）。
        fn wait_for_completion(&self, token: SubmitToken) -> CompletionState {
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
            self.poll(token)
        }

        /// 試験専用。投入に成功した回数の通算。
        #[cfg(test)]
        fn test_submitted_count(&self) -> u64 {
            self.lock_submissions().next_sequence - 1
        }

        /// 試験専用。再利用の一覧から取り出した回数の通算。
        #[cfg(test)]
        fn test_pool_reuse_count(&self) -> u64 {
            self.lock_buffers().pool.reuse_count
        }

        /// 試験専用。取り出した slot が直前に一覧へ入れた slot と一致した回数の通算。
        #[cfg(test)]
        fn test_pool_slot_match_count(&self) -> u64 {
            self.lock_buffers().pool.slot_match_count
        }

        /// 試験専用。再利用の一覧にあるバッファの数。
        #[cfg(test)]
        fn test_pool_len(&self) -> usize {
            self.lock_buffers().pool.entries
        }

        /// 試験専用。ハンドルが指す表のバッファの slot。表に無ければ `None`。
        #[cfg(test)]
        fn test_slot_of(&self, handle: &BufferHandle) -> Option<u64> {
            self.lock_buffers()
                .map
                .get(&handle.__backend_id())
                .map(|entry| entry.slot)
        }

        /// 試験専用。開いている記録先の数。
        #[cfg(test)]
        fn test_open_encoder_count(&self) -> usize {
            self.open.lock().len()
        }

        /// ハンドルを照合し、この device のバッファの表の項目を `&mut` で借りて `f` を呼ぶ。`f` の間は表の錠を持つ。
        ///
        /// 照合の順は、払い出し元の device の識別子（違えば `DeviceMismatch`）、表にある識別子、
        /// 世代の一致（表に無い・世代が違うものは偽造として `InvalidArgument`）。
        fn with_buffer<R>(
            &self,
            handle: &BufferHandle,
            f: impl FnOnce(&mut BufferEntry) -> Result<R, ContractError>,
        ) -> Result<R, ContractError> {
            if handle.__backend_device_id() != self.id {
                return Err(ContractError::DeviceMismatch);
            }
            let mut buffers = self.lock_buffers();
            let Some(entry) = buffers.map.get_mut(&handle.__backend_id()) else {
                return Err(ContractError::InvalidArgument(
                    "buffer handle is not in this device's table".into(),
                ));
            };
            if handle.__backend_generation() != self.generation {
                return Err(ContractError::InvalidArgument(
                    "buffer handle generation does not match this device".into(),
                ));
            }
            f(entry)
        }

        /// 範囲を検査してから `[offset, offset + len)` を読み出す。`download_*` が共有する。
        fn read_range(
            &self,
            src: &BufferHandle,
            src_offset: u64,
            byte_len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            self.with_buffer(src, |entry| {
                let buffer = &entry.buffer;
                let allocation_len = u64::try_from(buffer.length()).map_err(|_| {
                    ContractError::InvalidArgument("buffer length does not fit u64".into())
                })?;
                // `host_range` が検査と `usize` への変換を行い、収まらなければ `InvalidArgument` を返す。
                let (offset, len) = host_range(src_offset, byte_len, allocation_len)?;
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
                    // `buffer.length()` バイトの領域を指す。`host_range`（`validate_buffer_range` を呼ぶ）が
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

        /// `poll` の詳しい版。未完了は `Pending`、完了は `Complete`、`Error` で終わった列は `RuntimeError`。
        ///
        /// 別 device の印・世代違いの印は `poll` と同じく `Lost`。払い出していない番号（0 を含む）は `Unknown`。
        /// 回収済みの番号は watermark と失敗の集合で答える（失敗した投入を `Complete` とは返さない）。
        /// `poll` は失敗した列と払い出していない番号を `Lost` とし、`poll_ex` はこれを `RuntimeError`・`Unknown` に分ける。
        /// `Pending`・`Complete` は `poll` と同じ。
        fn poll_ex(&self, token: SubmitToken) -> CompletionStateEx {
            if check_token_owner(&token, self.id, self.generation).is_err() {
                return CompletionStateEx::Lost;
            }
            let mut subs = self.lock_submissions();
            subs.reclaim(group_state);
            subs.state_ex_of(token.sequence, group_state)
        }

        /// 完了まで呼び出し元のスレッドで待ってから、終端の状態（`Complete` か `Lost`）で `callback` を 1 回だけ呼ぶ。
        ///
        /// 回収した `poll` の値が終端ならその場で呼ぶ。別 device の印・世代違いの印・表に無い番号は待たずに `Lost`。
        /// 未完了なら表の錠を放してから待つ。`callback` は錠の外で呼ぶ。
        fn on_complete(
            &self,
            token: SubmitToken,
            callback: Box<dyn FnOnce(CompletionState) + Send>,
        ) {
            let state = match self.poll(token) {
                CompletionState::Pending => self.wait_for_completion(token),
                done => done,
            };
            callback(terminal_state(state));
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
                None => wait_result(self.wait_for_completion(token)),
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

    /// 範囲を検査してから、`bytes` を `entry` のバッファの `offset` から書き込む。`upload` とゼロ化が共有する。
    ///
    /// 検査に当たると、バッファの内容を変えずに `InvalidArgument` を返す。
    /// 書き込みの排他は `&mut BufferEntry` が型で示す。`upload` は表の錠の中の `get_mut` で借りた項目を、
    /// ゼロ化は表から外して所有している項目を渡す。共有参照からは書けない。非公開。
    fn write_range(
        entry: &mut BufferEntry,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        let buffer = &entry.buffer;
        let len = u64::try_from(bytes.len())
            .map_err(|_| ContractError::InvalidArgument("data length does not fit u64".into()))?;
        let allocation_len = u64::try_from(buffer.length())
            .map_err(|_| ContractError::InvalidArgument("buffer length does not fit u64".into()))?;
        let (offset, _) = host_range(offset, len, allocation_len)?;
        if !bytes.is_empty() {
            let base = buffer.contents().as_ptr().cast::<u8>();
            // SAFETY: バッファは shared の storage なので `contents()` は CPU から書ける
            // `buffer.length()` バイトの領域を指し、`entry` が `Retained` を持つので書き込みの間は生きている。
            // `host_range`（`validate_buffer_range` を呼ぶ）が `offset + bytes.len() <= length()` を保証している。
            // `&mut BufferEntry` を受けているので、この領域を CPU 側で読み書きする他の参照は同時に存在しない
            // （表の項目は表の錠の中で、または表から外して単独で所有した状態でしか借りられない）。
            // GPU の読み書きと重ならないことは、
            // この背後実装が投入するコマンドバッファがエンコーダを開かず、どのバッファも参照しない
            // （記録の操作を持たない）ことに根拠を置く（記録の操作を足す変更では、
            // このブロックの同期の根拠を必ず書き直す）。`bytes` は呼び出し側の借用した領域で、
            // Metal の領域とは重ならない。
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(offset), bytes.len());
            }
        }
        Ok(())
    }

    /// `entry` のバッファの全バイトを 0 にする。
    fn zero_fill(entry: &mut BufferEntry) -> Result<(), ContractError> {
        let total = u64::try_from(entry.buffer.length())
            .map_err(|_| ContractError::InvalidArgument("buffer length does not fit u64".into()))?;
        let mut offset: u64 = 0;
        while offset < total {
            let remaining = total - offset;
            let step = remaining.min(ZEROS.len() as u64);
            // `step` は `ZEROS.len()` 以下なので usize に収まる。
            let chunk = &ZEROS[..usize::try_from(step).unwrap_or(ZEROS.len())];
            write_range(entry, offset, chunk)?;
            offset += step;
        }
        Ok(())
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
            let mut buffers = self.lock_buffers();
            // ハンドルの識別子は再利用しない。解放済みのハンドルが新しい利用者のバッファに当たらないようにする。
            // 一覧から取る前に払い出すので、あふれて失敗しても一覧のバッファは減らない。
            let id = buffers.next_id;
            buffers.next_id = id.checked_add(1).ok_or_else(failed)?;
            // 同じ `byte_len` の再利用の一覧にあれば、それを使う（中身は一覧に入れる前に 0 にしてある）。
            // 無ければ Metal のバッファを新しく作る（確保 API が全バイトを 0 にする）。
            let (slot, buffer) = match buffers.pool.take(byte_len) {
                Some(pooled) => pooled,
                None => {
                    // 錠を放してから確保し、取り直す。
                    drop(buffers);
                    let buffer = self
                        .device
                        .newBufferWithLength_options(length, MTLResourceOptions::StorageModeShared)
                        .ok_or_else(failed)?;
                    buffers = self.lock_buffers();
                    let slot = buffers.next_slot;
                    buffers.next_slot = slot.checked_add(1).ok_or_else(failed)?;
                    (slot, buffer)
                }
            };
            buffers.map.insert(id, BufferEntry { buffer, slot });
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
            self.with_buffer(dst, |entry| write_range(entry, dst_offset, bytes))
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

        /// この device のハンドルなら、表から外し、全バイトを 0 にしてから再利用の一覧へ回す。
        ///
        /// 一覧が上限（件数・大きさの合計）に達していれば、ゼロ化せずに解放する。ゼロ化に失敗したバッファも
        /// 一覧に入れず解放する（以前の内容を次の `allocate` の利用者に見せない側に倒す）。
        /// 今の投入はどのバッファも参照しないので、一覧へは無条件に回す（参照する未完了の投入があるかの
        /// 実行時の検査は無い。記録の操作を足す変更では、遅延解放と合わせて見直す）。
        /// 一覧に入ったバッファは device の Drop まで保持される（合計の上限は 64 MiB）。
        /// 別の device のハンドルは何もしない（同じ識別子を持つ自 device のバッファを巻き込まない）。
        fn free(&self, buffer: BufferHandle) {
            if buffer.__backend_device_id() != self.id
                || buffer.__backend_generation() != self.generation
            {
                return;
            }
            let removed = self.lock_buffers().map.remove(&buffer.__backend_id());
            let Some(entry) = removed else {
                return;
            };
            // 一覧の分類は、ハンドルの申告ではなく Metal のバッファの実際の長さで決める。
            let Ok(byte_len) = u64::try_from(entry.buffer.length()) else {
                return;
            };
            // 表から外したので、このバッファはどのハンドルからも到達できない。ゼロ化は錠の外で行う。
            let rejected = retire(
                byte_len,
                entry,
                |len| self.lock_buffers().pool.would_accept(len),
                zero_fill,
                |entry| {
                    let BufferEntry { buffer, slot } = entry;
                    self.lock_buffers()
                        .pool
                        .put(byte_len, slot, buffer)
                        .map(|buffer| BufferEntry { buffer, slot })
                },
            );
            // 錠を放してから落とす。Metal のバッファの解放を錠の中で行わない。
            drop(rejected);
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
        use crate::backend_metal::POOL_MAX_ENTRIES;

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

        /// 一覧が空の状態の確保は、全バイトが 0（MEM-2 のポインタ）。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn a_fresh_allocation_reads_back_all_zero() {
            let device = new_device();
            for len in [1_u64, 4096, 4097, 65_536] {
                let h = device
                    .allocate(len, BufferUsage::empty())
                    .expect("allocate");
                let bytes = device.read_range(&h, 0, len).expect("read");
                assert_eq!(bytes.len() as u64, len);
                assert_eq!(bytes.iter().filter(|b| **b != 0).count(), 0, "len={len}");
            }
            assert_eq!(device.test_pool_reuse_count(), 0);
        }

        /// upload -> free -> allocate を 1,000 回くり返しても、再利用されたバッファから以前の内容が読めない
        /// （MEM-17 のポインタ）。再利用が実際に起きたこと（回数と slot の一致の回数）も確かめる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn reused_buffers_never_expose_previous_contents_over_1000_rounds() {
            const LEN: u64 = 4097;
            let device = new_device();
            let mut handle = device
                .allocate(LEN, BufferUsage::empty())
                .expect("allocate");
            let mut seen_slot = device.test_slot_of(&handle).expect("slot");
            for round in 0..1000_u32 {
                let fill = u8::try_from(round % 255).unwrap_or(0) + 1;
                device
                    .upload(&handle, 0, &vec![fill; LEN as usize])
                    .expect("upload");
                device.free(handle);
                handle = device
                    .allocate(LEN, BufferUsage::empty())
                    .expect("allocate");
                let bytes = device.read_range(&handle, 0, LEN).expect("read");
                let nonzero = bytes.iter().filter(|b| **b != 0).count();
                assert_eq!(nonzero, 0, "round={round}");
                let slot = device.test_slot_of(&handle).expect("slot");
                assert_eq!(slot, seen_slot, "round={round}: the slot was not reused");
                seen_slot = slot;
            }
            let reused = device.test_pool_reuse_count();
            let matched = device.test_pool_slot_match_count();
            println!("pool_reuse_count={reused} pool_slot_match_count={matched}");
            assert!(reused >= 1, "no reuse happened");
            assert_eq!(reused, 1000);
            assert_eq!(matched, reused);
        }

        /// 解放したハンドルは、同じ大きさの再利用の後も新しいハンドルに当たらない。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn a_freed_handle_does_not_alias_the_reused_buffer() {
            let device = new_device();
            let first = device.allocate(64, BufferUsage::empty()).expect("allocate");
            let stale = BufferHandle::__backend_new(
                first.__backend_generation(),
                first.__backend_id(),
                first.__backend_device_id(),
                64,
                BufferUsage::empty(),
            );
            let first_slot = device.test_slot_of(&first).expect("slot");
            device.free(first);
            let second = device.allocate(64, BufferUsage::empty()).expect("allocate");
            assert_ne!(stale.__backend_id(), second.__backend_id());
            assert_eq!(device.test_pool_reuse_count(), 1);
            assert_eq!(device.test_slot_of(&second), Some(first_slot));
            let r = device.upload(&stale, 0, &[1]);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            // 解放済みのハンドルでの二重の free は、一覧に何も足さない。
            device.free(stale);
            assert_eq!(device.test_pool_len(), 0);
        }

        /// 一覧が上限を超えて溜まらない。別の大きさは再利用されない。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn the_pool_is_bounded_and_matches_only_the_same_length() {
            let device = new_device();
            let handles: Vec<_> = (0..POOL_MAX_ENTRIES + 4)
                .map(|_| device.allocate(32, BufferUsage::empty()).expect("allocate"))
                .collect();
            for h in handles {
                device.free(h);
            }
            assert_eq!(device.test_pool_len(), POOL_MAX_ENTRIES);
            let other = device.allocate(33, BufferUsage::empty()).expect("allocate");
            assert_eq!(device.test_pool_reuse_count(), 0);
            assert_eq!(device.test_pool_len(), POOL_MAX_ENTRIES);
            device.free(other);
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

    /// `host_range` の境界。device に触らない。
    #[test]
    fn host_range_passes_in_range_and_rejects_the_rest() {
        assert_eq!(host_range(0, 16, 16).unwrap(), (0, 16));
        assert_eq!(host_range(8, 8, 16).unwrap(), (8, 8));
        assert_eq!(host_range(16, 0, 16).unwrap(), (16, 0));
        assert_eq!(host_range(0, 0, 16).unwrap(), (0, 0));
        for (offset, len) in [
            (8, 9),
            (0, 17),
            (17, 0),
            (u64::MAX, 1),
            (1, u64::MAX),
            (u64::MAX, u64::MAX),
        ] {
            let r = host_range(offset, len, 16);
            assert!(
                matches!(r, Err(ContractError::InvalidArgument(_))),
                "offset={offset} len={len}: {r:?}"
            );
        }
    }

    #[test]
    fn host_range_rejects_an_overflowing_end_even_for_the_largest_allocation() {
        let r = host_range(u64::MAX, 1, u64::MAX);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        assert_eq!(host_range(0, 0, 0).unwrap(), (0, 0));
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
    fn poll_ex_covers_pending_complete_runtime_error_and_unknown_as_distinct_values() {
        let mut subs = table(&[vec![Running], vec![Done], vec![Done, Failed]]);
        subs.reclaim(classify);
        let all = [
            subs.state_ex_of(1, classify),
            subs.state_ex_of(2, classify),
            subs.state_ex_of(3, classify),
            subs.state_ex_of(4, classify),
        ];
        assert_eq!(
            all,
            [
                CompletionStateEx::Pending,
                CompletionStateEx::Complete,
                CompletionStateEx::RuntimeError,
                CompletionStateEx::Unknown,
            ]
        );
    }

    #[test]
    fn poll_ex_of_a_reclaimed_group_is_complete_and_of_a_reclaimed_failure_is_runtime_error() {
        let mut subs = table(&[vec![Done], vec![Failed], vec![Done]]);
        subs.reclaim(classify);
        assert!(subs.pending.is_empty());
        assert_eq!(subs.state_ex_of(1, classify), CompletionStateEx::Complete);
        assert_eq!(
            subs.state_ex_of(2, classify),
            CompletionStateEx::RuntimeError
        );
        assert_eq!(subs.state_ex_of(3, classify), CompletionStateEx::Complete);
    }

    #[test]
    fn poll_ex_of_zero_and_unissued_sequences_is_unknown_never_complete() {
        let mut subs = table(&[vec![Done]]);
        subs.reclaim(classify);
        assert_eq!(subs.state_ex_of(0, classify), CompletionStateEx::Unknown);
        assert_eq!(subs.state_ex_of(2, classify), CompletionStateEx::Unknown);
        assert_eq!(
            subs.state_ex_of(u64::MAX, classify),
            CompletionStateEx::Unknown
        );
        let fresh: Submissions<Vec<BufState>> = Submissions::new();
        assert_eq!(fresh.state_ex_of(1, classify), CompletionStateEx::Unknown);
    }

    #[test]
    fn poll_ex_agrees_with_poll_except_for_the_two_extra_values() {
        let mut subs = table(&[vec![Running], vec![Done], vec![Failed]]);
        subs.reclaim(classify);
        for seq in 0..=4u64 {
            let ex = subs.state_ex_of(seq, classify);
            match subs.state_of(seq, classify) {
                CompletionState::Pending => assert_eq!(ex, CompletionStateEx::Pending),
                CompletionState::Complete => assert_eq!(ex, CompletionStateEx::Complete),
                CompletionState::Lost => assert!(
                    matches!(
                        ex,
                        CompletionStateEx::RuntimeError | CompletionStateEx::Unknown
                    ),
                    "{seq}: {ex:?}"
                ),
            }
        }
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
    fn on_complete_only_ever_sees_a_terminal_state() {
        assert_eq!(
            terminal_state(CompletionState::Complete),
            CompletionState::Complete
        );
        assert_eq!(terminal_state(CompletionState::Lost), CompletionState::Lost);
        assert_eq!(
            terminal_state(CompletionState::Pending),
            CompletionState::Lost
        );
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
    fn pool_returns_the_same_slot_only_for_the_same_length() {
        let mut pool: SizePool<&str> = SizePool::new();
        assert!(pool.put(16, 7, "a").is_none());
        assert!(pool.take(32).is_none());
        assert_eq!(pool.entries, 1);
        assert_eq!(pool.take(16), Some((7, "a")));
        assert_eq!(pool.entries, 0);
        assert_eq!(pool.bytes, 0);
        assert!(pool.take(16).is_none());
    }

    #[test]
    fn pool_counts_reuses_and_slot_matches() {
        let mut pool: SizePool<u8> = SizePool::new();
        assert!(pool.take(16).is_none());
        assert_eq!(pool.reuse_count, 0);
        for round in 0..10_u8 {
            assert!(pool.put(16, 3, round).is_none());
            assert_eq!(pool.take(16), Some((3, round)));
        }
        assert_eq!(pool.reuse_count, 10);
        assert_eq!(pool.slot_match_count, 10);
        // 別の slot を入れた後に古い slot を取り出しても、一致には数えない。
        assert!(pool.put(16, 4, 0).is_none());
        assert!(pool.put(16, 5, 1).is_none());
        assert_eq!(pool.take(16), Some((5, 1)));
        assert_eq!(pool.take(16), Some((4, 0)));
        assert_eq!(pool.reuse_count, 12);
        assert_eq!(pool.slot_match_count, 11);
    }

    #[test]
    fn pool_rejects_past_the_entry_limit_and_keeps_its_contents() {
        let mut pool: SizePool<usize> = SizePool::new();
        for i in 0..POOL_MAX_ENTRIES {
            assert!(pool.put(8, i as u64, i).is_none());
        }
        assert_eq!(pool.put(8, 99, 99), Some(99));
        assert_eq!(pool.entries, POOL_MAX_ENTRIES);
        assert_eq!(pool.bytes, 8 * POOL_MAX_ENTRIES as u64);
        // 取り出すと空きができる。
        assert!(pool.take(8).is_some());
        assert!(pool.put(8, 99, 99).is_none());
    }

    #[test]
    fn pool_rejects_past_the_byte_limit_and_overflow() {
        let mut pool: SizePool<u8> = SizePool::new();
        assert!(pool.put(POOL_MAX_BYTES, 1, 0).is_none());
        assert_eq!(pool.put(1, 2, 1), Some(1));
        assert_eq!(pool.bytes, POOL_MAX_BYTES);
        assert_eq!(pool.entries, 1);
        let mut empty: SizePool<u8> = SizePool::new();
        assert_eq!(empty.put(POOL_MAX_BYTES + 1, 1, 0), Some(0));
        assert_eq!(empty.put(u64::MAX, 1, 0), Some(0));
        assert_eq!(empty.entries, 0);
        assert!(empty.by_len.is_empty());
    }

    #[test]
    fn would_accept_follows_the_limits_without_changing_the_pool() {
        let mut pool: SizePool<u8> = SizePool::new();
        assert!(pool.would_accept(1));
        assert!(pool.would_accept(POOL_MAX_BYTES));
        assert!(!pool.would_accept(POOL_MAX_BYTES + 1));
        assert!(!pool.would_accept(u64::MAX));
        assert_eq!(pool.entries, 0);
        assert!(pool.put(POOL_MAX_BYTES - 1, 1, 0).is_none());
        assert!(pool.would_accept(1));
        assert!(!pool.would_accept(2));
        for i in 0..POOL_MAX_ENTRIES - 1 {
            let _ = pool.put(0, i as u64, 0);
        }
        assert!(!pool.would_accept(0));
    }

    /// `retire` の試験用に、呼ばれた順を記録する。
    fn run_retire(
        pool: &mut SizePool<u8>,
        byte_len: u64,
        zero_ok: bool,
        calls: &mut Vec<&'static str>,
    ) -> Option<u8> {
        let pool = std::cell::RefCell::new(pool);
        let calls = std::cell::RefCell::new(calls);
        retire(
            byte_len,
            9_u8,
            |len| {
                calls.borrow_mut().push("accepts");
                pool.borrow().would_accept(len)
            },
            |_buffer: &mut u8| {
                calls.borrow_mut().push("zero");
                if zero_ok { Ok(()) } else { Err(()) }
            },
            |buffer| {
                calls.borrow_mut().push("put");
                pool.borrow_mut().put(byte_len, 1, buffer)
            },
        )
    }

    #[test]
    fn retire_zeroes_then_puts_when_the_pool_accepts() {
        let mut pool: SizePool<u8> = SizePool::new();
        let mut calls = Vec::new();
        assert_eq!(run_retire(&mut pool, 16, true, &mut calls), None);
        assert_eq!(calls, ["accepts", "zero", "put"]);
        assert_eq!(pool.take(16), Some((1, 9)));
    }

    #[test]
    fn retire_does_not_put_a_buffer_that_failed_to_zero() {
        let mut pool: SizePool<u8> = SizePool::new();
        let mut calls = Vec::new();
        assert_eq!(run_retire(&mut pool, 16, false, &mut calls), Some(9));
        assert_eq!(calls, ["accepts", "zero"]);
        assert_eq!(pool.entries, 0);
    }

    #[test]
    fn retire_does_not_zero_a_size_the_pool_will_not_accept() {
        let mut pool: SizePool<u8> = SizePool::new();
        let mut calls = Vec::new();
        assert_eq!(
            run_retire(&mut pool, POOL_MAX_BYTES + 1, true, &mut calls),
            Some(9)
        );
        assert_eq!(calls, ["accepts"]);
        assert_eq!(pool.entries, 0);
    }

    #[test]
    fn take_next_does_not_move_an_exhausted_counter() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(take_next(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}
