//! CUDA の device で、門で止めたコピーの投入の状態と、そのコピーに使ったバッファの解放の保留を
//! 公開の口から照らす試験（TASK-347 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。どの試験も本体を別のスレッドで流し、上限の時間で打ち切る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionStateEx, ContractError};
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

/// 空の記録先を投入する。投入は回収の点を通る。
fn submit_empty(device: &dyn Device) {
    let queue = device.queue();
    if let Err(e) = queue.submit(queue.create_encoder(), &[]) {
        panic!("submit failed: {e:?}");
    }
}

fn pattern(seed: u8) -> Vec<u8> {
    (0..LEN).map(|i| (i as u8) ^ seed).collect()
}

// MEM-21
/// 門で止めたコピーの元を `free` すると数が 1 になり、期限 100 ms の待ちが `Timeout` を返した後も 1 のままで、
/// 状態は `Pending`。門を開けると `Complete` になり、回収の点を通ると 0 で、コピーの先は元の内容になる。
#[test]
#[ignore = "requires a CUDA device"]
fn a_timeout_keeps_the_freed_source_of_a_gated_copy_held() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let queue = d.queue();
        let src = allocate(d);
        let dst = allocate(d);
        let bytes = pattern(0xA5);
        assert_eq!(memory.upload(&src, 0, &bytes), Ok(()));
        assert_eq!(memory.pending_release_count(), Some(0));
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let token = submit_copy(d, &src, &dst);
        memory.free(src);
        assert_eq!(memory.pending_release_count(), Some(1));
        let start = Instant::now();
        assert_eq!(
            queue.wait_blocking(copy_of(&token), Some(Duration::from_millis(100))),
            Err(ContractError::Timeout)
        );
        assert!(start.elapsed() >= Duration::from_millis(100));
        assert_eq!(memory.pending_release_count(), Some(1));
        assert_eq!(queue.poll_ex(copy_of(&token)), CompletionStateEx::Pending);
        assert_eq!(memory.pending_release_count(), Some(1));
        assert_eq!(hooks(d).open_gate(), Ok(()));
        poll_ex_until_complete(d, &token);
        submit_empty(d);
        assert_eq!(memory.pending_release_count(), Some(0));
        assert_eq!(memory.download_blocking(&dst, 0, LEN), Ok(bytes));
        memory.free(dst);
    });
}

// MEM-22
/// 門で止めたコピーの後ろに積んだ門の無いコピーも、門を開けるまで `Pending` で、2 つのコピーの元の `free` は 2 と数える。
/// 期限 100 ms の待ちの `Timeout` と `poll_ex` の後も 2 のままで、門を開けて 2 つが `Complete` になり、
/// 回収の点を通ると 0 で、2 つのコピーの先はそれぞれの元の内容になる。
#[test]
#[ignore = "requires a CUDA device"]
fn sources_behind_a_gated_copy_stay_held_until_completion_is_observed() {
    within(|| {
        let device = new_device();
        let d = device.as_ref();
        let memory = d.memory();
        let queue = d.queue();
        let first_src = allocate(d);
        let first_dst = allocate(d);
        let second_src = allocate(d);
        let second_dst = allocate(d);
        let first = pattern(0x3C);
        let second = pattern(0xC3);
        assert_eq!(memory.upload(&first_src, 0, &first), Ok(()));
        assert_eq!(memory.upload(&second_src, 0, &second), Ok(()));
        assert_eq!(hooks(d).arm_gate(), Ok(()));
        let gated = submit_copy(d, &first_src, &first_dst);
        let behind = submit_copy(d, &second_src, &second_dst);
        memory.free(first_src);
        memory.free(second_src);
        assert_eq!(memory.pending_release_count(), Some(2));
        assert_eq!(queue.poll_ex(copy_of(&gated)), CompletionStateEx::Pending);
        assert_eq!(queue.poll_ex(copy_of(&behind)), CompletionStateEx::Pending);
        assert_eq!(
            queue.wait_blocking(copy_of(&behind), Some(Duration::from_millis(100))),
            Err(ContractError::Timeout)
        );
        assert_eq!(queue.poll_ex(copy_of(&gated)), CompletionStateEx::Pending);
        assert_eq!(queue.poll_ex(copy_of(&behind)), CompletionStateEx::Pending);
        assert_eq!(memory.pending_release_count(), Some(2));
        assert_eq!(hooks(d).open_gate(), Ok(()));
        poll_ex_until_complete(d, &behind);
        assert_eq!(queue.poll_ex(copy_of(&gated)), CompletionStateEx::Complete);
        submit_empty(d);
        assert_eq!(memory.pending_release_count(), Some(0));
        assert_eq!(memory.download_blocking(&first_dst, 0, LEN), Ok(first));
        assert_eq!(memory.download_blocking(&second_dst, 0, LEN), Ok(second));
        memory.free(first_dst);
        memory.free(second_dst);
    });
}
