//! WebGPU の口に当たる 2 行の能力の問合せの値と、共通の検査（`check_webgpu_ports`）の確認（TASK-155）。
//!
//! DEV-35 は device を取り出す口、DEV-36 はハンドルからテクスチャを作る口の鍵。
//! 2 つの鍵に `Unsupported`（理由 `NoApi`・根拠なし）を求める規則を `check_webgpu_ports` が確かめる。

use fandhe_silicon_core::capability::{
    Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
    WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT, WEBGPU_PORT_UNSUPPORTED,
};
use fandhe_silicon_core::capability_rule::{
    CapabilityRuleViolation, CapabilityTable, check_query, check_query_ex, check_webgpu_ports,
};
use fandhe_silicon_core::error::UnsupportedReason;

fn decided_table() -> CapabilityTable {
    CapabilityTable::new()
        .with(WEBGPU_DEVICE_PORT, WEBGPU_PORT_UNSUPPORTED)
        .with(WEBGPU_HANDLE_PORT, WEBGPU_PORT_UNSUPPORTED)
}

/// 違反した鍵の一覧を取り出す。
fn mismatched(r: Result<(), Vec<CapabilityRuleViolation>>) -> Vec<FeatureId> {
    r.unwrap_err()
        .into_iter()
        .map(|v| match v {
            CapabilityRuleViolation::WebgpuPortMismatch { feature, .. } => feature,
            other => panic!("unexpected violation: {other:?}"),
        })
        .collect()
}

#[test]
fn dev35_device_port_key_and_value() {
    assert_eq!(WEBGPU_DEVICE_PORT, FeatureId("webgpu-device-port"));
    let t = decided_table();
    assert_eq!(t.query(WEBGPU_DEVICE_PORT), CapabilityLevel::Unsupported);
    assert_eq!(
        t.query_ex(WEBGPU_DEVICE_PORT),
        CapabilityQuery {
            level: CapabilityLevel::Unsupported,
            unsupported_reason: Some(UnsupportedReason::NoApi),
            evidence: None,
        }
    );
}

#[test]
fn dev36_handle_port_key_and_value() {
    assert_eq!(WEBGPU_HANDLE_PORT, FeatureId("webgpu-handle-port"));
    let t = decided_table();
    assert_eq!(t.query(WEBGPU_HANDLE_PORT), CapabilityLevel::Unsupported);
    assert_eq!(t.query_ex(WEBGPU_HANDLE_PORT), WEBGPU_PORT_UNSUPPORTED);
}

#[test]
fn dev35_value_passes_the_common_capability_rules() {
    assert!(check_query(WEBGPU_DEVICE_PORT, WEBGPU_PORT_UNSUPPORTED).is_ok());
    assert_eq!(
        check_query_ex(&decided_table(), WEBGPU_DEVICE_PORT),
        Ok(WEBGPU_PORT_UNSUPPORTED)
    );
}

#[test]
fn dev36_check_accepts_the_decided_table_through_dyn() {
    let t = decided_table();
    let d: &dyn Capabilities = &t;
    assert!(check_webgpu_ports(d).is_ok());
}

#[test]
fn dev35_check_rejects_a_missing_row_that_returns_s0() {
    let t = CapabilityTable::new().with(WEBGPU_HANDLE_PORT, WEBGPU_PORT_UNSUPPORTED);
    assert_eq!(t.query(WEBGPU_DEVICE_PORT), CapabilityLevel::S0);
    assert_eq!(mismatched(check_webgpu_ports(&t)), [WEBGPU_DEVICE_PORT]);
}

#[test]
fn dev36_check_rejects_an_unknown_feature_reason() {
    let unknown = CapabilityQuery {
        unsupported_reason: Some(UnsupportedReason::UnknownFeature),
        ..WEBGPU_PORT_UNSUPPORTED
    };
    let t = CapabilityTable::new()
        .with(WEBGPU_DEVICE_PORT, WEBGPU_PORT_UNSUPPORTED)
        .with(WEBGPU_HANDLE_PORT, unknown);
    assert_eq!(mismatched(check_webgpu_ports(&t)), [WEBGPU_HANDLE_PORT]);
}

#[test]
fn dev35_check_rejects_an_evidence_and_collects_both_rows() {
    let with_evidence = CapabilityQuery {
        evidence: Some(CapabilityEvidence::RuntimeApi),
        ..WEBGPU_PORT_UNSUPPORTED
    };
    let t = CapabilityTable::new()
        .with(WEBGPU_DEVICE_PORT, with_evidence)
        .with(WEBGPU_HANDLE_PORT, with_evidence);
    assert_eq!(
        mismatched(check_webgpu_ports(&t)),
        [WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT]
    );
}

/// `query` と `query_ex` の段階が食い違う実装。
struct Disagree;

impl Capabilities for Disagree {
    fn query(&self, _: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S1
    }
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        unreachable!("limits is not read by the check")
    }
    fn query_ex(&self, _: FeatureId) -> CapabilityQuery {
        WEBGPU_PORT_UNSUPPORTED
    }
}

#[test]
fn dev36_check_rejects_query_and_query_ex_disagreeing() {
    let violations = check_webgpu_ports(&Disagree).unwrap_err();
    assert_eq!(
        violations,
        [WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT].map(|feature| {
            CapabilityRuleViolation::WebgpuPortMismatch {
                feature,
                level: CapabilityLevel::S1,
                query: WEBGPU_PORT_UNSUPPORTED,
            }
        })
    );
}

/// `query` だけ `Unsupported` を返し、`query_ex` は既定の実装のままの実装。
struct QueryOnly;

impl Capabilities for QueryOnly {
    fn query(&self, _: FeatureId) -> CapabilityLevel {
        CapabilityLevel::Unsupported
    }
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        unreachable!("limits is not read by the check")
    }
}

#[test]
fn dev35_check_rejects_query_only_implementation_without_reason() {
    assert_eq!(
        mismatched(check_webgpu_ports(&QueryOnly)),
        [WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT]
    );
}
