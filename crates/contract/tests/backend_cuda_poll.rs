//! CUDA の device で `poll`・`poll_ex` を公開の口から照らす試験（TASK-605 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。どの試験も本体を別のスレッドで流し、上限の時間で打ち切る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, DeviceLostReason};
use fandhe_silicon_core::execution::SubmitToken;
use fandhe_silicon_core::fault::FaultHooks;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 試験 1 本の上限。
const LIMIT: Duration = Duration::from_secs(30);

/// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
fn within(body: impl FnOnce() + Send + 'static) {
    let (sender, receiver) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        body();
        let _ = sender.send(());
    });
    match receiver.recv_timeout(LIMIT) {
        Ok(()) => {
            let _ = handle.join();
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
            panic!("the test body ended without reporting");
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("the test body did not end within {LIMIT:?}")
        }
    }
}

fn new_device() -> Box<dyn Device> {
    match CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

fn hooks(device: &dyn Device) -> &dyn FaultHooks {
    match device.fault_hooks() {
        Some(h) => h,
        None => panic!("the CUDA device has no fault hooks"),
    }
}

/// 印の写しを返す。
fn copy_of(token: &SubmitToken) -> SubmitToken {
    SubmitToken::__backend_new(
        token.generation,
        token.sequence,
        token.__backend_device_id(),
    )
}

/// 番号だけ替えた印を返す。
fn with_sequence(token: &SubmitToken, sequence: u64) -> SubmitToken {
    SubmitToken::__backend_new(token.generation, sequence, token.__backend_device_id())
}

/// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
fn poll_until_complete(device: &dyn Device, token: &SubmitToken) {
    let start = Instant::now();
    loop {
        match device.queue().poll(copy_of(token)) {
            CompletionState::Complete => return,
            CompletionState::Pending => {}
            other => panic!("unexpected state: {other:?}"),
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "submission did not complete in time"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// `poll_ex` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
fn poll_ex_until_complete(device: &dyn Device, token: &SubmitToken) {
    let start = Instant::now();
    loop {
        match device.queue().poll_ex(copy_of(token)) {
            CompletionStateEx::Complete => return,
            CompletionStateEx::Pending => {}
            other => panic!("unexpected state: {other:?}"),
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "submission did not complete in time"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn submit_empty(device: &dyn Device) -> SubmitToken {
    let queue = device.queue();
    match queue.submit(queue.create_encoder(), &[]) {
        Ok(t) => t,
        Err(e) => panic!("submit failed: {e:?}"),
    }
}

// CMD-22
/// 門で止めた投入は、`poll` が `Pending`、`poll_ex` が `Pending` で、どちらも門を待たずに戻る。門を開けると完了する。
#[test]
#[ignore = "requires a CUDA device"]
fn a_gated_submission_polls_pending_without_blocking() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let token = submit_empty(d);
        let start = Instant::now();
        assert_eq!(queue.poll(copy_of(&token)), CompletionState::Pending);
        assert_eq!(queue.poll_ex(copy_of(&token)), CompletionStateEx::Pending);
        assert!(
            start.elapsed() < Duration::from_millis(250),
            "a poll waited behind the gate"
        );
        assert_eq!(hooks(d).open_gate(), Ok(()));
        poll_until_complete(d, &token);
    });
}

// CMD-23
/// 完了した投入は、`poll` で `Complete` を見た後の `poll_ex` も、`poll_ex` で `Complete` を見た後の `poll` も `Complete`。
#[test]
#[ignore = "requires a CUDA device"]
fn a_finished_submission_polls_complete_through_both_calls() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        let first = submit_empty(d);
        poll_until_complete(d, &first);
        assert_eq!(queue.poll_ex(copy_of(&first)), CompletionStateEx::Complete);
        let second = submit_empty(d);
        poll_ex_until_complete(d, &second);
        assert_eq!(queue.poll(copy_of(&second)), CompletionState::Complete);
    });
}

// CMD-24
/// 回収した投入は、後ろの投入が門で止まっている間も、門を開けて完了した後も `Complete`。
#[test]
#[ignore = "requires a CUDA device"]
fn a_reclaimed_token_polls_complete_while_a_later_one_is_gated() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        let first = submit_empty(d);
        poll_until_complete(d, &first);
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let second = submit_empty(d);
        assert_eq!(queue.poll(copy_of(&second)), CompletionState::Pending);
        assert_eq!(queue.poll(copy_of(&first)), CompletionState::Complete);
        assert_eq!(queue.poll_ex(copy_of(&first)), CompletionStateEx::Complete);
        assert_eq!(hooks(d).open_gate(), Ok(()));
        poll_until_complete(d, &second);
        assert_eq!(queue.poll(copy_of(&first)), CompletionState::Complete);
        assert_eq!(queue.poll_ex(copy_of(&first)), CompletionStateEx::Complete);
    });
}

/// 模擬のロストの前に `Complete` と `Pending` だった印は、ロストの後は `poll`・`poll_ex` とも `Lost`。門を開けた後も `Lost`。
#[test]
#[ignore = "requires a CUDA device"]
fn tokens_from_before_a_simulated_loss_poll_lost() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        let finished = submit_empty(d);
        poll_until_complete(d, &finished);
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let gated = submit_empty(d);
        assert_eq!(queue.poll(copy_of(&gated)), CompletionState::Pending);
        assert_eq!(queue.poll_ex(copy_of(&gated)), CompletionStateEx::Pending);
        assert_eq!(queue.poll(copy_of(&finished)), CompletionState::Complete);
        assert_eq!(
            queue.poll_ex(copy_of(&finished)),
            CompletionStateEx::Complete
        );
        assert_eq!(
            hooks(d).simulate_device_lost(DeviceLostReason::Unknown),
            Ok(())
        );
        for token in [&finished, &gated] {
            assert_eq!(queue.poll(copy_of(token)), CompletionState::Lost);
            assert_eq!(queue.poll_ex(copy_of(token)), CompletionStateEx::Lost);
        }
        assert_eq!(hooks(d).open_gate(), Ok(()));
        assert_eq!(queue.poll(copy_of(&gated)), CompletionState::Lost);
        assert_eq!(queue.poll_ex(copy_of(&gated)), CompletionStateEx::Lost);
    });
}

/// 払い出していない番号（0 と最後の番号より大きいもの）は、`poll`・`poll_ex` とも `Lost`。払い出した番号は `Complete` のまま。
#[test]
#[ignore = "requires a CUDA device"]
fn unissued_sequences_poll_lost() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        let token = submit_empty(d);
        poll_until_complete(d, &token);
        for sequence in [0, token.sequence + 1, u64::MAX] {
            assert_eq!(
                queue.poll(with_sequence(&token, sequence)),
                CompletionState::Lost,
                "{sequence}"
            );
            assert_eq!(
                queue.poll_ex(with_sequence(&token, sequence)),
                CompletionStateEx::Lost,
                "{sequence}"
            );
        }
        assert_eq!(queue.poll(copy_of(&token)), CompletionState::Complete);
        assert_eq!(queue.poll_ex(copy_of(&token)), CompletionStateEx::Complete);
    });
}
