//! 模擬のロストと門の状態（スタブ）の確認（spec TASK-271 の 3/8）。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fandhe_silicon_core::error::{ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::fault::{FaultHooks, HookFault, inject};

fn counting(stub: &FaultStub) -> Arc<AtomicU64> {
    let n = Arc::new(AtomicU64::new(0));
    let n2 = Arc::clone(&n);
    stub.set_lost_callback(Box::new(move |_| {
        n2.fetch_add(1, Ordering::SeqCst);
    }));
    n
}

#[test]
fn lost_once_per_reason() {
    let reasons = [
        DeviceLostReason::Destroyed,
        DeviceLostReason::Unknown,
        DeviceLostReason::ValidationError("bad".to_string()),
    ];
    for r in reasons {
        let stub = FaultStub::new();
        let n = counting(&stub);
        assert!(!stub.is_lost());
        stub.simulate_device_lost(r.clone()).unwrap();
        assert!(stub.is_lost());
        assert_eq!(stub.lost_reason(), Some(r));
        assert_eq!(n.load(Ordering::SeqCst), 1);
        assert_eq!(stub.lost_callback_count(), 1);
    }
}

#[test]
fn second_loss_is_ignored() {
    let stub = FaultStub::new();
    let n = counting(&stub);
    stub.simulate_device_lost(DeviceLostReason::Destroyed)
        .unwrap();
    assert!(stub.simulate_device_lost(DeviceLostReason::Unknown).is_ok());
    assert_eq!(stub.lost_reason(), Some(DeviceLostReason::Destroyed));
    assert_eq!(n.load(Ordering::SeqCst), 1);
    assert_eq!(stub.lost_callback_count(), 1);
}

#[test]
fn no_callback_means_zero_count() {
    let stub = FaultStub::new();
    stub.simulate_device_lost(DeviceLostReason::Unknown)
        .unwrap();
    assert!(stub.is_lost());
    assert_eq!(stub.lost_callback_count(), 0);
}

#[test]
fn inject_routes_to_stub() {
    let stub = FaultStub::new();
    let n = counting(&stub);
    let hooks: &dyn FaultHooks = &stub;
    inject(
        Some(hooks),
        HookFault::DeviceLost(DeviceLostReason::Unknown),
    )
    .unwrap();
    assert!(stub.is_lost());
    assert_eq!(n.load(Ordering::SeqCst), 1);
}

#[test]
fn gate_state_follows_inject() {
    let stub = FaultStub::new();
    let hooks: &dyn FaultHooks = &stub;
    assert!(!stub.gate_armed());
    inject(Some(hooks), HookFault::ArmGate).unwrap();
    assert!(stub.gate_armed());
    inject(Some(hooks), HookFault::OpenGate).unwrap();
    assert!(!stub.gate_armed());
    assert!(inject(Some(hooks), HookFault::OpenGate).is_ok());
}

#[test]
fn runtime_error_stays_default() {
    let stub = FaultStub::new();
    assert_eq!(
        stub.simulate_runtime_error("x".to_string()),
        Err(ContractError::UnsupportedExtensionDefault {
            op: "simulate_runtime_error"
        })
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stub_is_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<FaultStub>();
}

#[test]
fn callback_may_reenter_stub() {
    let stub = Arc::new(FaultStub::new());
    let s2 = Arc::clone(&stub);
    let seen = Arc::new(AtomicU64::new(0));
    let seen2 = Arc::clone(&seen);
    stub.set_lost_callback(Box::new(move |_| {
        assert!(s2.is_lost());
        s2.simulate_device_lost(DeviceLostReason::Unknown).unwrap();
        seen2.fetch_add(1, Ordering::SeqCst);
    }));
    stub.simulate_device_lost(DeviceLostReason::Destroyed)
        .unwrap();
    assert_eq!(seen.load(Ordering::SeqCst), 1);
    assert_eq!(stub.lost_reason(), Some(DeviceLostReason::Destroyed));
}

#[test]
fn lost_is_notified_through_the_dyn_notify() {
    use fandhe_silicon_core::device_lost::DeviceLostNotify;
    let stub = FaultStub::new();
    let n = Arc::new(AtomicU64::new(0));
    let n2 = Arc::clone(&n);
    let notify: &dyn DeviceLostNotify = &stub;
    notify.set_lost_callback(Box::new(move |_| {
        n2.fetch_add(1, Ordering::SeqCst);
    }));
    stub.simulate_device_lost(DeviceLostReason::Unknown)
        .unwrap();
    assert_eq!(n.load(Ordering::SeqCst), 1);
    assert_eq!(stub.lost_callback_count(), 1);
}
