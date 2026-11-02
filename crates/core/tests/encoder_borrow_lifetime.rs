//! 記録先 `CommandEncoder` の借用の確認（TASK-772 のポインタ。COMP-15）。
//!
//! 記録する queue で、寿命の指定の無い構造体の欄に記録先を置けること、
//! 束縛と定数を渡した呼び出しの後も同じ記録先を使い続けて `submit` へ値で渡せることを固定する。
//! 機材には触れない。

use std::sync::Mutex;

use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    Binding, CommandEncoder, CommandQueue, ConstantBinding, ImportedSemaphoreHandle,
    PipelineHandle, SubmitToken, ThreadgroupMemory,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

/// `dispatch_ex` が受け取った引数の写し。
#[derive(Debug, PartialEq)]
struct Call {
    /// 渡された encoder の識別子。
    encoder: u64,
    /// (slot, offset, バッファの識別子)。
    bindings: Vec<(u32, u64, u64)>,
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
        bindings: &[Binding<'_>],
        consts: &[ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        self.calls.lock().unwrap().push(Call {
            encoder: encoder.__backend_id(),
            bindings: bindings
                .iter()
                .map(|b| (b.slot, b.offset, b.buffer.__backend_id()))
                .collect(),
            consts: consts.iter().map(|c| (c.slot, c.bytes.to_vec())).collect(),
        });
        Ok(())
    }
}

/// 寿命の引数を書かない構造体。記録先を欄に置く。
struct Holder {
    encoder: CommandEncoder,
    other: Vec<u8>,
}

fn pass_along(holder: Holder) -> Holder {
    holder
}

#[test]
fn encoder_sits_in_struct_without_lifetime_and_submits() {
    let queue = Recording::new();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    let holder = Holder {
        encoder: queue.create_encoder(),
        other: vec![1, 2, 3],
    };
    let mut holder = pass_along(holder);

    queue
        .dispatch(&mut holder.encoder, &pipeline, &[], [1, 1, 1])
        .unwrap();
    assert_eq!(holder.other, vec![1, 2, 3]);
    assert_eq!(
        queue.take(),
        vec![Call {
            encoder: 7,
            bindings: vec![],
            consts: vec![],
        }]
    );

    // 欄の記録先を値で取り出して投入できる。
    assert!(queue.submit(holder.encoder, &[]).is_ok());
    assert_eq!(holder.other.len(), 3);
}

#[test]
fn bindings_and_constants_do_not_outlive_the_call() {
    let queue = Recording::new();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    let mut encoder = queue.create_encoder();

    {
        let buffer = BufferHandle::__backend_new(0, 5, 1, 64, BufferUsage::STORAGE);
        let mut bytes = vec![9u8, 8, 7];
        let bindings = [Binding::new(0, &buffer, 16)];
        let consts = [ConstantBinding::new(1, &bytes)];
        queue
            .dispatch_ex(
                &mut encoder,
                &pipeline,
                &bindings,
                &consts,
                [1, 1, 1],
                [1, 1, 1],
                &[],
            )
            .unwrap();
        // 呼び出しの後にバイト列を書き換えても、記録は呼び出しの時点の値のまま。
        bytes[0] = 0;
        assert_eq!(bytes, vec![0, 8, 7]);
    }

    // 借用していた値はすでに無いが、同じ記録先をもう一度使える。
    queue
        .dispatch(&mut encoder, &pipeline, &[], [1, 1, 1])
        .unwrap();
    assert!(queue.submit(encoder, &[]).is_ok());

    assert_eq!(
        queue.take(),
        vec![
            Call {
                encoder: 7,
                bindings: vec![(0, 16, 5)],
                consts: vec![(1, vec![9, 8, 7])],
            },
            Call {
                encoder: 7,
                bindings: vec![],
                consts: vec![],
            },
        ]
    );
}

#[test]
fn encoder_is_reused_by_mut_until_submit() {
    let queue = Recording::new();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    let mut encoder = queue.create_encoder();

    queue
        .dispatch(&mut encoder, &pipeline, &[], [1, 1, 1])
        .unwrap();
    queue
        .dispatch(&mut encoder, &pipeline, &[], [2, 1, 1])
        .unwrap();
    queue
        .dispatch_ex(&mut encoder, &pipeline, &[], &[], [1, 1, 1], [1, 1, 1], &[])
        .unwrap();

    let calls = queue.take();
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().all(|c| c.encoder == 7));
    assert!(queue.submit(encoder, &[]).is_ok());
}
