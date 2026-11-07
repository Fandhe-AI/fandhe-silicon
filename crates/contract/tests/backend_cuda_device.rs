//! CUDA の device を実際に作り、診断・能力・メモリ操作を呼ぶ試験（TASK-160 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, CapabilityQuery, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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

// DEV-3
#[test]
#[ignore = "requires a CUDA device"]
fn request_returns_a_cuda_device_that_answers_queries_and_round_trips_bytes() {
    let device = new_device();
    assert_eq!(device.backend(), BackendKind::Cuda);
    assert!(matches!(
        device.ownership(),
        DeviceOwnership::ProductIndependent
    ));
    assert!(device.generation() >= 1);

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
    let handle = memory.allocate(64, usage()).expect("allocate");
    assert_eq!(handle.size(), 64);
    assert_eq!(handle.usage(), usage());
    memory.upload(&handle, 0, &payload).expect("upload");
    let back = memory.download_blocking(&handle, 0, 64).expect("download");
    assert_eq!(back, payload);
    memory.free(handle);
}

// DEV-67
#[test]
#[ignore = "requires a CUDA device"]
fn diagnostics_report_the_cuda_backend_name() {
    let device = new_device();
    let diag = device.diagnostics();
    assert_eq!(diag.backend_name, "cuda");
    assert!(!diag.adapter_name.is_empty());
}

// DEV-70
#[test]
#[ignore = "requires a CUDA device"]
fn adapter_name_carries_the_compute_capability() {
    let device = new_device();
    let diag = device.diagnostics();
    assert!(
        diag.adapter_name.contains(" (cuda compute_capability="),
        "{}",
        diag.adapter_name
    );
    // `--show-output` で見える。この出力は照合に使わない。
    println!("adapter_name={}", diag.adapter_name);
}

// DEV-16
#[test]
#[ignore = "requires a CUDA device"]
fn two_devices_do_not_accept_each_others_buffers() {
    let first = new_device();
    let second = new_device();
    assert!(second.generation() > first.generation());

    let from_first = first
        .memory()
        .allocate(16, usage())
        .expect("allocate first");
    let from_second = second
        .memory()
        .allocate(16, usage())
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

    // 2 つ目の device の同じ識別子のバッファは変わっていない。
    let kept = second
        .memory()
        .download_blocking(&from_second, 0, 16)
        .expect("download second");
    assert_eq!(kept, vec![0xAA; 16]);

    first.memory().free(from_first);
    second.memory().free(from_second);
}

#[test]
#[ignore = "requires a CUDA device"]
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
#[ignore = "requires a CUDA device"]
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
#[ignore = "requires a CUDA device"]
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
#[ignore = "requires a CUDA device"]
fn a_handle_that_differs_only_in_generation_is_rejected_and_ignored_by_free() {
    let device = new_device();
    let memory = device.memory();
    let own = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&own, 0, &[0x33; 16]).expect("upload");

    let wrong_generation = BufferHandle::__backend_new(
        own.__backend_generation() + 1,
        own.__backend_id(),
        own.__backend_device_id(),
        own.size(),
        own.usage(),
    );
    let up = memory.upload(&wrong_generation, 0, &[0xEE; 16]);
    assert!(
        matches!(up, Err(ContractError::InvalidArgument(_))),
        "{up:?}"
    );
    memory.free(wrong_generation);

    let kept = memory.download_blocking(&own, 0, 16).expect("download");
    assert_eq!(kept, vec![0x33; 16]);
    memory.free(own);
}

#[test]
#[ignore = "requires a CUDA device"]
fn allocate_of_zero_bytes_is_invalid_argument() {
    let device = new_device();
    let r = device.memory().allocate(0, usage());
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
}

#[test]
#[ignore = "requires a CUDA device"]
fn allocate_past_the_buffer_limit_is_invalid_argument() {
    let device = new_device();
    let Some(over) = device.limits().max_buffer_bytes.checked_add(1) else {
        return;
    };
    let r = device.memory().allocate(over, usage());
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
}

#[test]
#[ignore = "requires a CUDA device"]
fn out_of_range_upload_fails_and_leaves_the_contents_unchanged() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0x11; 16]).expect("upload");

    let cases: [(u64, &[u8]); 4] = [
        (8, &[0xFF; 16]),
        (17, &[]),
        (u64::MAX, &[0xFF; 1]),
        (u64::MAX - 7, &[0xFF; 16]),
    ];
    for (offset, data) in cases {
        let r = memory.upload(&handle, offset, data);
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "offset={offset} len={}: {r:?}",
            data.len()
        );
    }

    let kept = memory.download_blocking(&handle, 0, 16).expect("download");
    assert_eq!(kept, vec![0x11; 16]);
    memory.free(handle);
}

#[test]
#[ignore = "requires a CUDA device"]
fn out_of_range_download_blocking_fails_and_leaves_the_contents_unchanged() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0x22; 16]).expect("upload");

    for (offset, len) in [(8, 9), (0, 17), (17, 0), (u64::MAX, 1), (1, u64::MAX)] {
        let r = memory.download_blocking(&handle, offset, len);
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "offset={offset} len={len}: {r:?}"
        );
    }

    let kept = memory.download_blocking(&handle, 0, 16).expect("download");
    assert_eq!(kept, vec![0x22; 16]);
    memory.free(handle);
}
