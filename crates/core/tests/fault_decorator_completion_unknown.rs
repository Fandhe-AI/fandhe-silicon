//! デコレータの完了不明の差し込みの確認（spec TASK-271 の 3/8。ERR-12・ERR-26 の入力）。

use std::cell::Cell;

use fandhe_silicon_core::error::{CompletionStateEx, ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::fault::{FaultHooks, HookFault, inject};

/// 呼ばれたかを記録する偽の内側。口は既定の実装のまま。
#[derive(Default)]
struct Fake {
    called: Cell<bool>,
}
impl FaultHooks for Fake {}

#[test]
fn wait_returns_timeout_without_calling_inner() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::CompletionUnknown);
    let r = d.wait_blocking_with(|f| {
        f.called.set(true);
        Ok(())
    });
    assert_eq!(r, Err(ContractError::Timeout));
    assert!(!d.inner().called.get());
}

#[test]
fn poll_ex_returns_unknown_without_calling_inner() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::CompletionUnknown);
    let r = d.poll_ex_with(|f| {
        f.called.set(true);
        CompletionStateEx::Complete
    });
    assert_eq!(r, CompletionStateEx::Unknown);
    assert!(!d.inner().called.get());
}

#[test]
fn pass_through_returns_inner_values() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough);
    assert_eq!(*d.script(), FaultScript::PassThrough);
    assert_eq!(d.wait_blocking_with(|_| Ok(())), Ok(()));
    assert_eq!(
        d.wait_blocking_with(|_| Err(ContractError::UnsupportedExtensionDefault { op: "x" })),
        Err(ContractError::UnsupportedExtensionDefault { op: "x" })
    );
    assert_eq!(
        d.poll_ex_with(|_| CompletionStateEx::Complete),
        CompletionStateEx::Complete
    );
    assert_eq!(
        d.poll_ex_with(|_| CompletionStateEx::Pending),
        CompletionStateEx::Pending
    );
}

#[test]
fn values_are_distinct() {
    assert_ne!(
        ContractError::Timeout,
        ContractError::DeviceLost { generation: 0 }
    );
    assert_ne!(CompletionStateEx::Unknown, CompletionStateEx::RuntimeError);
    assert_ne!(CompletionStateEx::Unknown, CompletionStateEx::Lost);
}

#[test]
fn hooks_pass_through_to_inner() {
    let d = FaultInjecting::new(FaultStub::new(), FaultScript::PassThrough);
    let hooks: &dyn FaultHooks = &d;
    inject(
        Some(hooks),
        HookFault::DeviceLost(DeviceLostReason::Destroyed),
    )
    .unwrap();
    assert!(d.inner().is_lost());

    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough);
    let hooks: &dyn FaultHooks = &d;
    assert_eq!(
        inject(Some(hooks), HookFault::ArmGate),
        Err(ContractError::UnsupportedExtensionDefault { op: "arm_gate" })
    );
}
