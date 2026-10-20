//! エラー・完了状態・理由の型を、別の crate から構築できることだけを確かめる最小の試験（TASK-269 の 2/8）。
//!
//! 拡張性（ERR-18）・フィールドの読み戻し（ERR-53）・`Debug` の書式（ERR-54）は後の段で確かめる。

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, DeviceLostReason, IdentityMismatchReason,
    RetryPolicy, UnsupportedReason,
};

#[test]
fn contract_error_variants_construct_and_clone() {
    let values = vec![
        ContractError::InvalidCombination("a".into()),
        ContractError::DeviceUnavailable("a".into()),
        ContractError::DeviceLost { generation: 1 },
        ContractError::DeviceMismatch,
        ContractError::InvalidArgument("a".into()),
        ContractError::AllocationFailed { requested_bytes: 8 },
        ContractError::ArtifactRejected {
            kind: "k",
            reason: "r".into(),
        },
        ContractError::ValidationFailed("a".into()),
        ContractError::Timeout,
        ContractError::Unsupported {
            op: "o",
            backend: BackendKind::Metal,
        },
        ContractError::CapabilityInsufficient {
            feature: FeatureId("f"),
            required: CapabilityLevel::S3,
            actual: CapabilityLevel::S0,
        },
        ContractError::DeviceIdentityMismatch {
            reason: IdentityMismatchReason::Mismatch,
        },
        ContractError::UnsupportedExtensionDefault { op: "o" },
        ContractError::BackendFailure {
            op: "o",
            backend: BackendKind::Vulkan,
            detail: "d".into(),
        },
        ContractError::CapacityExceeded {
            op: "o",
            backend: BackendKind::Cuda,
            capacity: 4,
        },
    ];
    assert_eq!(values.len(), 15);
    for v in &values {
        assert_eq!(&v.clone(), v);
    }
}

#[test]
fn other_types_construct_and_clone() {
    let lost = [
        DeviceLostReason::Destroyed,
        DeviceLostReason::Unknown,
        DeviceLostReason::ValidationError("v".into()),
        DeviceLostReason::Unrecoverable("u".into()),
    ];
    for v in &lost {
        assert_eq!(&v.clone(), v);
    }
    for v in [
        CompletionState::Pending,
        CompletionState::Complete,
        CompletionState::Lost,
    ] {
        assert_eq!(v, v.clone());
    }
    for v in [
        CompletionStateEx::Pending,
        CompletionStateEx::Complete,
        CompletionStateEx::RuntimeError,
        CompletionStateEx::Lost,
        CompletionStateEx::Unknown,
    ] {
        assert_eq!(v, v.clone());
    }
    for v in [RetryPolicy::IdempotentSafe, RetryPolicy::NotIdempotent] {
        assert_eq!(v, v.clone());
    }
    for v in [
        IdentityMismatchReason::Mismatch,
        IdentityMismatchReason::Undeterminable,
    ] {
        assert_eq!(v, v.clone());
    }
    for v in [
        UnsupportedReason::NoApi,
        UnsupportedReason::NotSupported,
        UnsupportedReason::UnknownFeature,
    ] {
        assert_eq!(v, v.clone());
    }
    for v in [
        BackendKind::Metal,
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        assert_eq!(v, v.clone());
    }
    assert_ne!(CapabilityLevel::S0, CapabilityLevel::Unsupported);
}
