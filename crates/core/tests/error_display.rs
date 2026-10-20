//! `ContractError` の `Display` と `std::error::Error` を、別の crate から照合する公開テスト（TASK-269 の 7/8）。
//!
//! 仕組み: 全 15 値の `to_string()` が、値ごとの英語の決まった句で始まり、フィールドが
//! そのまま（エスケープなしで）載ることを部分文字列で見る（ERR-54。ポインタのみ）。
//! 文言の全体は契約に固定しない。`source()` が `None` であること、`dyn Error` と
//! `Box<dyn Error + Send + Sync>` への変換ができることも見る。`{:?}` の書式は 6/8
//! （`error_debug_format.rs`）、フィールドの読み戻しは 5/8（`error_fields.rs`）が持つので重ねない。
//!
//! 対象外: `CompletionStateEx`・`DeviceLostReason` の `Display`、`capability` モジュールの
//! 3 型（TASK-136 の持ち分。ポインタのみ）。

use std::error::Error;

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::error::{ContractError, IdentityMismatchReason};

const MULTILINE: &str = "line one\nline two: 失敗 \u{1F980}\n";

/// 値から、期待する前置と、出力に含まれるべき部分文字列の列を写す。
fn expected(v: &ContractError) -> (String, Vec<String>) {
    match v {
        ContractError::InvalidCombination(s) => ("invalid combination".into(), vec![s.clone()]),
        ContractError::DeviceUnavailable(s) => ("device unavailable".into(), vec![s.clone()]),
        ContractError::DeviceLost { generation } => {
            ("device lost".into(), vec![generation.to_string()])
        }
        ContractError::DeviceMismatch => ("handle belongs to a different device".into(), vec![]),
        ContractError::InvalidArgument(s) => ("invalid argument".into(), vec![s.clone()]),
        ContractError::AllocationFailed { requested_bytes } => (
            "allocation failed".into(),
            vec![requested_bytes.to_string()],
        ),
        ContractError::ArtifactRejected { kind, reason } => (
            "artifact rejected".into(),
            vec![kind.to_string(), reason.clone()],
        ),
        ContractError::ValidationFailed(s) => ("validation failed".into(), vec![s.clone()]),
        ContractError::Timeout => ("timed out".into(), vec![]),
        ContractError::Unsupported { op, backend } => (
            "unsupported".into(),
            vec![op.to_string(), format!("{backend:?}")],
        ),
        ContractError::CapabilityInsufficient {
            feature,
            required,
            actual,
        } => (
            "capability insufficient".into(),
            vec![
                feature.0.to_string(),
                format!("{required:?}"),
                format!("{actual:?}"),
            ],
        ),
        ContractError::DeviceIdentityMismatch { reason } => (
            "physical device identity mismatch".into(),
            vec![format!("{reason:?}")],
        ),
        ContractError::UnsupportedExtensionDefault { op } => (
            "extension operation not implemented".into(),
            vec![op.to_string()],
        ),
        ContractError::BackendFailure {
            op,
            backend,
            detail,
        } => (
            format!("{backend:?} call failed"),
            vec![op.to_string(), detail.clone()],
        ),
        ContractError::CapacityExceeded {
            op,
            backend,
            capacity,
        } => (
            "capacity exceeded".into(),
            vec![op.to_string(), format!("{backend:?}"), capacity.to_string()],
        ),
        other => panic!("value not covered by this test: {other:?}"),
    }
}

/// 全 15 値（文字列の欄には改行と非 ASCII を含む値も使う）。値を足したらここと `expected` を直す。
fn all_values() -> Vec<ContractError> {
    vec![
        ContractError::InvalidCombination(MULTILINE.to_string()),
        ContractError::DeviceUnavailable("unavailable".to_string()),
        ContractError::DeviceLost {
            generation: u64::MAX,
        },
        ContractError::DeviceMismatch,
        ContractError::InvalidArgument("bad \"arg\"".to_string()),
        ContractError::AllocationFailed {
            requested_bytes: 4096,
        },
        ContractError::ArtifactRejected {
            kind: "test-kind",
            reason: MULTILINE.to_string(),
        },
        ContractError::ValidationFailed(MULTILINE.to_string()),
        ContractError::Timeout,
        ContractError::Unsupported {
            op: "test-op",
            backend: BackendKind::Vulkan,
        },
        ContractError::CapabilityInsufficient {
            feature: FeatureId("test-feature"),
            required: CapabilityLevel::S3,
            actual: CapabilityLevel::Unsupported,
        },
        ContractError::DeviceIdentityMismatch {
            reason: IdentityMismatchReason::Undeterminable,
        },
        ContractError::UnsupportedExtensionDefault { op: "test-op" },
        ContractError::BackendFailure {
            op: "test-op",
            backend: BackendKind::Cuda,
            detail: MULTILINE.to_string(),
        },
        ContractError::CapacityExceeded {
            op: "test-op",
            backend: BackendKind::Dx12Compute,
            capacity: u32::MAX,
        },
    ]
}

#[test]
fn all_values_count_is_fixed() {
    assert_eq!(all_values().len(), 15);
}

#[test]
fn display_starts_with_phrase_and_is_not_empty() {
    for v in all_values() {
        let s = v.to_string();
        let (prefix, _) = expected(&v);
        assert!(!s.is_empty(), "{v:?}");
        assert!(
            s.starts_with(&prefix),
            "{prefix:?} is not a prefix of {s:?}"
        );
    }
}

#[test]
fn display_carries_fields_verbatim() {
    for v in all_values() {
        let s = v.to_string();
        let (_, parts) = expected(&v);
        for p in parts {
            assert!(s.contains(&p), "{p:?} not found in {s:?}");
        }
    }
}

#[test]
fn display_does_not_escape_string_fields() {
    let v = ContractError::ValidationFailed(MULTILINE.to_string());
    let s = v.to_string();
    assert!(s.contains('\n'), "{s:?}");
    assert!(s.contains("失敗 \u{1F980}"), "{s:?}");
    assert!(!s.contains("\\n"), "{s:?}");
    // `{:?}` の形とは違う。
    assert_ne!(s, format!("{v:?}"));
}

#[test]
fn feature_id_is_shown_as_plain_name() {
    let s = ContractError::CapabilityInsufficient {
        feature: FeatureId("x"),
        required: CapabilityLevel::S3,
        actual: CapabilityLevel::Unsupported,
    }
    .to_string();
    assert!(!s.contains("FeatureId"), "{s:?}");
}

#[test]
fn display_distinguishes_values_with_same_string() {
    let s = "same".to_string();
    let all = [
        ContractError::InvalidArgument(s.clone()).to_string(),
        ContractError::ValidationFailed(s.clone()).to_string(),
        ContractError::InvalidCombination(s.clone()).to_string(),
        ContractError::DeviceUnavailable(s).to_string(),
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j]);
        }
    }
}

#[test]
fn source_is_none_for_all_values() {
    for v in all_values() {
        assert!(v.source().is_none(), "{v:?}");
    }
}

fn assert_error<E: Error + Send + Sync + 'static>() {}

fn fails() -> Result<(), ContractError> {
    Err(ContractError::Timeout)
}

fn propagates() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    fails()?;
    Ok(())
}

#[test]
fn coerces_to_dyn_error() {
    assert_error::<ContractError>();
    let e = ContractError::DeviceMismatch;
    let d: &dyn Error = &e;
    assert_eq!(d.to_string(), e.to_string());
    let boxed = propagates().unwrap_err();
    assert_eq!(boxed.to_string(), ContractError::Timeout.to_string());
}

#[test]
fn format_macro_matches_to_string() {
    for v in all_values() {
        assert_eq!(format!("{v}"), v.to_string());
    }
}
