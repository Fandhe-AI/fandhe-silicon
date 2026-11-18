//! Metal の device で、確保のときの用途が足りないバッファへの `upload`・`download_blocking`・`download_async` が
//! `InvalidArgument` で拒まれることを公開の口越しに確かめる試験（TASK-316 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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

/// `InvalidArgument` の文字列を取り出す。ほかの結果なら panic する。
fn invalid_argument<T: std::fmt::Debug>(r: Result<T, ContractError>) -> String {
    match r {
        Err(ContractError::InvalidArgument(msg)) => msg,
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

/// 用途に `UPLOAD` が無いバッファへの `upload` は拒まれ、内容は変わらない（MEM-109 のポインタ）。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn upload_without_upload_usage_is_rejected_and_leaves_the_contents_unchanged() {
    let device = new_device();
    let memory = device.memory();
    let payload: Vec<u8> = (1..=16).collect();

    let closed = memory
        .allocate(16, BufferUsage::STORAGE | BufferUsage::DOWNLOAD)
        .expect("allocate");
    let msg = invalid_argument(memory.upload(&closed, 0, &payload));
    assert!(msg.contains("upload") && msg.contains("UPLOAD"), "{msg}");
    let back = memory.download_blocking(&closed, 0, 16).expect("download");
    assert_eq!(back, vec![0u8; 16]);

    let open = memory
        .allocate(
            16,
            BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD,
        )
        .expect("allocate");
    memory.upload(&open, 0, &payload).expect("upload");
    let back = memory.download_blocking(&open, 0, 16).expect("download");
    assert_eq!(back, payload);

    memory.free(closed);
    memory.free(open);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_blocking_without_download_usage_is_rejected() {
    let device = new_device();
    let memory = device.memory();
    let h = memory
        .allocate(16, BufferUsage::STORAGE | BufferUsage::UPLOAD)
        .expect("allocate");
    memory.upload(&h, 0, &[7u8; 16]).expect("upload");
    let msg = invalid_argument(memory.download_blocking(&h, 0, 16));
    assert!(
        msg.contains("download_blocking") && msg.contains("DOWNLOAD"),
        "{msg}"
    );
    memory.free(h);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_async_without_download_usage_delivers_one_invalid_argument() {
    let device = new_device();
    let memory = device.memory();
    let h = memory
        .allocate(16, BufferUsage::STORAGE | BufferUsage::UPLOAD)
        .expect("allocate");

    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(Mutex::new(None));
    let (c, r) = (Arc::clone(&calls), Arc::clone(&received));
    memory.download_async(
        &h,
        0,
        16,
        Box::new(move |result| {
            c.fetch_add(1, Ordering::SeqCst);
            *r.lock().unwrap() = Some(result);
        }),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let got = received.lock().unwrap().take().expect("callback ran");
    let msg = invalid_argument(got);
    assert!(
        msg.contains("download_async") && msg.contains("DOWNLOAD"),
        "{msg}"
    );
    memory.free(h);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn an_empty_usage_rejects_upload_and_download_even_for_zero_length() {
    let device = new_device();
    let memory = device.memory();
    let h = memory.allocate(16, BufferUsage::empty()).expect("allocate");
    let msg = invalid_argument(memory.upload(&h, 0, &[]));
    assert!(msg.contains("UPLOAD"), "{msg}");
    let msg = invalid_argument(memory.download_blocking(&h, 0, 0));
    assert!(msg.contains("DOWNLOAD"), "{msg}");
    memory.free(h);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn the_recorded_usage_decides_even_if_the_handle_claims_more() {
    let device = new_device();
    let memory = device.memory();
    let h = memory
        .allocate(16, BufferUsage::STORAGE | BufferUsage::DOWNLOAD)
        .expect("allocate");
    let claims_more = BufferHandle::__backend_new(
        h.__backend_generation(),
        h.__backend_id(),
        h.__backend_device_id(),
        h.size(),
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD,
    );
    let msg = invalid_argument(memory.upload(&claims_more, 0, &[9u8; 16]));
    assert!(msg.contains("UPLOAD"), "{msg}");
    let back = memory.download_blocking(&h, 0, 16).expect("download");
    assert_eq!(back, vec![0u8; 16]);
    memory.free(h);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn usage_is_checked_before_the_range() {
    let device = new_device();
    let memory = device.memory();

    let h = memory
        .allocate(16, BufferUsage::STORAGE | BufferUsage::DOWNLOAD)
        .expect("allocate");
    let msg = invalid_argument(memory.upload(&h, 8, &[1u8; 16]));
    assert!(msg.contains("upload") && msg.contains("UPLOAD"), "{msg}");
    memory.free(h);

    let h = memory
        .allocate(16, BufferUsage::STORAGE | BufferUsage::UPLOAD)
        .expect("allocate");
    let msg = invalid_argument(memory.download_blocking(&h, 17, 0));
    assert!(msg.contains("DOWNLOAD"), "{msg}");
    memory.free(h);
}
