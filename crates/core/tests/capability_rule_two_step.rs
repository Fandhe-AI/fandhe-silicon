//! 2 段の手順の確認（spec TASK-273 の 2/6。ERR-50・ERR-62 の入口）。

use std::sync::Arc;

use fandhe_silicon_core::capability::{Capabilities, CapabilityLevel, CapabilityQuery, FeatureId};
use fandhe_silicon_core::capability_rule::{CapabilityTable, Step, StepLog, query_after_acquire};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");

fn table(log: &Arc<StepLog>) -> CapabilityTable {
    CapabilityTable::new()
        .with(
            A,
            CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: Some(UnsupportedReason::NotSupported),
                evidence: None,
            },
        )
        .with_log(log.clone())
}

fn fail() -> ContractError {
    ContractError::DeviceUnavailable("test".to_string())
}

#[test]
fn enumerate_failure_returns_s0_and_stops() {
    let log = Arc::new(StepLog::new());
    let l = &log;
    let r = query_after_acquire(
        || {
            l.push(Step::Enumerate);
            Err(fail())
        },
        || {
            l.push(Step::RequestDevice);
            Ok(table(l))
        },
        A,
    );
    assert_eq!(r, CapabilityLevel::S0);
    assert_eq!(log.steps(), vec![Step::Enumerate]);
}

#[test]
fn acquire_failure_returns_s0_without_reading() {
    let log = Arc::new(StepLog::new());
    let l = &log;
    let r = query_after_acquire::<CapabilityTable, _, _>(
        || {
            l.push(Step::Enumerate);
            Ok(())
        },
        || {
            l.push(Step::RequestDevice);
            Err(fail())
        },
        A,
    );
    assert_eq!(r, CapabilityLevel::S0);
    assert_eq!(log.steps(), vec![Step::Enumerate, Step::RequestDevice]);
    assert_eq!(log.read_count(), 0);
}

#[test]
fn success_reads_value_after_both_steps() {
    let log = Arc::new(StepLog::new());
    let l = &log;
    let r = query_after_acquire(
        || {
            l.push(Step::Enumerate);
            Ok(())
        },
        || {
            l.push(Step::RequestDevice);
            Ok(table(l))
        },
        A,
    );
    assert_eq!(r, CapabilityLevel::Unsupported);
    assert_eq!(
        log.steps(),
        vec![Step::Enumerate, Step::RequestDevice, Step::ReadFeature(A)]
    );
}

#[test]
fn confirmed_unsupported_differs_from_failed_query_via_decorator() {
    let log = Arc::new(StepLog::new());
    let plain = table(&log);
    assert_eq!(plain.query(A), CapabilityLevel::Unsupported);
    assert_eq!(log.read_count(), 1);

    let log2 = Arc::new(StepLog::new());
    let d = FaultInjecting::new(table(&log2), FaultScript::CapabilityQueryFails);
    let failed = d.query(A);
    assert_eq!(failed, CapabilityLevel::S0);
    assert_ne!(failed, plain.query(A));
    assert_eq!(log2.read_count(), 0);
}
