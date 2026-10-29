//! `CommandQueue::poll_ex` の既定の本体の確認（TASK-270 のポインタ。ERR-27）。
//!
//! `poll_ex` を上書きしない試験用の queue で、`poll` の 3 値が対応する `CompletionStateEx` に写り、
//! `RuntimeError`・`Unknown` は既定の本体からは出ないことを固定する。機材には触れない。
//! 必須のメソッドだけの実装がそのままコンパイルできること自体も、この型の定義が確かめる。

use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, ContractError};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, ImportedSemaphoreHandle, SubmitToken,
};

/// `poll` が固定の状態を返すだけの queue。`poll_ex` を含む既定の本体は上書きしない。
struct FixedPoll(CompletionState);

impl CommandQueue for FixedPoll {
    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(1, None)
    }

    fn submit(
        &self,
        _commands: CommandEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(0, 0, 0))
    }

    fn submit_many(
        &self,
        commands: Vec<CommandEncoder>,
        _after: &[SubmitToken],
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        if commands.is_empty() {
            return Err(ContractError::InvalidArgument("empty commands".into()));
        }
        drop(commands);
        Ok(SubmitToken::__backend_new(0, 0, 0))
    }

    fn poll(&self, _token: SubmitToken) -> CompletionState {
        self.0
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        callback(self.0);
    }

    #[cfg(target_arch = "wasm32")]
    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState)>) {
        callback(self.0);
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

fn token() -> SubmitToken {
    SubmitToken::__backend_new(0, 0, 0)
}

#[test]
fn poll_ex_default_maps_pending() {
    let queue = FixedPoll(CompletionState::Pending);
    assert_eq!(queue.poll_ex(token()), CompletionStateEx::Pending);
}

#[test]
fn poll_ex_default_maps_complete() {
    let queue = FixedPoll(CompletionState::Complete);
    assert_eq!(queue.poll_ex(token()), CompletionStateEx::Complete);
}

#[test]
fn poll_ex_default_maps_lost() {
    let queue = FixedPoll(CompletionState::Lost);
    assert_eq!(queue.poll_ex(token()), CompletionStateEx::Lost);
}

#[test]
fn poll_ex_default_never_returns_runtime_error_or_unknown() {
    for state in [
        CompletionState::Pending,
        CompletionState::Complete,
        CompletionState::Lost,
    ] {
        let got = FixedPoll(state).poll_ex(token());
        assert_ne!(got, CompletionStateEx::RuntimeError, "{state:?}");
        assert_ne!(got, CompletionStateEx::Unknown, "{state:?}");
    }
}

#[test]
fn poll_ex_default_works_through_dyn_command_queue() {
    let queue: Box<dyn CommandQueue> = Box::new(FixedPoll(CompletionState::Lost));
    assert_eq!(queue.poll_ex(token()), CompletionStateEx::Lost);
}
