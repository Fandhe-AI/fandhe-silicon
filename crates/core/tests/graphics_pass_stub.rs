//! feature `test-support` と `graphics` の試験用の device で、描画パスの状態を設定する 8 操作の返しを
//! 公開のパスだけで確かめる統合テスト（TASK-838）。
//!
//! `backend` は `BackendKind::Vulkan`、所有形態は `DeviceOwnership::ProductIndependent` に固定する。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{BindingLayout, PipelineHandle, PipelineInput};
use fandhe_silicon_core::graphics::{
    GraphicsOps, IndexFormat, PixelFormat, RenderEncoder, ScissorRect, Viewport,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_core::render_target::RenderTargetHandle;
use fandhe_silicon_core::stub_device::StubDeviceProvider;

type OpResults = Vec<(&'static str, Result<(), ContractError>)>;

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

/// 8 操作を 1 回ずつ呼び、（操作名, 結果）を返す。
fn call_state_ops(
    g: &dyn GraphicsOps,
    enc: &mut RenderEncoder,
    pipeline: &PipelineHandle,
    buf: &BufferHandle,
) -> OpResults {
    let viewport = Viewport {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    let rect = ScissorRect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    vec![
        ("set_pipeline", g.set_pipeline(enc, pipeline)),
        (
            "set_vertex_buffer",
            g.set_vertex_buffer(enc, 0, buf, 0, None),
        ),
        (
            "set_index_buffer",
            g.set_index_buffer(enc, buf, IndexFormat::Uint16, 0, None),
        ),
        ("set_immediates", g.set_immediates(enc, 0, &[1, 2, 3, 4])),
        ("set_viewport", g.set_viewport(enc, viewport)),
        ("set_scissor_rect", g.set_scissor_rect(enc, rect)),
        ("set_blend_constant", g.set_blend_constant(enc, [0.0; 4])),
        ("set_stencil_reference", g.set_stencil_reference(enc, 0)),
    ]
}

// REND-52
#[test]
fn pass_state_ops_are_unsupported_on_live_encoder() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let mut enc = g.create_render_encoder(&t);
    let results = call_state_ops(g, &mut enc, &pipeline(d.as_ref()), &buffer(d.as_ref()));
    assert_eq!(results.len(), 8);
    for (name, r) in results {
        assert!(
            matches!(r, Err(ContractError::Unsupported { op, .. }) if op == name),
            "{name}"
        );
    }
}

#[test]
fn pass_state_ops_return_encoder_rejection() {
    let a = device();
    let b = device();
    let foreign = target(b.as_ref());
    let ga = a.graphics().expect("graphics");
    let mut enc = ga.create_render_encoder(&foreign);
    assert!(matches!(
        enc.__backend_rejected(),
        Some(ContractError::DeviceMismatch)
    ));
    for (name, r) in call_state_ops(ga, &mut enc, &pipeline(a.as_ref()), &buffer(a.as_ref())) {
        assert!(matches!(r, Err(ContractError::DeviceMismatch)), "{name}");
    }
}

#[test]
fn pass_state_ops_reject_destroyed_target() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let mut enc = g.create_render_encoder(&t);
    g.destroy_offscreen_target(t);
    for (name, r) in call_state_ops(g, &mut enc, &pipeline(d.as_ref()), &buffer(d.as_ref())) {
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "{name}"
        );
    }
}

#[test]
fn set_pipeline_rejects_other_device_pipeline() {
    let a = device();
    let b = device();
    let g = a.graphics().expect("graphics");
    let t = target(a.as_ref());
    let mut enc = g.create_render_encoder(&t);
    let foreign = pipeline(b.as_ref());
    assert!(matches!(
        g.set_pipeline(&mut enc, &foreign),
        Err(ContractError::DeviceMismatch)
    ));
}

#[test]
fn pass_state_ops_do_not_consume_encoder() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let mut enc = g.create_render_encoder(&t);
    let _ = call_state_ops(g, &mut enc, &pipeline(d.as_ref()), &buffer(d.as_ref()));
    let _ = g.submit_render(enc, &[]);
}
