//! 定数を dispatch ごとに渡す形の確認（TASK-772 のポインタ。COMP-15）。
//!
//! 記録する queue で、定数が `dispatch_ex` の呼び出しごとの引数だけで渡ること、
//! queue が写した値を記録して確かめる。機材には触れない。

use std::sync::Mutex;

use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    Binding, CommandEncoder, CommandQueue, ConstantBinding, ImportedSemaphoreHandle,
    PipelineHandle, SubmitToken, ThreadgroupMemory,
};

/// `dispatch_ex` が受け取った定数の写し。
#[derive(Debug, PartialEq)]
struct Call {
    /// 渡された encoder の識別子。
    encoder: u64,
    /// (slot, 呼び出しの時点のバイト列の写し)。
    consts: Vec<(u32, Vec<u8>)>,
}

/// `dispatch_ex` の呼び出しを記録する queue。`dispatch` は上書きしない。
struct Recording {
    calls: Mutex<Vec<Call>>,
}

impl Recording {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn take(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

impl CommandQueue for Recording {
    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(7, None)
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
        _commands: Vec<CommandEncoder>,
        _after: &[SubmitToken],
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(0, 0, 0))
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

    fn dispatch_ex(
        &self,
        encoder: &mut CommandEncoder,
        _pipeline: &PipelineHandle,
        _bindings: &[Binding<'_>],
        consts: &[ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        self.calls.lock().unwrap().push(Call {
            encoder: encoder.__backend_id(),
            consts: consts.iter().map(|c| (c.slot, c.bytes.to_vec())).collect(),
        });
        Ok(())
    }
}

#[test]
fn constants_are_not_carried_to_the_next_call() {
    let queue = Recording::new();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    let mut encoder = queue.create_encoder();

    let first = [0u8, 1, 2, 3];
    let second = [4u8, 5];
    let consts = [
        ConstantBinding::new(0, &first),
        ConstantBinding::new(1, &second),
    ];
    queue
        .dispatch_ex(
            &mut encoder,
            &pipeline,
            &[],
            &consts,
            [1, 1, 1],
            [1, 1, 1],
            &[],
        )
        .unwrap();
    queue
        .dispatch_ex(&mut encoder, &pipeline, &[], &[], [1, 1, 1], [1, 1, 1], &[])
        .unwrap();
    queue
        .dispatch(&mut encoder, &pipeline, &[], [1, 1, 1])
        .unwrap();

    let calls = queue.take();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[0].consts,
        vec![(0, vec![0, 1, 2, 3]), (1, vec![4, 5])]
    );
    for call in &calls[1..] {
        assert!(call.consts.is_empty());
    }
    assert!(calls.iter().all(|c| c.encoder == 7));
    assert!(queue.submit(encoder, &[]).is_ok());
}

#[test]
fn constants_are_copied_during_the_call() {
    let queue = Recording::new();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    let mut encoder = queue.create_encoder();

    {
        let mut bytes = vec![9u8, 8, 7];
        let consts = [ConstantBinding::new(2, &bytes)];
        queue
            .dispatch_ex(
                &mut encoder,
                &pipeline,
                &[],
                &consts,
                [1, 1, 1],
                [1, 1, 1],
                &[],
            )
            .unwrap();
        // 呼び出しの後に元を書き換えても、記録は呼び出しの時点の値のまま。
        bytes[0] = 0;
        assert_eq!(bytes, vec![0, 8, 7]);
    }

    let dropped = vec![1u8, 2];
    let consts = [ConstantBinding::new(3, &dropped)];
    queue
        .dispatch_ex(
            &mut encoder,
            &pipeline,
            &[],
            &consts,
            [1, 1, 1],
            [1, 1, 1],
            &[],
        )
        .unwrap();
    drop(dropped);

    assert_eq!(
        queue.take(),
        vec![
            Call {
                encoder: 7,
                consts: vec![(2, vec![9, 8, 7])],
            },
            Call {
                encoder: 7,
                consts: vec![(3, vec![1, 2])],
            },
        ]
    );
    assert!(queue.submit(encoder, &[]).is_ok());
}
