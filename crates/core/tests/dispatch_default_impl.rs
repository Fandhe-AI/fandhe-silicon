//! `CommandQueue::dispatch` の既定の本体と、compute の型の確認（TASK-769 のポインタ。COMP-34）。
//!
//! 呼び出しを記録する queue で、`dispatch` が `dispatch_ex` をちょうど 1 回呼び、
//! 束縛と起動の大きさをそのまま渡し、定数なし・`[1, 1, 1]`・共有メモリなしを補うこと、
//! `dispatch_ex` の結果をそのまま返すことを固定する。機材には触れない。

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
    pipeline: (u64, u64),
    /// (slot, offset, バッファの識別子)。
    bindings: Vec<(u32, u64, u64)>,
    consts_len: usize,
    dims: [u32; 3],
    tpt: [u32; 3],
    tg_mem: Vec<ThreadgroupMemory>,
}

/// `dispatch_ex` の呼び出しを記録し、決めた結果を返す queue。`dispatch` は上書きしない。
struct Recording {
    calls: Mutex<Vec<Call>>,
    result: Result<(), ContractError>,
}

impl Recording {
    fn new(result: Result<(), ContractError>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            result,
        }
    }
}

impl CommandQueue for Recording {
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
        pipeline: &PipelineHandle,
        bindings: &[Binding<'_>],
        consts: &[ConstantBinding<'_>],
        dispatch_dims: [u32; 3],
        threads_per_threadgroup: [u32; 3],
        threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        self.calls.lock().unwrap().push(Call {
            encoder: encoder.__backend_id(),
            pipeline: (pipeline.__backend_id(), pipeline.__backend_device_id()),
            bindings: bindings
                .iter()
                .map(|b| (b.slot, b.offset, b.buffer.__backend_id()))
                .collect(),
            consts_len: consts.len(),
            dims: dispatch_dims,
            tpt: threads_per_threadgroup,
            tg_mem: threadgroup_memory.to_vec(),
        });
        self.result.clone()
    }
}

fn buffer(id: u64) -> BufferHandle {
    BufferHandle::__backend_new(0, id, 1, 64, BufferUsage::STORAGE)
}

#[test]
fn dispatch_calls_dispatch_ex_once_with_defaults() {
    let q = Recording::new(Ok(()));
    let mut enc = CommandEncoder::__backend_new(77, None);
    let pipeline = PipelineHandle::__backend_new(9, 4);
    let (a, b) = (buffer(10), buffer(11));
    let bindings = [Binding::new(0, &a, 0), Binding::new(3, &b, 32)];
    assert_eq!(
        q.dispatch(&mut enc, &pipeline, &bindings, [8, 4, 2]),
        Ok(())
    );
    let calls = q.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        Call {
            encoder: 77,
            pipeline: (9, 4),
            bindings: vec![(0, 0, 10), (3, 32, 11)],
            consts_len: 0,
            dims: [8, 4, 2],
            tpt: [1, 1, 1],
            tg_mem: vec![],
        }
    );
}

#[test]
fn dispatch_passes_empty_bindings_through() {
    let q = Recording::new(Ok(()));
    let mut enc = q.create_encoder();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    assert_eq!(q.dispatch(&mut enc, &pipeline, &[], [1, 1, 1]), Ok(()));
    let calls = q.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].bindings.is_empty());
    assert_eq!(calls[0].dims, [1, 1, 1]);
}

#[test]
fn dispatch_returns_dispatch_ex_error_as_is() {
    let q = Recording::new(Err(ContractError::InvalidArgument("bad".into())));
    let mut enc = q.create_encoder();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    let r = q.dispatch(&mut enc, &pipeline, &[], [1, 1, 1]);
    assert_eq!(r, Err(ContractError::InvalidArgument("bad".into())));
    assert_eq!(q.calls.lock().unwrap().len(), 1);
}

#[test]
fn pipeline_handle_round_trips_backend_values() {
    let p = PipelineHandle::__backend_new(u64::MAX, 0);
    assert_eq!(p.__backend_id(), u64::MAX);
    assert_eq!(p.__backend_device_id(), 0);
}

#[test]
fn binding_types_keep_the_given_values() {
    let buf = buffer(5);
    let b = Binding::new(7, &buf, 128);
    assert_eq!((b.slot, b.offset), (7, 128));
    assert!(std::ptr::eq(b.buffer, &buf));
    let c = ConstantBinding::new(2, &[9, 8]);
    assert_eq!((c.slot, c.bytes), (2, &[9u8, 8][..]));
    assert_eq!(
        ThreadgroupMemory::new(1, 512),
        ThreadgroupMemory::new(1, 512)
    );
    assert_ne!(
        ThreadgroupMemory::new(1, 512),
        ThreadgroupMemory::new(2, 512)
    );
    assert_ne!(
        ThreadgroupMemory::new(1, 512),
        ThreadgroupMemory::new(1, 256)
    );
}
