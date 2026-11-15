//! Vulkan の背後実装の `request` の、組み合わせの検査の確認（TASK-284 のポインタ）。
//!
//! 検査で拒まれる要求を渡した `VulkanDeviceProvider::request` と、差し込みの provider（feature
//! `fault-injection`）だけを呼ぶので、device には触らない。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_vulkan::{
    VulkanCreateCommandPoolFailsProvider, VulkanCreateDeviceFailsProvider,
    VulkanCreateInstanceFailsProvider, VulkanDeviceProvider,
    VulkanEnumeratePhysicalDevicesFailsProvider, VulkanLoaderMissingProvider,
    VulkanNoPhysicalDeviceProvider, VulkanNoQueueFamilyProvider,
};
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;

type RequestFn = fn(DeviceRequest) -> Result<Box<dyn Device>, ContractError>;

/// `ProductIndependent` の要求を作る。
fn request(backend: BackendKind, profile: Profile) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

/// 差し込みの provider 7 個の `request`。
fn injected_providers() -> [(&'static str, RequestFn); 7] {
    [
        (
            "VulkanLoaderMissingProvider",
            VulkanLoaderMissingProvider::request,
        ),
        (
            "VulkanCreateInstanceFailsProvider",
            VulkanCreateInstanceFailsProvider::request,
        ),
        (
            "VulkanEnumeratePhysicalDevicesFailsProvider",
            VulkanEnumeratePhysicalDevicesFailsProvider::request,
        ),
        (
            "VulkanNoPhysicalDeviceProvider",
            VulkanNoPhysicalDeviceProvider::request,
        ),
        (
            "VulkanNoQueueFamilyProvider",
            VulkanNoQueueFamilyProvider::request,
        ),
        (
            "VulkanCreateDeviceFailsProvider",
            VulkanCreateDeviceFailsProvider::request,
        ),
        (
            "VulkanCreateCommandPoolFailsProvider",
            VulkanCreateCommandPoolFailsProvider::request,
        ),
    ]
}

/// `Box<dyn Device>` は `Debug` を持たないので、失敗側だけを取り出す。成功したら試験を落とす。
fn expect_err(label: &str, r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("{label}: expected an error"),
        Err(e) => e,
    }
}

/// ERR-1。Vulkan 以外の背後実装に `Graphics`・`CpuOnly` を付けた要求は、本物の provider でも差し込みの provider でも `InvalidCombination`。
#[test]
fn other_backends_with_graphics_or_cpu_only_are_invalid_combinations() {
    let mut providers: Vec<(&str, RequestFn)> =
        vec![("VulkanDeviceProvider", VulkanDeviceProvider::request)];
    providers.extend(injected_providers());
    for (label, call) in providers {
        for backend in [
            BackendKind::Metal,
            BackendKind::Cuda,
            BackendKind::Dx12Compute,
        ] {
            for profile in [Profile::Graphics, Profile::CpuOnly] {
                let e = expect_err(label, call(request(backend, profile)));
                assert!(
                    matches!(e, ContractError::InvalidCombination(_)),
                    "{label} {backend:?} {profile:?}: {e:?}"
                );
            }
        }
    }
}

/// feature `graphics` のビルドでは、`Vulkan`・`Graphics` の要求は検査を通り、差し込みの失敗の `DeviceUnavailable` まで進む。
#[cfg(feature = "graphics")]
#[test]
fn vulkan_graphics_passes_the_checks_in_a_graphics_build() {
    for (label, call) in injected_providers() {
        let e = expect_err(label, call(request(BackendKind::Vulkan, Profile::Graphics)));
        assert!(
            matches!(e, ContractError::DeviceUnavailable(_)),
            "{label}: {e:?}"
        );
    }
}

/// Vulkan の背後実装のソースは、ほかの背後実装の provider と module の名を含まない。
#[test]
fn the_vulkan_source_names_no_other_backend_provider() {
    const SOURCE: &str = include_str!("../src/backend_vulkan.rs");
    for word in [
        "MetalDeviceProvider",
        "CudaDeviceProvider",
        "backend_metal",
        "backend_cuda",
    ] {
        assert!(!SOURCE.contains(word), "{word}");
    }
}
