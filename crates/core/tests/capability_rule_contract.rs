//! 契約テストの組み合わせ（spec TASK-273 の 3/6。ERR-50・ERR-62 のポインタ）。
//!
//! 2/6 の部品（`CapabilityTable`・`StepLog`・`query_after_acquire`）と、デコレータ
//! （`FaultInjecting`・`FaultScript`）を組み合わせ、能力の取得の失敗（`S0`）と確認済みの非対応
//! （`Unsupported`）の区別、および「列挙と取得が成功してから読む」順を、段ごと・筋書きごとに確かめる。

use std::sync::Arc;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::capability_rule::{CapabilityTable, Step, StepLog, query_after_acquire};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");
const B: FeatureId = FeatureId("feature:B");
const UNLISTED: FeatureId = FeatureId("feature:unlisted");

fn unsupported() -> CapabilityQuery {
    CapabilityQuery {
        level: CapabilityLevel::Unsupported,
        unsupported_reason: Some(UnsupportedReason::NotSupported),
        evidence: None,
    }
}

fn s2() -> CapabilityQuery {
    CapabilityQuery {
        level: CapabilityLevel::S2,
        unsupported_reason: None,
        evidence: Some(CapabilityEvidence::RuntimeApi),
    }
}

/// A は確認済みの非対応、B は S2、それ以外は既定の S0 を返す表。
fn table(log: &Arc<StepLog>) -> CapabilityTable {
    CapabilityTable::new()
        .with(A, unsupported())
        .with(B, s2())
        .with_log(log.clone())
}

fn fail_variants() -> Vec<ContractError> {
    vec![
        ContractError::DeviceUnavailable("test".to_string()),
        ContractError::DeviceLost { generation: 3 },
        ContractError::Timeout,
        ContractError::BackendFailure {
            op: "test",
            backend: BackendKind::Metal,
            detail: "test".to_string(),
        },
    ]
}

/// 2 段の手順を記録つきで流す。`enumerate_err` が `Some` なら列挙が失敗する。
fn run<D: Capabilities>(
    log: &Arc<StepLog>,
    enumerate_err: Option<ContractError>,
    acquire: impl FnOnce() -> Result<D, ContractError>,
    feature: FeatureId,
) -> CapabilityLevel {
    query_after_acquire(
        || {
            log.push(Step::Enumerate);
            enumerate_err.map_or(Ok(()), Err)
        },
        || {
            log.push(Step::RequestDevice);
            acquire()
        },
        feature,
    )
}

#[test]
fn enumerate_failure_yields_s0_for_every_error_and_feature() {
    for err in fail_variants() {
        for feature in [A, B, UNLISTED] {
            let log = Arc::new(StepLog::new());
            let r = run(&log, Some(err.clone()), || Ok(table(&log)), feature);
            assert_eq!(r, CapabilityLevel::S0, "{err:?} {feature:?}");
            assert_eq!(log.steps(), vec![Step::Enumerate]);
        }
    }
}

#[test]
fn acquire_failure_yields_s0_for_every_error_and_feature() {
    for err in fail_variants() {
        for feature in [A, B, UNLISTED] {
            let log = Arc::new(StepLog::new());
            let r = run::<CapabilityTable>(&log, None, || Err(err.clone()), feature);
            assert_eq!(r, CapabilityLevel::S0, "{err:?} {feature:?}");
            assert_eq!(log.steps(), vec![Step::Enumerate, Step::RequestDevice]);
            assert_eq!(log.read_count(), 0);
        }
    }
}

#[test]
fn success_returns_inner_value_in_order() {
    for (feature, expected) in [
        (A, CapabilityLevel::Unsupported),
        (B, CapabilityLevel::S2),
        (UNLISTED, CapabilityLevel::S0),
    ] {
        let log = Arc::new(StepLog::new());
        let r = run(&log, None, || Ok(table(&log)), feature);
        assert_eq!(r, expected);
        assert_eq!(
            log.steps(),
            vec![
                Step::Enumerate,
                Step::RequestDevice,
                Step::ReadFeature(feature)
            ]
        );
    }
}

#[test]
fn failed_acquisition_s0_is_not_confirmed_unsupported() {
    let log = Arc::new(StepLog::new());
    let failed = run::<CapabilityTable>(&log, None, || Err(ContractError::Timeout), A);
    let log2 = Arc::new(StepLog::new());
    let confirmed = run(&log2, None, || Ok(table(&log2)), A);
    assert_eq!(failed, CapabilityLevel::S0);
    assert_eq!(confirmed, CapabilityLevel::Unsupported);
    assert_ne!(failed, confirmed);
}

#[test]
fn decorated_device_with_query_failure_yields_s0_without_reading_inner() {
    let log = Arc::new(StepLog::new());
    let r = run(
        &log,
        None,
        || {
            Ok(FaultInjecting::new(
                table(&log),
                FaultScript::CapabilityQueryFails,
            ))
        },
        A,
    );
    assert_eq!(r, CapabilityLevel::S0);
    assert_eq!(log.read_count(), 0);
    assert_eq!(log.steps(), vec![Step::Enumerate, Step::RequestDevice]);
}

#[test]
fn decorated_pass_through_differs_from_query_failure() {
    let log = Arc::new(StepLog::new());
    let passed = run(
        &log,
        None,
        || Ok(FaultInjecting::new(table(&log), FaultScript::PassThrough)),
        A,
    );
    assert_eq!(passed, CapabilityLevel::Unsupported);
    assert_eq!(log.read_count(), 1);

    let log2 = Arc::new(StepLog::new());
    let failed = run(
        &log2,
        None,
        || {
            Ok(FaultInjecting::new(
                table(&log2),
                FaultScript::CapabilityQueryFails,
            ))
        },
        A,
    );
    assert_ne!(passed, failed);
}

#[test]
fn decorated_capability_insufficient_overrides_only_target_feature() {
    let script = || FaultScript::CapabilityInsufficient {
        feature: A,
        required: CapabilityLevel::S2,
    };

    let log = Arc::new(StepLog::new());
    let ra = run(
        &log,
        None,
        || Ok(FaultInjecting::new(table(&log), script())),
        A,
    );
    assert_eq!(ra, CapabilityLevel::Unsupported);
    assert_eq!(log.read_count(), 0);

    let log = Arc::new(StepLog::new());
    let rb = run(
        &log,
        None,
        || Ok(FaultInjecting::new(table(&log), script())),
        B,
    );
    assert_eq!(rb, CapabilityLevel::S2);
    assert_eq!(
        log.steps(),
        vec![Step::Enumerate, Step::RequestDevice, Step::ReadFeature(B)]
    );
}

#[test]
fn query_ex_separates_failed_query_from_confirmed_unsupported() {
    let log = Arc::new(StepLog::new());
    let plain = table(&log).query_ex(A);
    let failed = FaultInjecting::new(table(&log), FaultScript::CapabilityQueryFails).query_ex(A);

    assert_eq!(plain.level, CapabilityLevel::Unsupported);
    assert_eq!(
        plain.unsupported_reason,
        Some(UnsupportedReason::NotSupported)
    );
    assert_eq!(failed.level, CapabilityLevel::S0);
    assert_eq!(failed.unsupported_reason, None);
    assert_eq!(failed.evidence, None);
    assert_ne!(plain.level, failed.level);
    assert_ne!(plain, failed);
}

/// 製品側に包括の実装は無いので、`dyn Capabilities` を持つ試験用の入れ物で 2 段の手順に渡す。
struct DynDevice(Box<dyn Capabilities + Send + Sync>);

impl Capabilities for DynDevice {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        self.0.query(feature)
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        self.0.query_ex(feature)
    }
}

#[test]
fn dyn_device_keeps_unsupported_and_s0_distinct() {
    let log = Arc::new(StepLog::new());
    let plain = run(&log, None, || Ok(DynDevice(Box::new(table(&log)))), A);
    assert_eq!(plain, CapabilityLevel::Unsupported);

    let log2 = Arc::new(StepLog::new());
    let failed = run(
        &log2,
        None,
        || {
            Ok(DynDevice(Box::new(FaultInjecting::new(
                table(&log2),
                FaultScript::CapabilityQueryFails,
            ))))
        },
        A,
    );
    assert_eq!(failed, CapabilityLevel::S0);
    assert_ne!(plain, failed);
}
