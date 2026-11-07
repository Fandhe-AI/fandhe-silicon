//! CUDA の provider の `request` の検査の試験（TASK-160 のポインタ）。
//!
//! 検査は device を作る前に済むので、ここの試験は device に触らない（`#[ignore]` 付きの 1 件を除く）。
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

/// 世代だけが違う偽のハンドルは、upload を拒まれ、free を無視される（device に触る試験）。
#[test]
#[ignore = "touches the CUDA device; run only in a dedicated device session"]
fn a_handle_with_another_generation_is_rejected_and_cannot_free_the_buffer() {
    use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

    let device = match CudaDeviceProvider::request(request(
        BackendKind::Cuda,
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    )) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    };
    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    let memory = device.memory();
    let own = memory.allocate(16, usage).expect("allocate");
    let forged = BufferHandle::__backend_new(
        own.__backend_generation() + 1,
        own.__backend_id(),
        own.__backend_device_id(),
        16,
        usage,
    );
    let e = memory.upload(&forged, 0, &[1_u8; 16]).unwrap_err();
    assert!(matches!(e, ContractError::InvalidArgument(_)), "{e:?}");
    memory.free(forged);
    // 正規のハンドルはまだ有効で、upload が通る。
    memory.upload(&own, 0, &[2_u8; 16]).expect("upload");
    memory.free(own);
}
