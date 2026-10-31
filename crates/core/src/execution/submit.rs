//! 投入まわりの公開の型と `CommandQueue` の trait（TASK-588 のポインタ）。
//!
//! 上（`fandhe-silicon-upper`）や下-2（`fandhe-silicon-exec`）が記録先を作って投入し、
//! 下-1（`fandhe-silicon-contract`）の背後実装が `CommandQueue` を実装して受け取る。
//! ここには型と trait の形だけを置き、チップ固有の型・unsafe は持たない。
//! `dispatch` 系・束縛の検査などは本モジュールの範囲の外。

use super::{Binding, ConstantBinding, ImportedSemaphoreHandle, PipelineHandle, ThreadgroupMemory};
use crate::error::{CompletionState, CompletionStateEx, ContractError, RetryPolicy};

/// 記録先の識別子の源。背後実装をまたいで、プロセス全体で通し番号にする。0 は払い出さない。
static NEXT_ENCODER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// 背後実装向けの口。記録先の識別子を 1 つ払い出す。
///
/// プロセスの中で背後実装・device をまたいで重ならず、1 から増え、0 は返さない。尽きたら `None`（源は動かさない）。
#[doc(hidden)]
pub fn __backend_next_encoder_id() -> Option<u64> {
    use std::sync::atomic::Ordering;
    let mut current = NEXT_ENCODER_ID.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1)?;
        match NEXT_ENCODER_ID.compare_exchange_weak(
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

/// 投入の結果を指す印。`CommandQueue::submit` が返し、`poll`・`on_complete`・`gpu_timing` などに渡す。
///
/// `#[non_exhaustive]` なので、crate の外からは構造体式で作れない。
/// 背後実装は `__backend_new` で device の識別子付きで作る。
///
/// ```compile_fail
/// use fandhe_silicon_core::execution::SubmitToken;
/// let _ = SubmitToken { generation: 0, sequence: 0 };
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmitToken {
    /// 世代。
    pub generation: u64,
    /// 投入の通し番号。
    pub sequence: u64,
    device_id: u64,
}

impl SubmitToken {
    /// 背後実装向けの組み立ての口。契約の利用側は使わない。
    #[doc(hidden)]
    pub fn __backend_new(generation: u64, sequence: u64, device_id: u64) -> Self {
        Self {
            generation,
            sequence,
            device_id,
        }
    }

    /// 背後実装向けの読み出しの口。印を作った device の識別子を返す。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }
}

/// 投入した処理の GPU 側の開始・終了時刻の問合せ結果。`CommandQueue::gpu_timing` が返す。
///
/// 問合せは読み取り専用で、回収を起こさない。時刻を残すかどうか、どの待ち方で残すか、
/// 回収の後も残すかは背後実装による（Metal は `docs/metal-device-request.md`）。
/// 正常に完了し、時刻が残っていない投入（回収済みを含む）は `Reclaimed` になる。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuTiming {
    /// 完了済みで時刻を読めた。秒単位で、基準の時刻は背後実装による。差を取るのは呼び出し側。
    Available {
        /// 開始時刻（秒）。
        start_seconds: f64,
        /// 終了時刻（秒）。
        end_seconds: f64,
    },
    /// まだ完了していない。
    Pending,
    /// 正常に完了したが、時刻が残っていない（回収済みを含む）。
    Reclaimed,
    /// 観測できない。既定の実装はこれを返す。
    Unavailable,
}

/// 背後実装が `CommandEncoder` の破棄時に呼ぶ後始末の hook（1 つの記録先ごとに持つ版）。
///
/// 共通の crate は `CommandEncoder` の `Drop` で高々 1 回だけ `close` を呼ぶ。
/// 実装する背後実装は、投入の内部で記録先を先に片付けていても `close` が無害であるようにする（二重の後始末にならない）。
/// `Send + Sync` を要求するのは `CommandEncoder` の `Send + Sync` を保つため。
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub trait __BackendEncoderCloser: Send + Sync {
    /// 記録先を閉じる。2 回目以降は何もしない。
    fn close(&mut self);
}

/// 背後実装が `CommandEncoder` の破棄時に呼ぶ後始末の hook（device で 1 つを共有する版）。
///
/// `Arc` の複製で各記録先に渡すので、記録先ごとの確保が起きない。共通の crate が呼ぶ回数と
/// 実装側に求める条件は `__BackendEncoderCloser` と同じ。
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub trait __BackendSharedEncoderCloser: Send + Sync {
    /// 識別子 `id` の記録先を閉じる。対象がすでに無ければ何もしない。
    fn close(&self, id: u64);
}

/// `CommandEncoder` が `Drop` で呼ぶ hook の持ち方。
enum EncoderCloser {
    None,
    Boxed(Box<dyn __BackendEncoderCloser>),
    Shared(std::sync::Arc<dyn __BackendSharedEncoderCloser>),
}

/// 処理の記録先。`CommandQueue::create_encoder` が返し、`submit` が値で消費する。
///
/// `Clone`・`Copy` を持たない。`submit` を呼ばずに破棄すると、`Drop` で背後実装の hook の `close` が呼ばれる。
/// フィールドは非公開で、背後実装は `__backend_` で始まる口から組み立てる（CMD-1）。
///
/// ```compile_fail
/// use fandhe_silicon_core::execution::CommandEncoder;
/// fn needs_clone<T: Clone>() {}
/// needs_clone::<CommandEncoder>();
/// ```
///
/// `Send + Sync` を満たす（CMD-9）。満たさない型が同じ境界で拒まれることの陽性対照:
///
/// ```compile_fail
/// fn needs_send_sync<T: Send + Sync>() {}
/// needs_send_sync::<std::rc::Rc<()>>();
/// ```
pub struct CommandEncoder {
    recorded: u32,
    id: u64,
    closer: EncoderCloser,
}

impl CommandEncoder {
    /// 背後実装向けの組み立ての口。`closer` は破棄時に呼ばれる hook（無ければ `None`）。
    #[doc(hidden)]
    pub fn __backend_new(id: u64, closer: Option<Box<dyn __BackendEncoderCloser>>) -> Self {
        Self {
            recorded: 0,
            id,
            closer: closer.map_or(EncoderCloser::None, EncoderCloser::Boxed),
        }
    }

    /// 背後実装向けの組み立ての口。device が持つ共有の hook を `Arc` の複製で渡す。
    #[doc(hidden)]
    pub fn __backend_new_shared(
        id: u64,
        closer: std::sync::Arc<dyn __BackendSharedEncoderCloser>,
    ) -> Self {
        Self {
            recorded: 0,
            id,
            closer: EncoderCloser::Shared(closer),
        }
    }

    /// 背後実装向けの読み出しの口。組み立て時の識別子を返す。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// 背後実装向けの口。記録した件数を 1 つ増やす。
    #[doc(hidden)]
    pub fn __backend_record(&mut self) {
        self.recorded = self.recorded.saturating_add(1);
    }

    /// 背後実装向けの口。`Drop` で呼ぶ hook を外す。呼ぶ側が記録先を先に片付けていること。
    #[doc(hidden)]
    pub fn __backend_disarm_closer(&mut self) {
        self.closer = EncoderCloser::None;
    }
}

impl std::fmt::Debug for CommandEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandEncoder")
            .field("recorded", &self.recorded)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Drop for CommandEncoder {
    fn drop(&mut self) {
        match std::mem::replace(&mut self.closer, EncoderCloser::None) {
            EncoderCloser::None => {}
            EncoderCloser::Boxed(mut closer) => closer.close(),
            EncoderCloser::Shared(closer) => closer.close(self.id),
        }
    }
}

/// 投入の窓口。背後実装が実装し、上や下-2 が `dyn CommandQueue` としても使う。
///
/// `create_encoder` は `Result` を返さず、その時点では何も投入しない（CMD-1）。
/// 既定の本体を持つメソッドは、背後実装が上書きしなければ次のように振る舞う:
/// `poll_ex` は `poll` の 3 値を写し、`submit_ex`・`submit_exportable` は `submit` に委ね、`gpu_timing` は `Unavailable`。
/// `submit_many` は既定の本体を持たず、背後実装が必ず実装する。
pub trait CommandQueue {
    /// 記録先を作る。何も投入しない。
    fn create_encoder(&self) -> CommandEncoder;

    /// 記録先を値で消費して投入し、結果を指す印を返す。`wait` は先に待つ取り込み済みセマフォ。
    ///
    /// `commands` は値で渡るので、投入後に同じ変数をもう一度使うと所有権の移動のエラーになる（CMD-5）:
    ///
    /// ```compile_fail,E0382
    /// use fandhe_silicon_core::execution::CommandQueue;
    /// fn f(queue: &dyn CommandQueue) {
    ///     let encoder = queue.create_encoder();
    ///     let _ = queue.submit(encoder, &[]);
    ///     let _ = queue.submit(encoder, &[]);
    /// }
    /// ```
    fn submit(
        &self,
        commands: CommandEncoder,
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError>;

    /// 複数の記録先を値で消費して 1 回で投入し、結果を指す印を 1 つ返す（CMD-67）。
    /// `after` は先に完了を待つ投入の印、`wait` は `submit` と同じく先に待つ取り込み済みセマフォ。
    fn submit_many(
        &self,
        commands: Vec<CommandEncoder>,
        after: &[SubmitToken],
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError>;

    /// 投入の完了状態を返す。
    fn poll(&self, token: SubmitToken) -> CompletionState;

    /// 完了したら `callback` に状態を渡す。
    ///
    /// native では callback が `Send` でなければならない（BUILD-28 のポインタ）。`Send` でない値を持つ callback は渡せない:
    ///
    /// ```compile_fail
    /// use fandhe_silicon_core::error::CompletionState;
    /// use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
    /// fn f(queue: &dyn CommandQueue, token: SubmitToken) {
    ///     let not_send = std::rc::Rc::new(0);
    ///     queue.on_complete(token, Box::new(move |_: CompletionState| drop(not_send)));
    /// }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>);

    /// `on_complete` の wasm32 版。単一スレッドなので `Send` を要求しない。
    #[cfg(target_arch = "wasm32")]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState)>);

    /// 完了まで呼び出し元のスレッドで待つ。native 向けで、feature `native-blocking-wait` の裏にある。
    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        token: SubmitToken,
        timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError>;

    /// `poll` の詳しい版。既定は `poll` の 3 値を対応する `CompletionStateEx` に写す。
    fn poll_ex(&self, token: SubmitToken) -> CompletionStateEx {
        match self.poll(token) {
            CompletionState::Pending => CompletionStateEx::Pending,
            CompletionState::Complete => CompletionStateEx::Complete,
            CompletionState::Lost => CompletionStateEx::Lost,
        }
    }

    /// 再試行可否の宣言付きの投入。既定は `retry` を見ずに `submit` を 1 回呼ぶ。再試行はしない。
    fn submit_ex(
        &self,
        commands: CommandEncoder,
        wait: &[ImportedSemaphoreHandle],
        _retry: RetryPolicy,
    ) -> Result<SubmitToken, ContractError> {
        self.submit(commands, wait)
    }

    /// セマフォを export する意図の宣言付きの投入。既定は `exportable` を見ずに `submit` を呼ぶ。
    /// `exportable` は宣言であり、この既定の実装は export できるセマフォを作らない。
    /// feature `ext-interop` の裏にある。
    #[cfg(feature = "ext-interop")]
    fn submit_exportable(
        &self,
        commands: CommandEncoder,
        wait: &[ImportedSemaphoreHandle],
        _exportable: bool,
    ) -> Result<SubmitToken, ContractError> {
        self.submit(commands, wait)
    }

    /// 投入の GPU 側の時刻を読む。読み取り専用で回収を起こさない。既定は `Unavailable`。
    fn gpu_timing(&self, _token: SubmitToken) -> GpuTiming {
        GpuTiming::Unavailable
    }

    /// compute の dispatch を記録先へ記録する。記録の中身・検査は背後実装が決める。既定の本体は無く、必ず実装する。
    ///
    /// 既定の `dispatch` はこのメソッドへ委ねる。
    ///
    /// `dispatch_ex` を持たない実装は拒まれる。
    ///
    /// ```compile_fail,E0046
    /// use fandhe_silicon_core::error::{CompletionState, ContractError};
    /// use fandhe_silicon_core::execution::{
    ///     CommandEncoder, CommandQueue, ImportedSemaphoreHandle, SubmitToken,
    /// };
    /// struct Q;
    /// impl CommandQueue for Q {
    ///     fn create_encoder(&self) -> CommandEncoder { CommandEncoder::__backend_new(1, None) }
    ///     fn submit(&self, _c: CommandEncoder, _w: &[ImportedSemaphoreHandle])
    ///         -> Result<SubmitToken, ContractError> { Ok(SubmitToken::__backend_new(0, 0, 0)) }
    ///     fn submit_many(&self, _c: Vec<CommandEncoder>, _a: &[SubmitToken], _w: &[ImportedSemaphoreHandle])
    ///         -> Result<SubmitToken, ContractError> { Ok(SubmitToken::__backend_new(0, 0, 0)) }
    ///     fn poll(&self, _t: SubmitToken) -> CompletionState { CompletionState::Complete }
    ///     fn on_complete(&self, _t: SubmitToken, cb: Box<dyn FnOnce(CompletionState) + Send>) {
    ///         cb(CompletionState::Complete);
    ///     }
    ///     #[cfg(feature = "native-blocking-wait")]
    ///     fn wait_blocking(&self, _t: SubmitToken, _o: Option<std::time::Duration>)
    ///         -> Result<(), ContractError> { Ok(()) }
    /// }
    /// ```
    ///
    /// 陽性対照: `dispatch_ex` を持てば通る。
    ///
    /// ```
    /// use fandhe_silicon_core::error::{CompletionState, ContractError};
    /// use fandhe_silicon_core::execution::{
    ///     Binding, CommandEncoder, CommandQueue, ConstantBinding, ImportedSemaphoreHandle,
    ///     PipelineHandle, SubmitToken, ThreadgroupMemory,
    /// };
    /// struct Q;
    /// impl CommandQueue for Q {
    ///     fn create_encoder(&self) -> CommandEncoder { CommandEncoder::__backend_new(1, None) }
    ///     fn submit(&self, _c: CommandEncoder, _w: &[ImportedSemaphoreHandle])
    ///         -> Result<SubmitToken, ContractError> { Ok(SubmitToken::__backend_new(0, 0, 0)) }
    ///     fn submit_many(&self, _c: Vec<CommandEncoder>, _a: &[SubmitToken], _w: &[ImportedSemaphoreHandle])
    ///         -> Result<SubmitToken, ContractError> { Ok(SubmitToken::__backend_new(0, 0, 0)) }
    ///     fn poll(&self, _t: SubmitToken) -> CompletionState { CompletionState::Complete }
    ///     fn on_complete(&self, _t: SubmitToken, cb: Box<dyn FnOnce(CompletionState) + Send>) {
    ///         cb(CompletionState::Complete);
    ///     }
    ///     #[cfg(feature = "native-blocking-wait")]
    ///     fn wait_blocking(&self, _t: SubmitToken, _o: Option<std::time::Duration>)
    ///         -> Result<(), ContractError> { Ok(()) }
    ///     fn dispatch_ex(
    ///         &self,
    ///         _e: &mut CommandEncoder,
    ///         _p: &PipelineHandle,
    ///         _b: &[Binding<'_>],
    ///         _c: &[ConstantBinding<'_>],
    ///         _d: [u32; 3],
    ///         _t: [u32; 3],
    ///         _m: &[ThreadgroupMemory],
    ///     ) -> Result<(), ContractError> { Ok(()) }
    /// }
    /// ```
    // 引数は dispatch の宣言の並びそのままで、まとめる型を挟まない。
    #[allow(clippy::too_many_arguments)]
    fn dispatch_ex(
        &self,
        encoder: &mut CommandEncoder,
        pipeline: &PipelineHandle,
        bindings: &[Binding<'_>],
        consts: &[ConstantBinding<'_>],
        dispatch_dims: [u32; 3],
        threads_per_threadgroup: [u32; 3],
        threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError>;

    /// 簡易版の dispatch。定数なし・`[1, 1, 1]` のスレッド数・共有メモリなしで `dispatch_ex` を呼び、結果をそのまま返す。
    /// 自分では検査しない。
    fn dispatch(
        &self,
        encoder: &mut CommandEncoder,
        pipeline: &PipelineHandle,
        bindings: &[Binding<'_>],
        dispatch_dims: [u32; 3],
    ) -> Result<(), ContractError> {
        self.dispatch_ex(
            encoder,
            pipeline,
            bindings,
            &[],
            dispatch_dims,
            [1, 1, 1],
            &[],
        )
    }
}
