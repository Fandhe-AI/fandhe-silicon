//! 筋書き `FaultScript::CapabilityOverride` の働きの確認（TASK-921）。指定した機能の `query`・`query_ex` だけが
//! 内側を呼ばずに渡した値になり、ほかの機能・差し込み・上限の働きは変わらないことを確かめる。

use std::cell::Cell;

use fandhe_silicon_core::capability::{
    Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, UnsupportedReason,
};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");
const B: FeatureId = FeatureId("feature:B");

/// 内側の上限の表。試験用の小さな数で、意味は持たない。
static INNER_LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1,
    max_dispatch_dims: [1, 1, 1],
    max_texture_size: 1,
};

/// 呼び出しを数え、差し替えの値と違う値を返す偽の内側。
#[derive(Default)]
struct Fake {
    query_calls: Cell<u32>,
    query_ex_calls: Cell<u32>,
}

impl Capabilities for Fake {
    fn limits(&self) -> &LimitsTable {
        &INNER_LIMITS
    }
    fn query(&self, _f: FeatureId) -> CapabilityLevel {
        self.query_calls.set(self.query_calls.get() + 1);
        CapabilityLevel::S2
    }
    fn query_ex(&self, _f: FeatureId) -> CapabilityQuery {
        self.query_ex_calls.set(self.query_ex_calls.get() + 1);
        inner_query_ex()
    }
}

fn inner_query_ex() -> CapabilityQuery {
    CapabilityQuery {
        level: CapabilityLevel::S2,
        unsupported_reason: None,
        evidence: Some(CapabilityEvidence::RuntimeApi),
    }
}

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

fn decorate(query: CapabilityQuery) -> FaultInjecting<Fake> {
    FaultInjecting::new(
        Fake::default(),
        FaultScript::CapabilityOverride { feature: A, query },
    )
}

#[test]
fn override_returns_the_given_query_for_the_target_key() {
    use CapabilityLevel::*;
    let ev = Some(CapabilityEvidence::RuntimeApi);
    let mut values = vec![
        q(S0, None, None),
        q(S1, None, ev),
        q(S2, None, None),
        q(S3, None, ev),
        q(S4, None, None),
        q(S5, None, ev),
        q(S6, None, None),
        q(Unsupported, None, None),
        q(Unsupported, Some(UnsupportedReason::NoApi), ev),
        q(Unsupported, Some(UnsupportedReason::NotSupported), None),
        q(Unsupported, Some(UnsupportedReason::UnknownFeature), ev),
    ];
    assert_eq!(values.len(), 11);
    for v in values.drain(..) {
        let d = decorate(v);
        assert_eq!(d.query_ex(A), v);
        assert_eq!(d.query(A), v.level);
        assert_eq!(d.inner().query_calls.get(), 0);
        assert_eq!(d.inner().query_ex_calls.get(), 0);
    }
}

#[test]
fn override_passes_other_keys_to_inner() {
    let d = decorate(q(CapabilityLevel::S6, None, None));
    assert_eq!(d.query(B), CapabilityLevel::S2);
    assert_eq!(d.query_ex(B), inner_query_ex());
    assert_eq!(d.inner().query_calls.get(), 1);
    assert_eq!(d.inner().query_ex_calls.get(), 1);
}

#[test]
fn override_passes_every_interception_to_inner() {
    for level in [CapabilityLevel::S6, CapabilityLevel::Unsupported] {
        let d = decorate(q(level, None, None));
        let n = Cell::new(0u32);
        assert_eq!(
            d.submit_with(|_| {
                n.set(n.get() + 1);
                Ok(1u32)
            }),
            Ok(1)
        );
        assert_eq!(
            d.allocate_with(64, |_, b| {
                n.set(n.get() + 1);
                Ok(b)
            }),
            Ok(64)
        );
        assert_eq!(
            d.wait_blocking_with(|_| {
                n.set(n.get() + 1);
                Ok(())
            }),
            Ok(())
        );
        assert_eq!(
            d.poll_ex_with(|_| {
                n.set(n.get() + 1);
                CompletionStateEx::Complete
            }),
            CompletionStateEx::Complete
        );
        assert_eq!(
            d.poll_with(|_| {
                n.set(n.get() + 1);
                CompletionState::Complete
            }),
            CompletionState::Complete
        );
        assert_eq!(
            d.create_pipeline_with(|_| {
                n.set(n.get() + 1);
                Ok::<u32, ContractError>(9)
            }),
            Ok(9)
        );
        assert_eq!(n.get(), 6);
        assert_eq!(d.submit_call_count(), 1);
        // 上限の働きは変わらない。
        assert_eq!(d.limits(), &INNER_LIMITS);
    }

    let replaced = LimitsTable {
        max_buffer_bytes: 5,
        max_dispatch_dims: [5, 5, 5],
        max_texture_size: 5,
    };
    let d = decorate(q(CapabilityLevel::S3, None, None)).with_limits_override(replaced);
    assert_eq!(d.limits(), &replaced);
    assert_eq!(
        d.limits_with::<LimitsTable, _>(|_| INNER_LIMITS).unwrap(),
        replaced
    );
}
