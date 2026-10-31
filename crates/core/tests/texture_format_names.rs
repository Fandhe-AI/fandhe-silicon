//! テクスチャの形式の型の値と名前の対応を確かめる公開テスト（TASK-1312）。
//!
//! 期待の名前は、比較基準の `wgpu-types` 30.0.1 の公開のソースの variant の綴りと宣言順から取った。
//! `TextureFormat` は 77 個（ASTC 以外の 76 は具体形式 1 件ずつ、`Astc` は 42 件）、`AstcBlock` は 14、
//! `AstcChannel` は 3。件数だけの確かめは `texture_format_counts.rs` が持つ。

use fandhe_silicon_core::texture_format::{AstcBlock, AstcChannel, TextureFormat};
use std::collections::HashSet;
use std::fmt::Debug;

const TEXTURE_FORMAT_NAMES: [&str; 77] = [
    "R8Unorm",
    "R8Snorm",
    "R8Uint",
    "R8Sint",
    "R16Uint",
    "R16Sint",
    "R16Unorm",
    "R16Snorm",
    "R16Float",
    "Rg8Unorm",
    "Rg8Snorm",
    "Rg8Uint",
    "Rg8Sint",
    "R32Uint",
    "R32Sint",
    "R32Float",
    "Rg16Uint",
    "Rg16Sint",
    "Rg16Unorm",
    "Rg16Snorm",
    "Rg16Float",
    "Rgba8Unorm",
    "Rgba8UnormSrgb",
    "Rgba8Snorm",
    "Rgba8Uint",
    "Rgba8Sint",
    "Bgra8Unorm",
    "Bgra8UnormSrgb",
    "Rgb9e5Ufloat",
    "Rgb10a2Uint",
    "Rgb10a2Unorm",
    "Rg11b10Ufloat",
    "R64Uint",
    "Rg32Uint",
    "Rg32Sint",
    "Rg32Float",
    "Rgba16Uint",
    "Rgba16Sint",
    "Rgba16Unorm",
    "Rgba16Snorm",
    "Rgba16Float",
    "Rgba32Uint",
    "Rgba32Sint",
    "Rgba32Float",
    "Stencil8",
    "Depth16Unorm",
    "Depth24Plus",
    "Depth24PlusStencil8",
    "Depth32Float",
    "Depth32FloatStencil8",
    "NV12",
    "P010",
    "Bc1RgbaUnorm",
    "Bc1RgbaUnormSrgb",
    "Bc2RgbaUnorm",
    "Bc2RgbaUnormSrgb",
    "Bc3RgbaUnorm",
    "Bc3RgbaUnormSrgb",
    "Bc4RUnorm",
    "Bc4RSnorm",
    "Bc5RgUnorm",
    "Bc5RgSnorm",
    "Bc6hRgbUfloat",
    "Bc6hRgbFloat",
    "Bc7RgbaUnorm",
    "Bc7RgbaUnormSrgb",
    "Etc2Rgb8Unorm",
    "Etc2Rgb8UnormSrgb",
    "Etc2Rgb8A1Unorm",
    "Etc2Rgb8A1UnormSrgb",
    "Etc2Rgba8Unorm",
    "Etc2Rgba8UnormSrgb",
    "EacR11Unorm",
    "EacR11Snorm",
    "EacRg11Unorm",
    "EacRg11Snorm",
    "Astc",
];

const ASTC_BLOCK_NAMES: [&str; 14] = [
    "B4x4", "B5x4", "B5x5", "B6x5", "B6x6", "B8x5", "B8x6", "B8x8", "B10x5", "B10x6", "B10x8",
    "B10x10", "B12x10", "B12x12",
];

const ASTC_CHANNEL_NAMES: [&str; 3] = ["Unorm", "UnormSrgb", "Hdr"];

/// `Debug` 表記の先頭の語（`Astc { .. }` は `Astc`）を返す。
fn head(v: &impl Debug) -> String {
    format!("{v:?}")
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn every_name_maps_to_values() {
    let all = TextureFormat::all();
    for name in TEXTURE_FORMAT_NAMES {
        let hits: Vec<&TextureFormat> = all.iter().filter(|f| head(*f) == name).collect();
        if name == "Astc" {
            assert_eq!(hits.len(), 42);
        } else {
            assert_eq!(hits.len(), 1, "{name}");
            assert_eq!(format!("{:?}", hits[0]), name);
        }
    }
}

#[test]
fn every_value_maps_to_a_name() {
    let mut order: Vec<String> = Vec::new();
    for f in TextureFormat::all() {
        let h = head(f);
        assert!(TEXTURE_FORMAT_NAMES.contains(&h.as_str()), "{h}");
        if !order.contains(&h) {
            order.push(h);
        }
    }
    assert_eq!(order, TEXTURE_FORMAT_NAMES);
}

#[test]
fn all_has_no_duplicates() {
    let all = TextureFormat::all();
    let set: HashSet<&TextureFormat> = all.iter().collect();
    assert_eq!(set.len(), all.len());
    assert_eq!(all.len(), 118);
}

#[test]
fn astc_concrete_formats_are_block_by_channel() {
    let mut pairs: Vec<(AstcBlock, AstcChannel)> = Vec::new();
    for f in TextureFormat::all() {
        if let TextureFormat::Astc { block, channel } = f {
            assert_eq!(
                format!("{f:?}"),
                format!("Astc {{ block: {block:?}, channel: {channel:?} }}")
            );
            pairs.push((*block, *channel));
        }
    }
    assert_eq!(pairs.len(), 42);
    for b in AstcBlock::all() {
        for c in AstcChannel::all() {
            let n = pairs.iter().filter(|p| p.0 == *b && p.1 == *c).count();
            assert_eq!(n, 1, "{b:?} {c:?}");
        }
    }
}

#[test]
fn astc_block_names_match() {
    let got: Vec<String> = AstcBlock::all().iter().map(|b| format!("{b:?}")).collect();
    assert_eq!(got, ASTC_BLOCK_NAMES);
    let set: HashSet<&String> = got.iter().collect();
    assert_eq!(set.len(), 14);
}

#[test]
fn astc_channel_names_match() {
    let got: Vec<String> = AstcChannel::all()
        .iter()
        .map(|c| format!("{c:?}"))
        .collect();
    assert_eq!(got, ASTC_CHANNEL_NAMES);
    let set: HashSet<&String> = got.iter().collect();
    assert_eq!(set.len(), 3);
}
