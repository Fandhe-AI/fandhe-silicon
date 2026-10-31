//! `CommandQueue::gpu_timing` の既定の本体の確認（TASK-589 のポインタ）。
//!
//! `gpu_timing` を上書きしない queue は、どの印を渡しても常に `GpuTiming::Unavailable` を返すことを固定する。
//! 機材には触れない。

use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, GpuTiming, ImportedSemaphoreHandle, SubmitToken,
};

/// 必須のメソッドだけを実装した queue。`poll` の結果は完了済み・未完了のどちらにも切り替えられる。
struct Plain(CompletionState);

impl CommandQueue for Plain {
    fn dispatch_ex(
        &self,
        _encoder: &mut fandhe_silicon_core::execution::CommandEncoder,
        _pipeline: &fandhe_silicon_core::execution::PipelineHandle,
        _bindings: &[fandhe_silicon_core::execution::Binding<'_>],
        _consts: &[fandhe_silicon_core::execution::ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[fandhe_silicon_core::execution::ThreadgroupMemory],
    ) -> Result<(), fandhe_silicon_core::error::ContractError> {
        Err(fandhe_silicon_core::error::ContractError::Unsupported {
            op: "dispatch_ex",
            backend: fandhe_silicon_core::capability::BackendKind::Metal,
        })
    }

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

#[test]
fn gpu_timing_default_is_unavailable_for_any_token() {
    let tokens = [
        SubmitToken::__backend_new(0, 0, 0),
        SubmitToken::__backend_new(1, 1, 1),
        SubmitToken::__backend_new(u64::MAX, u64::MAX, u64::MAX),
    ];
    for state in [
        CompletionState::Pending,
        CompletionState::Complete,
        CompletionState::Lost,
    ] {
        let queue = Plain(state);
        for token in tokens {
            assert_eq!(queue.gpu_timing(token), GpuTiming::Unavailable, "{state:?}");
        }
    }
}

#[test]
fn gpu_timing_default_is_unavailable_through_dyn_command_queue() {
    let queue: Box<dyn CommandQueue> = Box::new(Plain(CompletionState::Complete));
    let token = queue.submit(queue.create_encoder(), &[]).unwrap();
    assert_eq!(queue.gpu_timing(token), GpuTiming::Unavailable);
}
