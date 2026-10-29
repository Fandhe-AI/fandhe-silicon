//! 拡張性の 8 型を、別の crate からのワイルドカード付きの `match` で 1 つの試験にまとめて確かめる（ERR-18。TASK-554 のポインタ）。
//!
//! 8 型は `ContractError`・`DeviceLostReason`・`CompletionStateEx`・`RetryPolicy`・`UnsupportedReason`・
//! `IdentityMismatchReason`（`error` モジュール）と `CapabilityLevel`・`CapabilityEvidence`（`capability` モジュール）。
//! 仕組み: `#![deny(unreachable_patterns)]` のもとで全値を並べた後に `_` の腕を置く。別の crate からは
//! `#[non_exhaustive]` のせいで `_` が必須になり、属性が外れると `_` が到達不能になって落ちる。
//! 個別の型ごとの確認は `error_non_exhaustive.rs`・`device_capability_non_exhaustive.rs` が持つ。
//! 失敗の種類の型 `FailureClass` は別の関数（ERR-21 の試験）で確かめる。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::capability::{
    BackendKind, CapabilityEvidence, CapabilityLevel, FeatureId,
};
use fandhe_silicon_core::error::{
    CompletionStateEx, ContractError, DeviceLostReason, IdentityMismatchReason, RetryPolicy,
    UnsupportedReason,
};

#[test]
fn eight_types_accept_wildcard_match() {
    let mut matched = 0usize;

    for v in [
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
    ] {
        match v {
            ContractError::InvalidCombination(_)
            | ContractError::DeviceUnavailable(_)
            | ContractError::DeviceLost { .. }
            | ContractError::DeviceMismatch
            | ContractError::InvalidArgument(_)
            | ContractError::AllocationFailed { .. }
            | ContractError::ArtifactRejected { .. }
            | ContractError::ValidationFailed(_)
            | ContractError::Timeout
            | ContractError::Unsupported { .. }
            | ContractError::CapabilityInsufficient { .. }
            | ContractError::DeviceIdentityMismatch { .. }
            | ContractError::UnsupportedExtensionDefault { .. }
            | ContractError::BackendFailure { .. }
            | ContractError::CapacityExceeded { .. } => matched += 1,
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    for v in [
        DeviceLostReason::Destroyed,
        DeviceLostReason::Unknown,
        DeviceLostReason::ValidationError("v".into()),
        DeviceLostReason::Unrecoverable("u".into()),
    ] {
        match v {
            DeviceLostReason::Destroyed
            | DeviceLostReason::Unknown
            | DeviceLostReason::ValidationError(_)
            | DeviceLostReason::Unrecoverable(_) => matched += 1,
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    for v in [
        CompletionStateEx::Pending,
        CompletionStateEx::Complete,
        CompletionStateEx::RuntimeError,
        CompletionStateEx::Lost,
        CompletionStateEx::Unknown,
    ] {
        match v {
            CompletionStateEx::Pending
            | CompletionStateEx::Complete
            | CompletionStateEx::RuntimeError
            | CompletionStateEx::Lost
            | CompletionStateEx::Unknown => matched += 1,
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    for v in [RetryPolicy::IdempotentSafe, RetryPolicy::NotIdempotent] {
        match v {
            RetryPolicy::IdempotentSafe | RetryPolicy::NotIdempotent => matched += 1,
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    for v in [
        UnsupportedReason::NoApi,
        UnsupportedReason::NotSupported,
        UnsupportedReason::UnknownFeature,
    ] {
        match v {
            UnsupportedReason::NoApi
            | UnsupportedReason::NotSupported
            | UnsupportedReason::UnknownFeature => matched += 1,
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    for v in [
        IdentityMismatchReason::Mismatch,
        IdentityMismatchReason::Undeterminable,
    ] {
        match v {
            IdentityMismatchReason::Mismatch | IdentityMismatchReason::Undeterminable => {
                matched += 1
            }
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    for v in [
        CapabilityLevel::S0,
        CapabilityLevel::S1,
        CapabilityLevel::S2,
        CapabilityLevel::S3,
        CapabilityLevel::S4,
        CapabilityLevel::S5,
        CapabilityLevel::S6,
        CapabilityLevel::Unsupported,
    ] {
        match v {
            CapabilityLevel::S0
            | CapabilityLevel::S1
            | CapabilityLevel::S2
            | CapabilityLevel::S3
            | CapabilityLevel::S4
            | CapabilityLevel::S5
            | CapabilityLevel::S6
            | CapabilityLevel::Unsupported => matched += 1,
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    for v in [
        CapabilityEvidence::RuntimeApi,
        CapabilityEvidence::StaticVendorTable,
        CapabilityEvidence::ProbeCompile,
        CapabilityEvidence::ImplementationLedger,
        CapabilityEvidence::FieldRecord,
        CapabilityEvidence::PerformanceComparison,
    ] {
        match v {
            CapabilityEvidence::RuntimeApi
            | CapabilityEvidence::StaticVendorTable
            | CapabilityEvidence::ProbeCompile
            | CapabilityEvidence::ImplementationLedger
            | CapabilityEvidence::FieldRecord
            | CapabilityEvidence::PerformanceComparison => matched += 1,
            _ => unreachable!("a value listed above fell into the wildcard"),
        }
    }

    // 15 + 4 + 5 + 2 + 3 + 2 + 8 + 6
    assert_eq!(matched, 45);
}
