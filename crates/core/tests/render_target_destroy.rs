//! feature `test-support` と `graphics` の裏の試験用の device で、描画先の破棄の後と、作っていない描画先を
//! 使ったときの戻り値を確かめる統合テスト（TASK-372）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`error`・`execution`・`graphics`・`memory`・
//! `render_target`）だけを使う。`backend` は `BackendKind::Vulkan` に固定する。
//! 破棄は handle を消費するので、破棄の前に 3 つの値を読んで同じ値の handle を作り直し、破棄の後の入力にする。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{BindingLayout, PipelineHandle, PipelineInput};
use fandhe_silicon_core::graphics::{
    CompareFunction, DepthConfig, DepthFormat, PixelFormat, RenderEncoder,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_core::render_target::RenderTargetHandle;
use fandhe_silicon_core::stub_device::StubDeviceProvider;

fn device() -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Vulkan,
        profile: Profile::Graphics,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("device")
}

fn input(entry_point: &'static str) -> PipelineInput<'static> {
    PipelineInput::Msl {
        source: "",
        entry_point,
    }
}

fn pipeline(d: &dyn Device) -> PipelineHandle {
    d.graphics()
        .expect("graphics")
        .create_render_pipeline(input("vs"), input("fs"), &BindingLayout::new(vec![]))
        .expect("pipeline")
}

fn target(d: &dyn Device) -> RenderTargetHandle {
    d.graphics()
        .expect("graphics")
        .create_offscreen_target(16, 16, PixelFormat::Rgba8Unorm)
        .expect("target")
}

fn buffer(d: &dyn Device) -> BufferHandle {
    d.memory()
        .allocate(16, BufferUsage::STORAGE)
        .expect("buffer")
}

/// `t` と同じ 3 つの値を持つ handle。破棄の前に写して、破棄の後の入力にする。
fn forge(t: &RenderTargetHandle) -> RenderTargetHandle {
    RenderTargetHandle::__backend_new(
        t.__backend_generation(),
        t.__backend_id(),
        t.__backend_device_id(),
    )
}

/// `d` が作っていない番号の handle（通し番号が届かない番号）。
fn unknown(d: &dyn Device) -> RenderTargetHandle {
    RenderTargetHandle::__backend_new(d.generation(), u64::MAX, d.generation())
}

fn is_invalid(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

fn rejected_invalid(enc: &RenderEncoder) -> bool {
    matches!(
        enc.__backend_rejected(),
        Some(ContractError::InvalidArgument(_))
    )
}

fn submit(d: &dyn Device, enc: RenderEncoder) -> Result<(), ContractError> {
    d.graphics()
        .expect("graphics")
        .submit_render(enc, &[])
        .map(|_| ())
}

// TEX-13
#[test]
fn destroyed_target_is_rejected_by_encoder() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(&*d);
    let forged = forge(&t);
    g.destroy_offscreen_target(t);
    let mut enc = g.create_render_encoder(&forged);
    assert!(rejected_invalid(&enc));
    let (p, b) = (pipeline(&*d), buffer(&*d));
    assert!(is_invalid(&g.draw(&mut enc, &p, &b, 3)));
    assert!(is_invalid(&submit(&*d, enc)));
}

// TEX-19
#[test]
fn never_created_target_is_invalid_argument() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let u = unknown(&*d);
    assert!(rejected_invalid(&g.create_render_encoder(&u)));
    let mut cmd = d.queue().create_encoder();
    let r = g.copy_target_to_buffer(&mut cmd, &u, &buffer(&*d), 0);
    assert!(is_invalid(&r));
}

// TEX-19
#[test]
fn other_device_target_is_mismatch_before_table() {
    let (a, b) = (device(), device());
    let ga = a.graphics().expect("graphics");
    let gb = b.graphics().expect("graphics");
    let tb = target(&*b);
    let enc = ga.create_render_encoder(&tb);
    assert_eq!(
        enc.__backend_rejected(),
        Some(&ContractError::DeviceMismatch)
    );
    let mut cmd = a.queue().create_encoder();
    let r = ga.copy_target_to_buffer(&mut cmd, &tb, &buffer(&*a), 0);
    assert_eq!(r, Err(ContractError::DeviceMismatch));

    let tb2 = target(&*b);
    let forged = forge(&tb2);
    gb.destroy_offscreen_target(tb2);
    let enc = ga.create_render_encoder(&forged);
    assert_eq!(
        enc.__backend_rejected(),
        Some(&ContractError::DeviceMismatch)
    );
    let r = ga.copy_target_to_buffer(&mut cmd, &forged, &buffer(&*a), 0);
    assert_eq!(r, Err(ContractError::DeviceMismatch));
}

// TEX-19
#[test]
fn destroying_other_device_handle_keeps_own_target() {
    let (a, b) = (device(), device());
    let ga = a.graphics().expect("graphics");
    let ta = target(&*a);
    let alien =
        RenderTargetHandle::__backend_new(b.generation(), ta.__backend_id(), b.generation());
    ga.destroy_offscreen_target(alien);
    let enc = ga.create_render_encoder(&ta);
    assert!(enc.__backend_rejected().is_none());
    assert_eq!(submit(&*a, enc), Ok(()));
}

// TEX-19
#[test]
fn destroying_unknown_handle_is_noop() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(&*d);
    g.destroy_offscreen_target(unknown(&*d));
    assert!(g.create_render_encoder(&t).__backend_rejected().is_none());
}

// TEX-13
#[test]
fn recorded_then_destroyed_target_rejects_submit() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(&*d);
    let forged = forge(&t);
    let enc = g.create_render_encoder(&forged);
    assert!(enc.__backend_rejected().is_none());
    g.destroy_offscreen_target(t);
    assert!(is_invalid(&submit(&*d, enc)));
}

// TEX-13
#[test]
fn depth_pair_is_destroyed_one_by_one() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let depth = DepthConfig {
        format: DepthFormat::Depth32Float,
        clear_value: 1.0,
        compare: CompareFunction::Less,
        write_enabled: true,
    };
    let (color, depth_t) = g
        .create_offscreen_target_with_depth(16, 16, PixelFormat::Rgba8Unorm, depth)
        .expect("pair");
    let (color_f, depth_f) = (forge(&color), forge(&depth_t));
    g.destroy_offscreen_target(depth_t);
    let enc = g.create_render_encoder_with_depth(&color_f, &depth_f, [0.0; 4]);
    assert!(rejected_invalid(&enc));
    assert!(
        g.create_render_encoder(&color_f)
            .__backend_rejected()
            .is_none()
    );
}

// TEX-19
#[test]
fn destroyed_ids_are_not_reused() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(&*d);
    let old = t.__backend_id();
    g.destroy_offscreen_target(t);
    assert_ne!(target(&*d).__backend_id(), old);
}

// TEX-19
#[test]
fn rejection_is_repeatable() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(&*d);
    let destroyed = forge(&t);
    g.destroy_offscreen_target(t);
    let u = unknown(&*d);
    let buf = buffer(&*d);
    for h in [&destroyed, &u] {
        let first = g.create_render_encoder(h);
        let second = g.create_render_encoder(h);
        assert_eq!(first.__backend_rejected(), second.__backend_rejected());
        let mut cmd = d.queue().create_encoder();
        let r1 = g.copy_target_to_buffer(&mut cmd, h, &buf, 0);
        let r2 = g.copy_target_to_buffer(&mut cmd, h, &buf, 0);
        assert_eq!(r1, r2);
    }
}

// TEX-13
#[test]
fn rejection_is_invalid_argument_only() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let (p, b) = (pipeline(&*d), buffer(&*d));
    let t = target(&*d);
    let destroyed = forge(&t);
    g.destroy_offscreen_target(t);
    let u = unknown(&*d);
    for h in [&destroyed, &u] {
        let mut enc = g.create_render_encoder(h);
        assert!(rejected_invalid(&enc));
        assert!(is_invalid(&g.draw(&mut enc, &p, &b, 3)));
        assert!(is_invalid(&g.draw_ex(&mut enc, &p, &b, 3, &[])));
        assert!(is_invalid(&submit(&*d, enc)));
        let mut cmd = d.queue().create_encoder();
        assert!(is_invalid(&g.copy_target_to_buffer(&mut cmd, h, &b, 0)));
    }
}

// TEX-13
#[test]
fn recorded_then_destroyed_target_rejects_draw() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let (p, b) = (pipeline(&*d), buffer(&*d));
    let t = target(&*d);
    let forged = forge(&t);
    let mut enc = g.create_render_encoder(&forged);
    assert!(enc.__backend_rejected().is_none());
    g.destroy_offscreen_target(t);
    assert!(is_invalid(&g.draw(&mut enc, &p, &b, 3)));
    assert!(is_invalid(&g.draw_ex(&mut enc, &p, &b, 3, &[])));
}

// TEX-19
#[test]
fn other_device_encoder_is_mismatch_for_draw_and_submit() {
    let (a, b) = (device(), device());
    let ga = a.graphics().expect("graphics");
    let gb = b.graphics().expect("graphics");
    let (pa, buf) = (pipeline(&*a), buffer(&*a));
    let tb = target(&*b);
    let mut enc = gb.create_render_encoder(&tb);
    assert!(enc.__backend_rejected().is_none());
    assert_eq!(
        ga.draw(&mut enc, &pa, &buf, 3),
        Err(ContractError::DeviceMismatch)
    );
    assert_eq!(
        ga.draw_ex(&mut enc, &pa, &buf, 3, &[]),
        Err(ContractError::DeviceMismatch)
    );
    assert_eq!(submit(&*a, enc), Err(ContractError::DeviceMismatch));
}
