//! テクスチャのコピーの指定の型と view の型を、別の crate から公開のパスだけで確かめる統合テスト（TASK-372）。
//!
//! feature なしで流れる。値だけを扱う型の組み立て・読み出し・借用の関係を見る。

use fandhe_silicon_core::texture::{
    TextureCopySpec, TextureDescriptor, TextureDimension, TextureExtent, TextureHandle,
    TextureOrigin, TextureUsage, TextureView,
};
use fandhe_silicon_core::texture_format::TextureFormat;

fn handle(generation: u64, id: u64, device_id: u64) -> TextureHandle {
    let desc = TextureDescriptor::new(
        TextureDimension::D2,
        TextureExtent {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        TextureFormat::Rgba8Unorm,
        TextureUsage::empty(),
    );
    TextureHandle::__backend_new(generation, id, device_id, &desc)
}

// TEX-20
#[test]
fn copy_spec_whole_points_to_mip0_origin0() {
    let h = handle(1, 2, 3);
    let spec = TextureCopySpec::whole(&h);
    assert_eq!(spec.mip_level, 0);
    assert_eq!(spec.origin, TextureOrigin { x: 0, y: 0, z: 0 });
    assert!(core::ptr::eq(spec.texture, &h));
}

// TEX-20
#[test]
fn copy_spec_new_keeps_values_unchecked() {
    let h = handle(1, 2, 3);
    let spec = TextureCopySpec::new(&h, 3, TextureOrigin { x: 1, y: 2, z: 5 });
    assert_eq!(spec.mip_level, 3);
    assert_eq!(spec.origin, TextureOrigin { x: 1, y: 2, z: 5 });
    let max = u32::MAX;
    let origin = TextureOrigin {
        x: max,
        y: max,
        z: max,
    };
    let spec = TextureCopySpec::new(&h, max, origin);
    assert_eq!(spec.mip_level, max);
    assert_eq!(spec.origin, origin);
}

// TEX-20
#[test]
fn copy_spec_is_copy_with_borrowed_texture() {
    let h = handle(1, 2, 3);
    let a = TextureCopySpec::whole(&h);
    let b = a;
    assert!(core::ptr::eq(a.texture, &h));
    assert!(core::ptr::eq(b.texture, &h));
    assert!(core::ptr::eq(a.texture, b.texture));
}

// TEX-20
#[test]
fn copy_spec_fields_are_writable() {
    let (h, h2) = (handle(1, 2, 3), handle(4, 5, 6));
    let mut spec = TextureCopySpec::whole(&h);
    spec.mip_level = 2;
    spec.origin = TextureOrigin { x: 7, y: 8, z: 9 };
    spec.texture = &h2;
    assert_eq!(spec.mip_level, 2);
    assert_eq!(spec.origin, TextureOrigin { x: 7, y: 8, z: 9 });
    assert!(core::ptr::eq(spec.texture, &h2));
}

// TEX-30
#[test]
fn view_texture_returns_the_same_handle() {
    let h = handle(1, 2, 3);
    let v = TextureView::__backend_new(&h, 4, 5, 6);
    assert!(core::ptr::eq(v.texture(), &h));
}

// TEX-30
#[test]
fn view_texture_outlives_view() {
    let h = handle(1, 2, 3);
    let t = {
        let v = TextureView::__backend_new(&h, 1, 2, 3);
        v.texture()
    };
    assert!(core::ptr::eq(t, &h));
    assert_eq!(t.__backend_id(), 2);
}

// TEX-18
#[test]
fn two_views_borrow_one_texture() {
    let h = handle(1, 2, 3);
    let v1 = TextureView::__backend_new(&h, 10, 11, 12);
    let v2 = TextureView::__backend_new(&h, 20, 21, 22);
    assert!(core::ptr::eq(v1.texture(), &h));
    assert!(core::ptr::eq(v2.texture(), &h));
    assert_eq!(v1.__backend_id(), 11);
    assert_eq!(v2.__backend_id(), 21);
}

// TEX-18
#[test]
fn view_backend_readers_round_trip() {
    let h = handle(1, 2, 3);
    for (g, i, d) in [(7, 8, 9), (0, 0, 0), (u64::MAX, u64::MAX, u64::MAX)] {
        let v = TextureView::__backend_new(&h, g, i, d);
        assert_eq!(
            (
                v.__backend_generation(),
                v.__backend_id(),
                v.__backend_device_id()
            ),
            (g, i, d)
        );
    }
}
