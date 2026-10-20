//! contract の feature `fault-injection` だけで、core のデコレータとスタブが組み上がって働くことの確認
//! （spec TASK-271 の 7/8）。差し込むのは模擬であって、実際の故障ではない。

use std::cell::Cell;

use fandhe_silicon_core::error::{ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::fault::{HookFault, inject};

#[test]
fn decorator_over_stub_routes_simulated_loss_to_stub() {
    let d = FaultInjecting::new(FaultStub::new(), FaultScript::PassThrough);
    assert_eq!(
        inject(Some(&d), HookFault::DeviceLost(DeviceLostReason::Destroyed)),
        Ok(())
    );
    assert_eq!(d.inner().lost_reason(), Some(DeviceLostReason::Destroyed));
}

#[test]
fn allocation_fails_without_calling_inner() {
    let d = FaultInjecting::new(FaultStub::new(), FaultScript::AllocationFails);
    let called = Cell::new(false);
    let r: Result<u32, _> = d.allocate_with(128, |_, _| {
        called.set(true);
        Ok(1)
    });
    assert_eq!(
        r,
        Err(ContractError::AllocationFailed {
            requested_bytes: 128
        })
    );
    assert!(!called.get());
}
