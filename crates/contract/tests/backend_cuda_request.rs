//! CUDA の provider の `request` の検査の試験（TASK-160 のポインタ）。
//!
//! 検査は device を作る前に済むので、ここの試験は device に触らない。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
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

// DEV-208
#[test]
fn other_backends_are_invalid_combinations() {
    for backend in [
        BackendKind::Metal,
        BackendKind::Vulkan,
        BackendKind::Dx12Compute,
    ] {
        let e = expect_err(CudaDeviceProvider::request(request(
            backend,
            Profile::ComputeOnly,
            DeviceOwnership::ProductIndependent,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}

#[test]
fn graphics_and_cpu_only_are_invalid_combinations() {
    for profile in [Profile::Graphics, Profile::CpuOnly] {
        let e = expect_err(CudaDeviceProvider::request(request(
            BackendKind::Cuda,
            profile,
            DeviceOwnership::ProductIndependent,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}

#[test]
fn shared_and_imported_are_unsupported() {
    let e = expect_err(CudaDeviceProvider::request(request(
        BackendKind::Cuda,
        Profile::ComputeOnly,
        DeviceOwnership::Shared,
    )));
    assert!(
        matches!(
            e,
            ContractError::Unsupported {
                backend: BackendKind::Cuda,
                ..
            }
        ),
        "{e:?}"
    );
    let e = expect_err(CudaDeviceProvider::request(request(
        BackendKind::Cuda,
        Profile::ComputeOnly,
        DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(Box::new(0_u8)),
        },
    )));
    assert!(
        matches!(
            e,
            ContractError::Unsupported {
                backend: BackendKind::Cuda,
                ..
            }
        ),
        "{e:?}"
    );
}

// backend と profile の拒否はどちらも `InvalidCombination` なので、値の種類で確かめられるのは
// 所有形態の拒否（`Unsupported`）より先に見ること。
#[test]
fn the_backend_is_checked_before_the_profile_and_the_profile_before_the_ownership() {
    for (backend, profile) in [
        (BackendKind::Metal, Profile::CpuOnly),
        (BackendKind::Cuda, Profile::CpuOnly),
        (BackendKind::Metal, Profile::ComputeOnly),
    ] {
        let e = expect_err(CudaDeviceProvider::request(request(
            backend,
            profile,
            DeviceOwnership::Shared,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}
