//! contract の feature `fault-injection` だけで、core の故障の差し込み口に届くことの確認（spec TASK-271 の 2/8）。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::fault::{FaultHooks, HookFault, inject};

struct Empty;
impl FaultHooks for Empty {}

#[test]
fn contract_fault_injection_enables_core_fault_module() {
    assert_eq!(
        inject(Some(&Empty), HookFault::ArmGate),
        Err(ContractError::UnsupportedExtensionDefault { op: "arm_gate" })
    );
}
