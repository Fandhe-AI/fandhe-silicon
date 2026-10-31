//! コールバックの `Send` 要求を `cfg` ごとに型検査で確かめる（BUILD-26・BUILD-27・BUILD-28。TASK-240 のポインタ）。
//!
//! 対象は `MemoryOps::download_async` と `CommandQueue::on_complete` の callback。
//! - native: `Send` の関数は通る（この試験）。`Send` でない関数（`Rc` を捕まえる）が通らないことは、
//!   各メソッドの rustdoc の `compile_fail` の例が確かめる（`memory/ops.rs`・`execution/submit.rs`。`cargo test --doc`）。
//! - wasm32: `Rc` を捕まえる関数が通る（この試験の `cfg(target_arch = "wasm32")` の部分。実行はせず型検査だけ）。
//!   CI では `make wasm-verify` が `cargo check -p fandhe-silicon-core --tests --target wasm32-unknown-unknown`
//!   で本ファイルを型検査する。
//!
//! `set_lost_callback` の同種の確かめは `device_lost_notify.rs`（TASK-138）が持つ。

use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

/// `download_async` を受けて、読み出しの代わりに空のバイト列で 1 回呼ぶだけの口。
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

    /// 呼び出しは実行しない。`Send` の関数が `on_complete` に渡せることの型検査だけを見る。
    #[allow(dead_code)]
    fn on_complete_accepts_a_send_closure(queue: &dyn CommandQueue, token: SubmitToken) {
        let seen = Arc::new(Mutex::new(Vec::<CompletionState>::new()));
        queue.on_complete(token, Box::new(move |s| seen.lock().unwrap().push(s)));
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

    /// 呼び出しは実行しない。`Rc` を捕まえる関数が `on_complete` に渡せることの型検査だけを見る。
    #[allow(dead_code)]
    fn on_complete_accepts_a_closure_that_is_not_send(
        queue: &dyn CommandQueue,
        token: SubmitToken,
    ) {
        let seen = Rc::new(RefCell::new(None::<CompletionState>));
        queue.on_complete(token, Box::new(move |s| *seen.borrow_mut() = Some(s)));
    }
}
