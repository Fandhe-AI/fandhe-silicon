//! Vulkan の `request` の初期化の失敗を差し込みで確かめる試験（TASK-159 のポインタ）。
//!
//! 差し込みの provider と、検査で弾かれる入力だけを呼ぶので、device には触らない。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_vulkan::{
    VulkanCreateCommandPoolFailsProvider, VulkanCreateDeviceFailsProvider,
    VulkanCreateInstanceFailsProvider, VulkanEnumeratePhysicalDevicesFailsProvider,
    VulkanLoaderMissingProvider, VulkanNoPhysicalDeviceProvider, VulkanNoQueueFamilyProvider,
};
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionStateEx, ContractError, FailureClass};

type RequestFn = fn(DeviceRequest) -> Result<Box<dyn Device>, ContractError>;

fn request_with(backend: BackendKind, profile: Profile) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

fn compute_only() -> DeviceRequest {
    request_with(BackendKind::Vulkan, Profile::ComputeOnly)
}

/// 差し込みの provider 7 個の `request`。
fn injected_providers() -> [RequestFn; 7] {
    [
        VulkanLoaderMissingProvider::request,
        VulkanCreateInstanceFailsProvider::request,
        VulkanEnumeratePhysicalDevicesFailsProvider::request,
        VulkanNoPhysicalDeviceProvider::request,
        VulkanNoQueueFamilyProvider::request,
        VulkanCreateDeviceFailsProvider::request,
        VulkanCreateCommandPoolFailsProvider::request,
    ]
}

/// `Box<dyn Device>` は `Debug` を持たないので、失敗側だけを取り出す。成功したら試験を落とす。
fn expect_err(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

fn expect_initialization_failure(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    let e = expect_err(r);
    let ContractError::DeviceUnavailable(detail) = &e else {
        panic!("{e:?}");
    };
    assert!(detail.contains("injected"), "{detail}");
    assert_eq!(e.failure_class(), FailureClass::Initialization);
    e
}

// DEV-212
#[test]
fn injected_initialization_failures_are_device_unavailable() {
    for request in injected_providers() {
        expect_initialization_failure(request(compute_only()));
    }
}

// DEV-210
#[test]
fn an_injected_failure_is_distinct_from_the_other_kinds() {
    let e = expect_initialization_failure(VulkanLoaderMissingProvider::request(compute_only()));

    // 戻り値の値と種類だけで、完了不明・実行時エラー・ロスト・能力不足と区別できる。
    let timeout = ContractError::Timeout;
    let capability = ContractError::CapabilityInsufficient {
        feature: FeatureId("feature:contract"),
        required: CapabilityLevel::S2,
        actual: CapabilityLevel::S0,
    };
    assert_ne!(e, timeout);
    assert_ne!(e, capability);
    assert_eq!(timeout.failure_class(), FailureClass::CompletionUnknown);
    assert_eq!(capability.failure_class(), FailureClass::Capability);
    assert_ne!(e.failure_class(), timeout.failure_class());
    assert_ne!(e.failure_class(), capability.failure_class());

    let runtime = CompletionStateEx::RuntimeError;
    let lost = CompletionStateEx::Lost;
    assert_eq!(runtime.failure_class(), Some(FailureClass::Runtime));
    assert_eq!(lost.failure_class(), Some(FailureClass::DeviceLost));
    assert_ne!(Some(e.failure_class()), runtime.failure_class());
    assert_ne!(Some(e.failure_class()), lost.failure_class());

    match e.failure_class() {
        FailureClass::Initialization => {}
        _ => panic!("{e:?}"),
    }
}

#[test]
fn injected_providers_check_the_request_first() {
    for request in injected_providers() {
        let wrong_backend = expect_err(request(request_with(
            BackendKind::Metal,
            Profile::ComputeOnly,
        )));
        assert!(
            matches!(wrong_backend, ContractError::InvalidCombination(_)),
            "{wrong_backend:?}"
        );
        let cpu_only = expect_err(request(request_with(BackendKind::Vulkan, Profile::CpuOnly)));
        assert!(
            matches!(cpu_only, ContractError::InvalidCombination(_)),
            "{cpu_only:?}"
        );
    }
}
