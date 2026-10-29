//! `CommandQueue::submit_ex`・`submit_exportable` の既定の本体の確認（TASK-589 のポインタ。CMD-12・CMD-20）。
//!
//! 上書きしない queue の `submit` の中で、故障の差し込みの `FaultInjecting::submit_with` を通して内側を呼ぶ。
//! これで「内側の `submit` が呼ばれた回数」を `submit_call_count` で数える。
//! 既定の本体が内側の `submit` をちょうど 1 回だけ呼び、`Ok`・`Err` をそのまま返す（再試行しない）ことを固定する。
//! 再試行の decorator に固有の確認は範囲の外。機材には触れない。

use fandhe_silicon_core::error::{CompletionState, ContractError, RetryPolicy};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, ImportedSemaphoreHandle, SubmitToken,
};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

/// 内側が返す結果を持つだけの入れ物。`FaultInjecting` の内側として投入の回数を数えさせる。
struct Outcome(Result<SubmitToken, ContractError>);

/// `submit` を回数付きで内側へ渡す queue。`submit_ex`・`submit_exportable` は上書きしない。
struct CountingQueue {
    gate: FaultInjecting<Outcome>,
}

impl CountingQueue {
    fn new(result: Result<SubmitToken, ContractError>) -> Self {
        Self {
            gate: FaultInjecting::new(Outcome(result), FaultScript::PassThrough),
        }
    }

    fn inner_submit_calls(&self) -> u64 {
        self.gate.submit_call_count()
    }
}

impl CommandQueue for CountingQueue {
    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(1, None)
    }

    fn submit(
        &self,
        _commands: CommandEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        self.gate.submit_with(|inner| inner.0.clone())
    }

    fn poll(&self, _token: SubmitToken) -> CompletionState {
        CompletionState::Complete
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        callback(CompletionState::Complete);
    }

    #[cfg(target_arch = "wasm32")]
    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState)>) {
        callback(CompletionState::Complete);
    }

    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        _token: SubmitToken,
        _timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        Ok(())
    }
}

const POLICIES: [RetryPolicy; 2] = [RetryPolicy::IdempotentSafe, RetryPolicy::NotIdempotent];

fn ok_token() -> SubmitToken {
    SubmitToken::__backend_new(2, 9, 4)
}

#[test]
fn submit_ex_default_calls_submit_once_and_returns_ok() {
    for policy in POLICIES {
        let queue = CountingQueue::new(Ok(ok_token()));
        let got = queue.submit_ex(queue.create_encoder(), &[], policy);
        assert_eq!(got, Ok(ok_token()), "{policy:?}");
        assert_eq!(queue.inner_submit_calls(), 1, "{policy:?}");
    }
}

#[test]
fn submit_ex_default_calls_submit_once_and_returns_err_without_retry() {
    for policy in POLICIES {
        let queue = CountingQueue::new(Err(ContractError::Timeout));
        let got = queue.submit_ex(queue.create_encoder(), &[], policy);
        assert_eq!(got, Err(ContractError::Timeout), "{policy:?}");
        // 失敗でも 2 回目を呼ばない。
        assert_eq!(queue.inner_submit_calls(), 1, "{policy:?}");
    }
}

#[test]
fn submit_ex_default_passes_wait_semaphores_through() {
    let queue = CountingQueue::new(Ok(ok_token()));
    let wait = [ImportedSemaphoreHandle::__backend_new(5)];
    let got = queue.submit_ex(queue.create_encoder(), &wait, RetryPolicy::IdempotentSafe);
    assert_eq!(got, Ok(ok_token()));
    assert_eq!(queue.inner_submit_calls(), 1);
}

#[test]
fn submit_ex_default_works_through_dyn_command_queue() {
    let queue = CountingQueue::new(Ok(ok_token()));
    let dynamic: &dyn CommandQueue = &queue;
    let got = dynamic.submit_ex(dynamic.create_encoder(), &[], RetryPolicy::NotIdempotent);
    assert_eq!(got, Ok(ok_token()));
    assert_eq!(queue.inner_submit_calls(), 1);
}

#[cfg(feature = "ext-interop")]
#[test]
fn submit_exportable_default_calls_submit_once_and_returns_ok() {
    for exportable in [true, false] {
        let queue = CountingQueue::new(Ok(ok_token()));
        let got = queue.submit_exportable(queue.create_encoder(), &[], exportable);
        assert_eq!(got, Ok(ok_token()), "exportable={exportable}");
        assert_eq!(queue.inner_submit_calls(), 1, "exportable={exportable}");
    }
}

#[cfg(feature = "ext-interop")]
#[test]
fn submit_exportable_default_calls_submit_once_and_returns_err() {
    for exportable in [true, false] {
        let queue = CountingQueue::new(Err(ContractError::Timeout));
        let got = queue.submit_exportable(queue.create_encoder(), &[], exportable);
        assert_eq!(got, Err(ContractError::Timeout), "exportable={exportable}");
        assert_eq!(queue.inner_submit_calls(), 1, "exportable={exportable}");
    }
}
