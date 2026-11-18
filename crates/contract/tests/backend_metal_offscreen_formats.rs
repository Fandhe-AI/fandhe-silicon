//! Metal の `Graphics` の device が `Bgra8Unorm`・`Rgba32Float` の描画先を作ることの確認（TASK-380 のポインタ）。
//!
//! Metal の device に触るので `#[ignore]` を付ける。macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::graphics::PixelFormat;
use fandhe_silicon_core::memory::BufferUsage;

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn graphics_device_creates_bgra8unorm_and_rgba32float_targets() {
    let device: Box<dyn Device> = match MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::Graphics,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(device) => device,
        Err(e) => panic!("request must succeed: {e:?}"),
    };
    let graphics = device.graphics().expect("graphics ops");
    let memory = device.memory();
    let buffer = memory.allocate(16, BufferUsage::STORAGE).expect("allocate");
    for format in [PixelFormat::Bgra8Unorm, PixelFormat::Rgba32Float] {
        let target = graphics
            .create_offscreen_target(1280, 720, format)
            .expect("create color target");
        assert_eq!(target.__backend_device_id(), buffer.__backend_device_id());
        assert_eq!(target.__backend_generation(), buffer.__backend_generation());
    }
    memory.free(buffer);
}
