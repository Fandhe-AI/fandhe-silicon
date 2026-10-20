//! `error` モジュールの `#[non_exhaustive]` な 6 型を、別の crate から固定する公開テスト（TASK-269 の 4/8）。
//!
//! 仕組み: `#![deny(unreachable_patterns)]` のもとで全値を並べた後に `_` の腕を置く。
//! 別の crate からは `#[non_exhaustive]` のせいで `_` の腕が必須になり、属性が外れると `_` が
//! 到達不能になってコンパイルが落ちる（ERR-18。ポインタのみ）。
//!
//! 対象外: `CompletionState`（閉じた型。`completion_state_closed.rs` が確かめる）と、
//! `capability` モジュールの 3 型（TASK-136 の持ち分。ポインタのみ）。
//! フィールドの値は束縛せず `{ .. }` で受けるだけにする（読み戻しは後の段）。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::error::{
    CompletionStateEx, ContractError, DeviceLostReason, IdentityMismatchReason, RetryPolicy,
    UnsupportedReason,
};

#[test]
fn contract_error_stays_non_exhaustive() {
    fn label(v: &ContractError) -> &'static str {
        match v {
            ContractError::InvalidCombination(_) => "InvalidCombination",
            ContractError::DeviceUnavailable(_) => "DeviceUnavailable",
            ContractError::DeviceLost { .. } => "DeviceLost",
            ContractError::DeviceMismatch => "DeviceMismatch",
            ContractError::InvalidArgument(_) => "InvalidArgument",
            ContractError::AllocationFailed { .. } => "AllocationFailed",
            ContractError::ArtifactRejected { .. } => "ArtifactRejected",
            ContractError::ValidationFailed(_) => "ValidationFailed",
            ContractError::Timeout => "Timeout",
            ContractError::Unsupported { .. } => "Unsupported",
            ContractError::CapabilityInsufficient { .. } => "CapabilityInsufficient",
            ContractError::DeviceIdentityMismatch { .. } => "DeviceIdentityMismatch",
            ContractError::UnsupportedExtensionDefault { .. } => "UnsupportedExtensionDefault",
            ContractError::BackendFailure { .. } => "BackendFailure",
            ContractError::CapacityExceeded { .. } => "CapacityExceeded",
            _ => "other",
        }
    }
    let values = [
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
    let labels: Vec<_> = values.iter().map(label).collect();
    assert_eq!(
        labels,
        [
            "InvalidCombination",
            "DeviceUnavailable",
            "DeviceLost",
            "DeviceMismatch",
            "InvalidArgument",
            "AllocationFailed",
            "ArtifactRejected",
            "ValidationFailed",
            "Timeout",
            "Unsupported",
            "CapabilityInsufficient",
            "DeviceIdentityMismatch",
            "UnsupportedExtensionDefault",
            "BackendFailure",
            "CapacityExceeded",
        ]
    );
}

#[test]
fn device_lost_reason_stays_non_exhaustive() {
    fn label(v: &DeviceLostReason) -> &'static str {
        match v {
            DeviceLostReason::Destroyed => "Destroyed",
            DeviceLostReason::Unknown => "Unknown",
            DeviceLostReason::ValidationError(_) => "ValidationError",
            DeviceLostReason::Unrecoverable(_) => "Unrecoverable",
            _ => "other",
        }
    }
    let values = [
        DeviceLostReason::Destroyed,
        DeviceLostReason::Unknown,
        DeviceLostReason::ValidationError("v".into()),
        DeviceLostReason::Unrecoverable("u".into()),
    ];
    let labels: Vec<_> = values.iter().map(label).collect();
    assert_eq!(
        labels,
        ["Destroyed", "Unknown", "ValidationError", "Unrecoverable"]
    );
}

#[test]
fn completion_state_ex_stays_non_exhaustive() {
    fn label(v: CompletionStateEx) -> &'static str {
        match v {
            CompletionStateEx::Pending => "Pending",
            CompletionStateEx::Complete => "Complete",
            CompletionStateEx::RuntimeError => "RuntimeError",
            CompletionStateEx::Lost => "Lost",
            CompletionStateEx::Unknown => "Unknown",
            _ => "other",
        }
    }
    let labels: Vec<_> = [
        CompletionStateEx::Pending,
        CompletionStateEx::Complete,
        CompletionStateEx::RuntimeError,
        CompletionStateEx::Lost,
        CompletionStateEx::Unknown,
    ]
    .into_iter()
    .map(label)
    .collect();
    assert_eq!(
        labels,
        ["Pending", "Complete", "RuntimeError", "Lost", "Unknown"]
    );
}

#[test]
fn retry_policy_stays_non_exhaustive() {
    fn label(v: RetryPolicy) -> &'static str {
        match v {
            RetryPolicy::IdempotentSafe => "IdempotentSafe",
            RetryPolicy::NotIdempotent => "NotIdempotent",
            _ => "other",
        }
    }
    let labels: Vec<_> = [RetryPolicy::IdempotentSafe, RetryPolicy::NotIdempotent]
        .into_iter()
        .map(label)
        .collect();
    assert_eq!(labels, ["IdempotentSafe", "NotIdempotent"]);
}

#[test]
fn identity_mismatch_reason_stays_non_exhaustive() {
    fn label(v: IdentityMismatchReason) -> &'static str {
        match v {
            IdentityMismatchReason::Mismatch => "Mismatch",
            IdentityMismatchReason::Undeterminable => "Undeterminable",
            _ => "other",
        }
    }
    let labels: Vec<_> = [
        IdentityMismatchReason::Mismatch,
        IdentityMismatchReason::Undeterminable,
    ]
    .into_iter()
    .map(label)
    .collect();
    assert_eq!(labels, ["Mismatch", "Undeterminable"]);
}

#[test]
fn unsupported_reason_stays_non_exhaustive() {
    fn label(v: UnsupportedReason) -> &'static str {
        match v {
            UnsupportedReason::NoApi => "NoApi",
            UnsupportedReason::NotSupported => "NotSupported",
            UnsupportedReason::UnknownFeature => "UnknownFeature",
            _ => "other",
        }
    }
    let labels: Vec<_> = [
        UnsupportedReason::NoApi,
        UnsupportedReason::NotSupported,
        UnsupportedReason::UnknownFeature,
    ]
    .into_iter()
    .map(label)
    .collect();
    assert_eq!(labels, ["NoApi", "NotSupported", "UnknownFeature"]);
}
