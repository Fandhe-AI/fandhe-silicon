//! `FaultStub` の模擬のロストの前後で、描画先の handle の照合の関数の返す値を確かめる（spec TASK-375・TEX-12 のポインタ）。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::render_target::{RenderTargetHandle, check_render_target_device};
use fandhe_silicon_core::stub_device::StubDeviceProvider;

fn device() -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("device")
}

#[test]
fn lost_stub_returns_device_lost_with_given_generation() {
    let d = device();
    let g = d.generation();
    let stub = FaultStub::new();
    let h = RenderTargetHandle::__backend_new(g, 1, g);
    assert!(!stub.is_lost());
    assert_eq!(check_render_target_device(&h, g, g, stub.is_lost()), Ok(()));
    stub.simulate_device_lost(DeviceLostReason::Destroyed)
        .expect("lost");
    assert!(stub.is_lost());
    assert_eq!(
        check_render_target_device(&h, g, g, stub.is_lost()),
        Err(ContractError::DeviceLost { generation: g })
    );
}

#[test]
fn lost_is_checked_before_device_id() {
    let a = device();
    let b = device();
    let stub = FaultStub::new();
    stub.simulate_device_lost(DeviceLostReason::Destroyed)
        .expect("lost");
    let h = RenderTargetHandle::__backend_new(1, 1, a.generation());
    let r = check_render_target_device(&h, b.generation(), b.generation(), stub.is_lost());
    assert_eq!(
        r,
        Err(ContractError::DeviceLost {
            generation: b.generation()
        })
    );
    assert_ne!(r, Err(ContractError::DeviceMismatch));
}
