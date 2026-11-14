//! Vulkan の device を実際に作り、診断・能力・メモリ操作・投入の口を呼ぶ試験（TASK-159 のポインタ）。
//!
//! Vulkan の device が要る。どの試験も `#[ignore]` を付けていて、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_vulkan::VulkanDeviceProvider;
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel};
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::feature_key::FEATURE_SHADER_F16;
use fandhe_silicon_core::memory::BufferUsage;

fn request_with(profile: Profile) -> DeviceRequest {
    DeviceRequest {
        backend: BackendKind::Vulkan,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

fn new_device(profile: Profile) -> Box<dyn Device> {
    match VulkanDeviceProvider::request(request_with(profile)) {
        Ok(device) => device,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

// DEV-67
#[test]
#[ignore = "requires a Vulkan device"]
fn compute_only_request_returns_a_vulkan_device() {
    let device = new_device(Profile::ComputeOnly);
    assert_eq!(device.backend(), BackendKind::Vulkan);
    assert!(matches!(
        device.ownership(),
        DeviceOwnership::ProductIndependent
    ));
    let diagnostics = device.diagnostics();
    assert_eq!(diagnostics.backend_name, "vulkan");
    // adapter_name の値は環境ごとに違うので、書式の部品だけを確かめる。
    let name = diagnostics.adapter_name;
    println!("adapter_name={name}");
    let head = name.find(" (vulkan api=").expect("api part");
    assert!(head > 0);
    assert!(name.contains(" driver_version=0x"));
}

#[test]
#[ignore = "requires a Vulkan device"]
fn two_requests_take_increasing_generations() {
    let first = new_device(Profile::ComputeOnly);
    let second = new_device(Profile::ComputeOnly);
    assert!(second.generation() > first.generation());
}

#[test]
#[ignore = "requires a Vulkan device"]
fn query_ex_has_no_rows() {
    let device = new_device(Profile::ComputeOnly);
    assert_eq!(device.query_ex(FEATURE_SHADER_F16), UNLISTED_FEATURE_QUERY);
    assert_eq!(
        device.query(FEATURE_SHADER_F16),
        CapabilityLevel::Unsupported
    );
}

#[test]
#[ignore = "requires a Vulkan device"]
fn memory_and_queue_operations_are_unsupported() {
    let device = new_device(Profile::ComputeOnly);
    assert!(matches!(
        device.memory().allocate(4, BufferUsage::STORAGE),
        Err(ContractError::Unsupported {
            backend: BackendKind::Vulkan,
            ..
        })
    ));
    let queue = device.queue();
    let encoder = queue.create_encoder();
    assert!(matches!(
        queue.submit(encoder, &[]),
        Err(ContractError::Unsupported {
            backend: BackendKind::Vulkan,
            ..
        })
    ));
    assert_eq!(queue.max_buffer_bytes(), device.limits().max_buffer_bytes);
}

#[cfg(feature = "graphics")]
#[test]
#[ignore = "requires a Vulkan device"]
fn graphics_request_returns_a_device_with_feature_graphics() {
    let device = new_device(Profile::Graphics);
    assert_eq!(device.backend(), BackendKind::Vulkan);
    assert!(device.graphics().is_none());
}

#[test]
#[ignore = "requires a Vulkan device"]
fn dropping_a_device_does_not_panic() {
    for _ in 0..2 {
        let device = new_device(Profile::ComputeOnly);
        drop(device);
    }
}
