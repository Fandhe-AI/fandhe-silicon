//! CUDA の device で、未完了の投入が 9 を超える間の解放の保留の数と、保留したバッファを回収する呼び出しを
//! 公開の口から照らす試験（TASK-346 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。どの試験も本体を別のスレッドで流し、上限の時間で打ち切る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx};
use fandhe_silicon_core::execution::SubmitToken;
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const LEN: u64 = 64;

/// 門を仕込んでから積む投入の数。
const BURST: u64 = 32;

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

/// `pending_release_count` の値。`None` なら失敗とする。
fn held(memory: &dyn MemoryOps) -> u64 {
    match memory.pending_release_count() {
        Some(n) => n,
        None => panic!("pending_release_count returned None"),
    }
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

/// `buffer` 全体の 0 埋めを記録して投入する。
fn submit_clear(device: &dyn Device, buffer: &BufferHandle) -> SubmitToken {
    let queue = device.queue();
    let mut encoder = queue.create_encoder();
    assert_eq!(
        device.memory().clear_buffer(&mut encoder, buffer, 0, LEN),
        Ok(())
    );
    match queue.submit(encoder, &[]) {
        Ok(t) => t,
        Err(e) => panic!("submit failed: {e:?}"),
    }
}

// MEM-14
/// 門を仕込んでから 32 個のバッファの 0 埋めを 1 つずつ投入して `free` すると、どの呼び出しも門を待たずに成功し、
/// 解放の保留の数は 32 になる。門を開けて全部の印を `poll` で `Complete` まで待ち、もう 1 回 `poll` を呼ぶと 0。
#[test]
#[ignore = "requires a CUDA device"]
fn more_than_nine_unfinished_submissions_are_held_without_waiting() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let queue = d.queue();
        let buffers: Vec<BufferHandle> = (0..BURST).map(|_| allocate(d)).collect();
        assert_eq!(held(memory), 0);
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let start = Instant::now();
        let mut tokens = Vec::new();
        for buffer in buffers {
            tokens.push(submit_clear(d, &buffer));
            memory.free(buffer);
        }
        assert!(
            start.elapsed() < Duration::from_millis(250),
            "a submission or a free waited behind the gate"
        );
        assert_eq!(held(memory), BURST);
        for token in &tokens {
            assert_eq!(queue.poll(copy_of(token)), CompletionState::Pending);
        }
        assert_eq!(held(memory), BURST);
        assert!(
            start.elapsed() < Duration::from_millis(250),
            "the checks behind the gate took too long"
        );
        assert_eq!(hooks(d).open_gate(), Ok(()));
        for token in &tokens {
            poll_until_complete(d, token);
        }
        let last = match tokens.last() {
            Some(t) => copy_of(t),
            None => panic!("no token was returned"),
        };
        assert_eq!(queue.poll(last), CompletionState::Complete);
        assert_eq!(held(memory), 0);
    });
}

// MEM-15
/// 投入が終わった後も、回収する呼び出しを通るまで解放の保留の数は 1 のまま。`submit`・`poll`・`poll_ex`・
/// `wait_blocking`・`allocate` のどれを呼んでも 0 になる。
#[test]
#[ignore = "requires a CUDA device"]
fn held_buffers_are_released_by_each_of_the_five_calls() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let queue = d.queue();
        for call in ["submit", "poll", "poll_ex", "wait_blocking", "allocate"] {
            let buffer = allocate(d);
            let sync = allocate(d);
            assert_eq!(hooks(d).arm_gate(), Ok(()));
            let token = submit_clear(d, &buffer);
            memory.free(buffer);
            assert_eq!(held(memory), 1, "{call}: before opening the gate");
            assert_eq!(hooks(d).open_gate(), Ok(()));
            // 4 バイトの書き込みは stream を同期するが、保留の回収はしない。
            assert_eq!(memory.upload(&sync, 0, &[1u8; 4]), Ok(()), "{call}");
            std::thread::sleep(Duration::from_millis(50));
            assert_eq!(held(memory), 1, "{call}: after the stream finished");
            let extra = match call {
                "submit" => {
                    if let Err(e) = queue.submit(queue.create_encoder(), &[]) {
                        panic!("submit failed: {e:?}");
                    }
                    None
                }
                "poll" => {
                    assert_eq!(queue.poll(copy_of(&token)), CompletionState::Complete);
                    None
                }
                "poll_ex" => {
                    assert_eq!(queue.poll_ex(copy_of(&token)), CompletionStateEx::Complete);
                    None
                }
                "wait_blocking" => {
                    assert_eq!(
                        queue.wait_blocking(copy_of(&token), Some(Duration::from_secs(10))),
                        Ok(())
                    );
                    None
                }
                _ => Some(allocate(d)),
            };
            assert_eq!(held(memory), 0, "{call}: after the call");
            if let Some(h) = extra {
                memory.free(h);
            }
            memory.free(sync);
        }
    });
}
