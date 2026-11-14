//! Metal の device を作って落とす・明示に破棄する試験（TASK-189 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。macOS 以外ではこのファイルは 0 件になる。
//! 既定の device の確保の量はプロセスで共有するので、このファイルの試験は錠で 1 つずつ流す。

#![cfg(target_os = "macos")]

use std::sync::Mutex;

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::memory::BufferUsage;
use objc2::rc::autoreleasepool;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

/// このファイルの試験を 1 つずつ流す錠。
static SERIAL: Mutex<()> = Mutex::new(());

/// 1 本の大きさ（4 MiB）と本数。
const BUFFER_BYTES: u64 = 4 << 20;
const BUFFERS: u64 = 4;

fn new_device() -> Box<dyn Device> {
    match MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

/// 既定の device が確保している量（バイト）。
fn allocated() -> u64 {
    let device = MTLCreateSystemDefaultDevice().expect("the default Metal device");
    u64::try_from(device.currentAllocatedSize()).expect("fits in u64")
}

/// バッファを確保し（1 本は `free` して再利用の一覧へ回す）、空の投入を 1 つ積み、確保の量の増分を確かめた device を返す。
fn device_with_allocations(before: u64) -> Box<dyn Device> {
    let device = new_device();
    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    let mut handles = Vec::new();
    for _ in 0..BUFFERS {
        handles.push(
            device
                .memory()
                .allocate(BUFFER_BYTES, usage)
                .expect("allocate"),
        );
    }
    device.memory().free(handles.pop().expect("one handle"));
    let queue = device.queue();
    queue.submit(queue.create_encoder(), &[]).expect("submit");
    let during = allocated();
    assert!(
        during >= before + BUFFERS * BUFFER_BYTES,
        "before={before} during={during}"
    );
    device
}

/// DEV-148。確保した量は、device を落とした後に確保の前の量から増えた分だけ減る。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn dropping_the_device_returns_its_allocations() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let before = allocated();
    autoreleasepool(|_| {
        let device = device_with_allocations(before);
        drop(device);
    });
    let after = allocated();
    assert!(
        after < before + BUFFER_BYTES,
        "before={before} after={after}"
    );
}

/// DEV-151。明示に破棄しても、同じく確保の前の量から増えた分だけ減る。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn destroying_the_device_returns_its_allocations() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let before = allocated();
    autoreleasepool(|_| {
        let device = device_with_allocations(before);
        device.destroy();
    });
    let after = allocated();
    assert!(
        after < before + BUFFER_BYTES,
        "before={before} after={after}"
    );
}
