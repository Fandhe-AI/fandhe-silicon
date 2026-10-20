//! デコレータの模擬の OOM の差し込みの確認（spec TASK-271 の 5/8 の (6)。実際の確保は行わない）。

use std::cell::Cell;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::error::{CompletionStateEx, ContractError};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");

#[derive(Default)]
struct Fake {
    alloc_calls: Cell<u32>,
    last_requested: Cell<u64>,
}
impl Capabilities for Fake {
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

#[test]
fn oom_returns_allocation_failed_without_calling_inner() {
    for n in [0u64, 1, 4096, u64::MAX] {
        let d = FaultInjecting::new(Fake::default(), FaultScript::AllocationFails);
        let r: Result<u32, _> = d.allocate_with(n, |f, _| {
            f.alloc_calls.set(f.alloc_calls.get() + 1);
            Ok(1)
        });
        assert_eq!(
            r,
            Err(ContractError::AllocationFailed { requested_bytes: n })
        );
        assert_eq!(d.inner().alloc_calls.get(), 0);
    }
}

#[test]
fn other_scripts_call_inner_once_with_same_size() {
    let others = [
        FaultScript::PassThrough,
        FaultScript::CompletionUnknown,
        FaultScript::CapabilityInsufficient {
            feature: A,
            required: CapabilityLevel::S2,
        },
        FaultScript::CapabilityQueryFails,
        FaultScript::RuntimeError {
            backend: BackendKind::Metal,
            detail: String::new(),
        },
    ];
    for s in others {
        let d = FaultInjecting::new(Fake::default(), s.clone());
        let r: Result<u32, _> = d.allocate_with(123, |f, n| {
            f.alloc_calls.set(f.alloc_calls.get() + 1);
            f.last_requested.set(n);
            Ok(9)
        });
        assert_eq!(r, Ok(9), "{s:?}");
        assert_eq!(d.inner().alloc_calls.get(), 1, "{s:?}");
        assert_eq!(d.inner().last_requested.get(), 123, "{s:?}");
    }
}

#[test]
fn oom_script_leaves_other_hooks_pass_through() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::AllocationFails);
    assert_eq!(d.wait_blocking_with(|_| Ok(())), Ok(()));
    assert_eq!(
        d.poll_ex_with(|_| CompletionStateEx::Unknown),
        CompletionStateEx::Unknown
    );
    let p: Result<u32, _> = d.create_pipeline_with(|_| Ok(5));
    assert_eq!(p, Ok(5));
    let s: Result<u32, _> = d.submit_with(|_| Ok(6));
    assert_eq!(s, Ok(6));
    assert_eq!(d.submit_call_count(), 1);
    assert_eq!(d.query(A), CapabilityLevel::S2);
    assert_eq!(d.query_ex(A).level, CapabilityLevel::S2);
}

#[test]
fn display_contains_requested_size() {
    let e = ContractError::AllocationFailed {
        requested_bytes: 4096,
    };
    assert!(e.to_string().contains("4096"));
}
