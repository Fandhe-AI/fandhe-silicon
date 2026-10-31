//! テクスチャの形式の 3 型の `#[non_exhaustive]` を、別の crate から固定する公開テスト（TASK-1312）。
//!
//! 仕組みは `binding_non_exhaustive.rs` と同じ。`#![deny(unreachable_patterns)]` のもとで全 variant を
//! 並べた後に `_` の腕を置く。属性が外れると `_` が到達不能になって落ちる。`Astc` の variant 自体には
//! 属性が付かないので、別の crate が値を作り、フィールドを `..` なしで束縛できることも確かめる。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::texture_format::{AstcBlock, AstcChannel, TextureFormat};

#[test]
fn texture_format_stays_non_exhaustive() {
    fn label(v: TextureFormat) -> &'static str {
        use TextureFormat as F;
        match v {
            F::R8Unorm => "R8Unorm",
            F::R8Snorm => "R8Snorm",
            F::R8Uint => "R8Uint",
            F::R8Sint => "R8Sint",
            F::R16Uint => "R16Uint",
            F::R16Sint => "R16Sint",
            F::R16Unorm => "R16Unorm",
            F::R16Snorm => "R16Snorm",
            F::R16Float => "R16Float",
            F::Rg8Unorm => "Rg8Unorm",
            F::Rg8Snorm => "Rg8Snorm",
            F::Rg8Uint => "Rg8Uint",
            F::Rg8Sint => "Rg8Sint",
            F::R32Uint => "R32Uint",
            F::R32Sint => "R32Sint",
            F::R32Float => "R32Float",
            F::Rg16Uint => "Rg16Uint",
            F::Rg16Sint => "Rg16Sint",
            F::Rg16Unorm => "Rg16Unorm",
            F::Rg16Snorm => "Rg16Snorm",
            F::Rg16Float => "Rg16Float",
            F::Rgba8Unorm => "Rgba8Unorm",
            F::Rgba8UnormSrgb => "Rgba8UnormSrgb",
            F::Rgba8Snorm => "Rgba8Snorm",
            F::Rgba8Uint => "Rgba8Uint",
            F::Rgba8Sint => "Rgba8Sint",
            F::Bgra8Unorm => "Bgra8Unorm",
            F::Bgra8UnormSrgb => "Bgra8UnormSrgb",
            F::Rgb9e5Ufloat => "Rgb9e5Ufloat",
            F::Rgb10a2Uint => "Rgb10a2Uint",
            F::Rgb10a2Unorm => "Rgb10a2Unorm",
            F::Rg11b10Ufloat => "Rg11b10Ufloat",
            F::R64Uint => "R64Uint",
            F::Rg32Uint => "Rg32Uint",
            F::Rg32Sint => "Rg32Sint",
            F::Rg32Float => "Rg32Float",
            F::Rgba16Uint => "Rgba16Uint",
            F::Rgba16Sint => "Rgba16Sint",
            F::Rgba16Unorm => "Rgba16Unorm",
            F::Rgba16Snorm => "Rgba16Snorm",
            F::Rgba16Float => "Rgba16Float",
            F::Rgba32Uint => "Rgba32Uint",
            F::Rgba32Sint => "Rgba32Sint",
            F::Rgba32Float => "Rgba32Float",
            F::Stencil8 => "Stencil8",
            F::Depth16Unorm => "Depth16Unorm",
            F::Depth24Plus => "Depth24Plus",
            F::Depth24PlusStencil8 => "Depth24PlusStencil8",
            F::Depth32Float => "Depth32Float",
            F::Depth32FloatStencil8 => "Depth32FloatStencil8",
            F::NV12 => "NV12",
            F::P010 => "P010",
            F::Bc1RgbaUnorm => "Bc1RgbaUnorm",
            F::Bc1RgbaUnormSrgb => "Bc1RgbaUnormSrgb",
            F::Bc2RgbaUnorm => "Bc2RgbaUnorm",
            F::Bc2RgbaUnormSrgb => "Bc2RgbaUnormSrgb",
            F::Bc3RgbaUnorm => "Bc3RgbaUnorm",
            F::Bc3RgbaUnormSrgb => "Bc3RgbaUnormSrgb",
            F::Bc4RUnorm => "Bc4RUnorm",
            F::Bc4RSnorm => "Bc4RSnorm",
            F::Bc5RgUnorm => "Bc5RgUnorm",
            F::Bc5RgSnorm => "Bc5RgSnorm",
            F::Bc6hRgbUfloat => "Bc6hRgbUfloat",
            F::Bc6hRgbFloat => "Bc6hRgbFloat",
            F::Bc7RgbaUnorm => "Bc7RgbaUnorm",
            F::Bc7RgbaUnormSrgb => "Bc7RgbaUnormSrgb",
            F::Etc2Rgb8Unorm => "Etc2Rgb8Unorm",
            F::Etc2Rgb8UnormSrgb => "Etc2Rgb8UnormSrgb",
            F::Etc2Rgb8A1Unorm => "Etc2Rgb8A1Unorm",
            F::Etc2Rgb8A1UnormSrgb => "Etc2Rgb8A1UnormSrgb",
            F::Etc2Rgba8Unorm => "Etc2Rgba8Unorm",
            F::Etc2Rgba8UnormSrgb => "Etc2Rgba8UnormSrgb",
            F::EacR11Unorm => "EacR11Unorm",
            F::EacR11Snorm => "EacR11Snorm",
            F::EacRg11Unorm => "EacRg11Unorm",
            F::EacRg11Snorm => "EacRg11Snorm",
            F::Astc { .. } => "Astc",
            _ => "other",
        }
    }
    assert_eq!(TextureFormat::all().len(), 118);
    for f in TextureFormat::all() {
        let l = label(*f);
        assert_ne!(l, "other", "{f:?}");
        assert!(format!("{f:?}").starts_with(l), "{f:?}");
    }
}

#[test]
fn astc_block_stays_non_exhaustive() {
    fn label(v: AstcBlock) -> &'static str {
        match v {
            AstcBlock::B4x4 => "B4x4",
            AstcBlock::B5x4 => "B5x4",
            AstcBlock::B5x5 => "B5x5",
            AstcBlock::B6x5 => "B6x5",
            AstcBlock::B6x6 => "B6x6",
            AstcBlock::B8x5 => "B8x5",
            AstcBlock::B8x6 => "B8x6",
            AstcBlock::B8x8 => "B8x8",
            AstcBlock::B10x5 => "B10x5",
            AstcBlock::B10x6 => "B10x6",
            AstcBlock::B10x8 => "B10x8",
            AstcBlock::B10x10 => "B10x10",
            AstcBlock::B12x10 => "B12x10",
            AstcBlock::B12x12 => "B12x12",
            _ => "other",
        }
    }
    for b in AstcBlock::all() {
        assert_eq!(label(*b), format!("{b:?}"));
    }
}

#[test]
fn astc_channel_stays_non_exhaustive() {
    fn label(v: AstcChannel) -> &'static str {
        match v {
            AstcChannel::Unorm => "Unorm",
            AstcChannel::UnormSrgb => "UnormSrgb",
            AstcChannel::Hdr => "Hdr",
            _ => "other",
        }
    }
    for c in AstcChannel::all() {
        assert_eq!(label(*c), format!("{c:?}"));
    }
}

#[test]
fn astc_variant_is_constructible_and_matchable() {
    let made = TextureFormat::Astc {
        block: AstcBlock::B6x6,
        channel: AstcChannel::Hdr,
    };
    match made {
        TextureFormat::Astc { block, channel } => {
            assert_eq!(block, AstcBlock::B6x6);
            assert_eq!(channel, AstcChannel::Hdr);
        }
        _ => panic!("Astc 以外に一致した"),
    }
}
