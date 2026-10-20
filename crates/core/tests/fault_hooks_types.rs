//! 故障の差し込み口の型の確認（spec TASK-271 の 2/8）。既定の実装の戻り値と、`HookFault` の `non_exhaustive`。

use fandhe_silicon_core::error::{ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::{FaultHooks, HookFault};

struct Empty;
impl FaultHooks for Empty {}

fn op_of(r: Result<(), ContractError>) -> &'static str {
    match r {
        Err(ContractError::UnsupportedExtensionDefault { op }) => op,
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn default_methods_return_unsupported_with_own_name() {
    let e = Empty;
    assert_eq!(
        op_of(e.simulate_device_lost(DeviceLostReason::Unknown)),
        "simulate_device_lost"
    );
    assert_eq!(op_of(e.arm_gate()), "arm_gate");
    assert_eq!(op_of(e.open_gate()), "open_gate");
    assert_eq!(
        op_of(e.simulate_runtime_error("x".to_string())),
        "simulate_runtime_error"
    );
}

#[test]
fn usable_as_trait_object() {
    let r: &dyn FaultHooks = &Empty;
    assert_eq!(op_of(r.arm_gate()), "arm_gate");
    let b: Box<dyn FaultHooks> = Box::new(Empty);
    assert_eq!(op_of(b.open_gate()), "open_gate");
}

#[test]
fn hook_fault_is_non_exhaustive() {
    let f = HookFault::ArmGate;
    let name = match f {
        HookFault::DeviceLost(_) => "lost",
        HookFault::ArmGate => "arm",
        HookFault::OpenGate => "open",
        HookFault::RuntimeError(_) => "err",
        _ => "other",
    };
    assert_eq!(name, "arm");
}
