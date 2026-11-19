//! CUDA の device で、門で止めたコピーに使ったバッファの解放の保留の数と、門を仕込んだ後の `free` を
//! 公開の口から照らす試験（TASK-345 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::CompletionState;
use fandhe_silicon_core::execution::SubmitToken;
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use std::time::{Duration, Instant};

const LEN: u64 = 64;

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

// MEM-10
/// 門で止めたコピーの元を `free` すると数が 1 になり、門を開けて回収の点を通ると 0 になり、コピーの先は元の内容になる。
#[test]
#[ignore = "requires a CUDA device"]
fn a_freed_copy_source_is_counted_until_the_gated_copy_completes() {
    let device = new_device();
    let d = device.as_ref();
    let memory = d.memory();
    assert_eq!(memory.pending_release_count(), Some(0));
    let src = allocate(d);
    let dst = allocate(d);
    let unused = allocate(d);
    let bytes: Vec<u8> = (0..LEN).map(|i| (i as u8) ^ 0xA5).collect();
    assert_eq!(memory.upload(&src, 0, &bytes), Ok(()));
    let token = submit_gated_copy(d, &src, &dst);
    memory.free(unused);
    assert_eq!(memory.pending_release_count(), Some(0));
    memory.free(src);
    assert_eq!(memory.pending_release_count(), Some(1));
    assert_eq!(d.queue().poll(copy_of(&token)), CompletionState::Pending);
    assert_eq!(memory.pending_release_count(), Some(1));
    assert_eq!(hooks(d).open_gate(), Ok(()));
    poll_until_complete(d, &token);
    // `poll` は状態を読む前に回収するので、`Complete` を見た後の投入で回収してから数を照らす。
    let queue = d.queue();
    if let Err(e) = queue.submit(queue.create_encoder(), &[]) {
        panic!("submit failed: {e:?}");
    }
    assert_eq!(memory.pending_release_count(), Some(0));
    assert_eq!(memory.download_blocking(&dst, 0, LEN), Ok(bytes));
    memory.free(dst);
    assert_eq!(memory.pending_release_count(), Some(0));
}

// MEM-12
/// 数を読むだけでは回収しない。stream を同期した後も、回収の点を通るまで数は変わらない。
#[test]
#[ignore = "requires a CUDA device"]
fn reading_the_count_does_not_release_held_buffers() {
    let device = new_device();
    let d = device.as_ref();
    let memory = d.memory();
    let src = allocate(d);
    let dst = allocate(d);
    let sync = allocate(d);
    assert_eq!(memory.upload(&src, 0, &[1u8; LEN as usize]), Ok(()));
    let token = submit_gated_copy(d, &src, &dst);
    memory.free(src);
    assert_eq!(hooks(d).open_gate(), Ok(()));
    // 4 バイトの書き込みは stream を同期するが、保留の回収はしない。
    assert_eq!(memory.upload(&sync, 0, &[2u8; 4]), Ok(()));
    assert_eq!(memory.pending_release_count(), Some(1));
    assert_eq!(memory.pending_release_count(), Some(1));
    assert_eq!(d.queue().poll(copy_of(&token)), CompletionState::Complete);
    assert_eq!(memory.pending_release_count(), Some(0));
    memory.free(dst);
    memory.free(sync);
}

/// 門を仕込んだ後の `free` は門を使わない。続く同期は門の上限の半分より早く戻り、次の投入が門で止まる。
#[test]
#[ignore = "requires a CUDA device"]
fn freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission() {
    let device = new_device();
    let d = device.as_ref();
    let memory = d.memory();
    let unused = allocate(d);
    let sync = allocate(d);
    assert_eq!(hooks(d).arm_gate(), Ok(()));
    memory.free(unused);
    let start = Instant::now();
    assert_eq!(memory.upload(&sync, 0, &[3u8; 4]), Ok(()));
    assert!(
        start.elapsed() < Duration::from_millis(250),
        "the upload waited behind a gate"
    );
    let queue = d.queue();
    let token = match queue.submit(queue.create_encoder(), &[]) {
        Ok(t) => t,
        Err(e) => panic!("submit failed: {e:?}"),
    };
    assert_eq!(queue.poll(copy_of(&token)), CompletionState::Pending);
    assert_eq!(hooks(d).open_gate(), Ok(()));
    poll_until_complete(d, &token);
    memory.free(sync);
}
