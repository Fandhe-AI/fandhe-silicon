//! デコレータの模擬の実行時エラーの差し込みの確認（spec TASK-271 の 6/8 の (7)。実際の故障は起こさない）。

use std::cell::Cell;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, ContractError};
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");

#[derive(Default)]
struct Fake {
    calls: Cell<u32>,
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

    fn query(&self, _: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S2
    }
    fn query_ex(&self, _: FeatureId) -> CapabilityQuery {
        CapabilityQuery {
            level: CapabilityLevel::S2,
            unsupported_reason: None,
            evidence: None,
        }
    }
}
impl FaultHooks for Fake {}

fn rt(backend: BackendKind, detail: &str) -> FaultScript {
    FaultScript::RuntimeError {
        backend,
        detail: detail.to_string(),
    }
}

#[test]
fn wait_blocking_returns_backend_failure_without_calling_inner() {
    for backend in [BackendKind::Metal, BackendKind::Vulkan, BackendKind::Cuda] {
        for detail in ["", "boom", "失敗 ✗"] {
            let d = FaultInjecting::new(Fake::default(), rt(backend, detail));
            let r = d.wait_blocking_with(|f| {
                f.calls.set(f.calls.get() + 1);
                Ok(())
            });
            assert_eq!(
                r,
                Err(ContractError::BackendFailure {
                    op: "wait_blocking",
                    backend,
                    detail: detail.to_string(),
                })
            );
            assert_eq!(d.inner().calls.get(), 0);
        }
    }
}

#[test]
fn poll_ex_returns_runtime_error_without_calling_inner() {
    let d = FaultInjecting::new(Fake::default(), rt(BackendKind::Metal, "x"));
    let r = d.poll_ex_with(|f| {
        f.calls.set(f.calls.get() + 1);
        CompletionStateEx::Complete
    });
    assert_eq!(r, CompletionStateEx::RuntimeError);
    assert_eq!(d.inner().calls.get(), 0);
}

#[test]
fn poll_returns_lost_without_calling_inner() {
    let d = FaultInjecting::new(Fake::default(), rt(BackendKind::Vulkan, "x"));
    let r = d.poll_with(|f| {
        f.calls.set(f.calls.get() + 1);
        CompletionState::Complete
    });
    assert_eq!(r, CompletionState::Lost);
    assert_eq!(d.inner().calls.get(), 0);
}

#[test]
fn submit_passes_through_and_counts() {
    let d = FaultInjecting::new(Fake::default(), rt(BackendKind::Cuda, "x"));
    let r: Result<u32, _> = d.submit_with(|f| {
        f.calls.set(f.calls.get() + 1);
        Ok(7)
    });
    assert_eq!(r, Ok(7));
    assert_eq!(d.inner().calls.get(), 1);
    assert_eq!(d.submit_call_count(), 1);
}

#[test]
fn runtime_error_is_distinguishable_from_timeout_and_unknown() {
    let rte = FaultInjecting::new(Fake::default(), rt(BackendKind::Metal, "x"));
    let unk = FaultInjecting::new(Fake::default(), FaultScript::CompletionUnknown);
    let w_rte = rte.wait_blocking_with(|_| Ok(()));
    let w_unk = unk.wait_blocking_with(|_| Ok(()));
    assert_ne!(w_rte, Err(ContractError::Timeout));
    assert_eq!(w_unk, Err(ContractError::Timeout));
    assert_ne!(w_rte, w_unk);
    let p = |d: &FaultInjecting<Fake>| d.poll_ex_with(|_| CompletionStateEx::Pending);
    assert_eq!(p(&rte), CompletionStateEx::RuntimeError);
    assert_eq!(p(&unk), CompletionStateEx::Unknown);
}

#[test]
fn other_scripts_poll_returns_inner_value_once() {
    let others = [
        FaultScript::PassThrough,
        FaultScript::CompletionUnknown,
        FaultScript::CapabilityInsufficient {
            feature: A,
            required: CapabilityLevel::S2,
        },
        FaultScript::CapabilityQueryFails,
        FaultScript::AllocationFails,
    ];
    for s in others {
        for v in [
            CompletionState::Pending,
            CompletionState::Complete,
            CompletionState::Lost,
        ] {
            let d = FaultInjecting::new(Fake::default(), s.clone());
            let r = d.poll_with(|f| {
                f.calls.set(f.calls.get() + 1);
                v
            });
            assert_eq!(r, v, "{s:?}");
            assert_eq!(d.inner().calls.get(), 1, "{s:?}");
        }
    }
}

#[test]
fn runtime_error_leaves_other_hooks_pass_through() {
    let d = FaultInjecting::new(Fake::default(), rt(BackendKind::Metal, "x"));
    let p: Result<u32, _> = d.create_pipeline_with(|_| Ok(5));
    assert_eq!(p, Ok(5));
    let a: Result<u32, _> = d.allocate_with(10, |_, n| Ok(n as u32));
    assert_eq!(a, Ok(10));
    assert_eq!(d.query(A), CapabilityLevel::S2);
    assert_eq!(d.query_ex(A).level, CapabilityLevel::S2);
    assert!(d.simulate_runtime_error("x".into()).is_err());
}
