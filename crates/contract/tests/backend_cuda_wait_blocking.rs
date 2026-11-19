//! CUDA の device で `wait_blocking` を公開の口から照らす試験（TASK-606 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。どの試験も本体を別のスレッドで流し、上限の時間で打ち切る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError, DeviceLostReason};
use fandhe_silicon_core::execution::SubmitToken;
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const LEN: u64 = 64;

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

fn usage() -> BufferUsage {
    BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

fn allocate(device: &dyn Device) -> BufferHandle {
    match device.memory().allocate(LEN, usage()) {
        Ok(h) => h,
        Err(e) => panic!("allocate failed: {e:?}"),
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

fn submit_empty(device: &dyn Device) -> SubmitToken {
    let queue = device.queue();
    match queue.submit(queue.create_encoder(), &[]) {
        Ok(t) => t,
        Err(e) => panic!("submit failed: {e:?}"),
    }
}

/// `src` から `dst` へのコピーを記録して投入する。
fn submit_copy(device: &dyn Device, src: &BufferHandle, dst: &BufferHandle) -> SubmitToken {
    let queue = device.queue();
    let mut encoder = queue.create_encoder();
    assert_eq!(
        device
            .memory()
            .copy_buffer_to_buffer(&mut encoder, src, 0, dst, 0, LEN),
        Ok(())
    );
    match queue.submit(encoder, &[]) {
        Ok(t) => t,
        Err(e) => panic!("submit failed: {e:?}"),
    }
}

// CMD-43
/// 門で止めたコピーの投入に期限 100 ms で待つと `Timeout`。門を開けると `poll` は `Complete` で、コピーの先は元の内容。
#[test]
#[ignore = "requires a CUDA device"]
fn a_gated_copy_times_out_and_completes_after_the_gate_opens() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let queue = d.queue();
        let src = allocate(d);
        let dst = allocate(d);
        let bytes: Vec<u8> = (0..LEN).map(|i| (i as u8) ^ 0xA5).collect();
        assert_eq!(memory.upload(&src, 0, &bytes), Ok(()));
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let token = submit_copy(d, &src, &dst);
        let start = Instant::now();
        let waited = queue.wait_blocking(copy_of(&token), Some(Duration::from_millis(100)));
        println!("timeout_elapsed_ms={}", start.elapsed().as_millis());
        assert_eq!(waited, Err(ContractError::Timeout));
        assert_eq!(queue.poll(copy_of(&token)), CompletionState::Pending);
        assert_eq!(hooks(d).open_gate(), Ok(()));
        poll_until_complete(d, &token);
        assert_eq!(memory.download_blocking(&dst, 0, LEN), Ok(bytes));
        memory.free(src);
        memory.free(dst);
    });
}

// CMD-40
/// 期限つきの待ちは未完了の投入に期限まで待って `Timeout`、期限 0 は待たずに `Timeout`。完了した投入にはどちらも `Ok(())`。
#[test]
#[ignore = "requires a CUDA device"]
fn a_limited_wait_returns_timeout_at_the_limit() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let token = submit_empty(d);
        let start = Instant::now();
        assert_eq!(
            queue.wait_blocking(copy_of(&token), Some(Duration::ZERO)),
            Err(ContractError::Timeout)
        );
        assert!(start.elapsed() < Duration::from_millis(250));
        let start = Instant::now();
        assert_eq!(
            queue.wait_blocking(copy_of(&token), Some(Duration::from_millis(50))),
            Err(ContractError::Timeout)
        );
        assert!(start.elapsed() >= Duration::from_millis(50));
        assert_eq!(hooks(d).open_gate(), Ok(()));
        assert_eq!(queue.wait_blocking(copy_of(&token), None), Ok(()));
        assert_eq!(
            queue.wait_blocking(copy_of(&token), Some(Duration::ZERO)),
            Ok(())
        );
        assert_eq!(
            queue.wait_blocking(copy_of(&token), Some(Duration::from_millis(50))),
            Ok(())
        );
    });
}

// CMD-35
/// 上限なしの待ちの後、コピーの先は元の内容で、クリアした範囲は 0。
#[test]
#[ignore = "requires a CUDA device"]
fn an_unlimited_wait_returns_after_the_copy_and_the_clear() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let queue = d.queue();
        let src = allocate(d);
        let dst = allocate(d);
        let cleared = allocate(d);
        let bytes: Vec<u8> = (0..LEN).map(|i| (i as u8) ^ 0x5A).collect();
        assert_eq!(memory.upload(&src, 0, &bytes), Ok(()));
        assert_eq!(memory.upload(&cleared, 0, &[0xFF; LEN as usize]), Ok(()));
        let mut encoder = queue.create_encoder();
        assert_eq!(
            memory.copy_buffer_to_buffer(&mut encoder, &src, 0, &dst, 0, LEN),
            Ok(())
        );
        assert_eq!(memory.clear_buffer(&mut encoder, &cleared, 16, 32), Ok(()));
        let token = match queue.submit(encoder, &[]) {
            Ok(t) => t,
            Err(e) => panic!("submit failed: {e:?}"),
        };
        assert_eq!(queue.wait_blocking(copy_of(&token), None), Ok(()));
        assert_eq!(queue.poll(copy_of(&token)), CompletionState::Complete);
        assert_eq!(memory.download_blocking(&dst, 0, LEN), Ok(bytes));
        let mut expected = vec![0xFF; LEN as usize];
        expected[16..48].fill(0);
        assert_eq!(memory.download_blocking(&cleared, 0, LEN), Ok(expected));
        memory.free(src);
        memory.free(dst);
        memory.free(cleared);
    });
}

// MEM-15
/// 門で止めた投入に門を開けずに上限なしで待つと、保留したバッファを回し、登録した callback を呼んでから戻る。
#[test]
#[ignore = "requires a CUDA device"]
fn the_wait_releases_held_buffers_and_calls_registered_callbacks() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let queue = d.queue();
        let src = allocate(d);
        let dst = allocate(d);
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let token = submit_copy(d, &src, &dst);
        memory.free(src);
        assert_eq!(memory.pending_release_count(), Some(1));
        let (sender, receiver) = mpsc::channel();
        queue.on_complete(
            copy_of(&token),
            Box::new(move |state| {
                let _ = sender.send(state);
            }),
        );
        assert_eq!(receiver.try_recv(), Err(mpsc::TryRecvError::Empty));
        assert_eq!(queue.wait_blocking(copy_of(&token), None), Ok(()));
        assert_eq!(memory.pending_release_count(), Some(0));
        assert_eq!(receiver.try_recv(), Ok(CompletionState::Complete));
        assert_eq!(hooks(d).open_gate(), Ok(()));
        memory.free(dst);
    });
}

/// 回収済みの投入は、上限なしでも期限 0 でも `Ok(())`。
#[test]
#[ignore = "requires a CUDA device"]
fn a_reclaimed_submission_returns_ok() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        let first = submit_empty(d);
        assert_eq!(queue.wait_blocking(copy_of(&first), None), Ok(()));
        let second = submit_empty(d);
        assert_eq!(queue.wait_blocking(copy_of(&second), None), Ok(()));
        assert_eq!(queue.wait_blocking(copy_of(&first), None), Ok(()));
        assert_eq!(
            queue.wait_blocking(copy_of(&first), Some(Duration::ZERO)),
            Ok(())
        );
    });
}

// CMD-82
/// 払い出していない番号（0 と最後の番号より大きいもの）は、門で止めた投入があっても待たずに `InvalidArgument`。
#[test]
#[ignore = "requires a CUDA device"]
fn an_unissued_sequence_is_rejected_without_waiting() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let queue = d.queue();
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let token = submit_empty(d);
        for sequence in [0, token.sequence + 1, u64::MAX] {
            for timeout in [None, Some(Duration::ZERO), Some(Duration::from_secs(10))] {
                let start = Instant::now();
                let r = queue.wait_blocking(with_sequence(&token, sequence), timeout);
                assert!(
                    matches!(r, Err(ContractError::InvalidArgument(_))),
                    "{sequence} {timeout:?}: {r:?}"
                );
                assert!(start.elapsed() < Duration::from_millis(250));
            }
        }
        assert_eq!(queue.poll(copy_of(&token)), CompletionState::Pending);
        assert_eq!(hooks(d).open_gate(), Ok(()));
        assert_eq!(queue.wait_blocking(copy_of(&token), None), Ok(()));
    });
}

// CMD-81
/// 別の device の印は `DeviceMismatch`。
#[test]
#[ignore = "requires a CUDA device"]
fn a_token_of_another_device_is_a_mismatch() {
    within(|| {
        let a = new_device();
        let b = new_device();
        let token = submit_empty(a.as_ref());
        for timeout in [None, Some(Duration::ZERO)] {
            assert_eq!(
                b.queue().wait_blocking(copy_of(&token), timeout),
                Err(ContractError::DeviceMismatch)
            );
        }
        assert_eq!(a.queue().wait_blocking(copy_of(&token), None), Ok(()));
    });
}

// CMD-78
/// 模擬のロストの後は、ロストの前の印にも別の device の印にも、今の世代の `DeviceLost`。
#[test]
#[ignore = "requires a CUDA device"]
fn a_lost_device_returns_device_lost_before_the_owner_check() {
    within(|| {
        let a = new_device();
        let b = new_device();
        let token = submit_empty(a.as_ref());
        let foreign = submit_empty(b.as_ref());
        assert_eq!(
            hooks(a.as_ref()).simulate_device_lost(DeviceLostReason::Unknown),
            Ok(())
        );
        let lost = ContractError::DeviceLost {
            generation: a.generation(),
        };
        let queue = a.queue();
        assert_eq!(
            queue.wait_blocking(copy_of(&token), Some(Duration::from_millis(50))),
            Err(lost.clone())
        );
        assert_eq!(
            queue.wait_blocking(copy_of(&foreign), Some(Duration::from_millis(50))),
            Err(lost)
        );
        assert_eq!(b.queue().wait_blocking(copy_of(&foreign), None), Ok(()));
    });
}
