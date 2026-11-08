//! CUDA の device を作って明示に破棄する試験（TASK-191 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::memory::BufferUsage;

fn new_device() -> Box<dyn Device> {
    match CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

// DEV-151
#[test]
#[ignore = "requires a CUDA device"]
fn a_requested_device_can_be_destroyed() {
    let device = new_device();
    let _handle = device
        .memory()
        .allocate(64, BufferUsage::STORAGE)
        .expect("allocate");
    let encoder = device.queue().create_encoder();
    device.queue().submit(encoder, &[]).expect("submit");
    device.destroy();
}
