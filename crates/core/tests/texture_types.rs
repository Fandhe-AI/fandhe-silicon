//! テクスチャの記述子・handle の問い合わせと値の型を、別の crate から公開のパスだけで確かめる（TASK-371 のポインタ）。
//!
//! feature なしで流れる。

use fandhe_silicon_core::texture::{
    TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage,
};
use fandhe_silicon_core::texture_format::{AstcBlock, AstcChannel, TextureFormat};

const ALL_USAGES: [TextureUsage; 7] = [
    TextureUsage::COPY_SRC,
    TextureUsage::COPY_DST,
    TextureUsage::TEXTURE_BINDING,
    TextureUsage::STORAGE_BINDING,
    TextureUsage::RENDER_ATTACHMENT,
    TextureUsage::TRANSIENT_ATTACHMENT,
    TextureUsage::STORAGE_ATOMIC,
];

fn all_usage() -> TextureUsage {
    ALL_USAGES
        .iter()
        .fold(TextureUsage::empty(), |acc, u| acc | *u)
}

fn ext(width: u32, height: u32, depth_or_array_layers: u32) -> TextureExtent {
    TextureExtent {
        width,
        height,
        depth_or_array_layers,
    }
}

/// 名前つきの記述子の行。
fn rows() -> Vec<(&'static str, TextureDescriptor<'static>)> {
    let mut d1 = TextureDescriptor::new(
        TextureDimension::D1,
        ext(256, 1, 1),
        TextureFormat::Rgba8Unorm,
        TextureUsage::empty(),
    );
    d1.label = Some("d1");

    let d2 = TextureDescriptor::new(
        TextureDimension::D2,
        ext(640, 480, 1),
        TextureFormat::Depth32Float,
        TextureUsage::RENDER_ATTACHMENT,
    );

    let mut arr = TextureDescriptor::new(
        TextureDimension::D2,
        ext(512, 512, 6),
        TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Unorm,
        },
        TextureUsage::TEXTURE_BINDING | TextureUsage::COPY_DST,
    );
    arr.mip_level_count = 9;
    arr.label = Some("array");

    let mut ms = TextureDescriptor::new(
        TextureDimension::D2,
        ext(1920, 1080, 1),
        TextureFormat::Rgba8Unorm,
        all_usage(),
    );
    ms.sample_count = 4;
    ms.view_formats = &[TextureFormat::Rgba8UnormSrgb];

    let mut d3 = TextureDescriptor::new(
        TextureDimension::D3,
        ext(32, 32, 32),
        TextureFormat::Rgba8Unorm,
        TextureUsage::STORAGE_BINDING,
    );
    d3.mip_level_count = 6;

    vec![
        ("1d", d1),
        ("2d", d2),
        ("2d-array", arr),
        ("2d-multisample", ms),
        ("3d", d3),
    ]
}

fn check_queries(name: &str, h: &TextureHandle, desc: &TextureDescriptor<'_>) {
    assert_eq!(h.size(), desc.size, "{name}: size");
    assert_eq!(h.width(), desc.size.width, "{name}: width");
    assert_eq!(h.height(), desc.size.height, "{name}: height");
    assert_eq!(
        h.depth_or_array_layers(),
        desc.size.depth_or_array_layers,
        "{name}: depth_or_array_layers"
    );
    assert_eq!(
        h.mip_level_count(),
        desc.mip_level_count,
        "{name}: mip_level_count"
    );
    assert_eq!(h.sample_count(), desc.sample_count, "{name}: sample_count");
    assert_eq!(h.dimension(), desc.dimension, "{name}: dimension");
    assert_eq!(h.format(), desc.format, "{name}: format");
    assert_eq!(h.usage(), desc.usage, "{name}: usage");
}

/// 9 つの問い合わせが、渡した記述子の値をそのまま返す。
#[test]
fn queries_return_descriptor_values() {
    for (name, desc) in rows() {
        let h = TextureHandle::__backend_new(1, 2, 3, &desc);
        check_queries(name, &h, &desc);
    }
}

/// 0 と `u32::MAX` を入れても、値を検査せずそのまま返す。
#[test]
fn queries_return_boundary_values_unchanged() {
    for v in [0u32, u32::MAX] {
        let mut desc = TextureDescriptor::new(
            TextureDimension::D3,
            ext(v, v, v),
            TextureFormat::Rgba8Unorm,
            TextureUsage::COPY_SRC,
        );
        desc.mip_level_count = v;
        desc.sample_count = v;
        let h = TextureHandle::__backend_new(0, 0, 0, &desc);
        check_queries(&format!("boundary-{v}"), &h, &desc);
    }
}

/// `new` の既定の値と、渡した 4 つの値がそのまま入ること。
#[test]
fn descriptor_new_defaults() {
    let desc = TextureDescriptor::new(
        TextureDimension::D2,
        ext(7, 8, 9),
        TextureFormat::Depth32Float,
        TextureUsage::COPY_DST,
    );
    assert_eq!(desc.label, None);
    assert_eq!(desc.mip_level_count, 1);
    assert_eq!(desc.sample_count, 1);
    assert!(desc.view_formats.is_empty());
    assert_eq!(desc.dimension, TextureDimension::D2);
    assert_eq!(desc.size, ext(7, 8, 9));
    assert_eq!(desc.format, TextureFormat::Depth32Float);
    assert_eq!(desc.usage, TextureUsage::COPY_DST);
}

fn assert_static<T: 'static>() {}

/// 記述子と、それが借りていた値を破棄した後でも、handle の問い合わせが値を返す。
#[test]
fn handle_outlives_descriptor() {
    let h = {
        let label = String::from("short-lived");
        let views = vec![TextureFormat::Rgba8UnormSrgb];
        let mut desc = TextureDescriptor::new(
            TextureDimension::D2,
            ext(16, 8, 2),
            TextureFormat::Rgba8Unorm,
            TextureUsage::TEXTURE_BINDING,
        );
        desc.label = Some(&label);
        desc.view_formats = &views;
        desc.mip_level_count = 3;
        desc.sample_count = 4;
        TextureHandle::__backend_new(1, 2, 3, &desc)
    };
    assert_eq!(h.size(), ext(16, 8, 2));
    assert_eq!(h.mip_level_count(), 3);
    assert_eq!(h.sample_count(), 4);
    assert_eq!(h.dimension(), TextureDimension::D2);
    assert_eq!(h.format(), TextureFormat::Rgba8Unorm);
    assert_eq!(h.usage(), TextureUsage::TEXTURE_BINDING);
    assert_static::<TextureHandle>();
}

/// 背後実装向けの読み出しが、渡した値を往復で返す。
#[test]
fn backend_readers_round_trip() {
    let desc = TextureDescriptor::new(
        TextureDimension::D2,
        ext(1, 1, 1),
        TextureFormat::Rgba8Unorm,
        TextureUsage::empty(),
    );
    for (g, i, d) in [(0, 0, 0), (u64::MAX, u64::MAX, u64::MAX), (5, 1 << 40, 77)] {
        let h = TextureHandle::__backend_new(g, i, d, &desc);
        assert_eq!(h.__backend_generation(), g);
        assert_eq!(h.__backend_id(), i);
        assert_eq!(h.__backend_device_id(), d);
    }
}

/// 名前・定数・期待するビット値の表。
const USAGES: [(&str, TextureUsage, u32); 7] = [
    ("COPY_SRC", TextureUsage::COPY_SRC, 1 << 0),
    ("COPY_DST", TextureUsage::COPY_DST, 1 << 1),
    ("TEXTURE_BINDING", TextureUsage::TEXTURE_BINDING, 1 << 2),
    ("STORAGE_BINDING", TextureUsage::STORAGE_BINDING, 1 << 3),
    ("RENDER_ATTACHMENT", TextureUsage::RENDER_ATTACHMENT, 1 << 4),
    (
        "TRANSIENT_ATTACHMENT",
        TextureUsage::TRANSIENT_ATTACHMENT,
        1 << 5,
    ),
    ("STORAGE_ATOMIC", TextureUsage::STORAGE_ATOMIC, 1 << 6),
];

/// 用途の 7 定数が、表のビット値を持ち、互いに重ならず、論理和が 0x7f になる。
#[test]
fn usage_constants_have_listed_bits() {
    let mut union = 0u32;
    for (name, usage, expected) in USAGES {
        assert_eq!(usage.bits(), expected, "{name}: bits");
        assert_eq!(usage.bits().count_ones(), 1, "{name}: single bit");
        assert_eq!(union & usage.bits(), 0, "{name}: overlaps");
        union |= usage.bits();
    }
    assert_eq!(union, 0x7f);
    assert_eq!(all_usage().bits(), 0x7f);
}

fn dim_name(d: TextureDimension) -> &'static str {
    match d {
        TextureDimension::D1 => "1d",
        TextureDimension::D2 => "2d",
        TextureDimension::D3 => "3d",
        _ => "other",
    }
}

/// 次元の 3 値が外の crate から照合でき、互いに異なる。
#[test]
fn dimension_values_from_outside() {
    assert_eq!(dim_name(TextureDimension::D1), "1d");
    assert_eq!(dim_name(TextureDimension::D2), "2d");
    assert_eq!(dim_name(TextureDimension::D3), "3d");
    assert_ne!(TextureDimension::D1, TextureDimension::D2);
    assert_ne!(TextureDimension::D2, TextureDimension::D3);
    assert_ne!(TextureDimension::D1, TextureDimension::D3);
}
