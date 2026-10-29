//! Metal の背後実装の差し込み用の入口（実際の故障ではない）の確認（TASK-158 のポインタ。
//! DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265。失敗の種類の区別は TASK-272 の流儀）。
//!
//! 検査が device を作る前であることを、既定の device に触らない差し込みの入口で確かめる。
//! device に触る試験は `#[ignore]` を付け、専用の Mac の回でだけ流す。macOS 以外では 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::{
    MetalCommandQueueNoneProvider, MetalDefaultDeviceNoneProvider,
};
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::{CompletionStateEx, ContractError, FailureClass};

fn expect_err(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("request must fail"),
        Err(e) => e,
    }
}

fn compute_request(backend: BackendKind, ownership: DeviceOwnership) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership,
    }
}

#[test]
fn checks_run_before_the_device_is_created_for_other_backends() {
    // 差し込みの入口は device の取得を `None` にするが、検査が先なので `DeviceUnavailable` にならない。
    for backend in [
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        let e = expect_err(MetalDefaultDeviceNoneProvider::request(compute_request(
            backend,
            DeviceOwnership::ProductIndependent,
        )));
        assert!(
            matches!(e, ContractError::InvalidCombination(_)),
            "{backend:?}: {e:?}"
        );
    }
}

#[test]
fn checks_run_before_the_device_is_created_for_profiles() {
    let cpu = expect_err(MetalDefaultDeviceNoneProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::CpuOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }));
    assert!(
        matches!(cpu, ContractError::InvalidCombination(_)),
        "{cpu:?}"
    );

    #[cfg(not(feature = "graphics"))]
    {
        let graphics = expect_err(MetalDefaultDeviceNoneProvider::request(DeviceRequest {
            backend: BackendKind::Metal,
            profile: Profile::Graphics,
            ownership: DeviceOwnership::ProductIndependent,
        }));
        assert!(
            matches!(graphics, ContractError::InvalidCombination(_)),
            "{graphics:?}"
        );
    }
}

#[test]
fn checks_run_before_the_device_is_created_for_ownership() {
    for ownership in [
        DeviceOwnership::Shared,
        DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(0_u32),
        },
    ] {
        let e = expect_err(MetalDefaultDeviceNoneProvider::request(compute_request(
            BackendKind::Metal,
            ownership,
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
}

#[test]
fn default_device_none_is_an_initialization_failure_distinct_from_the_other_kinds() {
    let e = expect_err(MetalDefaultDeviceNoneProvider::request(compute_request(
        BackendKind::Metal,
        DeviceOwnership::ProductIndependent,
    )));
    assert!(matches!(e, ContractError::DeviceUnavailable(_)), "{e:?}");
    assert_eq!(e.failure_class(), FailureClass::Initialization);

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

    // 別の crate の利用側と同じ、`_` を持つ `match` でも同じ種類になる。
    let by_match = match &e {
        ContractError::InvalidCombination(_)
        | ContractError::DeviceUnavailable(_)
        | ContractError::ArtifactRejected { .. } => Some(FailureClass::Initialization),
        _ => None,
    };
    assert_eq!(by_match, Some(FailureClass::Initialization));
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated Mac mini session"]
fn command_queue_none_is_an_initialization_failure() {
    let e = expect_err(MetalCommandQueueNoneProvider::request(compute_request(
        BackendKind::Metal,
        DeviceOwnership::ProductIndependent,
    )));
    assert!(matches!(e, ContractError::DeviceUnavailable(_)), "{e:?}");
    assert_eq!(e.failure_class(), FailureClass::Initialization);
}
