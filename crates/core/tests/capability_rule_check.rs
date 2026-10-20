//! 能力の問合せの戻り値の検査関数の部品ごとの確認（spec TASK-273 の 4/6。ERR-51・ERR-52 の入力）。

use std::sync::Arc;

use fandhe_silicon_core::capability::{
    Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::capability_rule::{
    CapabilityRuleViolation, CapabilityTable, StepLog, check_query, check_query_ex,
    level_requires_evidence,
};
use fandhe_silicon_core::error::UnsupportedReason;

const A: FeatureId = FeatureId("feature:A");

const LOW: [CapabilityLevel; 3] = [
    CapabilityLevel::S0,
    CapabilityLevel::S1,
    CapabilityLevel::S2,
];
const HIGH: [CapabilityLevel; 4] = [
    CapabilityLevel::S3,
    CapabilityLevel::S4,
    CapabilityLevel::S5,
    CapabilityLevel::S6,
];
const EVIDENCES: [CapabilityEvidence; 6] = [
    CapabilityEvidence::RuntimeApi,
    CapabilityEvidence::StaticVendorTable,
    CapabilityEvidence::ProbeCompile,
    CapabilityEvidence::ImplementationLedger,
    CapabilityEvidence::FieldRecord,
    CapabilityEvidence::PerformanceComparison,
];
const REASONS: [UnsupportedReason; 3] = [
    UnsupportedReason::NoApi,
    UnsupportedReason::NotSupported,
    UnsupportedReason::UnknownFeature,
];

fn q(
    level: CapabilityLevel,
    reason: Option<UnsupportedReason>,
    evidence: Option<CapabilityEvidence>,
) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: reason,
        evidence,
    }
}

#[test]
fn unsupported_with_any_reason_passes() {
    for r in REASONS {
        assert_eq!(
            check_query(A, q(CapabilityLevel::Unsupported, Some(r), None)),
            Ok(())
        );
    }
}

#[test]
fn unsupported_without_reason_is_violation() {
    let v = q(CapabilityLevel::Unsupported, None, None);
    assert_eq!(
        check_query(A, v),
        Err(vec![CapabilityRuleViolation::MissingUnsupportedReason {
            feature: A,
            query: v
        }])
    );
}

#[test]
fn reason_on_non_unsupported_is_violation() {
    for level in LOW {
        let v = q(level, Some(UnsupportedReason::NoApi), None);
        assert_eq!(
            check_query(A, v),
            Err(vec![CapabilityRuleViolation::UnexpectedUnsupportedReason {
                feature: A,
                query: v
            }])
        );
    }
    for level in HIGH {
        let v = q(
            level,
            Some(UnsupportedReason::NoApi),
            Some(CapabilityEvidence::RuntimeApi),
        );
        assert_eq!(
            check_query(A, v),
            Err(vec![CapabilityRuleViolation::UnexpectedUnsupportedReason {
                feature: A,
                query: v
            }])
        );
    }
}

#[test]
fn non_unsupported_without_reason_passes() {
    for level in LOW {
        assert_eq!(check_query(A, q(level, None, None)), Ok(()));
    }
}

#[test]
fn high_levels_without_evidence_are_violations() {
    for level in HIGH {
        let v = q(level, None, None);
        assert_eq!(
            check_query(A, v),
            Err(vec![CapabilityRuleViolation::MissingEvidence {
                feature: A,
                query: v
            }])
        );
    }
}

#[test]
fn high_levels_with_each_evidence_pass() {
    for level in HIGH {
        for e in EVIDENCES {
            assert_eq!(check_query(A, q(level, None, Some(e))), Ok(()));
        }
    }
}

#[test]
fn evidence_on_low_levels_is_accepted() {
    for level in LOW {
        for e in EVIDENCES {
            assert_eq!(check_query(A, q(level, None, Some(e))), Ok(()));
        }
    }
    for e in EVIDENCES {
        let v = q(
            CapabilityLevel::Unsupported,
            Some(UnsupportedReason::NotSupported),
            Some(e),
        );
        assert_eq!(check_query(A, v), Ok(()));
    }
}

#[test]
fn level_requires_evidence_table() {
    let all = [
        CapabilityLevel::S0,
        CapabilityLevel::S1,
        CapabilityLevel::S2,
        CapabilityLevel::S3,
        CapabilityLevel::S4,
        CapabilityLevel::S5,
        CapabilityLevel::S6,
        CapabilityLevel::Unsupported,
    ];
    assert_eq!(
        all.iter().filter(|l| level_requires_evidence(**l)).count(),
        4
    );
    assert_eq!(
        all.iter().filter(|l| !level_requires_evidence(**l)).count(),
        4
    );
    for l in HIGH {
        assert!(level_requires_evidence(l));
    }
    for l in LOW {
        assert!(!level_requires_evidence(l));
    }
    assert!(!level_requires_evidence(CapabilityLevel::Unsupported));
}

#[test]
fn multiple_violations_are_reported_in_order() {
    let v = q(CapabilityLevel::S3, Some(UnsupportedReason::NoApi), None);
    assert_eq!(
        check_query(A, v),
        Err(vec![
            CapabilityRuleViolation::UnexpectedUnsupportedReason {
                feature: A,
                query: v
            },
            CapabilityRuleViolation::MissingEvidence {
                feature: A,
                query: v
            },
        ])
    );
}

#[test]
fn violation_carries_feature_and_query() {
    let f = FeatureId("feature:other");
    let v = q(CapabilityLevel::Unsupported, None, None);
    assert_eq!(
        check_query(f, v),
        Err(vec![CapabilityRuleViolation::MissingUnsupportedReason {
            feature: f,
            query: v
        }])
    );
}

#[test]
fn check_query_ex_reads_device_once_and_returns_value() {
    let log = Arc::new(StepLog::new());
    let value = q(
        CapabilityLevel::S3,
        None,
        Some(CapabilityEvidence::FieldRecord),
    );
    let table = CapabilityTable::new().with(A, value).with_log(log.clone());
    assert_eq!(check_query_ex(&table, A), Ok(value));
    assert_eq!(log.read_count(), 1);
}

#[test]
fn check_query_ex_returns_violations_for_bad_value() {
    let bad = q(CapabilityLevel::S4, None, None);
    let table = CapabilityTable::new().with(A, bad);
    assert_eq!(
        check_query_ex(&table, A),
        Err(vec![CapabilityRuleViolation::MissingEvidence {
            feature: A,
            query: bad
        }])
    );
}

/// `query` だけを実装し、既定の `query_ex` を使うスタブ。
struct QueryOnly(CapabilityLevel);

impl Capabilities for QueryOnly {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        self.0
    }
}

#[test]
fn default_query_ex_on_unsupported_fails_missing_reason() {
    let dev = QueryOnly(CapabilityLevel::Unsupported);
    let default_value = q(CapabilityLevel::Unsupported, None, None);
    assert_eq!(
        check_query_ex(&dev, A),
        Err(vec![CapabilityRuleViolation::MissingUnsupportedReason {
            feature: A,
            query: default_value
        }])
    );
}

#[test]
fn check_query_ex_accepts_dyn_capabilities() {
    let table = CapabilityTable::new();
    let dev: &dyn Capabilities = &table;
    assert_eq!(
        check_query_ex(dev, A),
        Ok(q(CapabilityLevel::S0, None, None))
    );
}
