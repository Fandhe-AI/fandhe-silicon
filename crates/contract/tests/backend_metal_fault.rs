//! Metal の背後実装の差し込み用の入口（実際の故障ではない）の確認（TASK-158 のポインタ。
//! DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265。失敗の種類の区別は TASK-272 の流儀）。
//!
//! 検査が device を作る前であることを、既定の device に触らない差し込みの入口で確かめる。
//! device に触る試験は `#[ignore]` を付け、専用の実機の回でだけ流す。macOS 以外では 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::{
    MetalCommandQueueNoneProvider, MetalDefaultDeviceNoneProvider, MetalDeviceProvider,
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
fn injected_provider_reports_the_backend_mismatch_first() {
    // 背後実装・プロファイル・所有形態の 3 つが当たる要求でも、`DeviceUnavailable` にはならず、
    // 本物の入口と同じ値（`Vulkan` を名指す `InvalidCombination`）が返る。
    let req = || DeviceRequest {
        backend: BackendKind::Vulkan,
        profile: Profile::CpuOnly,
        ownership: DeviceOwnership::Shared,
    };
    let injected = expect_err(MetalDefaultDeviceNoneProvider::request(req()));
    let real = expect_err(MetalDeviceProvider::request(req()));
    assert_eq!(injected, real);
    match &injected {
        ContractError::InvalidCombination(msg) => assert!(msg.contains("Vulkan"), "{msg}"),
        other => panic!("expected InvalidCombination, got {other:?}"),
    }
}

#[test]
fn injected_provider_checks_profile_before_ownership() {
    let req = || DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::CpuOnly,
        ownership: DeviceOwnership::Shared,
    };
    let injected = expect_err(MetalDefaultDeviceNoneProvider::request(req()));
    let real = expect_err(MetalDeviceProvider::request(req()));
    assert_eq!(injected, real);
    assert!(
        matches!(injected, ContractError::InvalidCombination(_)),
        "{injected:?}"
    );
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
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn command_queue_none_is_an_initialization_failure() {
    let e = expect_err(MetalCommandQueueNoneProvider::request(compute_request(
        BackendKind::Metal,
        DeviceOwnership::ProductIndependent,
    )));
    assert!(matches!(e, ContractError::DeviceUnavailable(_)), "{e:?}");
    assert_eq!(e.failure_class(), FailureClass::Initialization);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn fault_hooks_are_present_and_the_loss_moves_the_generation_once() {
    // TASK-2932・DEV-80 のポインタ。公開の `Device` 越しに見える範囲を確かめる。
    use fandhe_silicon_core::error::DeviceLostReason;
    use fandhe_silicon_core::fault::{HookFault, inject};

    let device = MetalDeviceProvider::request(compute_request(
        BackendKind::Metal,
        DeviceOwnership::ProductIndependent,
    ))
    .expect("request");
    assert!(device.fault_hooks().is_some());
    let gate = inject(device.fault_hooks(), HookFault::ArmGate);
    assert!(
        matches!(gate, Err(ContractError::UnsupportedExtensionDefault { .. })),
        "{gate:?}"
    );

    let before = device.generation();
    inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Unknown),
    )
    .expect("first loss");
    let after = device.generation();
    assert!(after > before);
    inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Destroyed),
    )
    .expect("second loss");
    assert_eq!(device.generation(), after);
}

/// TASK-274 のポインタ（ERR-43・ERR-44）。Metal の queue に完了不明の差し込みを被せ、宣言なしの `submit` を 1 回流す。
/// 完了不明を観測しても、被せた口を通した内側の `submit` は 1 回のまま（`submit_call_count`）で、
/// 実機の queue への投入も増えない（次の投入の通し番号が 1 だけ進む）。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn plain_submit_over_unknown_completion_is_not_retried() {
    use fandhe_silicon_core::error::CompletionStateEx;
    use fandhe_silicon_core::execution::CommandQueue;
    use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

    let device = MetalDeviceProvider::request(compute_request(
        BackendKind::Metal,
        DeviceOwnership::ProductIndependent,
    ))
    .expect("request");
    let queue: &dyn CommandQueue = device.queue();
    let gate = FaultInjecting::new(queue, FaultScript::CompletionUnknown);

    let token = gate
        .submit_with(|q| q.submit(q.create_encoder(), &[]))
        .expect("submit");
    assert_eq!(gate.submit_call_count(), 1);
    assert_eq!(
        gate.poll_ex_with(|q| q.poll_ex(token)),
        CompletionStateEx::Unknown
    );
    assert_eq!(
        gate.poll_ex_with(|q| q.poll_ex(token)),
        CompletionStateEx::Unknown
    );
    assert_eq!(gate.submit_call_count(), 1);

    let next = queue.submit(queue.create_encoder(), &[]).expect("next");
    assert_eq!(next.sequence, token.sequence + 1);
}
