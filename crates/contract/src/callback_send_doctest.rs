//! コールバックに `Send` でない関数を渡すコードが native で型として拒否されることを rustdoc の doctest で確かめる置き場（`doc(hidden)`）。
//!
//! 対象は `MemoryOps::download_async` と `CommandQueue::on_complete`（BUILD-27・BUILD-28。TASK-240 のポインタ）。
//! 両 trait は `fandhe-silicon-core` のもので、背後実装には依らないため `&dyn` を受ける関数で確かめる。
//! wasm32 では `Send` を要求しないので、本モジュールは native だけに置く。wasm32 側の確かめは `tests/callback_send_requirement.rs`。
//!
//! モジュールごと feature `test-support` の裏に置く（`make verify` の `--all-features` で流れる）。
//! stable の rustdoc は `compile_fail` のエラーコードを照合しないので、`Rc` を `Arc` に替えただけの
//! 通る例を対にして、import とシグネチャの正しさを担保する。

/// `Send` の関数は `download_async` に渡せる（下の `compile_fail` の例の対照）。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
///
/// fn f(ops: &dyn MemoryOps, buffer: &BufferHandle) {
///     let send = std::sync::Arc::new(0);
///     ops.download_async(
///         buffer,
///         0,
///         4,
///         Box::new(move |_: Result<Vec<u8>, ContractError>| drop(send)),
///     );
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct DownloadSend;

/// `Rc` を捕まえる関数は native の `download_async` に渡せない。
///
/// ```compile_fail
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
///
/// fn f(ops: &dyn MemoryOps, buffer: &BufferHandle) {
///     let not_send = std::rc::Rc::new(0);
///     ops.download_async(
///         buffer,
///         0,
///         4,
///         Box::new(move |_: Result<Vec<u8>, ContractError>| drop(not_send)),
///     );
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct DownloadNotSend;

/// `Send` の関数は `on_complete` に渡せる（下の `compile_fail` の例の対照）。
///
/// ```
/// use fandhe_silicon_core::error::CompletionState;
/// use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
///
/// fn f(queue: &dyn CommandQueue, token: SubmitToken) {
///     let send = std::sync::Arc::new(0);
///     queue.on_complete(token, Box::new(move |_: CompletionState| drop(send)));
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct OnCompleteSend;

/// `Rc` を捕まえる関数は native の `on_complete` に渡せない。
///
/// ```compile_fail
/// use fandhe_silicon_core::error::CompletionState;
/// use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
///
/// fn f(queue: &dyn CommandQueue, token: SubmitToken) {
///     let not_send = std::rc::Rc::new(0);
///     queue.on_complete(token, Box::new(move |_: CompletionState| drop(not_send)));
/// }
/// ```
#[allow(dead_code)] // doctest を載せるためだけのアイテムで、実行時には使わない。
pub struct OnCompleteNotSend;
