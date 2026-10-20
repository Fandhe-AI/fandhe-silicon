//! 表のスタブと呼び出しの記録の確認（spec TASK-273 の 2/6。ERR-50・ERR-62 の入力）。

use std::sync::Arc;

use fandhe_silicon_core::capability::{
    Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::capability_rule::{CapabilityTable, Step, StepLog};
use fandhe_silicon_core::error::UnsupportedReason;

const A: FeatureId = FeatureId("feature:A");
const B: FeatureId = FeatureId("feature:B");
const C: FeatureId = FeatureId("feature:C");

fn unsupported() -> CapabilityQuery {
    CapabilityQuery {
        level: CapabilityLevel::Unsupported,
        unsupported_reason: Some(UnsupportedReason::NotSupported),
        evidence: Some(CapabilityEvidence::RuntimeApi),
    }
}

fn s0() -> CapabilityQuery {
    CapabilityQuery {
        level: CapabilityLevel::S0,
        unsupported_reason: None,
        evidence: None,
    }
}

#[test]
fn table_returns_registered_values_and_distinguishes_s0_from_unsupported() {
    let t = CapabilityTable::new().with(A, unsupported()).with(B, s0());
    assert_eq!(t.query(A), CapabilityLevel::Unsupported);
    assert_eq!(t.query(B), CapabilityLevel::S0);
    assert_ne!(t.query(A), t.query(B));
    assert_eq!(t.query_ex(A), unsupported());
    assert_eq!(t.query_ex(B), s0());
}

#[test]
fn table_unknown_feature_uses_default() {
    assert_eq!(CapabilityTable::new().query_ex(C), s0());
    let t = CapabilityTable::new().with_default(unsupported());
    assert_eq!(t.query_ex(C), unsupported());
}

#[test]
fn later_entry_wins_for_same_feature() {
    let t = CapabilityTable::new().with(A, s0()).with(A, unsupported());
    assert_eq!(t.query(A), CapabilityLevel::Unsupported);
}

#[test]
fn log_records_each_read_in_order() {
    let log = Arc::new(StepLog::new());
    let t = CapabilityTable::new().with_log(log.clone());
    t.query(A);
    t.query_ex(B);
    assert_eq!(
        log.steps(),
        vec![Step::ReadFeature(A), Step::ReadFeature(B)]
    );
    assert_eq!(log.read_count(), 2);
    log.push(Step::Enumerate);
    assert_eq!(log.read_count(), 2);
}

#[test]
fn types_are_send_sync_and_dyn_usable() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<StepLog>();
    assert_send_sync::<CapabilityTable>();
    let t = CapabilityTable::new().with(A, unsupported());
    let d: &dyn Capabilities = &t;
    assert_eq!(d.query(A), CapabilityLevel::Unsupported);
}
