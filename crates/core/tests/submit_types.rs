//! 投入まわりの型と `CommandQueue` の形を別の crate から固定する公開テスト（TASK-588 のポインタ。CMD-1・CMD-5・CMD-9・CMD-12・CMD-36・CMD-44・CMD-80 のポインタのみ）。
//!
//! 試験用のダミーの queue はこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: 必須のメソッドだけで実装できること、`create_encoder` が `Result` でなく `CommandEncoder` を返すこと、
//! `dyn CommandQueue` で使えること、`CommandEncoder` の `Send + Sync` と `Drop` での hook、
//! `SubmitToken`・`GpuTiming` の derive と公開フィールド、feature ごとのメソッドの出し分け。
//! 外から構造体式で作れないことは `SubmitToken` の doc にある `compile_fail` の例が確かめる。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, ContractError, RetryPolicy};
use fandhe_silicon_core::execution::{
    __BackendEncoderCloser, __BackendSharedEncoderCloser, CommandEncoder, CommandQueue, GpuTiming,
    ImportedSemaphoreHandle, SubmitToken,
};

/// 必須のメソッドだけを実装した queue。既定の本体は上書きしない。
struct DummyQueue;

impl CommandQueue for DummyQueue {
    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(7, None)
    }

    fn submit(
        &self,
        commands: CommandEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(1, commands.__backend_id(), 3))
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

#[test]
fn create_encoder_returns_encoder_not_result() {
    let queue = DummyQueue;
    // 戻り値が `Result` でなく `CommandEncoder` そのものであることを型で固定する。
    let encoder: CommandEncoder = queue.create_encoder();
    assert_eq!(encoder.__backend_id(), 7);
}

#[test]
fn usable_as_dyn_command_queue() {
    let queue: Box<dyn CommandQueue> = Box::new(DummyQueue);
    let token = queue.submit(queue.create_encoder(), &[]).unwrap();
    assert_eq!(token.sequence, 7);
    assert_eq!(token.__backend_device_id(), 3);
    assert_eq!(queue.poll(token), CompletionState::Complete);
}

#[test]
fn default_bodies_are_callable() {
    let queue = DummyQueue;
    let token = SubmitToken::__backend_new(0, 0, 0);
    assert_eq!(queue.poll_ex(token), CompletionStateEx::Complete);
    assert_eq!(queue.gpu_timing(token), GpuTiming::Unavailable);
    let submitted = queue.submit_ex(queue.create_encoder(), &[], RetryPolicy::NotIdempotent);
    assert!(submitted.is_ok());
}

#[cfg(feature = "ext-interop")]
#[test]
fn submit_exportable_exists_with_ext_interop() {
    let queue = DummyQueue;
    let handle = ImportedSemaphoreHandle::__backend_new(5);
    assert_eq!(handle.__backend_id(), 5);
    let submitted = queue.submit_exportable(queue.create_encoder(), &[handle], true);
    assert!(submitted.is_ok());
}

#[cfg(feature = "native-blocking-wait")]
#[test]
fn wait_blocking_exists_with_feature() {
    let queue = DummyQueue;
    let token = SubmitToken::__backend_new(0, 0, 0);
    assert!(queue.wait_blocking(token, None).is_ok());
}

#[test]
fn on_complete_runs_callback() {
    let queue = DummyQueue;
    let token = SubmitToken::__backend_new(0, 0, 0);
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_in_callback = Arc::clone(&seen);
    // native の callback は `Send` を要求するので、`Arc` を move して渡せる。
    #[cfg(not(target_arch = "wasm32"))]
    queue.on_complete(
        token,
        Box::new(move |state| {
            assert_eq!(state, CompletionState::Complete);
            seen_in_callback.fetch_add(1, Ordering::SeqCst);
        }),
    );
    #[cfg(target_arch = "wasm32")]
    queue.on_complete(
        token,
        Box::new(move |_| {
            seen_in_callback.fetch_add(1, Ordering::SeqCst);
        }),
    );
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}

/// `Rc` のように `Send` でない値を持つ callback は native では渡せない（doc の compile_fail が確かめる）。
/// ここでは `Send` を満たす callback の型が受け付けられることだけを固定する。
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn on_complete_callback_type_is_send() {
    fn assert_send<T: Send + ?Sized>() {}
    assert_send::<dyn FnOnce(CompletionState) + Send>();
}

#[test]
fn command_encoder_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CommandEncoder>();
}

/// `Clone` を持たない型には、`.clone()` が型の `Clone` ではなく参照の複製へ解決される曖昧さの回避で調べる。
/// ここでは `CommandEncoder: Clone` が成り立たないことを、トレイトの有無の判定で固定する。
#[test]
fn command_encoder_is_not_clone() {
    trait NotClone {
        const IS_CLONE: bool = false;
    }
    impl<T> NotClone for T {}
    struct Probe<T>(std::marker::PhantomData<T>);
    impl<T: Clone> Probe<T> {
        #[allow(dead_code)]
        const IS_CLONE: bool = true;
    }
    const { assert!(!<Probe<CommandEncoder>>::IS_CLONE) };
    const { assert!(<Probe<u64>>::IS_CLONE) };
}

struct CountingCloser(Arc<AtomicUsize>);

impl __BackendEncoderCloser for CountingCloser {
    fn close(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct SharedCloser(Arc<AtomicUsize>);

impl __BackendSharedEncoderCloser for SharedCloser {
    fn close(&self, id: u64) {
        self.0.fetch_add(id as usize, Ordering::SeqCst);
    }
}

#[test]
fn drop_calls_closer_once() {
    let count = Arc::new(AtomicUsize::new(0));
    let encoder =
        CommandEncoder::__backend_new(1, Some(Box::new(CountingCloser(Arc::clone(&count)))));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    drop(encoder);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn disarm_prevents_close() {
    let count = Arc::new(AtomicUsize::new(0));
    let mut encoder =
        CommandEncoder::__backend_new(1, Some(Box::new(CountingCloser(Arc::clone(&count)))));
    encoder.__backend_disarm_closer();
    drop(encoder);
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn shared_closer_receives_id_on_drop() {
    let count = Arc::new(AtomicUsize::new(0));
    let encoder =
        CommandEncoder::__backend_new_shared(5, Arc::new(SharedCloser(Arc::clone(&count))));
    drop(encoder);
    assert_eq!(count.load(Ordering::SeqCst), 5);
}

#[test]
fn encoder_debug_hides_closer() {
    let mut encoder = CommandEncoder::__backend_new(9, None);
    encoder.__backend_record();
    let text = format!("{encoder:?}");
    assert!(text.contains("CommandEncoder"));
    assert!(text.contains("recorded: 1"));
    assert!(text.contains("id: 9"));
    assert!(text.contains(".."));
}

#[test]
fn submit_token_fields_and_derives() {
    let a = SubmitToken::__backend_new(2, 3, 4);
    let b = a;
    assert_eq!(a, b);
    assert_eq!(
        (a.generation, a.sequence, a.__backend_device_id()),
        (2, 3, 4)
    );
    assert_ne!(a, SubmitToken::__backend_new(2, 3, 5));
    assert!(format!("{a:?}").contains("SubmitToken"));
}

#[test]
fn gpu_timing_variants_and_partial_eq() {
    let t = GpuTiming::Available {
        start_seconds: 1.0,
        end_seconds: 2.5,
    };
    assert_eq!(t, t);
    assert_ne!(t, GpuTiming::Pending);
    // `f64` を持つので `PartialEq` だけで `Eq` は持たない（`Eq` を足すとここは型エラーにならないため、
    // NaN が自分自身と等しくないことで `PartialEq` の意味を確かめる）。
    let nan = GpuTiming::Available {
        start_seconds: f64::NAN,
        end_seconds: 0.0,
    };
    assert_ne!(nan, nan);
    for v in [
        GpuTiming::Pending,
        GpuTiming::Reclaimed,
        GpuTiming::Unavailable,
    ] {
        // `#[non_exhaustive]` の外からは `_` の腕が要る。
        let label = match v {
            GpuTiming::Pending => "pending",
            GpuTiming::Reclaimed => "reclaimed",
            GpuTiming::Unavailable => "unavailable",
            _ => "other",
        };
        assert!(!label.is_empty());
    }
}

#[test]
fn gpu_timing_has_no_eq() {
    trait NotEq {
        const HAS_EQ: bool = false;
    }
    impl<T> NotEq for T {}
    struct Probe<T>(std::marker::PhantomData<T>);
    impl<T: Eq> Probe<T> {
        #[allow(dead_code)]
        const HAS_EQ: bool = true;
    }
    const { assert!(!<Probe<GpuTiming>>::HAS_EQ) };
    const { assert!(<Probe<SubmitToken>>::HAS_EQ) };
}
