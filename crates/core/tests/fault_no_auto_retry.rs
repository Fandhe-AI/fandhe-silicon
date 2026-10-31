//! 完了不明になった投入を、基盤が自動で再試行しないことの確認（TASK-274 のポインタ。ERR-43・ERR-44・ERR-45）。
//!
//! 試験側の queue が、`submit` の中で `FaultInjecting::submit_with` を通して内側の投入を数える。
//! 筋書きは `CompletionUnknown`（完了の確認は `Unknown`、待機は `Timeout`）。投入の後に完了不明を観測しても、
//! 内側の `submit` の回数が 1 のまま（0 回でも 2 回以上でもない）ことを `submit_call_count` で確かめる。
//! 宣言なしの `submit`・`NotIdempotent`・`IdempotentSafe` の 3 つの入力で回数が同じになる。
//! `submit_ex` の既定の本体が `submit` を 1 回呼ぶこと自体は `submit_default_impls.rs`（TASK-589）の持ち分。
//! Metal の実機に被せた確認は `crates/contract/tests/backend_metal_fault.rs`（宣言なしの `submit`）。
//! ここでの宣言なしの `submit` は、スタブの上で同じ形を流すだけで Metal の代わりにはならない。
//! 機材には触れない。

#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, ContractError, RetryPolicy};
use fandhe_silicon_core::execution::{
    Binding, CommandEncoder, CommandQueue, ConstantBinding, ImportedSemaphoreHandle,
    PipelineHandle, SubmitToken, ThreadgroupMemory,
};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

/// 内側が返す印を持つだけの入れ物。`FaultInjecting` の内側として投入の回数を数えさせる。
struct Inner(SubmitToken);

/// `submit` と完了の確認を差し込みの口に通す queue。`submit_ex` は上書きしない（既定の本体のまま）。
struct UnknownCompletionQueue {
    gate: FaultInjecting<Inner>,
}

impl UnknownCompletionQueue {
    fn new() -> Self {
        Self {
            gate: FaultInjecting::new(
                Inner(SubmitToken::__backend_new(1, 1, 7)),
                FaultScript::CompletionUnknown,
            ),
        }
    }
}

impl CommandQueue for UnknownCompletionQueue {
    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(1, None)
    }

    fn submit(
        &self,
        _commands: CommandEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        self.gate.submit_with(|inner| Ok(inner.0))
    }

    fn submit_many(
        &self,
        _commands: Vec<CommandEncoder>,
        _after: &[SubmitToken],
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Err(ContractError::Unsupported {
            op: "submit_many",
            backend: BackendKind::Metal,
        })
    }

    fn poll(&self, _token: SubmitToken) -> CompletionState {
        self.gate.poll_with(|_| CompletionState::Pending)
    }

    fn poll_ex(&self, _token: SubmitToken) -> CompletionStateEx {
        self.gate.poll_ex_with(|_| CompletionStateEx::Complete)
    }

    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        callback(CompletionState::Pending);
    }

    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        _token: SubmitToken,
        _timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        self.gate.wait_blocking_with(|_| Ok(()))
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
            backend: BackendKind::Metal,
        })
    }
}

/// 投入の後に完了不明を観測し、そのあとの確認を重ねても、内側の投入が 1 回のままであることを照合する。
fn observe_unknown_and_check_single_submit(queue: &UnknownCompletionQueue, token: SubmitToken) {
    assert_eq!(queue.gate.submit_call_count(), 1);
    assert_eq!(queue.poll_ex(token), CompletionStateEx::Unknown);
    #[cfg(feature = "native-blocking-wait")]
    assert_eq!(
        queue.wait_blocking(token, None),
        Err(ContractError::Timeout)
    );
    assert_eq!(queue.poll_ex(token), CompletionStateEx::Unknown);
    assert_eq!(queue.gate.submit_call_count(), 1);
}

/// ERR-44: 宣言なしの `submit` は、完了不明を観測しても再投入されない。
#[test]
fn plain_submit_is_not_retried_after_completion_is_unknown() {
    let queue = UnknownCompletionQueue::new();
    let token = queue.submit(queue.create_encoder(), &[]).expect("submit");
    observe_unknown_and_check_single_submit(&queue, token);
}

/// ERR-44: `NotIdempotent` を宣言した `submit_ex` は、完了不明を観測しても再投入されない。
#[test]
fn submit_ex_not_idempotent_is_not_retried_after_completion_is_unknown() {
    let queue = UnknownCompletionQueue::new();
    let token = queue
        .submit_ex(queue.create_encoder(), &[], RetryPolicy::NotIdempotent)
        .expect("submit_ex");
    observe_unknown_and_check_single_submit(&queue, token);
}

/// ERR-45: `IdempotentSafe` を宣言した `submit_ex` でも、完了不明を観測しても再投入されない。
#[test]
fn submit_ex_idempotent_safe_is_not_retried_after_completion_is_unknown() {
    let queue = UnknownCompletionQueue::new();
    let token = queue
        .submit_ex(queue.create_encoder(), &[], RetryPolicy::IdempotentSafe)
        .expect("submit_ex");
    observe_unknown_and_check_single_submit(&queue, token);
}

/// ERR-43: 3 つの入力で内側の投入の回数が同じ（どれも 1 回）で、返る印も同じ。
#[test]
fn submit_count_and_token_are_the_same_for_all_three_inputs() {
    let plain = UnknownCompletionQueue::new();
    let t_plain = plain.submit(plain.create_encoder(), &[]).unwrap();
    let mut counts = vec![plain.gate.submit_call_count()];
    for policy in [RetryPolicy::NotIdempotent, RetryPolicy::IdempotentSafe] {
        let q = UnknownCompletionQueue::new();
        let t = q.submit_ex(q.create_encoder(), &[], policy).unwrap();
        assert_eq!(t, t_plain, "{policy:?}");
        counts.push(q.gate.submit_call_count());
    }
    assert_eq!(counts, vec![1, 1, 1]);
}
