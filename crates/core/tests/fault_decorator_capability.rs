//! デコレータの能力不足と能力の取得の失敗の差し込みの確認（spec TASK-271 の 4/8。ERR-14・ERR-50 の入力）。

use std::cell::Cell;

use fandhe_silicon_core::capability::{
    Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::error::{CompletionStateEx, ContractError, UnsupportedReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");
const B: FeatureId = FeatureId("feature:B");

/// 問合せの回数を数える偽の内側。
#[derive(Default)]
struct Fake {
    query_calls: Cell<u32>,
    query_ex_calls: Cell<u32>,
}

/// `Capabilities::limits` の試験用の表。値は試験用の小さな数で、意味は持たない。
static TEST_LIMITS: fandhe_silicon_core::capability::LimitsTable =
    fandhe_silicon_core::capability::LimitsTable {
        max_buffer_bytes: 1,
        max_dispatch_dims: [1, 1, 1],
        max_texture_size: 1,
    };

impl Capabilities for Fake {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, f: FeatureId) -> CapabilityLevel {
        self.query_calls.set(self.query_calls.get() + 1);
        if f == A {
            CapabilityLevel::S2
        } else {
            CapabilityLevel::Unsupported
        }
    }
    fn query_ex(&self, f: FeatureId) -> CapabilityQuery {
        self.query_ex_calls.set(self.query_ex_calls.get() + 1);
        CapabilityQuery {
            level: if f == A {
                CapabilityLevel::S2
            } else {
                CapabilityLevel::Unsupported
            },
            unsupported_reason: None,
            evidence: Some(CapabilityEvidence::RuntimeApi),
        }
    }
}

fn insufficient() -> FaultScript {
    FaultScript::CapabilityInsufficient {
        feature: A,
        required: CapabilityLevel::S2,
    }
}

#[test]
fn insufficient_create_pipeline_returns_error_without_calling_inner() {
    let d = FaultInjecting::new(Fake::default(), insufficient());
    let called = Cell::new(false);
    let r: Result<u32, _> = d.create_pipeline_with(|_| {
        called.set(true);
        Ok(1)
    });
    assert_eq!(
        r,
        Err(ContractError::CapabilityInsufficient {
            feature: A,
            required: CapabilityLevel::S2,
            actual: CapabilityLevel::Unsupported,
        })
    );
    assert!(!called.get());
}

#[test]
fn insufficient_query_replaces_only_target_feature() {
    let d = FaultInjecting::new(Fake::default(), insufficient());
    assert_eq!(d.query(A), CapabilityLevel::Unsupported);
    let q = d.query_ex(A);
    assert_eq!(q.level, CapabilityLevel::Unsupported);
    assert_eq!(q.unsupported_reason, Some(UnsupportedReason::NotSupported));
    assert_eq!(q.evidence, None);
    assert_eq!(d.inner().query_calls.get(), 0);
    assert_eq!(d.inner().query_ex_calls.get(), 0);

    assert_eq!(d.query(B), CapabilityLevel::Unsupported);
    assert_eq!(d.inner().query_calls.get(), 1);
    let q = d.query_ex(B);
    assert_eq!(q.evidence, Some(CapabilityEvidence::RuntimeApi));
    assert_eq!(q.unsupported_reason, None);
    assert_eq!(d.inner().query_ex_calls.get(), 1);
}

#[test]
fn query_fails_returns_s0_not_unsupported() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::CapabilityQueryFails);
    for f in [A, B] {
        assert_eq!(d.query(f), CapabilityLevel::S0);
        assert_eq!(
            d.query_ex(f),
            CapabilityQuery {
                level: CapabilityLevel::S0,
                unsupported_reason: None,
                evidence: None,
            }
        );
    }
    assert_ne!(CapabilityLevel::S0, CapabilityLevel::Unsupported);
    assert_eq!(d.inner().query_calls.get(), 0);
    assert_eq!(d.inner().query_ex_calls.get(), 0);
}

#[test]
fn query_fails_does_not_replace_other_calls() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::CapabilityQueryFails);
    assert_eq!(d.create_pipeline_with(|_| Ok(7)), Ok(7));
    assert_eq!(d.wait_blocking_with(|_| Ok(())), Ok(()));
    assert_eq!(
        d.poll_ex_with(|_| CompletionStateEx::Complete),
        CompletionStateEx::Complete
    );
}

#[test]
fn capability_scripts_do_not_affect_completion() {
    let d = FaultInjecting::new(Fake::default(), insufficient());
    assert_eq!(d.wait_blocking_with(|_| Ok(())), Ok(()));
    assert_eq!(
        d.poll_ex_with(|_| CompletionStateEx::Complete),
        CompletionStateEx::Complete
    );
}

#[test]
fn pass_through_and_completion_unknown_delegate_capabilities() {
    for script in [FaultScript::PassThrough, FaultScript::CompletionUnknown] {
        let d = FaultInjecting::new(Fake::default(), script);
        assert_eq!(d.query(A), CapabilityLevel::S2);
        assert_eq!(d.query_ex(A).evidence, Some(CapabilityEvidence::RuntimeApi));
        assert_eq!(d.create_pipeline_with(|_| Ok(3)), Ok(3));
    }
}

#[test]
fn usable_as_dyn_capabilities() {
    let d = FaultInjecting::new(Fake::default(), insufficient());
    let c: &dyn Capabilities = &d;
    assert_eq!(c.query(A), CapabilityLevel::Unsupported);
    let d = FaultInjecting::new(Fake::default(), FaultScript::CapabilityQueryFails);
    let c: &dyn Capabilities = &d;
    assert_eq!(c.query(A), CapabilityLevel::S0);
}

#[test]
fn insufficient_is_distinct_from_other_failures() {
    let e = ContractError::CapabilityInsufficient {
        feature: A,
        required: CapabilityLevel::S2,
        actual: CapabilityLevel::Unsupported,
    };
    assert_ne!(e, ContractError::Timeout);
    assert_ne!(e, ContractError::DeviceUnavailable(String::new()));
    assert!(!matches!(e, ContractError::DeviceLost { .. }));
}
