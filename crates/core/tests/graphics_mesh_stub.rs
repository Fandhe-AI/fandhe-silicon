//! feature `test-support` と `graphics` の試験用の device で、mesh shader の描画の 4 操作の返しを
//! 公開のパスだけで確かめる統合テスト（TASK-839）。
//!
//! `backend` は `BackendKind::Vulkan`、所有形態は `DeviceOwnership::ProductIndependent` に固定する。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::graphics::{GraphicsOps, PixelFormat, RenderEncoder};
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

/// 4 操作に渡す値の組。
struct Inputs<'a> {
    group_count: [u32; 3],
    indirect: &'a BufferHandle,
    indirect_offset: u64,
    count: u32,
    count_buffer: &'a BufferHandle,
    count_offset: u64,
    max_count: u32,
}

impl<'a> Inputs<'a> {
    fn plain(buf: &'a BufferHandle) -> Self {
        Inputs {
            group_count: [1, 1, 1],
            indirect: buf,
            indirect_offset: 0,
            count: 1,
            count_buffer: buf,
            count_offset: 0,
            max_count: 1,
        }
    }
}

/// 4 操作を 1 回ずつ呼び、（操作名, 結果）を返す。
fn call_mesh_ops(g: &dyn GraphicsOps, enc: &mut RenderEncoder, i: &Inputs) -> OpResults {
    vec![
        ("draw_mesh_tasks", g.draw_mesh_tasks(enc, i.group_count)),
        (
            "draw_mesh_tasks_indirect",
            g.draw_mesh_tasks_indirect(enc, i.indirect, i.indirect_offset),
        ),
        (
            "multi_draw_mesh_tasks_indirect",
            g.multi_draw_mesh_tasks_indirect(enc, i.indirect, i.indirect_offset, i.count),
        ),
        (
            "multi_draw_mesh_tasks_indirect_count",
            g.multi_draw_mesh_tasks_indirect_count(
                enc,
                i.indirect,
                i.indirect_offset,
                i.count_buffer,
                i.count_offset,
                i.max_count,
            ),
        ),
    ]
}

fn assert_all_unsupported(results: OpResults) {
    assert_eq!(results.len(), 4);
    for (name, r) in results {
        assert!(
            matches!(r, Err(ContractError::Unsupported { op, .. }) if op == name),
            "{name}"
        );
    }
}

#[test]
fn mesh_ops_are_unsupported_on_live_encoder() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let buf = buffer(d.as_ref());
    let mut enc = g.create_render_encoder(&t);
    assert_all_unsupported(call_mesh_ops(g, &mut enc, &Inputs::plain(&buf)));
}

#[test]
fn mesh_ops_are_unsupported_on_other_device_encoder() {
    let a = device();
    let b = device();
    let foreign = target(b.as_ref());
    let ga = a.graphics().expect("graphics");
    let buf = buffer(a.as_ref());
    let mut enc = ga.create_render_encoder(&foreign);
    assert!(matches!(
        enc.__backend_rejected(),
        Some(ContractError::DeviceMismatch)
    ));
    assert_all_unsupported(call_mesh_ops(ga, &mut enc, &Inputs::plain(&buf)));
}

#[test]
fn mesh_ops_are_unsupported_on_destroyed_target_encoder() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let buf = buffer(d.as_ref());
    let mut enc = g.create_render_encoder(&t);
    g.destroy_offscreen_target(t);
    assert_all_unsupported(call_mesh_ops(g, &mut enc, &Inputs::plain(&buf)));
}

#[test]
fn mesh_ops_ignore_argument_values() {
    let d = device();
    let other = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let own = buffer(d.as_ref());
    let foreign = buffer(other.as_ref());
    let mut enc = g.create_render_encoder(&t);
    let cases = [
        Inputs {
            group_count: [0, 0, 0],
            count: 0,
            max_count: 0,
            ..Inputs::plain(&own)
        },
        Inputs {
            group_count: [u32::MAX; 3],
            indirect_offset: u64::MAX,
            count_offset: u64::MAX,
            count: u32::MAX,
            max_count: u32::MAX,
            ..Inputs::plain(&own)
        },
        Inputs {
            indirect_offset: 1,
            count_offset: 1,
            ..Inputs::plain(&own)
        },
        Inputs {
            indirect: &foreign,
            count_buffer: &foreign,
            ..Inputs::plain(&own)
        },
    ];
    for i in &cases {
        assert_all_unsupported(call_mesh_ops(g, &mut enc, i));
    }
}

#[test]
fn mesh_ops_are_repeatable() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let buf = buffer(d.as_ref());
    let mut enc = g.create_render_encoder(&t);
    let first = call_mesh_ops(g, &mut enc, &Inputs::plain(&buf));
    let second = call_mesh_ops(g, &mut enc, &Inputs::plain(&buf));
    assert_eq!(first, second);
}

#[test]
fn mesh_ops_do_not_consume_encoder() {
    let d = device();
    let g = d.graphics().expect("graphics");
    let t = target(d.as_ref());
    let buf = buffer(d.as_ref());
    let mut enc = g.create_render_encoder(&t);
    let _ = call_mesh_ops(g, &mut enc, &Inputs::plain(&buf));
    let _ = g.submit_render(enc, &[]);
}
