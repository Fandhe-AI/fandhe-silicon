//! Vulkan の provider の `request` の検査の試験（TASK-159 のポインタ）。
//!
//! 検査は device を作る前に済むので、ここの試験は device に触らない。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_vulkan::VulkanDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::ContractError;

/// `Box<dyn Device>` は `Debug` を持たないので、失敗側だけを取り出す。成功したら試験を落とす。
fn expect_err(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

fn request(backend: BackendKind, profile: Profile, ownership: DeviceOwnership) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile,
        ownership,
    }
}

// DEV-207
#[test]
fn non_vulkan_backend_is_invalid_combination() {
    for backend in [
        BackendKind::Metal,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        let e = expect_err(VulkanDeviceProvider::request(request(
            backend,
            Profile::ComputeOnly,
            DeviceOwnership::ProductIndependent,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}

#[test]
fn cpu_only_is_invalid_combination() {
    let e = expect_err(VulkanDeviceProvider::request(request(
        BackendKind::Vulkan,
        Profile::CpuOnly,
        DeviceOwnership::ProductIndependent,
    )));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
}

#[cfg(not(feature = "graphics"))]
#[test]
fn graphics_without_feature_is_invalid_combination() {
    let e = expect_err(VulkanDeviceProvider::request(request(
        BackendKind::Vulkan,
        Profile::Graphics,
        DeviceOwnership::ProductIndependent,
    )));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
}

#[test]
fn shared_and_imported_are_unsupported() {
    let e = expect_err(VulkanDeviceProvider::request(request(
        BackendKind::Vulkan,
        Profile::ComputeOnly,
        DeviceOwnership::Shared,
    )));
    assert!(
        matches!(
            e,
            ContractError::Unsupported {
                backend: BackendKind::Vulkan,
                ..
            }
        ),
        "{e:?}"
    );
    let e = expect_err(VulkanDeviceProvider::request(request(
        BackendKind::Vulkan,
        Profile::ComputeOnly,
        DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(Box::new(0_u8)),
        },
    )));
    assert!(
        matches!(
            e,
            ContractError::Unsupported {
                backend: BackendKind::Vulkan,
                ..
            }
        ),
        "{e:?}"
    );
}

// 背後実装の拒否は `InvalidCombination`、所有形態の拒否は `Unsupported` なので、
// 値の種類で背後実装が先に見られることを確かめられる。
#[test]
fn backend_is_checked_before_profile_and_ownership() {
    let e = expect_err(VulkanDeviceProvider::request(request(
        BackendKind::Metal,
        Profile::CpuOnly,
        DeviceOwnership::Shared,
    )));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
}
