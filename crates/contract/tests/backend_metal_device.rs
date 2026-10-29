//! Metal の device を実際に作り、診断・能力・メモリ操作を呼ぶ確認（TASK-158 のポインタ。
//! DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265。バッファの照合は MEM-79・MEM-80 のポインタ）。
//!
//! どの試験も Metal の device に触るので `#[ignore]` を付け、専用の実機の回でだけ流す。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, CapabilityQuery, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

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

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes() {
    let device = new_device();
    assert_eq!(device.backend(), BackendKind::Metal);
    assert!(matches!(
        device.ownership(),
        DeviceOwnership::ProductIndependent
    ));
    assert!(device.generation() >= 1);

    let diag = device.diagnostics();
    assert_eq!(diag.backend_name, "metal");
    assert!(!diag.adapter_name.is_empty());
    let expected_name = MTLCreateSystemDefaultDevice()
        .expect("the default Metal device must exist in this session")
        .name()
        .to_string();
    assert_eq!(diag.adapter_name, expected_name);

    let feature = FeatureId("feature:any");
    assert_eq!(device.query(feature), CapabilityLevel::Unsupported);
    assert_eq!(
        device.query_ex(feature),
        CapabilityQuery {
            level: CapabilityLevel::Unsupported,
            unsupported_reason: Some(UnsupportedReason::UnknownFeature),
            evidence: None,
        }
    );
    assert!(device.limits().max_buffer_bytes > 0);

    let memory = device.memory();
    let payload: Vec<u8> = (0..64_u8).collect();
    let handle = memory
        .allocate(
            64,
            BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD,
        )
        .expect("allocate");
    assert_eq!(handle.size(), 64);
    assert_eq!(
        handle.usage(),
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    );
    memory.upload(&handle, 0, &payload).expect("upload");
    let back = memory.download_blocking(&handle, 0, 64).expect("download");
    assert_eq!(back, payload);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn two_devices_do_not_accept_each_others_buffers() {
    let first = new_device();
    let second = new_device();
    assert!(second.generation() > first.generation());

    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    let from_first = first.memory().allocate(16, usage).expect("allocate first");
    let from_second = second
        .memory()
        .allocate(16, usage)
        .expect("allocate second");
    assert_ne!(
        from_first.__backend_device_id(),
        from_second.__backend_device_id()
    );
    // 各 device の最初のバッファは同じ識別子を持つ（取り違えが最も起きやすい組）。
    assert_eq!(from_first.__backend_id(), from_second.__backend_id());

    second
        .memory()
        .upload(&from_second, 0, &[0xAA; 16])
        .expect("upload second");

    let up = second.memory().upload(&from_first, 0, &[0xFF; 16]);
    assert_eq!(up, Err(ContractError::DeviceMismatch));
    let down = second.memory().download_blocking(&from_first, 0, 16);
    assert_eq!(down, Err(ContractError::DeviceMismatch));

    // 2 つ目の device の同じ識別子の buffer は変わっていない。
    let kept = second
        .memory()
        .download_blocking(&from_second, 0, 16)
        .expect("download second");
    assert_eq!(kept, vec![0xAA; 16]);

    first.memory().free(from_first);
    second.memory().free(from_second);
}

fn usage() -> BufferUsage {
    BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

/// `BufferHandle` は `Clone` を持たないので、同じ識別子・世代・device の識別子で作り直した写しを返す。
fn forged_copy(h: &BufferHandle) -> BufferHandle {
    BufferHandle::__backend_new(
        h.__backend_generation(),
        h.__backend_id(),
        h.__backend_device_id(),
        h.size(),
        h.usage(),
    )
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn upload_to_a_freed_handle_is_rejected_as_invalid_argument() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    let stale = forged_copy(&handle);
    memory.free(handle);

    let up = memory.upload(&stale, 0, &[1; 16]);
    assert!(
        matches!(up, Err(ContractError::InvalidArgument(_))),
        "{up:?}"
    );
    let down = memory.download_blocking(&stale, 0, 16);
    assert!(
        matches!(down, Err(ContractError::InvalidArgument(_))),
        "{down:?}"
    );
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_async_of_a_freed_handle_calls_back_once_with_invalid_argument() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    let stale = forged_copy(&handle);
    memory.free(handle);

    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(Mutex::new(None));
    let (c, r) = (Arc::clone(&calls), Arc::clone(&received));
    memory.download_async(
        &stale,
        0,
        16,
        Box::new(move |result| {
            c.fetch_add(1, Ordering::SeqCst);
            *r.lock().unwrap() = Some(result);
        }),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let got = received.lock().unwrap().take().expect("callback ran");
    assert!(
        matches!(got, Err(ContractError::InvalidArgument(_))),
        "{got:?}"
    );
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn freeing_another_devices_handle_keeps_the_buffer_with_the_same_id() {
    let first = new_device();
    let second = new_device();
    let from_first = first
        .memory()
        .allocate(16, usage())
        .expect("allocate first");
    let from_second = second
        .memory()
        .allocate(16, usage())
        .expect("allocate second");
    assert_eq!(from_first.__backend_id(), from_second.__backend_id());
    second
        .memory()
        .upload(&from_second, 0, &[0x5A; 16])
        .expect("upload second");

    // 別の device のハンドルを渡しても、自 device の同じ識別子のバッファは解放されない。
    second.memory().free(forged_copy(&from_first));

    let kept = second
        .memory()
        .download_blocking(&from_second, 0, 16)
        .expect("download second");
    assert_eq!(kept, vec![0x5A; 16]);
    // 払い出し元の device のバッファも残っている。
    first
        .memory()
        .download_blocking(&from_first, 0, 16)
        .expect("download first");

    first.memory().free(from_first);
    second.memory().free(from_second);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn out_of_range_upload_fails_and_leaves_the_contents_unchanged() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0x11; 16]).expect("upload");

    let past_end = memory.upload(&handle, 8, &[0xEE; 16]);
    assert!(
        matches!(past_end, Err(ContractError::InvalidArgument(_))),
        "{past_end:?}"
    );
    let offset_past_end = memory.upload(&handle, 17, &[]);
    assert!(
        matches!(offset_past_end, Err(ContractError::InvalidArgument(_))),
        "{offset_past_end:?}"
    );

    let back = memory.download_blocking(&handle, 0, 16).expect("download");
    assert_eq!(back, vec![0x11; 16]);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_async_calls_back_once_with_the_same_bytes() {
    let device = new_device();
    let memory = device.memory();
    let payload: Vec<u8> = (0..32_u8).collect();
    let handle = memory.allocate(32, usage()).expect("allocate");
    memory.upload(&handle, 0, &payload).expect("upload");

    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(Mutex::new(None));
    let (c, r) = (Arc::clone(&calls), Arc::clone(&received));
    memory.download_async(
        &handle,
        0,
        32,
        Box::new(move |result| {
            c.fetch_add(1, Ordering::SeqCst);
            *r.lock().unwrap() = Some(result);
        }),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let got = received.lock().unwrap().take().expect("callback ran");
    assert_eq!(got, Ok(payload));
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn allocate_past_the_buffer_limit_is_invalid_argument() {
    let device = new_device();
    let limit = device.limits().max_buffer_bytes;
    // 上限が u64 の最大なら、それを超える大きさは作れない。あふれさせずに抜ける。
    let Some(past_limit) = limit.checked_add(1) else {
        return;
    };
    let r = device.memory().allocate(past_limit, usage());
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn limits_report_the_dispatch_and_texture_bounds() {
    let device = new_device();
    let limits = device.limits();
    assert_eq!(limits.max_dispatch_dims, [u32::MAX; 3]);
    assert_eq!(limits.max_texture_size, 16384);
    let native_max = MTLCreateSystemDefaultDevice()
        .expect("the default Metal device must exist in this session")
        .maxBufferLength();
    assert_eq!(limits.max_buffer_bytes, u64::try_from(native_max).unwrap());
}
