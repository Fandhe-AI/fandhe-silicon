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

/// 門を仕込んでから、解放の保留の数が 0 になるまでの上限。門の自動解除（500 ms）より短くして、
/// 自動解除ではなく `open_gate` で回収が進んだことを示す。
const RELEASE_DEADLINE: Duration = Duration::from_millis(450);

/// 門で止めた投入の後に、保留の数を照らし終えるまでの上限。
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
/// `since` から `RELEASE_DEADLINE` を過ぎたら、数が 0 になった回でも失敗とする。
fn poll_until_released(device: &dyn Device, token: SubmitToken, since: Instant) {
    loop {
        assert_eq!(device.queue().poll(token), CompletionState::Lost);
        let released = device.memory().pending_release_count() == Some(0);
        // 数が 0 になった回も期限を照らし、門の自動解除の後の回収では成功にしない。
        assert!(
            since.elapsed() < RELEASE_DEADLINE,
            "held buffers were not released before the gate would expire"
        );
        if released {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

// MEM-30
/// 模擬のロストの後に、投入に使っていないバッファと、門で止めたコピーの元と先を `free` すると、保留の数は 2 になり、
/// ロストの前の印の `poll` が `Lost` を返しても 2 のまま。門を開けてから `poll` を繰り返すと数は 0 になる。
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
        // 門の自動解除より前に確認が収まったことを示すため、門を仕込む前から計る。
        let submitted = Instant::now();
        let token = submit_gated_copy(d, &src, &dst);
        assert_eq!(
            hooks(d).simulate_device_lost(DeviceLostReason::Unknown),
            Ok(())
        );
        memory.free(unused);
        assert_eq!(memory.pending_release_count(), Some(0));
        memory.free(src);
        assert_eq!(memory.pending_release_count(), Some(1));
        memory.free(dst);
        assert_eq!(memory.pending_release_count(), Some(2));
        assert!(
            submitted.elapsed() < WINDOW,
            "the loss and the frees took too long"
        );
        assert_eq!(d.queue().poll(token), CompletionState::Lost);
        assert_eq!(memory.pending_release_count(), Some(2));
        assert!(
            submitted.elapsed() < WINDOW,
            "the reads after the loss took too long"
        );
        assert_eq!(hooks(d).open_gate(), Ok(()));
        poll_until_released(d, token, submitted);
        assert_eq!(memory.pending_release_count(), Some(0));
    });
}
