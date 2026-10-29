//! 失敗の種類を返す入口 `failure_class` の確認（ERR-21。TASK-554 のポインタ）。
//!
//! `ContractError` の全 15 値と `CompletionStateEx` の全 5 値が、期待の種類を返すこと。
//! 2 つの型の `match` による判別も、入口と並んで使えること。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::error::{
    CompletionStateEx, ContractError, FailureClass, IdentityMismatchReason,
};

fn all_contract_errors() -> Vec<(ContractError, FailureClass)> {
    use FailureClass::*;
    vec![
        (
            ContractError::InvalidCombination("a".into()),
            Initialization,
        ),
        (ContractError::DeviceUnavailable("a".into()), Initialization),
        (
            ContractError::ArtifactRejected {
                kind: "k",
                reason: "r".into(),
            },
            Initialization,
        ),
        (ContractError::InvalidArgument("a".into()), Runtime),
        (ContractError::DeviceMismatch, Runtime),
        (
            ContractError::AllocationFailed { requested_bytes: 8 },
            Runtime,
        ),
        (ContractError::ValidationFailed("a".into()), Runtime),
        (
            ContractError::BackendFailure {
                op: "o",
                backend: BackendKind::Vulkan,
                detail: "d".into(),
            },
            Runtime,
        ),
        (
            ContractError::DeviceIdentityMismatch {
                reason: IdentityMismatchReason::Mismatch,
            },
            Runtime,
        ),
        (
            ContractError::CapacityExceeded {
                op: "o",
                backend: BackendKind::Cuda,
                capacity: 4,
            },
            Runtime,
        ),
        (ContractError::DeviceLost { generation: 1 }, DeviceLost),
        (ContractError::Timeout, CompletionUnknown),
        (
            ContractError::CapabilityInsufficient {
                feature: FeatureId("f"),
                required: CapabilityLevel::S3,
                actual: CapabilityLevel::S0,
            },
            Capability,
        ),
        (
            ContractError::Unsupported {
                op: "o",
                backend: BackendKind::Metal,
            },
            Capability,
        ),
        (
            ContractError::UnsupportedExtensionDefault { op: "o" },
            Capability,
        ),
    ]
}

#[test]
fn every_contract_error_value_has_the_expected_class() {
    let all = all_contract_errors();
    assert_eq!(all.len(), 15);
    for (err, class) in &all {
        assert_eq!(err.failure_class(), *class, "{err:?}");
    }
}

#[test]
fn every_completion_state_ex_value_has_the_expected_class() {
    let cases = [
        (CompletionStateEx::Pending, None),
        (CompletionStateEx::Complete, None),
        (CompletionStateEx::RuntimeError, Some(FailureClass::Runtime)),
        (CompletionStateEx::Lost, Some(FailureClass::DeviceLost)),
        (
            CompletionStateEx::Unknown,
            Some(FailureClass::CompletionUnknown),
        ),
    ];
    for (state, class) in cases {
        assert_eq!(state.failure_class(), class, "{state:?}");
    }
}

#[test]
fn all_five_classes_are_reachable_and_distinct() {
    let mut seen = Vec::new();
    for (err, _) in all_contract_errors() {
        let c = err.failure_class();
        if !seen.contains(&c) {
            seen.push(c);
        }
    }
    assert_eq!(seen.len(), 5);
}

#[test]
fn failure_class_stays_non_exhaustive() {
    fn label(c: FailureClass) -> &'static str {
        match c {
            FailureClass::Initialization => "Initialization",
            FailureClass::Runtime => "Runtime",
            FailureClass::DeviceLost => "DeviceLost",
            FailureClass::CompletionUnknown => "CompletionUnknown",
            FailureClass::Capability => "Capability",
            _ => "other",
        }
    }
    let labels: Vec<_> = [
        FailureClass::Initialization,
        FailureClass::Runtime,
        FailureClass::DeviceLost,
        FailureClass::CompletionUnknown,
        FailureClass::Capability,
    ]
    .into_iter()
    .map(label)
    .collect();
    assert_eq!(
        labels,
        [
            "Initialization",
            "Runtime",
            "DeviceLost",
            "CompletionUnknown",
            "Capability"
        ]
    );
}

/// `match` による判別も、入口と並んで使える（種類は入口と食い違わない）。
#[test]
fn match_based_discrimination_agrees_with_the_entry() {
    let lost = ContractError::DeviceLost { generation: 3 };
    let by_match = match &lost {
        ContractError::DeviceLost { .. } => Some(FailureClass::DeviceLost),
        _ => None,
    };
    assert_eq!(by_match, Some(lost.failure_class()));
    let ex = CompletionStateEx::Unknown;
    let by_match = match ex {
        CompletionStateEx::Unknown => Some(FailureClass::CompletionUnknown),
        _ => None,
    };
    assert_eq!(by_match, ex.failure_class());
}
