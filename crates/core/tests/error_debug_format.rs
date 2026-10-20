//! `ContractError` の `{:?}` の書式と `ValidationFailed(String)` の定義を、別の crate から照合する公開テスト（TASK-269 の 6/8）。
//!
//! 仕組み: 全 15 値の `{:?}` の出力に、境界つきの値の名前の前置と、各フィールドの
//! `名前: 値の {:?}` の部分文字列があることを見る（ERR-54。ポインタのみ）。派生の `Debug` の
//! 出力全体の文字列は Rust が安定を約束しないので、全体の一致は照合しない。
//! `ValidationFailed` は `fn(String) -> ContractError` への強制で形を固定し、同じ文字列の
//! ほかの値と区別できることを見る（ERR-8。ポインタのみ）。中身の読み戻しは 5/8
//! （`error_fields.rs`）が持つので重ねない。
//!
//! 対象外: `Display`・`Error` と wasm32（7/8）、`capability` モジュールの 3 型の拡張性
//! （TASK-136 の持ち分。ポインタのみ）。

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::error::{ContractError, IdentityMismatchReason};

const MULTILINE: &str = "line one\nline two: 失敗 \u{1F980}\n";

/// 値から、期待する名前と、出力に含まれるべき部分文字列の列を写す。
fn expected(v: &ContractError) -> (&'static str, Vec<String>) {
    match v {
        ContractError::InvalidCombination(s) => ("InvalidCombination", vec![format!("{s:?}")]),
        ContractError::DeviceUnavailable(s) => ("DeviceUnavailable", vec![format!("{s:?}")]),
        ContractError::InvalidArgument(s) => ("InvalidArgument", vec![format!("{s:?}")]),
        ContractError::ValidationFailed(s) => ("ValidationFailed", vec![format!("{s:?}")]),
        ContractError::DeviceMismatch => ("DeviceMismatch", vec![]),
        ContractError::Timeout => ("Timeout", vec![]),
        ContractError::DeviceLost { generation } => {
            ("DeviceLost", vec![format!("generation: {generation:?}")])
        }
        ContractError::AllocationFailed { requested_bytes } => (
            "AllocationFailed",
            vec![format!("requested_bytes: {requested_bytes:?}")],
        ),
        ContractError::ArtifactRejected { kind, reason } => (
            "ArtifactRejected",
            vec![format!("kind: {kind:?}"), format!("reason: {reason:?}")],
        ),
        ContractError::Unsupported { op, backend } => (
            "Unsupported",
            vec![format!("op: {op:?}"), format!("backend: {backend:?}")],
        ),
        ContractError::CapabilityInsufficient {
            feature,
            required,
            actual,
        } => (
            "CapabilityInsufficient",
            vec![
                format!("feature: {feature:?}"),
                format!("required: {required:?}"),
                format!("actual: {actual:?}"),
            ],
        ),
        ContractError::DeviceIdentityMismatch { reason } => (
            "DeviceIdentityMismatch",
            vec![format!("reason: {reason:?}")],
        ),
        ContractError::UnsupportedExtensionDefault { op } => {
            ("UnsupportedExtensionDefault", vec![format!("op: {op:?}")])
        }
        ContractError::BackendFailure {
            op,
            backend,
            detail,
        } => (
            "BackendFailure",
            vec![
                format!("op: {op:?}"),
                format!("backend: {backend:?}"),
                format!("detail: {detail:?}"),
            ],
        ),
        ContractError::CapacityExceeded {
            op,
            backend,
            capacity,
        } => (
            "CapacityExceeded",
            vec![
                format!("op: {op:?}"),
                format!("backend: {backend:?}"),
                format!("capacity: {capacity:?}"),
            ],
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
            backend: BackendKind::Dx12Compute,
            detail: MULTILINE.to_string(),
        },
        ContractError::CapacityExceeded {
            op: "test-op",
            backend: BackendKind::Cuda,
            capacity: u32::MAX,
        },
    ]
}

#[test]
fn all_values_count_is_fixed() {
    assert_eq!(all_values().len(), 15);
}

#[test]
fn debug_output_has_name_prefix_and_fields() {
    for v in all_values() {
        let s = format!("{v:?}");
        let (name, parts) = expected(&v);
        let rest = s
            .strip_prefix(name)
            .unwrap_or_else(|| panic!("name {name} is not a prefix of {s:?}"));
        // 名前の直後が区切りであること（`Unsupported` が `UnsupportedExtensionDefault` に合格しない）。
        assert!(
            rest.is_empty() || rest.starts_with('(') || rest.starts_with(" {"),
            "name {name} is not a whole word in {s:?}"
        );
        for p in parts {
            assert!(s.contains(&p), "{p:?} not found in {s:?}");
        }
    }
}

#[test]
fn debug_output_names_nested_types() {
    let s = format!(
        "{:?}",
        ContractError::Unsupported {
            op: "o",
            backend: BackendKind::Vulkan,
        }
    );
    assert!(s.contains("Vulkan"), "{s:?}");
    let s = format!(
        "{:?}",
        ContractError::DeviceIdentityMismatch {
            reason: IdentityMismatchReason::Undeterminable,
        }
    );
    assert!(s.contains("Undeterminable"), "{s:?}");
}

#[test]
fn unit_values_debug_to_their_names() {
    assert_eq!(
        format!("{:?}", ContractError::DeviceMismatch),
        "DeviceMismatch"
    );
    assert_eq!(format!("{:?}", ContractError::Timeout), "Timeout");
}

#[test]
fn validation_failed_is_a_string_constructor() {
    // 構築子の型を固定する。フィールドが `String` 1 つでなくなるとコンパイルが落ちる。
    let f: fn(String) -> ContractError = ContractError::ValidationFailed;
    let v = f("x".to_string());
    assert!(matches!(v, ContractError::ValidationFailed(_)));
    assert!(format!("{v:?}").starts_with("ValidationFailed("));
}

#[test]
fn validation_failed_is_distinct_from_other_string_values() {
    let v = ContractError::ValidationFailed("x".to_string());
    assert_ne!(v, ContractError::InvalidArgument("x".to_string()));
    assert_ne!(v, ContractError::InvalidCombination("x".to_string()));
    assert_ne!(v, ContractError::DeviceUnavailable("x".to_string()));
    assert_eq!(v, ContractError::ValidationFailed("x".to_string()));
}
