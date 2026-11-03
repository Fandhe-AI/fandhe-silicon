//! 試験用の device の `TextureOps` で、テクスチャと view の破棄を確かめる統合テスト（TASK-372）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`texture`）だけを使う。
//! `backend` は `BackendKind::Vulkan` に固定する。番号の具体の値は固定せず、違うことと続きであることだけを見る。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::stub_device::StubDeviceProvider;
use fandhe_silicon_core::texture::{
    TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureOps, TextureUsage,
    TextureView,
};
use fandhe_silicon_core::texture_format::TextureFormat;

fn device(profile: Profile) -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Vulkan,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("device")
}

fn desc() -> TextureDescriptor<'static> {
    TextureDescriptor::new(
        TextureDimension::D2,
        TextureExtent {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        TextureFormat::Rgba8Unorm,
        TextureUsage::TEXTURE_BINDING,
    )
}

fn create(ops: &dyn TextureOps) -> TextureHandle {
    ops.create_texture(&desc()).expect("texture")
}

// TEX-19
#[test]
fn destroyed_texture_id_is_not_reused() {
    for profile in [Profile::ComputeOnly, Profile::Graphics] {
        let d = device(profile);
        let ops = d.textures().expect("textures");
        let first = create(ops);
        let first_id = first.__backend_id();
        ops.destroy_texture(first);
        let second = create(ops);
        assert_ne!(second.__backend_id(), first_id);
    }
}

// TEX-19
#[test]
fn destroying_unknown_texture_keeps_numbering() {
    let d = device(Profile::ComputeOnly);
    let ops = d.textures().expect("textures");
    let first = create(ops);
    let first_id = first.__backend_id();
    let unknown =
        TextureHandle::__backend_new(d.generation(), first_id + 1000, d.generation(), &desc());
    ops.destroy_texture(unknown);
    let second = create(ops);
    assert_eq!(second.__backend_id(), first_id + 1);
}

// TEX-19
#[test]
fn destroying_other_device_texture_is_noop() {
    let a = device(Profile::ComputeOnly);
    let b = device(Profile::ComputeOnly);
    let ops_a = a.textures().expect("textures");
    let ops_b = b.textures().expect("textures");
    let ha = create(ops_a);
    let hb = create(ops_b);
    ops_b.destroy_texture(ha);
    ops_a.destroy_texture(hb);
    assert!(ops_a.create_texture(&desc()).is_ok());
    assert!(ops_b.create_texture(&desc()).is_ok());
}

// TEX-19
#[test]
fn destroy_view_then_texture_through_dyn() {
    let d = device(Profile::Graphics);
    let ops: &dyn TextureOps = d.textures().expect("textures");
    let h = create(ops);
    let v = TextureView::__backend_new(&h, 1, 2, 3);
    ops.destroy_view(v);
    ops.destroy_texture(h);
}
