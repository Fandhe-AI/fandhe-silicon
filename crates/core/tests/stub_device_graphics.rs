//! feature `test-support` と `graphics` の裏の試験用の device の、プロファイル・所有形態ごとの `graphics()` の答えと、
//! 描画の操作の振る舞いを確かめる統合テスト（TASK-837 の 2/4。DEV-59・DEV-60・DEV-63 のポインタ）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`execution`・`graphics`）だけを使う。
//! `backend` は `BackendKind::Vulkan` に固定し、`Imported` は `NativeDeviceHandle::new(7u32)` を包む。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    BindingLayout, ImportedSemaphoreHandle, PipelineHandle, PipelineInput,
};
use fandhe_silicon_core::graphics::{
    CompareFunction, DepthConfig, DepthFormat, PixelFormat, RenderEncoder,
};
use fandhe_silicon_core::memory::BufferUsage;
use fandhe_silicon_core::stub_device::StubDeviceProvider;

fn device(profile: Profile, ownership: DeviceOwnership) -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Vulkan,
        profile,
        ownership,
    })
    .expect("device")
}

fn imported() -> DeviceOwnership {
    DeviceOwnership::Imported {
        native: NativeDeviceHandle::new(7u32),
    }
}

fn input(entry_point: &'static str) -> PipelineInput<'static> {
    PipelineInput::Msl {
        source: "",
        entry_point,
    }
}

fn assert_no_graphics(d: &dyn Device) {
    assert!(d.graphics().is_none());
}

fn pipeline(d: &dyn Device) -> PipelineHandle {
    d.graphics()
        .expect("graphics")
        .create_render_pipeline(input("vs"), input("fs"), &BindingLayout::new(vec![]))
        .expect("pipeline")
}

fn assert_reaches_graphics(d: &dyn Device) {
    let g = d.graphics().expect("graphics");
    let target = g
        .create_offscreen_target(16, 16, PixelFormat::Rgba8Unorm)
        .expect("target");
    assert_eq!(target.__backend_device_id(), d.generation());
    let p = pipeline(d);
    assert_eq!(p.__backend_device_id(), d.generation());
    let encoder = g.create_render_encoder(&target);
    assert_eq!(encoder.__backend_device_id(), d.generation());
    assert_eq!(encoder.__backend_rejected(), None);
    let token = g.submit_render(encoder, &[]).expect("submit");
    assert_eq!(d.queue().poll(token), CompletionState::Complete);
}

#[test]
fn cpu_only_product_independent_graphics_is_none() {
    assert_no_graphics(&*device(
        Profile::CpuOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

#[test]
fn cpu_only_shared_graphics_is_none() {
    assert_no_graphics(&*device(Profile::CpuOnly, DeviceOwnership::Shared));
}

#[test]
fn cpu_only_imported_graphics_is_none() {
    assert_no_graphics(&*device(Profile::CpuOnly, imported()));
}

#[test]
fn compute_only_product_independent_graphics_is_none() {
    assert_no_graphics(&*device(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

#[test]
fn compute_only_shared_graphics_is_none() {
    assert_no_graphics(&*device(Profile::ComputeOnly, DeviceOwnership::Shared));
}

#[test]
fn compute_only_imported_graphics_is_none() {
    assert_no_graphics(&*device(Profile::ComputeOnly, imported()));
}

#[test]
fn graphics_product_independent_graphics_reaches_ops() {
    assert_reaches_graphics(&*device(
        Profile::Graphics,
        DeviceOwnership::ProductIndependent,
    ));
}

#[test]
fn graphics_shared_graphics_reaches_ops() {
    assert_reaches_graphics(&*device(Profile::Graphics, DeviceOwnership::Shared));
}

#[test]
fn graphics_imported_graphics_reaches_ops() {
    assert_reaches_graphics(&*device(Profile::Graphics, imported()));
}

#[test]
fn draw_draw_ex_and_copy_target_to_buffer_are_unsupported() {
    let d = device(Profile::Graphics, DeviceOwnership::Shared);
    let g = d.graphics().expect("graphics");
    let p = pipeline(&*d);
    let buf = d.memory().allocate(16, BufferUsage::STORAGE).expect("buf");
    let target = g
        .create_offscreen_target(16, 16, PixelFormat::Rgba8Unorm)
        .expect("target");
    let mut render = g.create_render_encoder(&target);
    assert!(matches!(
        g.draw(&mut render, &p, &buf, 3),
        Err(ContractError::Unsupported { op: "draw", .. })
    ));
    assert!(matches!(
        g.draw_ex(&mut render, &p, &buf, 3, &[]),
        Err(ContractError::Unsupported { op: "draw_ex", .. })
    ));
    let mut enc = d.queue().create_encoder();
    assert!(matches!(
        g.copy_target_to_buffer(&mut enc, &target, &buf, 0),
        Err(ContractError::Unsupported {
            op: "copy_target_to_buffer",
            ..
        })
    ));
}

#[test]
fn submit_render_with_wait_is_unsupported() {
    let d = device(Profile::Graphics, DeviceOwnership::Shared);
    let g = d.graphics().expect("graphics");
    let target = g
        .create_offscreen_target(16, 16, PixelFormat::Rgba8Unorm)
        .expect("target");
    let encoder = g.create_render_encoder(&target);
    let wait = [ImportedSemaphoreHandle::__backend_new(1)];
    assert!(matches!(
        g.submit_render(encoder, &wait),
        Err(ContractError::Unsupported {
            op: "submit_render(wait)",
            ..
        })
    ));
}

#[test]
fn offscreen_target_with_depth_returns_distinct_ids() {
    let d = device(Profile::Graphics, DeviceOwnership::Shared);
    let g = d.graphics().expect("graphics");
    let depth = DepthConfig {
        format: DepthFormat::Depth32Float,
        clear_value: 1.0,
        compare: CompareFunction::Less,
        write_enabled: true,
    };
    let (color, depth_target) = g
        .create_offscreen_target_with_depth(16, 16, PixelFormat::Rgba8Unorm, depth)
        .expect("targets");
    assert_ne!(color.__backend_id(), depth_target.__backend_id());
    assert_eq!(color.__backend_device_id(), d.generation());
    assert_eq!(depth_target.__backend_device_id(), d.generation());
}

#[test]
fn submit_render_returns_error_held_by_encoder() {
    let d = device(Profile::Graphics, DeviceOwnership::Shared);
    let g = d.graphics().expect("graphics");
    let rejected = RenderEncoder::__backend_new_rejected(
        d.generation(),
        0,
        d.generation(),
        ContractError::DeviceMismatch,
    );
    assert_eq!(
        g.submit_render(rejected, &[]),
        Err(ContractError::DeviceMismatch)
    );
}
