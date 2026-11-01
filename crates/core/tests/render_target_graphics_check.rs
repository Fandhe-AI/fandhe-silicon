//! feature `test-support` と `graphics` の裏の試験用の device 2 つで、描画の操作に別 device の描画先の handle・
//! パイプラインを渡したときの戻り値を確かめる統合テスト（TASK-375 の 5/5。TEX-12・TASK-837 のポインタ）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`error`・`execution`・`graphics`・`memory`・`render_target`）だけを使う。
//! `backend` は `BackendKind::Vulkan` に固定する。device A の handle を device B の `graphics()` に渡す。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    BindingLayout, ImportedSemaphoreHandle, PipelineHandle, PipelineInput,
};
use fandhe_silicon_core::graphics::{PixelFormat, RenderEncoder};
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

/// `d` の `submit_render` を待ちなしで呼び、印は捨てて成否だけ返す。
fn submit_result(d: &dyn Device, enc: RenderEncoder) -> Result<(), ContractError> {
    d.graphics()
        .expect("graphics")
        .submit_render(enc, &[])
        .map(|_| ())
}

/// `b` に A の描画先を渡して作った記録先。拒否の値を持つことも確かめる。
fn rejected_encoder(b: &dyn Device, a_target: &RenderTargetHandle) -> RenderEncoder {
    let enc = b
        .graphics()
        .expect("graphics")
        .create_render_encoder(a_target);
    assert_eq!(
        enc.__backend_rejected(),
        Some(&ContractError::DeviceMismatch)
    );
    enc
}

#[test]
fn render_encoder_from_other_device_target_returns_device_mismatch() {
    let (a, b) = (device(), device());
    let (ta, tb) = (target(&*a), target(&*b));
    let enc = rejected_encoder(&*b, &ta);
    assert_eq!(submit_result(&*b, enc), Err(ContractError::DeviceMismatch));
    let ok = b.graphics().expect("graphics").create_render_encoder(&tb);
    assert_eq!(submit_result(&*b, ok), Ok(()));
}

#[test]
fn render_encoder_with_depth_checks_both_targets() {
    let (a, b) = (device(), device());
    let (ca, da) = (target(&*a), target(&*a));
    let (cb, db) = (target(&*b), target(&*b));
    let g = b.graphics().expect("graphics");
    let clear = [0.0; 4];
    let mixed_depth = g.create_render_encoder_with_depth(&ca, &db, clear);
    assert_eq!(
        submit_result(&*b, mixed_depth),
        Err(ContractError::DeviceMismatch)
    );
    let mixed_color = g.create_render_encoder_with_depth(&cb, &da, clear);
    assert_eq!(
        submit_result(&*b, mixed_color),
        Err(ContractError::DeviceMismatch)
    );
    let same = g.create_render_encoder_with_depth(&cb, &db, clear);
    assert_eq!(submit_result(&*b, same), Ok(()));
}

#[test]
fn draw_returns_device_mismatch_held_by_encoder() {
    let (a, b) = (device(), device());
    let ta = target(&*a);
    let g = b.graphics().expect("graphics");
    let (pb, vb) = (pipeline(&*b), buffer(&*b));
    let mut enc = rejected_encoder(&*b, &ta);
    assert_eq!(
        g.draw(&mut enc, &pb, &vb, 3),
        Err(ContractError::DeviceMismatch)
    );
    assert_eq!(
        g.draw_ex(&mut enc, &pb, &vb, 3, &[]),
        Err(ContractError::DeviceMismatch)
    );
}

#[test]
fn submit_render_returns_held_value_before_wait() {
    let (a, b) = (device(), device());
    let ta = target(&*a);
    let enc = rejected_encoder(&*b, &ta);
    let wait = [ImportedSemaphoreHandle::__backend_new(1)];
    let r = b
        .graphics()
        .expect("graphics")
        .submit_render(enc, &wait)
        .map(|_| ());
    assert_eq!(r, Err(ContractError::DeviceMismatch));
}

#[test]
fn draw_with_other_device_pipeline_returns_device_mismatch() {
    let (a, b) = (device(), device());
    let tb = target(&*b);
    let g = b.graphics().expect("graphics");
    let (pa, pb, vb) = (pipeline(&*a), pipeline(&*b), buffer(&*b));
    let mut enc = g.create_render_encoder(&tb);
    assert_eq!(
        g.draw(&mut enc, &pa, &vb, 3),
        Err(ContractError::DeviceMismatch)
    );
    assert_eq!(
        g.draw_ex(&mut enc, &pa, &vb, 3, &[]),
        Err(ContractError::DeviceMismatch)
    );
    assert!(matches!(
        g.draw(&mut enc, &pb, &vb, 3),
        Err(ContractError::Unsupported { op: "draw", .. })
    ));
    assert!(matches!(
        g.draw_ex(&mut enc, &pb, &vb, 3, &[]),
        Err(ContractError::Unsupported { op: "draw_ex", .. })
    ));
}

#[test]
fn copy_target_to_buffer_from_other_device_target_returns_device_mismatch() {
    let (a, b) = (device(), device());
    let (ta, tb, vb) = (target(&*a), target(&*b), buffer(&*b));
    let g = b.graphics().expect("graphics");
    let mut enc = b.queue().create_encoder();
    assert_eq!(
        g.copy_target_to_buffer(&mut enc, &ta, &vb, 0),
        Err(ContractError::DeviceMismatch)
    );
    assert!(matches!(
        g.copy_target_to_buffer(&mut enc, &tb, &vb, 0),
        Err(ContractError::Unsupported {
            op: "copy_target_to_buffer",
            ..
        })
    ));
}

#[test]
fn rejection_matches_buffer_and_pipeline_rejection() {
    let (a, b) = (device(), device());
    let (ta, tb) = (target(&*a), target(&*b));
    let (pa, vb, buf_a) = (pipeline(&*a), buffer(&*b), buffer(&*a));
    let g = b.graphics().expect("graphics");

    let by_buffer = b.memory().upload(&buf_a, 0, &[1]);
    let mut enc_b = g.create_render_encoder(&tb);
    let by_pipeline = g.draw(&mut enc_b, &pa, &vb, 3);
    let mut cmd = b.queue().create_encoder();
    let by_copy = g.copy_target_to_buffer(&mut cmd, &ta, &vb, 0);
    let by_submit = submit_result(&*b, rejected_encoder(&*b, &ta));

    assert_eq!(by_buffer, Err(ContractError::DeviceMismatch));
    assert_eq!(by_pipeline, by_buffer);
    assert_eq!(by_copy, by_buffer);
    assert_eq!(by_submit, by_buffer);
}

#[test]
fn rejected_encoder_does_not_advance_serial() {
    let (a, b) = (device(), device());
    let (ta, tb) = (target(&*a), target(&*b));
    let g = b.graphics().expect("graphics");
    let first = g.create_render_encoder(&tb);
    let n = first.__backend_id();
    let rejected = g.create_render_encoder(&ta);
    assert_eq!(rejected.__backend_id(), 0);
    assert_eq!(
        rejected.__backend_rejected(),
        Some(&ContractError::DeviceMismatch)
    );
    let next = g.create_render_encoder(&tb);
    assert_eq!(next.__backend_id(), n + 1);
}
