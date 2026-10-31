//! 契約の crate 側で、コールバックの `Send` 要求を `cfg` ごとに型検査で確かめる（BUILD-27・BUILD-28。TASK-240 のポインタ）。
//!
//! 対象は `MemoryOps::download_async` と `CommandQueue::on_complete` の callback。
//! 両 trait は `fandhe-silicon-core` のもので、この crate は再公開しないため core から直接取り込む。
//! 機材に触れる実装は使わず、試験の中に最小の実装 `Stub` を置く。
//! - native: `Send` の関数は通る（この試験）。`Send` でない関数（`Rc` を捕まえる）が通らないことは、
//!   `src/callback_send_doctest.rs` の `compile_fail` の例が確かめる（`cargo test --doc --all-features`）。
//! - wasm32: `Rc` を捕まえる関数が通る（この試験の `cfg(target_arch = "wasm32")` の部分。実行はせず型検査だけ）。
//!   CI では `make wasm-verify` が `cargo check -p fandhe-silicon-contract --tests --target wasm32-unknown-unknown`
//!   で本ファイルを型検査する。

use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, ImportedSemaphoreHandle, SubmitToken,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

/// 読み出しも完了も即時に空の値で 1 回呼ぶだけの口。`MemoryOps` と `CommandQueue` の両方を実装する。
struct Stub;

impl MemoryOps for Stub {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(0, 1, 1, byte_len, usage))
    }

    fn upload(&self, _: &BufferHandle, _: u64, _: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, _: BufferHandle) {}
}

impl CommandQueue for Stub {
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
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn download_async_accepts_a_send_closure() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = Arc::clone(&seen);
        let ops: &dyn MemoryOps = &Stub;
        let h = ops.allocate(4, BufferUsage::STORAGE).unwrap();
        ops.download_async(
            &h,
            0,
            4,
            Box::new(move |r| seen2.lock().unwrap().push(r.is_ok())),
        );
        assert_eq!(*seen.lock().unwrap(), vec![true]);
    }

    #[test]
    fn on_complete_accepts_a_send_closure() {
        let seen = Arc::new(Mutex::new(Vec::<CompletionState>::new()));
        let seen2 = Arc::clone(&seen);
        let queue: &dyn CommandQueue = &Stub;
        queue.on_complete(
            SubmitToken::__backend_new(0, 0, 0),
            Box::new(move |s| seen2.lock().unwrap().push(s)),
        );
        assert_eq!(*seen.lock().unwrap(), vec![CompletionState::Complete]);
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm32 {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// `Rc` を捕まえる（`Send` でない）関数が `download_async` に渡せること。
    #[test]
    fn download_async_accepts_a_closure_that_is_not_send() {
        let seen = Rc::new(RefCell::new(None));
        let seen2 = Rc::clone(&seen);
        let ops: &dyn MemoryOps = &Stub;
        let h = ops.allocate(4, BufferUsage::STORAGE).unwrap();
        ops.download_async(
            &h,
            0,
            4,
            Box::new(move |r| *seen2.borrow_mut() = Some(r.is_ok())),
        );
        assert_eq!(*seen.borrow(), Some(true));
    }

    /// `Rc` を捕まえる関数が `on_complete` に渡せること。
    #[test]
    fn on_complete_accepts_a_closure_that_is_not_send() {
        let seen = Rc::new(RefCell::new(None::<CompletionState>));
        let seen2 = Rc::clone(&seen);
        let queue: &dyn CommandQueue = &Stub;
        queue.on_complete(
            SubmitToken::__backend_new(0, 0, 0),
            Box::new(move |s| *seen2.borrow_mut() = Some(s)),
        );
        assert_eq!(*seen.borrow(), Some(CompletionState::Complete));
    }
}
