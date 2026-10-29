//! Metal の device を実際に作り、診断・能力・メモリ操作を呼ぶ確認（TASK-158 のポインタ。
//! DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265。バッファの照合は MEM-79・MEM-80 のポインタ）。
//!
//! どの試験も Metal の device に触るので `#[ignore]` を付け、専用の Mac の回でだけ流す。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, CapabilityQuery, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::memory::BufferUsage;
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
#[ignore = "touches the Metal device; run only in a dedicated Mac mini session"]
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
    memory.upload(&handle, 0, &payload).expect("upload");
    let back = memory.download_blocking(&handle, 0, 64).expect("download");
    assert_eq!(back, payload);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated Mac mini session"]
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
