//! CUDA の device で、模擬のロストの後に、門で止めたコピーに使ったバッファを `free` したときの解放の保留と、
//! 門を開けた後の回収を公開の口から照らす試験（TASK-348 のポインタ）。
//!
//! 試験は device に触るので `#[ignore]` を付ける。本体を別のスレッドで流し、上限の時間で打ち切る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, DeviceLostReason};
use fandhe_silicon_core::execution::SubmitToken;
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 試験 1 本の上限。
const LIMIT: Duration = Duration::from_secs(30);

/// 門を仕込む前から、解放の保留の数を読み終え、門を開けた後に数が 0 になるのを見るまでの上限。
const WINDOW: Duration = Duration::from_millis(250);

/// 確保の長さ。
const LEN: u64 = 64;

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

fn allocate(memory: &dyn MemoryOps) -> BufferHandle {
    match memory.allocate(LEN, usage()) {
        Ok(h) => h,
        Err(e) => panic!("allocate failed: {e:?}"),
    }
}

/// 門を仕込み、`src` から `dst` へのコピーを記録して投入する。
fn submit_gated_copy(device: &dyn Device, src: &BufferHandle, dst: &BufferHandle) -> SubmitToken {
    assert_eq!(hooks(device).arm_gate(), Ok(()));
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

/// `token` の `poll` を 1 ms 間隔で繰り返し、解放の保留の数が 0 になるまで待つ。`poll` はどの回も `Lost`。
/// 回ごとに `poll` と数を読んだ後で `started` からの経過を照らし、数が 0 の回も `WINDOW` を過ぎていれば失敗とする。
fn poll_until_released(device: &dyn Device, token: SubmitToken, started: Instant) {
    loop {
        let state = device.queue().poll(token);
        let count = device.memory().pending_release_count();
        assert!(
            started.elapsed() < WINDOW,
            "held buffers were not released in time"
        );
        assert_eq!(state, CompletionState::Lost);
        if count == Some(0) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

// MEM-30
/// 模擬のロストの後に、投入に使っていないバッファ、門で止めたコピーの元、先の順に `free` すると、保留の数は 0・1・2 と進み、
/// ロストの前の印の `poll` が `Lost` を返しても 2 のまま。門を開けてから `poll` を繰り返すと数は 0 になる。
/// 経過は門を仕込む前から計り、数を読んだ後に、その数より先に照らす。
#[test]
#[ignore = "requires a CUDA device"]
fn buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let src = allocate(memory);
        let dst = allocate(memory);
        let unused = allocate(memory);
        assert_eq!(memory.upload(&src, 0, &[0x5A; LEN as usize]), Ok(()));
        assert_eq!(memory.pending_release_count(), Some(0));
        let started = Instant::now();
        let token = submit_gated_copy(d, &src, &dst);
        assert_eq!(
            hooks(d).simulate_device_lost(DeviceLostReason::Unknown),
            Ok(())
        );
        memory.free(unused);
        let after_unused = memory.pending_release_count();
        memory.free(src);
        let after_src = memory.pending_release_count();
        memory.free(dst);
        let after_dst = memory.pending_release_count();
        assert!(
            started.elapsed() < WINDOW,
            "the loss and the frees took too long"
        );
        assert_eq!(after_unused, Some(0));
        assert_eq!(after_src, Some(1));
        assert_eq!(after_dst, Some(2));
        let state = d.queue().poll(token);
        let after_poll = memory.pending_release_count();
        assert!(
            started.elapsed() < WINDOW,
            "the reads after the loss took too long"
        );
        assert_eq!(state, CompletionState::Lost);
        assert_eq!(after_poll, Some(2));
        assert_eq!(hooks(d).open_gate(), Ok(()));
        poll_until_released(d, token, started);
        assert_eq!(memory.pending_release_count(), Some(0));
    });
}
