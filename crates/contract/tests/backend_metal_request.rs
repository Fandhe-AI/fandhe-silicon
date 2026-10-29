//! Metal の背後実装の `DeviceProvider::request` が、device を作る前の検査で返す値の確認
//! （TASK-158 のポインタ。DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265）。
//!
//! ここの試験は Metal の device に触らない。検査は device を作る前に済むため、既定の `cargo test` で流れる。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::ContractError;

/// `Box<dyn Device>` は `Debug` を持たないので、失敗側だけを取り出す。成功したら試験を落とす。
fn expect_err(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("request must fail before a device is created"),
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

#[test]
fn request_rejects_other_backends_as_invalid_combination() {
    for backend in [
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        let e = expect_err(MetalDeviceProvider::request(request(
            backend,
            Profile::ComputeOnly,
            DeviceOwnership::ProductIndependent,
        )));
        assert!(
            matches!(e, ContractError::InvalidCombination(_)),
            "{backend:?}: {e:?}"
        );
    }
}

#[test]
fn request_rejects_cpu_only_as_invalid_combination() {
    let e = expect_err(MetalDeviceProvider::request(request(
        BackendKind::Metal,
        Profile::CpuOnly,
        DeviceOwnership::ProductIndependent,
    )));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
}

#[cfg(not(feature = "graphics"))]
#[test]
fn request_rejects_graphics_without_the_graphics_feature() {
    let e = expect_err(MetalDeviceProvider::request(request(
        BackendKind::Metal,
        Profile::Graphics,
        DeviceOwnership::ProductIndependent,
    )));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
}

#[test]
fn request_reports_shared_as_unsupported_on_metal() {
    let e = expect_err(MetalDeviceProvider::request(request(
        BackendKind::Metal,
        Profile::ComputeOnly,
        DeviceOwnership::Shared,
    )));
    assert!(
        matches!(
            e,
            ContractError::Unsupported {
                backend: BackendKind::Metal,
                ..
            }
        ),
        "{e:?}"
    );
}

#[test]
fn request_reports_imported_as_unsupported_on_metal() {
    let e = expect_err(MetalDeviceProvider::request(request(
        BackendKind::Metal,
        Profile::ComputeOnly,
        DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(0_u32),
        },
    )));
    assert!(
        matches!(
            e,
            ContractError::Unsupported {
                backend: BackendKind::Metal,
                ..
            }
        ),
        "{e:?}"
    );
}

#[test]
fn request_reports_the_backend_mismatch_when_profile_and_ownership_also_fail() {
    // 3 つとも当たる要求では、背後実装の検査の値が返る。プロファイルの検査の文言は
    // 要求の背後実装ではなく Metal を名指すので、`Vulkan` を含む値は背後実装の検査から来たものと区別できる。
    let e = expect_err(MetalDeviceProvider::request(request(
        BackendKind::Vulkan,
        Profile::CpuOnly,
        DeviceOwnership::Shared,
    )));
    match &e {
        ContractError::InvalidCombination(msg) => {
            assert!(msg.contains("Vulkan"), "{msg}");
        }
        other => panic!("expected InvalidCombination, got {other:?}"),
    }
}

#[test]
fn request_checks_profile_before_ownership() {
    // `Metal` + `CpuOnly` + `Shared` は、所有形態の `Unsupported` ではなくプロファイルの `InvalidCombination`。
    let e = expect_err(MetalDeviceProvider::request(request(
        BackendKind::Metal,
        Profile::CpuOnly,
        DeviceOwnership::Shared,
    )));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
}
