//! 試験用の device 2 つで、描画先の handle の照合の関数の返す値を公開のパスだけで確かめる（spec TASK-375・TEX-12 のポインタ）。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::BufferUsage;
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

fn handle_for(device_id: u64) -> RenderTargetHandle {
    RenderTargetHandle::__backend_new(1, 1, device_id)
}

#[test]
fn handle_from_other_stub_device_is_device_mismatch() {
    let a = device();
    let b = device();
    assert_ne!(a.generation(), b.generation());
    let h = handle_for(a.generation());
    assert_eq!(
        check_render_target_device(&h, b.generation(), b.generation(), false),
        Err(ContractError::DeviceMismatch)
    );
    assert_eq!(
        check_render_target_device(&h, a.generation(), a.generation(), false),
        Ok(())
    );
}

#[test]
fn rejection_matches_buffer_handle_rejection() {
    let a = device();
    let b = device();
    let buf = a
        .memory()
        .allocate(4, BufferUsage::STORAGE | BufferUsage::UPLOAD)
        .expect("allocate");
    assert_eq!(buf.__backend_device_id(), a.generation());
    let buf_err = b.memory().upload(&buf, 0, &[1]);
    let rt_err = check_render_target_device(
        &handle_for(a.generation()),
        b.generation(),
        b.generation(),
        false,
    );
    assert_eq!(buf_err, rt_err);
    assert_eq!(rt_err, Err(ContractError::DeviceMismatch));
}

#[test]
fn handle_from_recreated_device_is_device_mismatch() {
    let a = device();
    let old = a.generation();
    drop(a);
    let b = device();
    assert_ne!(old, b.generation());
    assert_eq!(
        check_render_target_device(&handle_for(old), b.generation(), b.generation(), false),
        Err(ContractError::DeviceMismatch)
    );
}

#[test]
fn forged_handle_is_judged_by_device_id_only() {
    let d = device();
    let own = d.generation();
    assert_ne!(own, u64::MAX);
    assert_ne!(own, 0);
    for (g, i) in [(0, 0), (u64::MAX, u64::MAX)] {
        let h = RenderTargetHandle::__backend_new(g, i, own);
        assert_eq!(check_render_target_device(&h, own, own, false), Ok(()));
    }
    for other in [u64::MAX, 0] {
        let h = RenderTargetHandle::__backend_new(0, 0, other);
        assert_eq!(
            check_render_target_device(&h, own, own, false),
            Err(ContractError::DeviceMismatch)
        );
    }
}
