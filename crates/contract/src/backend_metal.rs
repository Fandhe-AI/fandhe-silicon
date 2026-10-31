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
//! `create_pipeline`（TASK-773 のポインタ）は MSL のソースから compute パイプライン状態を作り、パイプラインの表に
//! `layout` の写しと一緒に置く。function constant の設定の `unsafe` ブロックは 1 つで、記録の操作とは関係しない。
//! 投入済みの表は device の錠の中に置き、記録先の後始末の hook は識別子の集合だけを握る。
//! `on_complete` の登録の表は投入済みの表と同じ錠の中に置き、callback は錠を放してから呼ぶ。
//! 駆動は `submit`・`submit_many`・`poll`・`poll_ex`・`wait_blocking`・`allocate` の中で行い、待ちのスレッドは持たない。

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, ContractError};
use fandhe_silicon_core::execution::{
    __BackendSharedEncoderCloser, FunctionConstant, FunctionConstantValue, GpuTiming,
    ImportedSemaphoreHandle, PipelineInput, SubmitToken,
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

/// `create_pipeline` が受け付ける入力から取り出した、ソース・入口名・function constant の並び。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
type AcceptedPipelineInput<'a> = (&'a str, &'a str, &'a [FunctionConstant]);

/// `create_pipeline` の入力を分類する。device にも Metal の API にも触らない。
///
/// `Msl`・`MslSpecialized` は取り出して返す（`Msl` の function constant は空の並び）。
/// コンパイル選択つきの MSL と WGSL は、この背後実装が提供しない操作として `Unsupported` にする。
/// ほかの言語の成果物は `ArtifactRejected { kind: "unsupported-input", .. }` にし、理由に受け取った
/// variant 名と受け付ける variant を英語で載せる。`PipelineInput` は `#[non_exhaustive]` なので、
/// 知らない variant も `Unsupported` にする（fail-closed）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn classify_pipeline_input<'a>(
    input: &PipelineInput<'a>,
) -> Result<AcceptedPipelineInput<'a>, ContractError> {
    let unsupported = || ContractError::Unsupported {
        op: "create_pipeline",
        backend: BackendKind::Metal,
    };
    let rejected = |variant: &str| ContractError::ArtifactRejected {
        kind: "unsupported-input",
        reason: format!(
            "pipeline input {variant} is not accepted by the Metal backend; accepted inputs are Msl and MslSpecialized"
        ),
    };
    match input {
        PipelineInput::Msl {
            source,
            entry_point,
        } => Ok((source, entry_point, &[])),
        PipelineInput::MslSpecialized {
            source,
            entry_point,
            function_constants,
        } => Ok((source, entry_point, function_constants)),
        PipelineInput::MslWithOptions { .. }
        | PipelineInput::MslSpecializedWithOptions { .. }
        | PipelineInput::Wgsl { .. } => Err(unsupported()),
        PipelineInput::SpirV { .. } => Err(rejected("SpirV")),
        PipelineInput::CudaC { .. } => Err(rejected("CudaC")),
        PipelineInput::Ptx { .. } => Err(rejected("Ptx")),
        PipelineInput::Dxil { .. } => Err(rejected("Dxil")),
        _ => Err(unsupported()),
    }
}

/// `layout.slots` に同じ番号が 2 つ以上あれば `InvalidArgument`（重複した最初の番号を載せる）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_unique_slots(slots: &[u32]) -> Result<(), ContractError> {
    let mut seen = HashSet::new();
    for slot in slots {
        if !seen.insert(*slot) {
            return Err(ContractError::InvalidArgument(format!(
                "duplicate slot in binding layout: {slot}"
            )));
        }
    }
    Ok(())
}

/// function constant の `index` に重複があれば `InvalidArgument`（重複した最初の番号を載せる）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn check_unique_constants(constants: &[FunctionConstant]) -> Result<(), ContractError> {
    let mut seen = HashSet::new();
    for constant in constants {
        if !seen.insert(constant.index) {
            return Err(ContractError::InvalidArgument(format!(
                "duplicate function constant index: {}",
                constant.index
            )));
        }
    }
    Ok(())
}

/// function constant の値の種別。Metal の `MTLDataType` の `UInt`・`Bool` に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum ConstantKind {
    /// 32 ビット符号なし整数（`MTLDataType::UInt`。4 バイト）。
    UInt,
    /// 真偽値（`MTLDataType::Bool`。1 バイト）。
    Bool,
}

impl ConstantKind {
    /// この種別の値が Metal に渡るときのバイト数。
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    const fn byte_len(self) -> usize {
        match self {
            Self::UInt => 4,
            Self::Bool => 1,
        }
    }
}

/// function constant の値を（種別, ネイティブエンディアンのバイト列）に写す。
///
/// `FunctionConstantValue` は `#[non_exhaustive]` なので、知らない型は `Unsupported`（fail-closed）。
/// 返すバイト列の長さは種別の `byte_len` と一致する（`Bool` は偽が 0、真が 1）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn constant_wire(value: &FunctionConstantValue) -> Result<(ConstantKind, Vec<u8>), ContractError> {
    match value {
        FunctionConstantValue::U32(v) => Ok((ConstantKind::UInt, v.to_ne_bytes().to_vec())),
        FunctionConstantValue::Bool(v) => Ok((ConstantKind::Bool, vec![u8::from(*v)])),
        _ => Err(ContractError::Unsupported {
            op: "create_pipeline",
            backend: BackendKind::Metal,
        }),
    }
}

/// function constant の並びを、重複の検査と値の写しを済ませた（番号, 種別, バイト列）の並びにする。
/// Metal の API を呼ぶ前に、`create_pipeline` から呼ばれる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn prepare_constants(
    constants: &[FunctionConstant],
) -> Result<Vec<(u32, ConstantKind, Vec<u8>)>, ContractError> {
    check_unique_constants(constants)?;
    constants
        .iter()
        .map(|c| constant_wire(&c.value).map(|(kind, bytes)| (c.index, kind, bytes)))
        .collect()
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

/// 時刻の表が持つ件数の上限。超えたら通し番号の小さいものから捨てる。
#[cfg(any(test, feature = "native-blocking-wait"))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const TIMING_TABLE_MAX: usize = 1024;

/// コマンドバッファの列の (開始, 終了)（秒）から、投入全体の (最小の開始, 最大の終了) を求める。
///
/// 空の列・有限でない値・0 以下の値（Metal は時刻を得られないとき 0 を返す）・終了が開始より前の組があれば
/// `None`（時刻として残さない）。
#[cfg(any(test, feature = "native-blocking-wait"))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn span_of(times: impl IntoIterator<Item = (f64, f64)>) -> Option<(f64, f64)> {
    let mut span: Option<(f64, f64)> = None;
    for (start, end) in times {
        if !start.is_finite() || !end.is_finite() || start <= 0.0 || end <= 0.0 || end < start {
            return None;
        }
        span = Some(match span {
            None => (start, end),
            Some((s, e)) => (s.min(start), e.max(end)),
        });
    }
    span
}

/// `on_complete` に渡された callback。native 版の `CommandQueue::on_complete` の引数と同じ型。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
type Callback = Box<dyn FnOnce(CompletionState) + Send>;

/// 投入済みの表と、`on_complete` の登録の表。投入の通し番号は device ごとに 1 から払い出す。
///
/// `G` は 1 回の投入が持つ列の型で、本物では Objective-C のコマンドバッファの列、試験では偽の状態の列。
/// `C` は登録された callback の型。
///
/// 登録の表と投入済みの表は同じ錠の中に置く（device にはほかに `buffers`・`open` の錠があるが、この表の操作は
/// それらを同時に取らない）。callback はどの錠も握らずに呼ぶ。
/// 登録の判定は `state_of` が行い、`pending` から回収で外れた番号でも watermark と失敗の集合から
/// 終端の値が決まるので、登録の表は `pending` と独立に持てる。
/// callback は必ず表から外して錠を放してから呼ぶ。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct Submissions<G, C = Callback> {
    /// 次に払い出す通し番号。
    next_sequence: u64,
    /// この番号以下は回収済み（失敗の集合にあるものを除き完了）。
    watermark: u64,
    /// 通し番号から列への表。回収されるまで保持する。
    pending: BTreeMap<u64, G>,
    /// 失敗として回収した通し番号。watermark 以下の失敗の番号だけを持つので、増えるのは失敗の件数だけ。
    failed: HashSet<u64>,
    /// 通し番号から (開始, 終了)（秒）への表。書くのは完了で起きる待ち（`wait_blocking(None)`）が完了を検出した時だけ。
    /// `TIMING_TABLE_MAX` 件を超えたら通し番号の最も小さいものから捨てる。
    timings: BTreeMap<u64, (f64, f64)>,
    /// 未完了のまま登録された callback。登録の順に持つ。呼ぶときに表から外すので、どれも 1 回しか呼ばれない。
    callbacks: Vec<(u64, C)>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl<G, C> Submissions<G, C> {
    /// 何も投入していない表。
    fn new() -> Self {
        Self {
            next_sequence: 1,
            watermark: 0,
            pending: BTreeMap::new(),
            failed: HashSet::new(),
            timings: BTreeMap::new(),
            callbacks: Vec::new(),
        }
    }

    /// 時刻を表に書く。上限を超えた分は通し番号の最も小さいものから捨てる。
    #[cfg(any(test, feature = "native-blocking-wait"))]
    fn record_timing(&mut self, sequence: u64, start_seconds: f64, end_seconds: f64) {
        self.timings.insert(sequence, (start_seconds, end_seconds));
        while self.timings.len() > TIMING_TABLE_MAX {
            self.timings.pop_first();
        }
    }

    /// 完了を待ち終えた時の時刻の扱い。`record` が真で `span` が `Some` の時だけ表へ書き、書いたかを返す。
    ///
    /// `wait_blocking(None)` だけが `record = true` で呼ぶ。期限付きの待ちと `on_complete` は呼ばない。
    /// `span` は完了を確かめた列の `span_of` の値で、未完了・不正な時刻なら `None`。
    #[cfg(any(test, feature = "native-blocking-wait"))]
    fn complete_wait(&mut self, sequence: u64, span: Option<(f64, f64)>, record: bool) -> bool {
        match (record, span) {
            (true, Some((start, end))) => {
                self.record_timing(sequence, start, end);
                true
            }
            _ => false,
        }
    }

    /// 通し番号の GPU 側の時刻の問合せ。表と投入済みの表を見るだけで、回収も状態の変更もしない。
    ///
    /// 時刻の表にあれば `Available`。無ければ、投入済みの表にあるものは未完了が `Pending`、
    /// 時刻を残さずに完了したものが `Reclaimed`、失敗が `Unavailable`。表から外れた番号は、失敗の集合にあれば
    /// `Unavailable`、watermark 以下の他は（時刻を残さずに完了した・表から捨てられたものとして）`Reclaimed`。
    /// 0 と払い出していない番号は `Unavailable`。
    fn timing_of(&self, sequence: u64, classify: impl Fn(&G) -> GroupState) -> GpuTiming {
        if let Some(&(start_seconds, end_seconds)) = self.timings.get(&sequence) {
            return GpuTiming::Available {
                start_seconds,
                end_seconds,
            };
        }
        if let Some(group) = self.pending.get(&sequence) {
            return match classify(group) {
                GroupState::Pending => GpuTiming::Pending,
                GroupState::Completed => GpuTiming::Reclaimed,
                GroupState::Failed => GpuTiming::Unavailable,
            };
        }
        if self.failed.contains(&sequence) {
            return GpuTiming::Unavailable;
        }
        if sequence >= 1 && sequence <= self.watermark {
            GpuTiming::Reclaimed
        } else {
            GpuTiming::Unavailable
        }
    }

    /// callback を登録する。回収はしない（登録は駆動の呼び出しではない）。
    ///
    /// 番号の状態が終端なら登録せず、callback と状態を返す。呼ぶのは錠を放してから。
    /// `Pending` なら表に積んで `None`。
    fn register(
        &mut self,
        sequence: u64,
        callback: C,
        classify: impl Fn(&G) -> GroupState,
    ) -> Option<(C, CompletionState)> {
        match self.state_of(sequence, classify) {
            CompletionState::Pending => {
                self.callbacks.push((sequence, callback));
                None
            }
            done => Some((callback, done)),
        }
    }

    /// 回収してから、終端になった登録を表から外して返す。`Pending` のものは残す。
    ///
    /// 先頭が未完了で回収が止まっていても、後ろの完了済みの列の登録は外れる。呼ぶのは錠を放してから。
    /// 登録の件数に比例して走査する。判定（`classify`）をすべて済ませてから外すので、判定が panic しても登録は失われない。
    fn take_terminal(
        &mut self,
        classify: impl Fn(&G) -> GroupState + Copy,
    ) -> Vec<(C, CompletionState)> {
        self.reclaim(classify);
        let states: Vec<CompletionState> = self
            .callbacks
            .iter()
            .map(|(sequence, _)| self.state_of(*sequence, classify))
            .collect();
        let mut ready = Vec::new();
        let mut kept = Vec::new();
        for ((sequence, callback), state) in
            std::mem::take(&mut self.callbacks).into_iter().zip(states)
        {
            match state {
                CompletionState::Pending => kept.push((sequence, callback)),
                done => ready.push((callback, done)),
            }
        }
        self.callbacks = kept;
        ready
    }

    /// 登録されている番号の列の完了を `wait` で待つ。`Drop` が残りを呼び切る前に使う。
    fn wait_registered(&self, wait: impl Fn(&G)) {
        for (sequence, _) in &self.callbacks {
            if let Some(group) = self.pending.get(sequence) {
                wait(group);
            }
        }
    }

    /// 登録を全部外して返す。待ったあとでも `Pending` の番号は `Lost`（起きないはずの経路の fail-closed）。
    ///
    /// 判定を済ませてから外すので、判定が panic しても登録は失われない。
    fn take_all(
        &mut self,
        classify: impl Fn(&G) -> GroupState + Copy,
    ) -> Vec<(C, CompletionState)> {
        self.reclaim(classify);
        let states: Vec<CompletionState> = self
            .callbacks
            .iter()
            .map(|(sequence, _)| terminal_state(self.state_of(*sequence, classify)))
            .collect();
        std::mem::take(&mut self.callbacks)
            .into_iter()
            .zip(states)
            .map(|((_, callback), state)| (callback, state))
            .collect()
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

/// 使わない panic の中身を捨てる。中身の `Drop` が panic しても、そこから巻き戻らない。
///
/// 破棄で出た panic の中身も `forget` で捨てる（その破棄から呼び出し元へ巻き戻らないため。漏れるのは panic の中身だけで、
/// メモリ安全には触れない）。残りの callback を呼び切る経路と `Drop` の経路が、破棄で途切れないために通す。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn discard_panic(payload: Box<dyn std::any::Any + Send>) {
    if let Err(inner) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload))) {
        std::mem::forget(inner);
    }
}

/// 外した callback を全部呼ぶ。1 つが panic しても残りを呼び切り、最初の panic の中身を返す。
///
/// 2 個目以降の panic の中身は `discard_panic` で捨てる（その破棄の panic でも残りを呼び切る）。
///
/// 錠を放してから呼ぶこと。呼ぶ側が駆動の呼び出しの最後に `resume_unwind` で再送出する
/// （`Drop` では捨てる）。取りこぼしと二重呼び出しを作らないため、各 callback は呼ぶ前にすでに表から外れている。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn run_callbacks<C>(
    ready: Vec<(C, CompletionState)>,
    call: impl Fn(C, CompletionState),
) -> Option<Box<dyn std::any::Any + Send>> {
    let mut first_panic = None;
    for (callback, state) in ready {
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(callback, state)));
        if let Err(payload) = outcome {
            if first_panic.is_none() {
                first_panic = Some(payload);
            } else {
                discard_panic(payload);
            }
        }
    }
    first_panic
}

/// 表の錠を取る。毒化していても中身を使う（表は壊れない更新だけを行う）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn lock_table<G, C>(table: &Mutex<Submissions<G, C>>) -> MutexGuard<'_, Submissions<G, C>> {
    table.lock().unwrap_or_else(|e| e.into_inner())
}

/// 駆動の本体。回収して終端の登録を外すまでを錠の中で行い、錠を放してから `call` で 1 回ずつ呼ぶ。
///
/// 登録が空なら回収もしない。callback が panic しても残りを呼び切り、最後に最初の panic を
/// `resume_unwind` で伝える（利用者の callback の panic を伝えるもので、この関数自身は panic しない）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn drive_table<G, C>(
    table: &Mutex<Submissions<G, C>>,
    classify: impl Fn(&G) -> GroupState + Copy,
    call: impl Fn(C, CompletionState),
) {
    let ready = {
        let mut subs = lock_table(table);
        if subs.callbacks.is_empty() {
            return;
        }
        subs.take_terminal(classify)
    };
    if let Some(payload) = run_callbacks(ready, call) {
        std::panic::resume_unwind(payload);
    }
}

/// 登録の本体。終端ならその場で（錠を放してから）`call` で 1 回呼び、未完了なら表に積む。回収はしない。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn register_table<G, C>(
    table: &Mutex<Submissions<G, C>>,
    sequence: u64,
    callback: C,
    classify: impl Fn(&G) -> GroupState,
    call: impl Fn(C, CompletionState),
) {
    let ready = lock_table(table).register(sequence, callback, classify);
    if let Some((callback, state)) = ready {
        call(callback, state);
    }
}

/// `Drop` の本体。登録が残っていれば、その列を `wait` で待ってから、残りを `call` で 1 回ずつ呼ぶ。
///
/// `&mut` なので錠は取らない。callback の panic は捨てる（`Drop` から伝えない）。捨てる中身の破棄の panic も止める。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn finish_table<G, C>(
    subs: &mut Submissions<G, C>,
    wait: impl Fn(&G),
    classify: impl Fn(&G) -> GroupState + Copy,
    call: impl Fn(C, CompletionState),
) {
    if subs.callbacks.is_empty() {
        return;
    }
    subs.wait_registered(wait);
    let ready = subs.take_all(classify);
    if let Some(payload) = run_callbacks(ready, call) {
        discard_panic(payload);
    }
}

/// 終端の状態に寄せる。待ったあとでも `Pending` なら（起きないはずだが）失敗側の `Lost` にする。
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
    use super::span_of;
    #[cfg(feature = "native-blocking-wait")]
    use super::wait_result;
    use super::{
        BufState, Callback, ConstantKind, GroupState, OpenEncoders, SizePool, Submissions,
        check_token_owner, check_unique_slots, classify_pipeline_input, drive_table, finish_table,
        group_state_of, precheck_submit, precheck_submit_many, prepare_constants, register_table,
        retire, take_next,
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
        __BackendSharedEncoderCloser, __backend_next_encoder_id, BindingLayout, CommandEncoder,
        CommandQueue, ComputeOps, GpuTiming, ImportedSemaphoreHandle, PipelineHandle,
        PipelineInput, SubmitToken,
    };
    use fandhe_silicon_core::memory::{
        BufferHandle, BufferUsage, MemoryOps, validate_allocation_size,
    };
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    // コマンドラインのプログラムで `MTLCreateSystemDefaultDevice` が既定の device を返すには、
    // Core Graphics がリンクされている必要がある。参照を置いてリンクを確実にする（シンボルは使わない）。
    use objc2_core_graphics as _;
    use objc2_foundation::NSString;
    use objc2_metal::{
        MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandQueue,
        MTLComputePipelineState, MTLCreateSystemDefaultDevice, MTLDataType, MTLDevice,
        MTLFunctionConstantValues, MTLLibrary, MTLResourceOptions,
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

    /// device のパイプラインの表。識別子は device ごとに 1 から払い出す。
    struct Pipelines {
        /// 次に払い出すパイプラインの識別子の源。`take_next` で払い出す。
        next_id: AtomicU64,
        /// 識別子から（パイプライン状態, 作成時に渡された `BindingLayout` の写し）への表。
        /// パイプラインの破棄の口は無く、作った分だけ件数が増え続ける（上限も無い）。
        map: HashMap<
            u64,
            (
                Retained<ProtocolObject<dyn MTLComputePipelineState>>,
                BindingLayout,
            ),
        >,
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
        /// パイプラインの表。作成した状態は device が持ち、破棄の口は持たない。
        pipelines: Mutex<Pipelines>,
        /// 開いている（まだ投入も破棄もされていない）記録先の識別子の集合。hook と共有する。
        open: Arc<OpenEncoders>,
        /// 投入済みの表。Objective-C の型を持つので、この錠の中だけに置く。
        submissions: Mutex<Submissions<Vec<CommandBuffer>>>,
        /// 試験専用。`Some` の間は `commit_buffers` が確定を遅らせ、ここに溜める（`None` なら従来どおり即確定）。
        /// 確定していないコマンドバッファは完了しないので、登録の時点で未完了の状態を決定的に作れる。
        /// 製品のビルドには存在しない。
        #[cfg(test)]
        commit_hold: Mutex<Option<Vec<CommandBuffer>>>,
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

        /// パイプラインの表の錠を取る。表は壊れない更新だけなので、汚れていても中身を使う。
        fn lock_pipelines(&self) -> MutexGuard<'_, Pipelines> {
            self.pipelines.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// 投入済みの表の錠を取る。表は壊れない更新だけなので、汚れていても中身を使う。
        fn lock_submissions(&self) -> MutexGuard<'_, Submissions<Vec<CommandBuffer>>> {
            self.submissions.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// 空のコマンドバッファを 1 つ作って確定し、通し番号で表に登録する。
        ///
        /// 一括投入も 1 つだけ作る（記録の操作が無いので 1 つで足りる）。作成・確定・採番・登録を表の錠の中で行う。
        /// 錠の中の `commandBuffer()` は、キューの未完了の上限に当たると確定済みの仕事の進みを待つだけで戻る。
        /// 製品の経路では、確定していないコマンドバッファを錠の中で溜めない（溜めると戻らなくなる）。
        /// 試験ビルドだけ、`commit_or_hold` で確定を遅らせて溜めることがある（数個まで。上限には届かない）。
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
            #[cfg(not(test))]
            buffer.commit();
            #[cfg(test)]
            self.commit_or_hold(&buffer);
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
                pipelines: Mutex::new(Pipelines {
                    next_id: AtomicU64::new(1),
                    map: HashMap::new(),
                }),
                open: Arc::new(OpenEncoders::new()),
                submissions: Mutex::new(Submissions::new()),
                #[cfg(test)]
                commit_hold: Mutex::new(None),
            }
        }

        /// 試験専用。確定を遅らせる状態なら溜め、そうでなければ確定する。`commit_buffers` から投入の錠の中で呼ぶ。
        ///
        /// 錠の順は 投入の錠 -> `commit_hold` の錠。`release_held_commits` は `commit_hold` の錠だけを取る。
        #[cfg(test)]
        fn commit_or_hold(&self, buffer: &CommandBuffer) {
            let mut hold = self.commit_hold.lock().unwrap_or_else(|e| e.into_inner());
            match hold.as_mut() {
                Some(held) => held.push(buffer.clone()),
                None => buffer.commit(),
            }
        }

        /// 試験専用。以後の投入の確定を遅らせる。
        #[cfg(test)]
        fn hold_commits(&self) {
            let mut hold = self.commit_hold.lock().unwrap_or_else(|e| e.into_inner());
            hold.get_or_insert_with(Vec::new);
        }

        /// 試験専用。確定を遅らせる状態をやめ、溜めたコマンドバッファを投入順に確定する。何度呼んでもよい。
        #[cfg(test)]
        fn release_held_commits(&self) -> Vec<CommandBuffer> {
            let held = self
                .commit_hold
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            let held: Vec<CommandBuffer> = held.into_iter().flatten().collect();
            for buffer in &held {
                buffer.commit();
            }
            held
        }

        /// 登録済みの callback のうち終端になったものを、呼び出し元のスレッドで 1 回ずつ呼ぶ。
        ///
        /// `submit`・`submit_many`・`poll`・`poll_ex`・`wait_blocking`・`allocate` の中で呼ぶ駆動点。本体は `drive_table`。
        fn drive_callbacks(&self) {
            drive_table(
                &self.submissions,
                group_state,
                |callback: Callback, state| callback(state),
            );
        }

        /// 対象の列の完了を呼び出し元のスレッドで待ち、その後の `poll` の値を返す。`wait_blocking(None)` が使う。
        ///
        /// 列を clone して表の錠を放してから待つ。待つ間も同じ錠を取る `poll`・`submit` を妨げない（錠を握ったまま待たない）。
        #[cfg(feature = "native-blocking-wait")]
        fn wait_for_completion(&self, token: SubmitToken, record_timing: bool) -> CompletionState {
            // 列は回収の前に取る（完了済みで先頭の列は回収で表から外れ、時刻を読めなくなる）。
            let group = {
                let mut subs = self.lock_submissions();
                let group = subs.pending.get(&token.sequence).cloned();
                subs.reclaim(group_state);
                group
            };
            if let Some(group) = &group {
                for buffer in group {
                    buffer.waitUntilCompleted();
                }
            }
            // 時刻を書くのは `wait_blocking(None)` だけ（書くかどうかは `complete_wait` が決める）。
            let span = group
                .as_ref()
                .filter(|group| group_state(group) == GroupState::Completed)
                .and_then(|group| {
                    span_of(
                        group
                            .iter()
                            .map(|buffer| (buffer.GPUStartTime(), buffer.GPUEndTime())),
                    )
                });
            self.lock_submissions()
                .complete_wait(token.sequence, span, record_timing);
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

    impl Drop for MetalDevice {
        /// 登録が残っていれば、その列の完了を待ってから、残りの callback を `poll` と同じ判定の値で 1 回ずつ呼ぶ。
        ///
        /// 登録が無ければ待たない。待ちに上限は無く、確定済みの command buffer が終わるまで戻らない。
        /// 「確定済みの command buffer は必ず終わる」ことに頼る。この前提は、投入するコマンドバッファが空である
        /// 現在の形に対するもので、記録の操作を足す変更では見直す。callback の panic は捨てる。本体は `finish_table`。
        fn drop(&mut self) {
            let subs = self
                .submissions
                .get_mut()
                .unwrap_or_else(|e| e.into_inner());
            finish_table(
                subs,
                |group| {
                    for buffer in group {
                        buffer.waitUntilCompleted();
                    }
                },
                group_state,
                |callback: Callback, state| callback(state),
            );
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
            self.drive_callbacks();
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
            self.drive_callbacks();
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
        ///
        /// 回収のあとで、終端になった登録済みの callback を呼ぶ（呼び出しが戻る前、このスレッドで）。
        fn poll(&self, token: SubmitToken) -> CompletionState {
            if check_token_owner(&token, self.id, self.generation).is_err() {
                return CompletionState::Lost;
            }
            let state = {
                let mut subs = self.lock_submissions();
                subs.reclaim(group_state);
                subs.state_of(token.sequence, group_state)
            };
            self.drive_callbacks();
            state
        }

        /// `poll` の詳しい版。未完了は `Pending`、完了は `Complete`、`Error` で終わった列は `RuntimeError`。
        ///
        /// 別 device の印・世代違いの印は `poll` と同じく `Lost`。払い出していない番号（0 を含む）は `Unknown`。
        /// 回収済みの番号は watermark と失敗の集合で答える（失敗した投入を `Complete` とは返さない）。
        /// `poll` は失敗した列と払い出していない番号を `Lost` とし、`poll_ex` はこれを `RuntimeError`・`Unknown` に分ける。
        /// `Pending`・`Complete` は `poll` と同じ。
        ///
        /// 回収のあとで、終端になった登録済みの callback を呼ぶ（`poll` と同じく、錠を放してからこのスレッドで）。
        fn poll_ex(&self, token: SubmitToken) -> CompletionStateEx {
            if check_token_owner(&token, self.id, self.generation).is_err() {
                return CompletionStateEx::Lost;
            }
            let state = {
                let mut subs = self.lock_submissions();
                subs.reclaim(group_state);
                subs.state_ex_of(token.sequence, group_state)
            };
            self.drive_callbacks();
            state
        }

        /// 終端の状態（`Complete` か `Lost`）で `callback` をちょうど 1 回呼ぶ。`Pending` は渡さない。待たず、ブロックしない。
        ///
        /// 別 device の印・世代違いの印・表に無い番号・回収済みの番号は、登録の呼び出しの中でその場で呼ぶ。
        /// 未完了なら登録して戻り、`submit`・`submit_many`・`poll`・`poll_ex`・`wait_blocking`・`allocate` の
        /// どれかの中で終端になったと分かった時に、その呼び出しを行ったスレッドで呼ぶ（待ちのスレッドも完了ハンドラも使わない）。
        /// 登録は回収せず、ほかの登録済みの callback も呼ばない。callback は錠の外で呼ぶ。
        ///
        /// # 注意
        ///
        /// 上の駆動する呼び出し以外（`upload`・`download_*`・`free` など）では callback を呼ばない。
        /// 呼ばれるには駆動する呼び出しを続ける必要がある。駆動しなければ、`MetalDevice` を落とすまで残る
        /// （落とすときに、その列の完了を待ってから 1 回呼ぶ。待ちに上限は無い）。
        ///
        /// callback の panic は、駆動した呼び出し（`submit` 系・`allocate`・`poll`・`wait_blocking`）から伝わる。
        /// そのとき `submit` 系に渡した `CommandEncoder` は閉じられて戻らず、`wait_blocking` は結果を返さずに巻き戻る。
        /// 登録の呼び出しの中でその場で呼んだ callback の panic は、`on_complete` 自身から伝わる。
        /// その場で呼ぶときも錠の外で呼ぶので、表の錠は毒化せず、そのあとも登録・駆動を続けられる。
        /// これは利用者の callback の panic を伝えるもので、この背後実装自身は panic しない。
        /// 残りの callback は panic のあとでも呼び切る。2 個目以降の panic の中身の `Drop` が panic しても、そこで止めずに呼び切る。
        /// `Drop` の中の callback の panic と、その中身の破棄の panic は捨てる。
        fn on_complete(
            &self,
            token: SubmitToken,
            callback: Box<dyn FnOnce(CompletionState) + Send>,
        ) {
            let call = |callback: Callback, state| callback(state);
            if check_token_owner(&token, self.id, self.generation).is_err() {
                call(callback, CompletionState::Lost);
                return;
            }
            register_table(
                &self.submissions,
                token.sequence,
                callback,
                group_state,
                call,
            );
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
                None => wait_result(self.wait_for_completion(token, true)),
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

        /// 投入の GPU 側の開始・終了時刻（秒）。読み取り専用で、回収も `on_complete` の駆動も状態の変更もしない。
        ///
        /// 時刻は、feature `native-blocking-wait` の `wait_blocking(token, None)` が完了を検出した時にだけ device の表へ書かれる
        /// （`poll`・`poll_ex`・`submit` の回収と、期限付きの待ちは書かない）。表の上限は 1,024 件で、
        /// 超えたら通し番号の最も小さいものから捨てる。返す値:
        ///
        /// - `Available`: 表に時刻がある。開始は列のコマンドバッファの最小、終了は最大で、終了は開始以上。差分は呼び出し側で取る
        /// - `Pending`: 投入済みで未完了
        /// - `Reclaimed`: 完了して回収された、時刻を残さずに完了した、または表から捨てられた投入
        /// - `Unavailable`: `Error` で終わった投入、別 device の印、世代違いの印、払い出していない番号（0 を含む）
        fn gpu_timing(&self, token: SubmitToken) -> GpuTiming {
            if check_token_owner(&token, self.id, self.generation).is_err() {
                return GpuTiming::Unavailable;
            }
            self.lock_submissions()
                .timing_of(token.sequence, group_state)
        }

        // dispatch の記録は TASK-775 のポインタ。この実装は引数を読まず、encoder に何も積まない。
        fn dispatch_ex(
            &self,
            _encoder: &mut CommandEncoder,
            _pipeline: &fandhe_silicon_core::execution::PipelineHandle,
            _bindings: &[fandhe_silicon_core::execution::Binding<'_>],
            _consts: &[fandhe_silicon_core::execution::ConstantBinding<'_>],
            _dispatch_dims: [u32; 3],
            _threads_per_threadgroup: [u32; 3],
            _threadgroup_memory: &[fandhe_silicon_core::execution::ThreadgroupMemory],
        ) -> Result<(), ContractError> {
            Err(ContractError::Unsupported {
                op: "dispatch_ex",
                backend: fandhe_silicon_core::capability::BackendKind::Metal,
            })
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

    /// `values` の `index` 番の function constant に、種別 `kind` の値 `bytes` を設定する。
    ///
    /// `ComputeOps::create_pipeline` が `prepare_constants` の結果を渡して呼ぶ。`U32` と `Bool` の両方を
    /// この 1 つの関数が扱う。`bytes` の長さが `kind` の大きさと違えば、Metal を呼ばずに失敗で返す。
    fn set_constant(
        values: &MTLFunctionConstantValues,
        index: u32,
        kind: ConstantKind,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        let failed = |detail: &str| ContractError::BackendFailure {
            op: "create_pipeline",
            backend: BACKEND,
            detail: detail.into(),
        };
        if bytes.len() != kind.byte_len() {
            return Err(failed("function constant size does not match its type"));
        }
        let data_type = match kind {
            ConstantKind::UInt => MTLDataType::UInt,
            ConstantKind::Bool => MTLDataType::Bool,
        };
        let index = usize::try_from(index).map_err(|_| failed("function constant index"))?;
        // 4 バイトにそろえた局所の値へ写して、その先頭を渡す。`UInt` は 4 バイト、`Bool` は 1 バイトだけが読まれる。
        let mut local = Aligned4([0_u8; 4]);
        local.0[..bytes.len()].copy_from_slice(bytes);
        let value = core::ptr::NonNull::from(&mut local).cast::<core::ffi::c_void>();
        // SAFETY: `setConstantValue_type_atIndex` に渡す `value` は、局所の値 `local` を指す非 null のポインタで、
        // この呼び出しの間だけ生きる。`local` は 4 バイトで 4 バイトにそろえてあり、Metal が読む大きさ
        // （`UInt` は 4 バイト、`Bool` は 1 バイト）とアラインメントが `data_type` と一致する。
        // `bytes` の長さは直前で `kind.byte_len()` と照合した。`index` は呼び手の値を変換せずに渡す。
        // objc2-metal は index が範囲検査されない可能性を挙げており、ここではその扱いを安全の根拠に使わない。
        unsafe {
            values.setConstantValue_type_atIndex(value, data_type, index);
        }
        Ok(())
    }

    /// function constant の値を置く 4 バイトの領域。`UInt` の読み出しに要る 4 バイトのアラインメントをそろえる。
    #[repr(align(4))]
    struct Aligned4([u8; 4]);

    /// NSError の説明文を取り出す。
    fn describe(error: &objc2_foundation::NSError) -> String {
        error.localizedDescription().to_string()
    }

    impl ComputeOps for MetalDevice {
        /// MSL のソースから compute パイプラインを作り、表に登録して返す。
        ///
        /// 受け付けるのは `Msl` と `MslSpecialized`。検査（入力の分類・`layout.slots` と function constant の
        /// 重複）はすべて Metal の API を呼ぶ前に行う。Metal 側の失敗は握りつぶさず `ArtifactRejected` に写す。
        /// `layout` は検査したうえで写しを表に持つだけで、カーネルの引数とは突き合わせない。
        fn create_pipeline(
            &self,
            input: PipelineInput<'_>,
            layout: &BindingLayout,
        ) -> Result<PipelineHandle, ContractError> {
            let (source, entry_point, constants) = classify_pipeline_input(&input)?;
            check_unique_slots(&layout.slots)?;
            let prepared = prepare_constants(constants)?;

            let rejected = |kind: &'static str, what: &str, error: &objc2_foundation::NSError| {
                ContractError::ArtifactRejected {
                    kind,
                    reason: format!("{what}: {}", describe(error)),
                }
            };
            let library = self
                .device
                .newLibraryWithSource_options_error(&NSString::from_str(source), None)
                .map_err(|e| rejected("msl", "MSL compilation failed", &e))?;
            let name = NSString::from_str(entry_point);
            let found = if prepared.is_empty() {
                library.newFunctionWithName(&name)
            } else {
                let values = MTLFunctionConstantValues::new();
                for (index, kind, bytes) in &prepared {
                    set_constant(&values, *index, *kind, bytes)?;
                }
                Some(
                    library
                        .newFunctionWithName_constantValues_error(&name, &values)
                        .map_err(|e| rejected("msl", "MSL function specialization failed", &e))?,
                )
            };
            let function = found.ok_or_else(|| ContractError::ArtifactRejected {
                kind: "msl",
                reason: format!("entry point `{entry_point}` was not found in the MSL source"),
            })?;
            let state = self
                .device
                .newComputePipelineStateWithFunction_error(&function)
                .map_err(|e| rejected("compute-pipeline", "pipeline state creation failed", &e))?;

            let mut pipelines = self.lock_pipelines();
            let id =
                take_next(&pipelines.next_id).ok_or_else(|| ContractError::BackendFailure {
                    op: "create_pipeline",
                    backend: BACKEND,
                    detail: "pipeline id space is exhausted".into(),
                })?;
            pipelines.map.insert(id, (state, layout.clone()));
            Ok(PipelineHandle::__backend_new(id, self.id))
        }
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
            self.drive_callbacks();
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
        use fandhe_silicon_core::execution::{FunctionConstant, FunctionConstantValue};

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

        /// 試験専用。確定を遅らせている間、溜めたコマンドバッファを持つ。落とすと必ず確定する。
        ///
        /// assert が落ちて巻き戻っても、確定していないコマンドバッファは `Drop` の完了待ちで永久に待つので、
        /// device より先に解放する。device の後に宣言し（先に落ちる）、device を落とす試験では明示で解放してから落とす。
        struct HeldCommits<'a>(&'a MetalDevice);

        impl<'a> HeldCommits<'a> {
            fn new(device: &'a MetalDevice) -> Self {
                device.hold_commits();
                Self(device)
            }

            fn release(&self) {
                self.0.release_held_commits();
            }

            /// 確定し、溜めていたコマンドバッファがすべて終わるまで待つ。submission の表は駆動しない
            /// （コマンドバッファ自身の完了待ちなので、callback は呼ばれない）。
            fn release_and_wait_all(&self) {
                for buffer in self.0.release_held_commits() {
                    buffer.waitUntilCompleted();
                }
            }
        }

        impl Drop for HeldCommits<'_> {
            fn drop(&mut self) {
                self.0.release_held_commits();
            }
        }

        type Seen = Arc<Mutex<Vec<CompletionState>>>;

        /// 登録の時点では未完了（確定していない）なので、callback は登録の中で呼ばれない。それを固定して記録先を返す。
        fn register_pending(device: &MetalDevice, token: SubmitToken) -> Seen {
            let seen: Seen = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&seen);
            device.on_complete(
                token,
                Box::new(move |state| sink.lock().expect("lock").push(state)),
            );
            let at_registration = !seen.lock().expect("lock").is_empty();
            println!("called_at_registration={at_registration}");
            assert!(!at_registration, "callback was called at registration");
            seen
        }

        /// 確定前は駆動しても呼ばれず、確定後に `drive` を 10 秒まで繰り返すと `Complete` で 1 回だけ呼ばれる。
        fn assert_driven_once(held: &HeldCommits<'_>, seen: &Seen, mut drive: impl FnMut()) {
            drive();
            assert!(
                seen.lock().expect("lock").is_empty(),
                "called while pending"
            );
            held.release();
            let started = std::time::Instant::now();
            while seen.lock().expect("lock").is_empty() {
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(10),
                    "callback was not called"
                );
                drive();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            drive();
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
        }

        fn submit_empty(device: &MetalDevice) -> SubmitToken {
            device.submit(device.create_encoder(), &[]).expect("submit")
        }

        /// TASK-595。`submit` の中で駆動される経路を決定的に通す。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_submit_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                submit_empty(&device);
            });
        }

        /// TASK-595。`submit_many` の中で駆動される経路を決定的に通す。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_submit_many_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                device
                    .submit_many(vec![device.create_encoder()], &[], &[])
                    .expect("submit_many");
            });
        }

        /// TASK-595。`allocate` の中で駆動される経路を決定的に通す。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_allocate_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                let h = device.allocate(16, BufferUsage::empty()).expect("allocate");
                device.free(h);
            });
        }

        /// TASK-595。`poll` の中で駆動される経路を決定的に通す。未確定の間の `poll` は `Pending`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_poll_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            assert_eq!(device.poll(token), CompletionState::Pending);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                device.poll(token);
            });
        }

        /// TASK-595。`poll_ex` の中で駆動される経路を決定的に通す。未確定の間の `poll_ex` は `Pending`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_poll_ex_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            assert_eq!(device.poll_ex(token), CompletionStateEx::Pending);
            let seen = register_pending(&device, token);
            assert_driven_once(&held, &seen, || {
                device.poll_ex(token);
            });
        }

        /// TASK-595。`wait_blocking(None)` の中で駆動される経路だけを決定的に通す。
        ///
        /// 未確定のコマンドバッファの完了は待てないので、確定してから待つ。確定までは呼ばれていないことを固定する。
        #[cfg(feature = "native-blocking-wait")]
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_wait_blocking_calls_a_pending_registration() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert!(seen.lock().expect("lock").is_empty());
            held.release();
            device.wait_blocking(token, None).expect("wait_blocking");
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
        }

        /// TASK-595。panic が駆動の呼び出し（この試験では `poll`）から 1 回だけ伝わり、残りの callback は 1 回呼ばれ、
        /// device は使い続けられる。この試験は `poll` からの伝わりだけを確かめる。
        ///
        /// 確定を保留して 2 つを登録し、確定して両方の完了を待ってから 1 回だけ駆動する。panic する callback と
        /// その後ろの callback は、毎回同じ駆動に入る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_panic_propagates_from_drive_once_and_the_rest_are_called() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let first = submit_empty(&device);
            let second = submit_empty(&device);
            let at_registration = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                device.on_complete(first, Box::new(|_| panic!("callback failed")));
            }))
            .is_err();
            assert!(!at_registration, "the panic came from registration");
            let seen = register_pending(&device, second);
            // 両方のコマンドバッファが終わってから 1 回だけ駆動し、panic する callback と
            // その後ろの callback が同じ駆動に入る経路を必ず通す。
            held.release_and_wait_all();
            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                device.poll(second);
            }))
            .is_err();
            assert!(
                panicked,
                "the drive did not propagate the panic (the commits were not held, or the callback was not called)"
            );
            assert_eq!(
                *seen.lock().expect("lock"),
                vec![CompletionState::Complete],
                "the callback after the panicking one was not called in the same drive"
            );
            println!("panic_propagated_from=drive");
            // 以後の駆動では panic せず、残りの callback も 1 回のまま。device は使い続けられる。
            for _ in 0..3 {
                assert_eq!(device.poll(second), CompletionState::Complete);
            }
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
            assert_eq!(device.poll(first), CompletionState::Complete);
            submit_empty(&device);
        }

        /// TASK-595。未確定の登録が残ったまま device を落とすと、`Drop` の中で残りの callback が 1 回呼ばれる。
        ///
        /// 未確定のまま落とすと `Drop` の完了待ちが戻らないので、確定してから落とす。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn drive_path_dropping_the_device_calls_a_pending_registration_once() {
            let device = new_device();
            let held = HeldCommits::new(&device);
            let token = submit_empty(&device);
            let seen = register_pending(&device, token);
            assert!(seen.lock().expect("lock").is_empty());
            held.release();
            drop(held);
            drop(device);
            assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
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

        const KERNEL: &str = "#include <metal_stdlib>\n\
            using namespace metal;\n\
            kernel void k(device float* out [[buffer(0)]], uint tid [[thread_position_in_grid]]) {\n\
                out[tid] = 1.0;\n\
            }\n";

        const SPECIALIZED_KERNEL: &str = "#include <metal_stdlib>\n\
            using namespace metal;\n\
            constant uint N [[function_constant(0)]];\n\
            constant bool B [[function_constant(1)]];\n\
            kernel void k(device uint* out [[buffer(0)]]) {\n\
                out[0] = N;\n\
                if (B) { out[1] = 1; }\n\
            }\n";

        fn msl(entry_point: &str) -> PipelineInput<'_> {
            PipelineInput::Msl {
                source: KERNEL,
                entry_point,
            }
        }

        /// TASK-773 のポインタ。MSL から `Ok(PipelineHandle)` が返り、device の識別子が入る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn create_pipeline_returns_a_handle_of_this_device() {
            let device = new_device();
            let h = device
                .create_pipeline(msl("k"), &BindingLayout::new(vec![0]))
                .expect("create_pipeline");
            assert_eq!(h.__backend_device_id(), device.id);
        }

        /// 2 回作ると識別子が違う。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn create_pipeline_twice_gives_different_ids() {
            let device = new_device();
            let layout = BindingLayout::new(vec![0]);
            let a = device.create_pipeline(msl("k"), &layout).expect("a");
            let b = device.create_pipeline(msl("k"), &layout).expect("b");
            assert_ne!(a.__backend_id(), b.__backend_id());
            assert_eq!(device.lock_pipelines().map.len(), 2);
        }

        /// `[[function_constant(N)]]` を持つ MSL は、`U32` と `Bool` を与えて `Ok` になる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn specialized_msl_with_u32_and_bool_constants_is_created() {
            let device = new_device();
            let constants = [
                FunctionConstant {
                    index: 0,
                    value: FunctionConstantValue::U32(u32::MAX),
                },
                FunctionConstant {
                    index: 1,
                    value: FunctionConstantValue::Bool(true),
                },
            ];
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: SPECIALIZED_KERNEL,
                    entry_point: "k",
                    function_constants: &constants,
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(r.is_ok(), "{r:?}");
        }

        /// 文法の誤った MSL は `ArtifactRejected { kind: "msl" }`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn invalid_msl_is_artifact_rejected() {
            let device = new_device();
            let r = device.create_pipeline(
                PipelineInput::Msl {
                    source: "kernel void k( {",
                    entry_point: "k",
                },
                &BindingLayout::new(vec![]),
            );
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                "{r:?}"
            );
        }

        /// 存在しない入口名は `ArtifactRejected { kind: "msl" }`。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn missing_entry_point_is_artifact_rejected() {
            let device = new_device();
            let r = device.create_pipeline(msl("no_such_entry"), &BindingLayout::new(vec![]));
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                "{r:?}"
            );
        }

        /// `function_constant(0)` を `uint` で宣言したソースに、`Bool` を渡すと型違いで拒まれる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn constant_of_the_wrong_type_is_artifact_rejected() {
            let device = new_device();
            let constants = [FunctionConstant {
                index: 0,
                value: FunctionConstantValue::Bool(true),
            }];
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: "#include <metal_stdlib>\n\
                        using namespace metal;\n\
                        constant uint N [[function_constant(0)]];\n\
                        kernel void k(device uint* out [[buffer(0)]]) { out[0] = N; }\n",
                    entry_point: "k",
                    function_constants: &constants,
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { kind: "msl", .. })),
                "{r:?}"
            );
        }

        /// `function_constant` を宣言したソースに、空の定数の `MslSpecialized` を渡すと拒まれる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn specialized_msl_with_no_constants_is_artifact_rejected() {
            let device = new_device();
            let r = device.create_pipeline(
                PipelineInput::MslSpecialized {
                    source: SPECIALIZED_KERNEL,
                    entry_point: "k",
                    function_constants: &[],
                },
                &BindingLayout::new(vec![0]),
            );
            assert!(
                matches!(r, Err(ContractError::ArtifactRejected { .. })),
                "{r:?}"
            );
        }

        /// 重複した slot は Metal を呼ぶ前に `InvalidArgument` で拒まれ、表に何も入らない。
        /// 検査そのものは device なしでも `precheck_tests` で確かめる。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn duplicate_slots_are_invalid_argument_and_leave_the_table_empty() {
            let device = new_device();
            let r = device.create_pipeline(msl("k"), &BindingLayout::new(vec![0, 1, 0]));
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
            assert!(device.lock_pipelines().map.is_empty());
        }

        /// `slots` がカーネルの引数と食い違っても生成は `Ok`。表には `layout` の写しが入る。
        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn mismatched_slots_still_create_and_the_layout_copy_is_stored() {
            let device = new_device();
            let layout = BindingLayout::new(vec![9]);
            let h = device.create_pipeline(msl("k"), &layout).expect("create");
            let pipelines = device.lock_pipelines();
            let entry = pipelines.map.values().next().expect("one entry");
            assert_eq!(entry.1, layout);
            assert_eq!(pipelines.map.len(), 1);
            drop(pipelines);
            assert_eq!(h.__backend_device_id(), device.id);
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
    use std::sync::Arc;
    use std::sync::atomic::AtomicU8;

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

    // ---- on_complete の登録の表（device に触らない）----

    /// 偽の列。0 = 実行中、1 = 完了、2 = 失敗。`wait` で完了にできる。
    type Fake = Arc<AtomicU8>;

    fn fake_state(group: &Fake) -> GroupState {
        match group.load(Ordering::SeqCst) {
            0 => GroupState::Pending,
            2 => GroupState::Failed,
            _ => GroupState::Completed,
        }
    }

    type Seen = Arc<Mutex<Vec<(u64, CompletionState)>>>;

    fn recorder(seen: &Seen, tag: u64) -> Callback {
        let seen = Arc::clone(seen);
        Box::new(move |state| seen.lock().expect("lock").push((tag, state)))
    }

    fn fake_table(states: &[u8]) -> (Submissions<Fake>, Vec<Fake>) {
        let mut subs = Submissions::new();
        let mut groups = Vec::new();
        for state in states {
            let group: Fake = Arc::new(AtomicU8::new(*state));
            subs.pending.insert(subs.next_sequence, Arc::clone(&group));
            subs.next_sequence += 1;
            groups.push(group);
        }
        (subs, groups)
    }

    fn call_all(ready: Vec<(Callback, CompletionState)>) {
        let panic = run_callbacks(ready, |callback, state| callback(state));
        assert!(panic.is_none());
    }

    #[test]
    fn register_calls_back_at_once_only_for_a_terminal_sequence() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[1, 0]);
        // 回収しないので、完了済みの列は表に残ったままでも `Complete` が決まる。
        let ready = subs.register(1, recorder(&seen, 1), fake_state);
        assert!(matches!(ready, Some((_, CompletionState::Complete))));
        assert!(subs.register(2, recorder(&seen, 2), fake_state).is_none());
        // 払い出していない番号は `Lost` でその場。
        let ready = subs.register(9, recorder(&seen, 9), fake_state);
        assert!(matches!(ready, Some((_, CompletionState::Lost))));
        assert_eq!(subs.callbacks.len(), 1);
        assert!(seen.lock().expect("lock").is_empty());
    }

    #[test]
    fn take_terminal_returns_only_finished_registrations_exactly_once() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0, 0, 0]);
        for sequence in 1..=3 {
            assert!(
                subs.register(sequence, recorder(&seen, sequence), fake_state)
                    .is_none()
            );
        }
        assert!(subs.take_terminal(fake_state).is_empty());

        // 先頭が実行中のまま、後ろの 2 つが終わる。回収は止まったままでも後ろの登録は外れる。
        groups[2].store(1, Ordering::SeqCst);
        groups[1].store(2, Ordering::SeqCst);
        call_all(subs.take_terminal(fake_state));
        assert_eq!(subs.watermark, 0);
        assert_eq!(subs.callbacks.len(), 1);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(2, CompletionState::Lost), (3, CompletionState::Complete)]
        );

        // 2 回目の駆動では何も呼ばれず、先頭が終わると 1 回だけ呼ばれる。
        call_all(subs.take_terminal(fake_state));
        groups[0].store(1, Ordering::SeqCst);
        call_all(subs.take_terminal(fake_state));
        call_all(subs.take_terminal(fake_state));
        assert_eq!(seen.lock().expect("lock").len(), 3);
        assert_eq!(
            seen.lock().expect("lock")[2],
            (1, CompletionState::Complete)
        );
        assert!(subs.callbacks.is_empty());
    }

    #[test]
    fn a_registration_for_a_reclaimed_sequence_is_resolved_by_the_watermark() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0]);
        assert!(subs.register(1, recorder(&seen, 1), fake_state).is_none());
        groups[0].store(1, Ordering::SeqCst);
        subs.reclaim(fake_state);
        assert!(subs.pending.is_empty());
        call_all(subs.take_terminal(fake_state));
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(1, CompletionState::Complete)]
        );
    }

    #[test]
    fn take_all_after_waiting_returns_the_actual_state_and_never_pending() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0, 0]);
        for sequence in 1..=2 {
            assert!(
                subs.register(sequence, recorder(&seen, sequence), fake_state)
                    .is_none()
            );
        }
        groups[1].store(2, Ordering::SeqCst);
        subs.wait_registered(|group| {
            if group.load(Ordering::SeqCst) == 0 {
                group.store(1, Ordering::SeqCst);
            }
        });
        call_all(subs.take_all(fake_state));
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(1, CompletionState::Complete), (2, CompletionState::Lost)]
        );
        assert!(subs.callbacks.is_empty());

        // 待たなかった（実行中のままの）列は `Lost` に寄せる。
        let (mut subs, _groups) = fake_table(&[0]);
        assert!(subs.register(1, recorder(&seen, 7), fake_state).is_none());
        call_all(subs.take_all(fake_state));
        assert_eq!(seen.lock().expect("lock")[2], (7, CompletionState::Lost));
    }

    #[test]
    fn a_panicking_callback_does_not_stop_the_rest_and_is_reported_once() {
        let seen: Seen = Arc::default();
        let ready: Vec<(Callback, CompletionState)> = vec![
            (recorder(&seen, 1), CompletionState::Complete),
            (
                Box::new(|_| panic!("callback failed")),
                CompletionState::Complete,
            ),
            (recorder(&seen, 3), CompletionState::Lost),
        ];
        let payload = run_callbacks(ready, |callback, state| callback(state));
        assert!(payload.is_some());
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(1, CompletionState::Complete), (3, CompletionState::Lost)]
        );
    }

    /// 駆動の本体 `drive_table` を通す。callback の中から表の錠を取って登録・駆動しても詰まらない。
    #[test]
    fn callbacks_run_outside_the_lock_so_reentry_does_not_deadlock() {
        type Table = Mutex<Submissions<Fake>>;
        fn drive(table: &Table) {
            drive_table(table, fake_state, |callback: Callback, state| {
                callback(state)
            });
        }

        let (subs, groups) = fake_table(&[0, 0]);
        let table: Arc<Table> = Arc::new(Mutex::new(subs));
        let seen: Seen = Arc::default();
        let inner_seen = Arc::clone(&seen);
        let inner_table = Arc::clone(&table);
        let second_group = Arc::clone(&groups[1]);
        let outer: Callback = Box::new(move |state| {
            inner_seen.lock().expect("lock").push((1, state));
            // 錠が空いている。
            assert!(inner_table.try_lock().is_ok());
            // 再入: 登録と駆動を、この callback の中から行う。
            register_table(
                &inner_table,
                2,
                recorder(&inner_seen, 22),
                fake_state,
                |callback: Callback, state| callback(state),
            );
            second_group.store(1, Ordering::SeqCst);
            drive(&inner_table);
        });
        lock_table(&table).callbacks.push((1, outer));
        drive(&table);
        assert!(seen.lock().expect("lock").is_empty());

        groups[0].store(1, Ordering::SeqCst);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker_table = Arc::clone(&table);
        std::thread::spawn(move || {
            drive(&worker_table);
            done_tx.send(()).expect("send");
        });
        done_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("re-entering from a callback must not deadlock");
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![
                (1, CompletionState::Complete),
                (22, CompletionState::Complete)
            ]
        );
        drive(&table);
        assert_eq!(seen.lock().expect("lock").len(), 2);
    }

    #[test]
    fn drive_table_runs_the_rest_then_propagates_the_first_panic() {
        let seen: Seen = Arc::default();
        let (subs, _groups) = fake_table(&[1, 1, 1]);
        let table = Mutex::new(subs);
        {
            let mut subs = lock_table(&table);
            subs.callbacks.push((1, recorder(&seen, 1)));
            subs.callbacks
                .push((2, Box::new(|_| panic!("callback failed"))));
            subs.callbacks.push((3, recorder(&seen, 3)));
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drive_table(&table, fake_state, |callback: Callback, state| {
                callback(state)
            });
        }));
        assert!(outcome.is_err());
        assert_eq!(seen.lock().expect("lock").len(), 2);
        // 登録は残らず、次の駆動で二重に呼ばれない。
        drive_table(&table, fake_state, |callback: Callback, state| {
            callback(state)
        });
        assert_eq!(seen.lock().expect("lock").len(), 2);
        assert!(lock_table(&table).callbacks.is_empty());
    }

    #[test]
    fn register_table_propagates_the_panic_of_an_at_once_callback_and_keeps_the_lock_usable() {
        let seen: Seen = Arc::default();
        // 番号 1 は完了済み（登録の時点で終端）、番号 2 は実行中。
        let (subs, groups) = fake_table(&[1, 0]);
        let table = Mutex::new(subs);
        let call = |callback: Callback, state| callback(state);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            register_table(
                &table,
                1,
                Box::new(|_| panic!("callback failed")),
                fake_state,
                call,
            );
        }));
        // その場で呼んだ callback の panic は、登録の呼び出しから伝わる。
        assert!(outcome.is_err());
        // 錠は毒化せず放されていて、登録は残らない。
        assert!(table.try_lock().is_ok());
        assert!(lock_table(&table).callbacks.is_empty());
        // そのあとも、登録と駆動が使える。
        register_table(&table, 2, recorder(&seen, 2), fake_state, call);
        assert!(seen.lock().expect("lock").is_empty());
        groups[1].store(1, Ordering::SeqCst);
        drive_table(&table, fake_state, call);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(2, CompletionState::Complete)]
        );
    }

    #[test]
    fn finish_table_waits_calls_the_rest_and_swallows_panics() {
        let seen: Seen = Arc::default();
        let (mut subs, groups) = fake_table(&[0, 0]);
        subs.callbacks
            .push((1, Box::new(|_| panic!("callback failed"))));
        subs.callbacks.push((2, recorder(&seen, 2)));
        finish_table(
            &mut subs,
            |group| group.store(1, Ordering::SeqCst),
            fake_state,
            |callback: Callback, state| callback(state),
        );
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(2, CompletionState::Complete)]
        );
        assert!(subs.callbacks.is_empty());
        assert_eq!(groups[0].load(Ordering::SeqCst), 1);
    }

    /// `Drop` が panic する panic の中身（`panic_any` に渡す）。
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("payload drop failed");
        }
    }

    fn drop_bomb() -> Callback {
        Box::new(|_| std::panic::panic_any(PanicOnDrop))
    }

    #[test]
    fn drive_table_calls_the_rest_when_a_discarded_payload_panics_on_drop() {
        let seen: Seen = Arc::default();
        let (subs, _groups) = fake_table(&[1, 1, 1, 1]);
        let table = Mutex::new(subs);
        {
            let mut subs = lock_table(&table);
            subs.callbacks.push((1, drop_bomb()));
            subs.callbacks.push((2, drop_bomb()));
            subs.callbacks.push((3, recorder(&seen, 3)));
            subs.callbacks.push((4, recorder(&seen, 4)));
        }
        let call = |callback: Callback, state| callback(state);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drive_table(&table, fake_state, call);
        }));
        // 最初の中身だけが伝わり、2 個目の中身の破棄の panic は止まる。後ろの callback も 1 回ずつ呼ばれる。
        let payload = outcome.expect_err("the first payload is propagated");
        assert!(payload.is::<PanicOnDrop>());
        std::mem::forget(payload);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![
                (3, CompletionState::Complete),
                (4, CompletionState::Complete)
            ]
        );
        assert!(lock_table(&table).callbacks.is_empty());
    }

    #[test]
    fn finish_table_does_not_unwind_when_a_discarded_payload_panics_on_drop() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[0, 0, 0]);
        subs.callbacks.push((1, drop_bomb()));
        subs.callbacks.push((2, drop_bomb()));
        subs.callbacks.push((3, recorder(&seen, 3)));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            finish_table(
                &mut subs,
                |group| group.store(1, Ordering::SeqCst),
                fake_state,
                |callback: Callback, state| callback(state),
            );
        }));
        assert!(outcome.is_ok());
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![(3, CompletionState::Complete)]
        );
        assert!(subs.callbacks.is_empty());
    }

    #[test]
    fn a_panicking_classify_does_not_lose_registrations() {
        let seen: Seen = Arc::default();
        let (mut subs, _groups) = fake_table(&[0]);
        assert!(subs.register(1, recorder(&seen, 1), fake_state).is_none());
        let boom = |_: &Fake| -> GroupState { panic!("classify failed") };
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| subs.take_terminal(boom)));
        assert!(outcome.is_err());
        assert_eq!(subs.callbacks.len(), 1);
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

    fn available(start_seconds: f64, end_seconds: f64) -> GpuTiming {
        GpuTiming::Available {
            start_seconds,
            end_seconds,
        }
    }

    #[test]
    fn gpu_timing_covers_the_four_returns() {
        let mut subs = table(&[vec![Done], vec![Running], vec![Failed], vec![Done]]);
        subs.record_timing(1, 1.0, 2.5);
        assert_eq!(subs.timing_of(1, classify), available(1.0, 2.5));
        assert_eq!(subs.timing_of(2, classify), GpuTiming::Pending);
        assert_eq!(subs.timing_of(3, classify), GpuTiming::Unavailable);
        // 完了していて時刻を残していない投入。
        assert_eq!(subs.timing_of(4, classify), GpuTiming::Reclaimed);
    }

    #[test]
    fn gpu_timing_boundaries_follow_the_watermark_and_the_failed_set() {
        let mut subs = table(&[vec![Done], vec![Failed], vec![Done], vec![Running]]);
        subs.reclaim(classify);
        assert_eq!(subs.watermark, 3);
        assert_eq!(subs.timing_of(0, classify), GpuTiming::Unavailable);
        assert_eq!(subs.timing_of(1, classify), GpuTiming::Reclaimed);
        assert_eq!(subs.timing_of(2, classify), GpuTiming::Unavailable);
        assert_eq!(subs.timing_of(3, classify), GpuTiming::Reclaimed);
        assert_eq!(subs.timing_of(4, classify), GpuTiming::Pending);
        assert_eq!(subs.timing_of(5, classify), GpuTiming::Unavailable);
        assert_eq!(subs.timing_of(u64::MAX, classify), GpuTiming::Unavailable);
    }

    #[test]
    fn gpu_timing_keeps_a_recorded_time_after_reclaiming() {
        let mut subs = table(&[vec![Done]]);
        subs.record_timing(1, 3.0, 4.0);
        subs.reclaim(classify);
        assert_eq!(subs.timing_of(1, classify), available(3.0, 4.0));
    }

    #[test]
    fn the_timing_table_drops_the_smallest_sequence_past_1024_entries() {
        let mut subs: Submissions<Vec<BufState>> = Submissions::new();
        for sequence in 1..=1025u64 {
            subs.pending.insert(sequence, vec![Done]);
            subs.next_sequence = sequence + 1;
            subs.record_timing(sequence, sequence as f64, sequence as f64 + 0.5);
            subs.reclaim(classify);
        }
        assert_eq!(subs.timings.len(), 1024);
        assert_eq!(subs.timing_of(1, classify), GpuTiming::Reclaimed);
        assert_eq!(subs.timing_of(2, classify), available(2.0, 2.5));
        assert_eq!(subs.timing_of(1025, classify), available(1025.0, 1025.5));
    }

    #[test]
    fn gpu_timing_does_not_reclaim_or_change_the_table() {
        let subs = table(&[vec![Done], vec![Done]]);
        for sequence in 0..4 {
            let _ = subs.timing_of(sequence, classify);
        }
        assert_eq!(subs.watermark, 0);
        assert_eq!(subs.pending.len(), 2);
        assert!(subs.failed.is_empty());
        assert!(subs.timings.is_empty());
    }

    #[test]
    fn span_takes_the_earliest_start_and_the_latest_end() {
        assert_eq!(
            span_of([(2.0, 3.0), (1.0, 2.5), (2.5, 4.0)]),
            Some((1.0, 4.0))
        );
        assert_eq!(span_of([(1.0, 1.0)]), Some((1.0, 1.0)));
    }

    #[test]
    fn span_rejects_empty_non_finite_and_reversed_times() {
        assert_eq!(span_of([]), None);
        assert_eq!(span_of([(1.0, f64::NAN)]), None);
        assert_eq!(span_of([(f64::INFINITY, f64::INFINITY)]), None);
        assert_eq!(span_of([(1.0, 2.0), (3.0, 2.0)]), None);
    }

    #[test]
    fn span_rejects_zero_times_even_when_other_pairs_are_valid() {
        assert_eq!(span_of([(0.0, 0.0)]), None);
        assert_eq!(span_of([(0.0, 1.0)]), None);
        assert_eq!(span_of([(1.0, 0.0)]), None);
        assert_eq!(span_of([(1.0, 2.0), (0.0, 0.0)]), None);
        assert_eq!(span_of([(-1.0, 2.0)]), None);
    }

    #[test]
    fn complete_wait_records_only_for_the_unbounded_wait_with_a_valid_span() {
        let mut subs = table(&[]);
        assert!(!subs.complete_wait(1, Some((1.0, 2.0)), false));
        assert!(subs.timings.is_empty());
        assert!(!subs.complete_wait(2, None, true));
        assert!(subs.timings.is_empty());
        assert!(subs.complete_wait(3, Some((1.0, 2.0)), true));
        assert_eq!(subs.timing_of(3, classify), available(1.0, 2.0));
        assert_eq!(subs.timings.len(), 1);
    }

    #[test]
    fn reclaiming_never_writes_the_timing_table() {
        let mut subs = table(&[vec![Done], vec![Done]]);
        subs.reclaim(classify);
        assert!(subs.timings.is_empty());
        assert_eq!(subs.timing_of(1, classify), GpuTiming::Reclaimed);
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

    // ---- create_pipeline の純粋な部分（TASK-773 のポインタ。device に触らない） ----

    use fandhe_silicon_core::execution::{MslFloatFunctions, MslMathMode};

    fn constant(index: u32, value: FunctionConstantValue) -> FunctionConstant {
        FunctionConstant { index, value }
    }

    #[test]
    fn classify_accepts_msl_and_msl_specialized() {
        let (source, entry, constants) = classify_pipeline_input(&PipelineInput::Msl {
            source: "src",
            entry_point: "k",
        })
        .unwrap();
        assert_eq!((source, entry, constants.len()), ("src", "k", 0));

        let given = [constant(2, FunctionConstantValue::U32(5))];
        let (source, entry, constants) = classify_pipeline_input(&PipelineInput::MslSpecialized {
            source: "s2",
            entry_point: "e2",
            function_constants: &given,
        })
        .unwrap();
        assert_eq!((source, entry, constants.len()), ("s2", "e2", 1));
        assert_eq!(constants[0].index, 2);
    }

    #[test]
    fn classify_marks_option_variants_and_wgsl_unsupported() {
        let inputs = [
            PipelineInput::MslWithOptions {
                source: "s",
                entry_point: "k",
                math_mode: MslMathMode::Safe,
            },
            PipelineInput::MslSpecializedWithOptions {
                source: "s",
                entry_point: "k",
                function_constants: &[],
                math_mode: MslMathMode::Fast,
                float_functions: MslFloatFunctions::Precise,
            },
            PipelineInput::Wgsl {
                source: "s",
                entry_point: "k",
            },
        ];
        for input in &inputs {
            let r = classify_pipeline_input(input);
            assert!(
                matches!(
                    r,
                    Err(ContractError::Unsupported {
                        op: "create_pipeline",
                        backend: BackendKind::Metal
                    })
                ),
                "{input:?}: {r:?}"
            );
        }
    }

    #[test]
    fn classify_rejects_other_languages_and_names_the_variant() {
        let words = [0_u32; 1];
        let bytes = [0_u8; 1];
        let cases = [
            (
                PipelineInput::SpirV {
                    words: &words,
                    entry_point: "k",
                },
                "SpirV",
            ),
            (
                PipelineInput::CudaC {
                    source: "s",
                    entry_point: "k",
                },
                "CudaC",
            ),
            (
                PipelineInput::Ptx {
                    source: "s",
                    entry_point: "k",
                },
                "Ptx",
            ),
            (
                PipelineInput::Dxil {
                    bytes: &bytes,
                    entry_point: "k",
                },
                "Dxil",
            ),
        ];
        for (input, name) in &cases {
            match classify_pipeline_input(input) {
                Err(ContractError::ArtifactRejected { kind, reason }) => {
                    assert_eq!(kind, "unsupported-input");
                    assert_eq!(
                        reason,
                        format!(
                            "pipeline input {name} is not accepted by the Metal backend; accepted inputs are Msl and MslSpecialized"
                        )
                    );
                }
                other => panic!("{name}: {other:?}"),
            }
        }
    }

    #[test]
    fn slots_empty_and_distinct_pass_duplicates_are_invalid_argument() {
        assert!(check_unique_slots(&[]).is_ok());
        assert!(check_unique_slots(&[0, 1, 4, u32::MAX]).is_ok());
        for slots in [&[3_u32, 3][..], &[0, 1, 2, 1], &[7, 0, 7]] {
            let r = check_unique_slots(slots);
            assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        }
        let Err(ContractError::InvalidArgument(msg)) = check_unique_slots(&[0, 5, 5]) else {
            panic!("expected InvalidArgument");
        };
        assert!(msg.contains('5'), "{msg}");
    }

    #[test]
    fn duplicate_constant_indexes_are_invalid_argument() {
        let ok = [
            constant(0, FunctionConstantValue::U32(1)),
            constant(1, FunctionConstantValue::Bool(true)),
        ];
        assert!(check_unique_constants(&[]).is_ok());
        assert!(check_unique_constants(&ok).is_ok());
        let dup = [
            constant(4, FunctionConstantValue::U32(1)),
            constant(4, FunctionConstantValue::Bool(false)),
        ];
        let r = check_unique_constants(&dup);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
        assert!(matches!(
            prepare_constants(&dup),
            Err(ContractError::InvalidArgument(_))
        ));
    }

    #[test]
    fn constant_wire_maps_u32_and_bool() {
        for v in [0_u32, 1, u32::MAX] {
            let (kind, bytes) = constant_wire(&FunctionConstantValue::U32(v)).unwrap();
            assert_eq!(kind, ConstantKind::UInt);
            assert_eq!(bytes, v.to_ne_bytes());
            assert_eq!(bytes.len(), kind.byte_len());
        }
        for (v, byte) in [(false, 0_u8), (true, 1)] {
            let (kind, bytes) = constant_wire(&FunctionConstantValue::Bool(v)).unwrap();
            assert_eq!(kind, ConstantKind::Bool);
            assert_eq!(bytes, [byte]);
            assert_eq!(bytes.len(), kind.byte_len());
        }
    }

    #[test]
    fn prepare_constants_keeps_order_index_and_kind() {
        let given = [
            constant(9, FunctionConstantValue::Bool(true)),
            constant(0, FunctionConstantValue::U32(0)),
        ];
        let prepared = prepare_constants(&given).unwrap();
        assert_eq!(prepared.len(), 2);
        assert_eq!((prepared[0].0, prepared[0].1), (9, ConstantKind::Bool));
        assert_eq!((prepared[1].0, prepared[1].1), (0, ConstantKind::UInt));
        assert!(prepare_constants(&[]).unwrap().is_empty());
    }
}
