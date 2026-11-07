//! CUDA の `request` の初期化の失敗を差し込みで確かめる試験（TASK-160 のポインタ）。
//!
//! 差し込みの provider と、検査で弾かれる入力だけを呼ぶので、device には触らない。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::{
    CudaComputeCapabilityFailsProvider, CudaContextNewFailsProvider, CudaDeviceProvider,
    CudaDriverMissingProvider, CudaNameFailsProvider, CudaTotalMemFailsProvider,
};
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::{CompletionStateEx, ContractError, FailureClass};

fn compute_only() -> DeviceRequest {
    DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }
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

#[test]
fn each_injected_failure_is_an_initialization_failure() {
    expect_initialization_failure(CudaDriverMissingProvider::request(compute_only()));
    expect_initialization_failure(CudaContextNewFailsProvider::request(compute_only()));
    expect_initialization_failure(CudaNameFailsProvider::request(compute_only()));
    expect_initialization_failure(CudaComputeCapabilityFailsProvider::request(compute_only()));
    expect_initialization_failure(CudaTotalMemFailsProvider::request(compute_only()));
}

/// 検査で弾かれる入力の `index` 番目と、その拒否が `InvalidCombination` かどうか。
/// `Imported` が所有する値は複製できないので、呼び出しごとに作り直す。
fn rejected_request(index: usize) -> (DeviceRequest, bool) {
    let with = |backend, profile, ownership, invalid_combination| {
        (
            DeviceRequest {
                backend,
                profile,
                ownership,
            },
            invalid_combination,
        )
    };
    match index {
        0 => with(
            BackendKind::Metal,
            Profile::ComputeOnly,
            DeviceOwnership::ProductIndependent,
            true,
        ),
        1 => with(
            BackendKind::Cuda,
            Profile::Graphics,
            DeviceOwnership::ProductIndependent,
            true,
        ),
        2 => with(
            BackendKind::Cuda,
            Profile::CpuOnly,
            DeviceOwnership::ProductIndependent,
            true,
        ),
        3 => with(
            BackendKind::Cuda,
            Profile::ComputeOnly,
            DeviceOwnership::Shared,
            false,
        ),
        _ => with(
            BackendKind::Cuda,
            Profile::ComputeOnly,
            DeviceOwnership::Imported {
                native: NativeDeviceHandle::new(Box::new(0_u8)),
            },
            false,
        ),
    }
}

#[test]
fn the_request_checks_come_before_an_injected_failure() {
    for index in 0..5 {
        let (req, invalid_combination) = rejected_request(index);
        let real = expect_err(CudaDeviceProvider::request(req));
        for injected in [
            expect_err(CudaDriverMissingProvider::request(
                rejected_request(index).0,
            )),
            expect_err(CudaContextNewFailsProvider::request(
                rejected_request(index).0,
            )),
            expect_err(CudaNameFailsProvider::request(rejected_request(index).0)),
            expect_err(CudaComputeCapabilityFailsProvider::request(
                rejected_request(index).0,
            )),
            expect_err(CudaTotalMemFailsProvider::request(
                rejected_request(index).0,
            )),
        ] {
            assert_eq!(injected, real);
            assert!(!matches!(injected, ContractError::DeviceUnavailable(_)));
            if invalid_combination {
                assert!(matches!(injected, ContractError::InvalidCombination(_)));
            } else {
                assert!(matches!(
                    injected,
                    ContractError::Unsupported {
                        backend: BackendKind::Cuda,
                        ..
                    }
                ));
            }
        }
    }
}

// DEV-210
#[test]
fn an_injected_failure_is_distinct_from_the_other_kinds() {
    let e = expect_initialization_failure(CudaDriverMissingProvider::request(compute_only()));

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
