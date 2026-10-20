//! デコレータの投入の回数の計数の確認（spec TASK-271 の 5/8。TASK-274 の入力）。

use std::cell::Cell;

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

#[derive(Default)]
struct Fake {
    calls: Cell<u32>,
}

fn scripts() -> Vec<FaultScript> {
    vec![
        FaultScript::PassThrough,
        FaultScript::CompletionUnknown,
        FaultScript::CapabilityInsufficient {
            feature: FeatureId("feature:A"),
            required: CapabilityLevel::S2,
        },
        FaultScript::CapabilityQueryFails,
        FaultScript::AllocationFails,
        FaultScript::RuntimeError {
            backend: BackendKind::Metal,
            detail: String::new(),
        },
    ]
}

#[test]
fn count_is_zero_before_any_submit() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough);
    assert_eq!(d.submit_call_count(), 0);
}

#[test]
fn pass_through_counts_each_call_and_returns_inner_value() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough);
    let r: Result<u32, _> = d.submit_with(|f| {
        f.calls.set(f.calls.get() + 1);
        Ok(7)
    });
    assert_eq!(r, Ok(7));
    assert_eq!(d.submit_call_count(), 1);
    assert_eq!(d.inner().calls.get(), 1);
    let _: Result<u32, _> = d.submit_with(|_| Ok(8));
    assert_eq!(d.submit_call_count(), 2);
}

#[test]
fn inner_error_is_counted_and_returned() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough);
    let r: Result<u32, _> = d.submit_with(|_| Err(ContractError::Timeout));
    assert_eq!(r, Err(ContractError::Timeout));
    assert_eq!(d.submit_call_count(), 1);
}

#[test]
fn completion_unknown_submits_once_then_wait_times_out() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::CompletionUnknown);
    let r: Result<u32, _> = d.submit_with(|f| {
        f.calls.set(f.calls.get() + 1);
        Ok(1)
    });
    assert_eq!(r, Ok(1));
    assert_eq!(d.submit_call_count(), 1);
    assert_eq!(d.inner().calls.get(), 1);
    assert_eq!(
        d.wait_blocking_with(|_| Ok(())),
        Err(ContractError::Timeout)
    );
    assert_eq!(d.submit_call_count(), 1);
}

#[test]
fn every_script_passes_submit_through_and_counts() {
    for s in scripts() {
        let d = FaultInjecting::new(Fake::default(), s.clone());
        let r: Result<u32, _> = d.submit_with(|_| Ok(3));
        assert_eq!(r, Ok(3), "{s:?}");
        assert_eq!(d.submit_call_count(), 1, "{s:?}");
    }
}

#[test]
fn decorator_is_send_and_sync_when_inner_is() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<FaultInjecting<u32>>();
}
