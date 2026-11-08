//! CUDA の provider が `CpuOnly`・`Graphics` の要求を所有形態によらず拒むことを確かめる試験（TASK-169 のポインタ）。
//!
//! 検査で弾かれる入力だけを呼ぶので、device には触らない。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::ContractError;

/// 3 つの所有形態。`Imported` が所有する値は複製できないので、呼び出しごとに作り直す。
fn ownerships() -> [DeviceOwnership; 3] {
    [
        DeviceOwnership::ProductIndependent,
        DeviceOwnership::Shared,
        DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(Box::new(0_u8)),
        },
    ]
}

fn cuda(profile: Profile, ownership: DeviceOwnership) -> DeviceRequest {
    DeviceRequest {
        backend: BackendKind::Cuda,
        profile,
        ownership,
    }
}

/// `Box<dyn Device>` は `Debug` を持たないので、失敗側だけを取り出す。成功したら試験を落とす。
fn expect_err(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

// DEV-64
#[test]
fn cpu_only_is_an_invalid_combination_for_every_ownership() {
    for ownership in ownerships() {
        let e = expect_err(CudaDeviceProvider::request(cuda(
            Profile::CpuOnly,
            ownership,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}

// DEV-209
#[test]
fn graphics_is_an_invalid_combination_for_every_ownership() {
    for ownership in ownerships() {
        let e = expect_err(CudaDeviceProvider::request(cuda(
            Profile::Graphics,
            ownership,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}
