//! Metal の背後実装の描画先の作成が、device の上で返す handle の値の確認（TASK-377 のポインタ）。
//!
//! `Profile::Graphics` の device の `graphics()` が描画先の作成の口を持つこと、作った handle の device の識別子と
//! 世代が同じ device の別の操作の値と等しいこと、`Profile::ComputeOnly` の device には口が無いことを確かめる。
//! どの試験も Metal の device に触るので `#[ignore]` を付け、専用の実機の回でだけ流す。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::graphics::{CompareFunction, DepthConfig, DepthFormat, PixelFormat};
use fandhe_silicon_core::memory::BufferUsage;

/// 指定のプロファイルで device を作る。`Box<dyn Device>` は `Debug` を持たないので `match` で取り出す。
fn new_device(profile: Profile) -> Box<dyn Device> {
    match MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(device) => device,
        Err(e) => panic!("request must succeed: {e:?}"),
    }
}

fn depth_config() -> DepthConfig {
    DepthConfig {
        format: DepthFormat::Depth32Float,
        clear_value: 1.0,
        compare: CompareFunction::Less,
        write_enabled: true,
    }
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn graphics_device_creates_a_target_owned_by_the_same_device() {
    let device = new_device(Profile::Graphics);
    let graphics = device.graphics().expect("graphics ops");
    let target = graphics
        .create_offscreen_target(1280, 720, PixelFormat::Rgba8Unorm)
        .expect("create color target");

    let memory = device.memory();
    let buffer = memory.allocate(16, BufferUsage::STORAGE).expect("allocate");
    assert_eq!(target.__backend_device_id(), buffer.__backend_device_id());
    assert_eq!(target.__backend_generation(), buffer.__backend_generation());
    memory.free(buffer);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn graphics_device_creates_color_and_depth_targets_with_distinct_ids() {
    let device = new_device(Profile::Graphics);
    let graphics = device.graphics().expect("graphics ops");
    let (color, depth) = graphics
        .create_offscreen_target_with_depth(1280, 720, PixelFormat::Rgba8Unorm, depth_config())
        .expect("create color and depth targets");

    assert_ne!(color.__backend_id(), depth.__backend_id());
    assert_eq!(color.__backend_device_id(), depth.__backend_device_id());
    assert_eq!(color.__backend_generation(), depth.__backend_generation());
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn compute_only_device_has_no_graphics_ops() {
    let device = new_device(Profile::ComputeOnly);
    assert!(device.graphics().is_none());
}
