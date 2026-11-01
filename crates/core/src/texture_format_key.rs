//! テクスチャの形式の問合せに渡す鍵の一覧と、機能一覧の行と鍵・形式の値の対応の表（TASK-1313）。
//!
//! 鍵は静的な定数（`FeatureId`）で、`TextureFormat` の値ごとに 1 件ある（118 件）。
//! 能力問合せの口（`capability` モジュール）へ渡す側が、形式の値から決まった鍵を引けるようにする置き場で、
//! 背後実装の型・規則は含まない。feature の条件なしで公開される。
//!
//! 鍵の文字列は `texformat:` に続けて、ASTC 以外は `TextureFormat` の variant 名、ASTC は
//! `Astc.<block>.<channel>`（`AstcBlock`・`AstcChannel` の variant 名）を並べる。

use crate::capability::FeatureId;
use crate::texture_format::{AstcBlock, AstcChannel, TextureFormat};

/// 機能一覧の 1 つの鍵と、対応する `TextureFormat` の値の組。
///
/// # Examples
///
/// ```
/// use fandhe_silicon_core::texture_format::TextureFormat;
/// use fandhe_silicon_core::texture_format_key::{TEXFORMAT_R8_UNORM, TEXTURE_FORMAT_ROWS};
///
/// let first = TEXTURE_FORMAT_ROWS[0].keys[0];
/// assert_eq!(first.key, TEXFORMAT_R8_UNORM);
/// assert_eq!(first.format, TextureFormat::R8Unorm);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureFormatKey {
    /// 形式の問合せに渡す鍵。
    pub key: FeatureId,
    /// 鍵に対応する `TextureFormat` の値。
    pub format: TextureFormat,
}

/// 機能一覧の 1 行（形式の名前）と、その行に属する鍵の並び。
///
/// ASTC 以外の行は鍵が 1 件、`"Astc"` の行は 42 件（block の順を外、channel の順を内）。
///
/// # Examples
///
/// ```
/// use fandhe_silicon_core::texture_format_key::{TEXFORMAT_ASTC_B4X4_UNORM, TEXTURE_FORMAT_ROWS};
///
/// assert_eq!(TEXTURE_FORMAT_ROWS.len(), 77);
/// let astc = TEXTURE_FORMAT_ROWS.last().unwrap();
/// assert_eq!(astc.row, "Astc");
/// assert_eq!(astc.keys.len(), 42);
/// assert_eq!(TEXFORMAT_ASTC_B4X4_UNORM.0, "texformat:Astc.B4x4.Unorm");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureFormatRow {
    /// 機能一覧の行の名前（形式の名前。ASTC の行は `"Astc"`）。
    pub row: &'static str,
    /// 行に属する鍵と形式の値の組。
    pub keys: &'static [TextureFormatKey],
}

/// `TextureFormat::R8Unorm` の鍵。
pub const TEXFORMAT_R8_UNORM: FeatureId = FeatureId("texformat:R8Unorm");

/// `TextureFormat::R8Snorm` の鍵。
pub const TEXFORMAT_R8_SNORM: FeatureId = FeatureId("texformat:R8Snorm");

/// `TextureFormat::R8Uint` の鍵。
pub const TEXFORMAT_R8_UINT: FeatureId = FeatureId("texformat:R8Uint");

/// `TextureFormat::R8Sint` の鍵。
pub const TEXFORMAT_R8_SINT: FeatureId = FeatureId("texformat:R8Sint");

/// `TextureFormat::R16Uint` の鍵。
pub const TEXFORMAT_R16_UINT: FeatureId = FeatureId("texformat:R16Uint");

/// `TextureFormat::R16Sint` の鍵。
pub const TEXFORMAT_R16_SINT: FeatureId = FeatureId("texformat:R16Sint");

/// `TextureFormat::R16Unorm` の鍵。
pub const TEXFORMAT_R16_UNORM: FeatureId = FeatureId("texformat:R16Unorm");

/// `TextureFormat::R16Snorm` の鍵。
pub const TEXFORMAT_R16_SNORM: FeatureId = FeatureId("texformat:R16Snorm");

/// `TextureFormat::R16Float` の鍵。
pub const TEXFORMAT_R16_FLOAT: FeatureId = FeatureId("texformat:R16Float");

/// `TextureFormat::Rg8Unorm` の鍵。
pub const TEXFORMAT_RG8_UNORM: FeatureId = FeatureId("texformat:Rg8Unorm");

/// `TextureFormat::Rg8Snorm` の鍵。
pub const TEXFORMAT_RG8_SNORM: FeatureId = FeatureId("texformat:Rg8Snorm");

/// `TextureFormat::Rg8Uint` の鍵。
pub const TEXFORMAT_RG8_UINT: FeatureId = FeatureId("texformat:Rg8Uint");

/// `TextureFormat::Rg8Sint` の鍵。
pub const TEXFORMAT_RG8_SINT: FeatureId = FeatureId("texformat:Rg8Sint");

/// `TextureFormat::R32Uint` の鍵。
pub const TEXFORMAT_R32_UINT: FeatureId = FeatureId("texformat:R32Uint");

/// `TextureFormat::R32Sint` の鍵。
pub const TEXFORMAT_R32_SINT: FeatureId = FeatureId("texformat:R32Sint");

/// `TextureFormat::R32Float` の鍵。
pub const TEXFORMAT_R32_FLOAT: FeatureId = FeatureId("texformat:R32Float");

/// `TextureFormat::Rg16Uint` の鍵。
pub const TEXFORMAT_RG16_UINT: FeatureId = FeatureId("texformat:Rg16Uint");

/// `TextureFormat::Rg16Sint` の鍵。
pub const TEXFORMAT_RG16_SINT: FeatureId = FeatureId("texformat:Rg16Sint");

/// `TextureFormat::Rg16Unorm` の鍵。
pub const TEXFORMAT_RG16_UNORM: FeatureId = FeatureId("texformat:Rg16Unorm");

/// `TextureFormat::Rg16Snorm` の鍵。
pub const TEXFORMAT_RG16_SNORM: FeatureId = FeatureId("texformat:Rg16Snorm");

/// `TextureFormat::Rg16Float` の鍵。
pub const TEXFORMAT_RG16_FLOAT: FeatureId = FeatureId("texformat:Rg16Float");

/// `TextureFormat::Rgba8Unorm` の鍵。
pub const TEXFORMAT_RGBA8_UNORM: FeatureId = FeatureId("texformat:Rgba8Unorm");

/// `TextureFormat::Rgba8UnormSrgb` の鍵。
pub const TEXFORMAT_RGBA8_UNORM_SRGB: FeatureId = FeatureId("texformat:Rgba8UnormSrgb");

/// `TextureFormat::Rgba8Snorm` の鍵。
pub const TEXFORMAT_RGBA8_SNORM: FeatureId = FeatureId("texformat:Rgba8Snorm");

/// `TextureFormat::Rgba8Uint` の鍵。
pub const TEXFORMAT_RGBA8_UINT: FeatureId = FeatureId("texformat:Rgba8Uint");

/// `TextureFormat::Rgba8Sint` の鍵。
pub const TEXFORMAT_RGBA8_SINT: FeatureId = FeatureId("texformat:Rgba8Sint");

/// `TextureFormat::Bgra8Unorm` の鍵。
pub const TEXFORMAT_BGRA8_UNORM: FeatureId = FeatureId("texformat:Bgra8Unorm");

/// `TextureFormat::Bgra8UnormSrgb` の鍵。
pub const TEXFORMAT_BGRA8_UNORM_SRGB: FeatureId = FeatureId("texformat:Bgra8UnormSrgb");

/// `TextureFormat::Rgb9e5Ufloat` の鍵。
pub const TEXFORMAT_RGB9E5_UFLOAT: FeatureId = FeatureId("texformat:Rgb9e5Ufloat");

/// `TextureFormat::Rgb10a2Uint` の鍵。
pub const TEXFORMAT_RGB10A2_UINT: FeatureId = FeatureId("texformat:Rgb10a2Uint");

/// `TextureFormat::Rgb10a2Unorm` の鍵。
pub const TEXFORMAT_RGB10A2_UNORM: FeatureId = FeatureId("texformat:Rgb10a2Unorm");

/// `TextureFormat::Rg11b10Ufloat` の鍵。
pub const TEXFORMAT_RG11B10_UFLOAT: FeatureId = FeatureId("texformat:Rg11b10Ufloat");

/// `TextureFormat::R64Uint` の鍵。
pub const TEXFORMAT_R64_UINT: FeatureId = FeatureId("texformat:R64Uint");

/// `TextureFormat::Rg32Uint` の鍵。
pub const TEXFORMAT_RG32_UINT: FeatureId = FeatureId("texformat:Rg32Uint");

/// `TextureFormat::Rg32Sint` の鍵。
pub const TEXFORMAT_RG32_SINT: FeatureId = FeatureId("texformat:Rg32Sint");

/// `TextureFormat::Rg32Float` の鍵。
pub const TEXFORMAT_RG32_FLOAT: FeatureId = FeatureId("texformat:Rg32Float");

/// `TextureFormat::Rgba16Uint` の鍵。
pub const TEXFORMAT_RGBA16_UINT: FeatureId = FeatureId("texformat:Rgba16Uint");

/// `TextureFormat::Rgba16Sint` の鍵。
pub const TEXFORMAT_RGBA16_SINT: FeatureId = FeatureId("texformat:Rgba16Sint");

/// `TextureFormat::Rgba16Unorm` の鍵。
pub const TEXFORMAT_RGBA16_UNORM: FeatureId = FeatureId("texformat:Rgba16Unorm");

/// `TextureFormat::Rgba16Snorm` の鍵。
pub const TEXFORMAT_RGBA16_SNORM: FeatureId = FeatureId("texformat:Rgba16Snorm");

/// `TextureFormat::Rgba16Float` の鍵。
pub const TEXFORMAT_RGBA16_FLOAT: FeatureId = FeatureId("texformat:Rgba16Float");

/// `TextureFormat::Rgba32Uint` の鍵。
pub const TEXFORMAT_RGBA32_UINT: FeatureId = FeatureId("texformat:Rgba32Uint");

/// `TextureFormat::Rgba32Sint` の鍵。
pub const TEXFORMAT_RGBA32_SINT: FeatureId = FeatureId("texformat:Rgba32Sint");

/// `TextureFormat::Rgba32Float` の鍵。
pub const TEXFORMAT_RGBA32_FLOAT: FeatureId = FeatureId("texformat:Rgba32Float");

/// `TextureFormat::Stencil8` の鍵。
pub const TEXFORMAT_STENCIL8: FeatureId = FeatureId("texformat:Stencil8");

/// `TextureFormat::Depth16Unorm` の鍵。
pub const TEXFORMAT_DEPTH16_UNORM: FeatureId = FeatureId("texformat:Depth16Unorm");

/// `TextureFormat::Depth24Plus` の鍵。
pub const TEXFORMAT_DEPTH24_PLUS: FeatureId = FeatureId("texformat:Depth24Plus");

/// `TextureFormat::Depth24PlusStencil8` の鍵。
pub const TEXFORMAT_DEPTH24_PLUS_STENCIL8: FeatureId = FeatureId("texformat:Depth24PlusStencil8");

/// `TextureFormat::Depth32Float` の鍵。
pub const TEXFORMAT_DEPTH32_FLOAT: FeatureId = FeatureId("texformat:Depth32Float");

/// `TextureFormat::Depth32FloatStencil8` の鍵。
pub const TEXFORMAT_DEPTH32_FLOAT_STENCIL8: FeatureId = FeatureId("texformat:Depth32FloatStencil8");

/// `TextureFormat::NV12` の鍵。
pub const TEXFORMAT_NV12: FeatureId = FeatureId("texformat:NV12");

/// `TextureFormat::P010` の鍵。
pub const TEXFORMAT_P010: FeatureId = FeatureId("texformat:P010");

/// `TextureFormat::Bc1RgbaUnorm` の鍵。
pub const TEXFORMAT_BC1_RGBA_UNORM: FeatureId = FeatureId("texformat:Bc1RgbaUnorm");

/// `TextureFormat::Bc1RgbaUnormSrgb` の鍵。
pub const TEXFORMAT_BC1_RGBA_UNORM_SRGB: FeatureId = FeatureId("texformat:Bc1RgbaUnormSrgb");

/// `TextureFormat::Bc2RgbaUnorm` の鍵。
pub const TEXFORMAT_BC2_RGBA_UNORM: FeatureId = FeatureId("texformat:Bc2RgbaUnorm");

/// `TextureFormat::Bc2RgbaUnormSrgb` の鍵。
pub const TEXFORMAT_BC2_RGBA_UNORM_SRGB: FeatureId = FeatureId("texformat:Bc2RgbaUnormSrgb");

/// `TextureFormat::Bc3RgbaUnorm` の鍵。
pub const TEXFORMAT_BC3_RGBA_UNORM: FeatureId = FeatureId("texformat:Bc3RgbaUnorm");

/// `TextureFormat::Bc3RgbaUnormSrgb` の鍵。
pub const TEXFORMAT_BC3_RGBA_UNORM_SRGB: FeatureId = FeatureId("texformat:Bc3RgbaUnormSrgb");

/// `TextureFormat::Bc4RUnorm` の鍵。
pub const TEXFORMAT_BC4_R_UNORM: FeatureId = FeatureId("texformat:Bc4RUnorm");

/// `TextureFormat::Bc4RSnorm` の鍵。
pub const TEXFORMAT_BC4_R_SNORM: FeatureId = FeatureId("texformat:Bc4RSnorm");

/// `TextureFormat::Bc5RgUnorm` の鍵。
pub const TEXFORMAT_BC5_RG_UNORM: FeatureId = FeatureId("texformat:Bc5RgUnorm");

/// `TextureFormat::Bc5RgSnorm` の鍵。
pub const TEXFORMAT_BC5_RG_SNORM: FeatureId = FeatureId("texformat:Bc5RgSnorm");

/// `TextureFormat::Bc6hRgbUfloat` の鍵。
pub const TEXFORMAT_BC6H_RGB_UFLOAT: FeatureId = FeatureId("texformat:Bc6hRgbUfloat");

/// `TextureFormat::Bc6hRgbFloat` の鍵。
pub const TEXFORMAT_BC6H_RGB_FLOAT: FeatureId = FeatureId("texformat:Bc6hRgbFloat");

/// `TextureFormat::Bc7RgbaUnorm` の鍵。
pub const TEXFORMAT_BC7_RGBA_UNORM: FeatureId = FeatureId("texformat:Bc7RgbaUnorm");

/// `TextureFormat::Bc7RgbaUnormSrgb` の鍵。
pub const TEXFORMAT_BC7_RGBA_UNORM_SRGB: FeatureId = FeatureId("texformat:Bc7RgbaUnormSrgb");

/// `TextureFormat::Etc2Rgb8Unorm` の鍵。
pub const TEXFORMAT_ETC2_RGB8_UNORM: FeatureId = FeatureId("texformat:Etc2Rgb8Unorm");

/// `TextureFormat::Etc2Rgb8UnormSrgb` の鍵。
pub const TEXFORMAT_ETC2_RGB8_UNORM_SRGB: FeatureId = FeatureId("texformat:Etc2Rgb8UnormSrgb");

/// `TextureFormat::Etc2Rgb8A1Unorm` の鍵。
pub const TEXFORMAT_ETC2_RGB8_A1_UNORM: FeatureId = FeatureId("texformat:Etc2Rgb8A1Unorm");

/// `TextureFormat::Etc2Rgb8A1UnormSrgb` の鍵。
pub const TEXFORMAT_ETC2_RGB8_A1_UNORM_SRGB: FeatureId = FeatureId("texformat:Etc2Rgb8A1UnormSrgb");

/// `TextureFormat::Etc2Rgba8Unorm` の鍵。
pub const TEXFORMAT_ETC2_RGBA8_UNORM: FeatureId = FeatureId("texformat:Etc2Rgba8Unorm");

/// `TextureFormat::Etc2Rgba8UnormSrgb` の鍵。
pub const TEXFORMAT_ETC2_RGBA8_UNORM_SRGB: FeatureId = FeatureId("texformat:Etc2Rgba8UnormSrgb");

/// `TextureFormat::EacR11Unorm` の鍵。
pub const TEXFORMAT_EAC_R11_UNORM: FeatureId = FeatureId("texformat:EacR11Unorm");

/// `TextureFormat::EacR11Snorm` の鍵。
pub const TEXFORMAT_EAC_R11_SNORM: FeatureId = FeatureId("texformat:EacR11Snorm");

/// `TextureFormat::EacRg11Unorm` の鍵。
pub const TEXFORMAT_EAC_RG11_UNORM: FeatureId = FeatureId("texformat:EacRg11Unorm");

/// `TextureFormat::EacRg11Snorm` の鍵。
pub const TEXFORMAT_EAC_RG11_SNORM: FeatureId = FeatureId("texformat:EacRg11Snorm");

/// `TextureFormat::Astc { block: AstcBlock::B4x4, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B4X4_UNORM: FeatureId = FeatureId("texformat:Astc.B4x4.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B4x4, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B4X4_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B4x4.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B4x4, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B4X4_HDR: FeatureId = FeatureId("texformat:Astc.B4x4.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B5x4, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B5X4_UNORM: FeatureId = FeatureId("texformat:Astc.B5x4.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B5x4, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B5X4_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B5x4.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B5x4, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B5X4_HDR: FeatureId = FeatureId("texformat:Astc.B5x4.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B5x5, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B5X5_UNORM: FeatureId = FeatureId("texformat:Astc.B5x5.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B5x5, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B5X5_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B5x5.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B5x5, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B5X5_HDR: FeatureId = FeatureId("texformat:Astc.B5x5.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B6x5, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B6X5_UNORM: FeatureId = FeatureId("texformat:Astc.B6x5.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B6x5, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B6X5_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B6x5.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B6x5, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B6X5_HDR: FeatureId = FeatureId("texformat:Astc.B6x5.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B6x6, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B6X6_UNORM: FeatureId = FeatureId("texformat:Astc.B6x6.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B6x6, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B6X6_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B6x6.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B6x6, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B6X6_HDR: FeatureId = FeatureId("texformat:Astc.B6x6.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B8x5, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B8X5_UNORM: FeatureId = FeatureId("texformat:Astc.B8x5.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B8x5, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B8X5_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B8x5.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B8x5, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B8X5_HDR: FeatureId = FeatureId("texformat:Astc.B8x5.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B8x6, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B8X6_UNORM: FeatureId = FeatureId("texformat:Astc.B8x6.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B8x6, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B8X6_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B8x6.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B8x6, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B8X6_HDR: FeatureId = FeatureId("texformat:Astc.B8x6.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B8x8, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B8X8_UNORM: FeatureId = FeatureId("texformat:Astc.B8x8.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B8x8, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B8X8_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B8x8.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B8x8, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B8X8_HDR: FeatureId = FeatureId("texformat:Astc.B8x8.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B10x5, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B10X5_UNORM: FeatureId = FeatureId("texformat:Astc.B10x5.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B10x5, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B10X5_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B10x5.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B10x5, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B10X5_HDR: FeatureId = FeatureId("texformat:Astc.B10x5.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B10x6, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B10X6_UNORM: FeatureId = FeatureId("texformat:Astc.B10x6.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B10x6, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B10X6_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B10x6.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B10x6, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B10X6_HDR: FeatureId = FeatureId("texformat:Astc.B10x6.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B10x8, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B10X8_UNORM: FeatureId = FeatureId("texformat:Astc.B10x8.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B10x8, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B10X8_UNORM_SRGB: FeatureId = FeatureId("texformat:Astc.B10x8.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B10x8, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B10X8_HDR: FeatureId = FeatureId("texformat:Astc.B10x8.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B10x10, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B10X10_UNORM: FeatureId = FeatureId("texformat:Astc.B10x10.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B10x10, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B10X10_UNORM_SRGB: FeatureId =
    FeatureId("texformat:Astc.B10x10.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B10x10, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B10X10_HDR: FeatureId = FeatureId("texformat:Astc.B10x10.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B12x10, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B12X10_UNORM: FeatureId = FeatureId("texformat:Astc.B12x10.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B12x10, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B12X10_UNORM_SRGB: FeatureId =
    FeatureId("texformat:Astc.B12x10.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B12x10, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B12X10_HDR: FeatureId = FeatureId("texformat:Astc.B12x10.Hdr");

/// `TextureFormat::Astc { block: AstcBlock::B12x12, channel: AstcChannel::Unorm }` の鍵。
pub const TEXFORMAT_ASTC_B12X12_UNORM: FeatureId = FeatureId("texformat:Astc.B12x12.Unorm");

/// `TextureFormat::Astc { block: AstcBlock::B12x12, channel: AstcChannel::UnormSrgb }` の鍵。
pub const TEXFORMAT_ASTC_B12X12_UNORM_SRGB: FeatureId =
    FeatureId("texformat:Astc.B12x12.UnormSrgb");

/// `TextureFormat::Astc { block: AstcBlock::B12x12, channel: AstcChannel::Hdr }` の鍵。
pub const TEXFORMAT_ASTC_B12X12_HDR: FeatureId = FeatureId("texformat:Astc.B12x12.Hdr");

/// 形式の問合せに渡す鍵の全件（118 件）。並びは `TextureFormat::all()` と同じで、ASTC 以外の 76 を宣言順に、続けて ASTC を block の順を外、channel の順を内にして 42 件並べる（TASK-1313）。
pub const TEXTURE_FORMAT_QUERY_KEYS: &[FeatureId] = &[
    TEXFORMAT_R8_UNORM,
    TEXFORMAT_R8_SNORM,
    TEXFORMAT_R8_UINT,
    TEXFORMAT_R8_SINT,
    TEXFORMAT_R16_UINT,
    TEXFORMAT_R16_SINT,
    TEXFORMAT_R16_UNORM,
    TEXFORMAT_R16_SNORM,
    TEXFORMAT_R16_FLOAT,
    TEXFORMAT_RG8_UNORM,
    TEXFORMAT_RG8_SNORM,
    TEXFORMAT_RG8_UINT,
    TEXFORMAT_RG8_SINT,
    TEXFORMAT_R32_UINT,
    TEXFORMAT_R32_SINT,
    TEXFORMAT_R32_FLOAT,
    TEXFORMAT_RG16_UINT,
    TEXFORMAT_RG16_SINT,
    TEXFORMAT_RG16_UNORM,
    TEXFORMAT_RG16_SNORM,
    TEXFORMAT_RG16_FLOAT,
    TEXFORMAT_RGBA8_UNORM,
    TEXFORMAT_RGBA8_UNORM_SRGB,
    TEXFORMAT_RGBA8_SNORM,
    TEXFORMAT_RGBA8_UINT,
    TEXFORMAT_RGBA8_SINT,
    TEXFORMAT_BGRA8_UNORM,
    TEXFORMAT_BGRA8_UNORM_SRGB,
    TEXFORMAT_RGB9E5_UFLOAT,
    TEXFORMAT_RGB10A2_UINT,
    TEXFORMAT_RGB10A2_UNORM,
    TEXFORMAT_RG11B10_UFLOAT,
    TEXFORMAT_R64_UINT,
    TEXFORMAT_RG32_UINT,
    TEXFORMAT_RG32_SINT,
    TEXFORMAT_RG32_FLOAT,
    TEXFORMAT_RGBA16_UINT,
    TEXFORMAT_RGBA16_SINT,
    TEXFORMAT_RGBA16_UNORM,
    TEXFORMAT_RGBA16_SNORM,
    TEXFORMAT_RGBA16_FLOAT,
    TEXFORMAT_RGBA32_UINT,
    TEXFORMAT_RGBA32_SINT,
    TEXFORMAT_RGBA32_FLOAT,
    TEXFORMAT_STENCIL8,
    TEXFORMAT_DEPTH16_UNORM,
    TEXFORMAT_DEPTH24_PLUS,
    TEXFORMAT_DEPTH24_PLUS_STENCIL8,
    TEXFORMAT_DEPTH32_FLOAT,
    TEXFORMAT_DEPTH32_FLOAT_STENCIL8,
    TEXFORMAT_NV12,
    TEXFORMAT_P010,
    TEXFORMAT_BC1_RGBA_UNORM,
    TEXFORMAT_BC1_RGBA_UNORM_SRGB,
    TEXFORMAT_BC2_RGBA_UNORM,
    TEXFORMAT_BC2_RGBA_UNORM_SRGB,
    TEXFORMAT_BC3_RGBA_UNORM,
    TEXFORMAT_BC3_RGBA_UNORM_SRGB,
    TEXFORMAT_BC4_R_UNORM,
    TEXFORMAT_BC4_R_SNORM,
    TEXFORMAT_BC5_RG_UNORM,
    TEXFORMAT_BC5_RG_SNORM,
    TEXFORMAT_BC6H_RGB_UFLOAT,
    TEXFORMAT_BC6H_RGB_FLOAT,
    TEXFORMAT_BC7_RGBA_UNORM,
    TEXFORMAT_BC7_RGBA_UNORM_SRGB,
    TEXFORMAT_ETC2_RGB8_UNORM,
    TEXFORMAT_ETC2_RGB8_UNORM_SRGB,
    TEXFORMAT_ETC2_RGB8_A1_UNORM,
    TEXFORMAT_ETC2_RGB8_A1_UNORM_SRGB,
    TEXFORMAT_ETC2_RGBA8_UNORM,
    TEXFORMAT_ETC2_RGBA8_UNORM_SRGB,
    TEXFORMAT_EAC_R11_UNORM,
    TEXFORMAT_EAC_R11_SNORM,
    TEXFORMAT_EAC_RG11_UNORM,
    TEXFORMAT_EAC_RG11_SNORM,
    TEXFORMAT_ASTC_B4X4_UNORM,
    TEXFORMAT_ASTC_B4X4_UNORM_SRGB,
    TEXFORMAT_ASTC_B4X4_HDR,
    TEXFORMAT_ASTC_B5X4_UNORM,
    TEXFORMAT_ASTC_B5X4_UNORM_SRGB,
    TEXFORMAT_ASTC_B5X4_HDR,
    TEXFORMAT_ASTC_B5X5_UNORM,
    TEXFORMAT_ASTC_B5X5_UNORM_SRGB,
    TEXFORMAT_ASTC_B5X5_HDR,
    TEXFORMAT_ASTC_B6X5_UNORM,
    TEXFORMAT_ASTC_B6X5_UNORM_SRGB,
    TEXFORMAT_ASTC_B6X5_HDR,
    TEXFORMAT_ASTC_B6X6_UNORM,
    TEXFORMAT_ASTC_B6X6_UNORM_SRGB,
    TEXFORMAT_ASTC_B6X6_HDR,
    TEXFORMAT_ASTC_B8X5_UNORM,
    TEXFORMAT_ASTC_B8X5_UNORM_SRGB,
    TEXFORMAT_ASTC_B8X5_HDR,
    TEXFORMAT_ASTC_B8X6_UNORM,
    TEXFORMAT_ASTC_B8X6_UNORM_SRGB,
    TEXFORMAT_ASTC_B8X6_HDR,
    TEXFORMAT_ASTC_B8X8_UNORM,
    TEXFORMAT_ASTC_B8X8_UNORM_SRGB,
    TEXFORMAT_ASTC_B8X8_HDR,
    TEXFORMAT_ASTC_B10X5_UNORM,
    TEXFORMAT_ASTC_B10X5_UNORM_SRGB,
    TEXFORMAT_ASTC_B10X5_HDR,
    TEXFORMAT_ASTC_B10X6_UNORM,
    TEXFORMAT_ASTC_B10X6_UNORM_SRGB,
    TEXFORMAT_ASTC_B10X6_HDR,
    TEXFORMAT_ASTC_B10X8_UNORM,
    TEXFORMAT_ASTC_B10X8_UNORM_SRGB,
    TEXFORMAT_ASTC_B10X8_HDR,
    TEXFORMAT_ASTC_B10X10_UNORM,
    TEXFORMAT_ASTC_B10X10_UNORM_SRGB,
    TEXFORMAT_ASTC_B10X10_HDR,
    TEXFORMAT_ASTC_B12X10_UNORM,
    TEXFORMAT_ASTC_B12X10_UNORM_SRGB,
    TEXFORMAT_ASTC_B12X10_HDR,
    TEXFORMAT_ASTC_B12X12_UNORM,
    TEXFORMAT_ASTC_B12X12_UNORM_SRGB,
    TEXFORMAT_ASTC_B12X12_HDR,
];

/// ASTC の行に属する 42 件の鍵と形式の値（block の順を外、channel の順を内）。
const ASTC_KEYS: [TextureFormatKey; 42] = [
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B4X4_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B4X4_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B4X4_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B5X4_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B5X4_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B5X4_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B5X5_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B5X5_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B5X5_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B6X5_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B6X5_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B6X5_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B6X6_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B6X6_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B6X6_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X5_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X5_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X5_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X6_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X6_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X6_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X8_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X8_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B8X8_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X5_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X5_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X5_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X6_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X6_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X6_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X8_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X8_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X8_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X10_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X10_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B10X10_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B12X10_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B12X10_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B12X10_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::Hdr,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B12X12_UNORM,
        format: TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Unorm,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B12X12_UNORM_SRGB,
        format: TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::UnormSrgb,
        },
    },
    TextureFormatKey {
        key: TEXFORMAT_ASTC_B12X12_HDR,
        format: TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::Hdr,
        },
    },
];

/// 機能一覧の 77 行（`TextureFormat` の宣言順で、ASTC の行が最後）。ASTC の行は 42 件、ほかの行は 1 件の鍵を持つ（TASK-1313）。
pub const TEXTURE_FORMAT_ROWS: &[TextureFormatRow] = &[
    TextureFormatRow {
        row: "R8Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R8_UNORM,
            format: TextureFormat::R8Unorm,
        }],
    },
    TextureFormatRow {
        row: "R8Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R8_SNORM,
            format: TextureFormat::R8Snorm,
        }],
    },
    TextureFormatRow {
        row: "R8Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R8_UINT,
            format: TextureFormat::R8Uint,
        }],
    },
    TextureFormatRow {
        row: "R8Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R8_SINT,
            format: TextureFormat::R8Sint,
        }],
    },
    TextureFormatRow {
        row: "R16Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R16_UINT,
            format: TextureFormat::R16Uint,
        }],
    },
    TextureFormatRow {
        row: "R16Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R16_SINT,
            format: TextureFormat::R16Sint,
        }],
    },
    TextureFormatRow {
        row: "R16Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R16_UNORM,
            format: TextureFormat::R16Unorm,
        }],
    },
    TextureFormatRow {
        row: "R16Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R16_SNORM,
            format: TextureFormat::R16Snorm,
        }],
    },
    TextureFormatRow {
        row: "R16Float",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R16_FLOAT,
            format: TextureFormat::R16Float,
        }],
    },
    TextureFormatRow {
        row: "Rg8Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG8_UNORM,
            format: TextureFormat::Rg8Unorm,
        }],
    },
    TextureFormatRow {
        row: "Rg8Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG8_SNORM,
            format: TextureFormat::Rg8Snorm,
        }],
    },
    TextureFormatRow {
        row: "Rg8Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG8_UINT,
            format: TextureFormat::Rg8Uint,
        }],
    },
    TextureFormatRow {
        row: "Rg8Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG8_SINT,
            format: TextureFormat::Rg8Sint,
        }],
    },
    TextureFormatRow {
        row: "R32Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R32_UINT,
            format: TextureFormat::R32Uint,
        }],
    },
    TextureFormatRow {
        row: "R32Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R32_SINT,
            format: TextureFormat::R32Sint,
        }],
    },
    TextureFormatRow {
        row: "R32Float",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R32_FLOAT,
            format: TextureFormat::R32Float,
        }],
    },
    TextureFormatRow {
        row: "Rg16Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG16_UINT,
            format: TextureFormat::Rg16Uint,
        }],
    },
    TextureFormatRow {
        row: "Rg16Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG16_SINT,
            format: TextureFormat::Rg16Sint,
        }],
    },
    TextureFormatRow {
        row: "Rg16Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG16_UNORM,
            format: TextureFormat::Rg16Unorm,
        }],
    },
    TextureFormatRow {
        row: "Rg16Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG16_SNORM,
            format: TextureFormat::Rg16Snorm,
        }],
    },
    TextureFormatRow {
        row: "Rg16Float",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG16_FLOAT,
            format: TextureFormat::Rg16Float,
        }],
    },
    TextureFormatRow {
        row: "Rgba8Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA8_UNORM,
            format: TextureFormat::Rgba8Unorm,
        }],
    },
    TextureFormatRow {
        row: "Rgba8UnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA8_UNORM_SRGB,
            format: TextureFormat::Rgba8UnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Rgba8Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA8_SNORM,
            format: TextureFormat::Rgba8Snorm,
        }],
    },
    TextureFormatRow {
        row: "Rgba8Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA8_UINT,
            format: TextureFormat::Rgba8Uint,
        }],
    },
    TextureFormatRow {
        row: "Rgba8Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA8_SINT,
            format: TextureFormat::Rgba8Sint,
        }],
    },
    TextureFormatRow {
        row: "Bgra8Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BGRA8_UNORM,
            format: TextureFormat::Bgra8Unorm,
        }],
    },
    TextureFormatRow {
        row: "Bgra8UnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BGRA8_UNORM_SRGB,
            format: TextureFormat::Bgra8UnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Rgb9e5Ufloat",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGB9E5_UFLOAT,
            format: TextureFormat::Rgb9e5Ufloat,
        }],
    },
    TextureFormatRow {
        row: "Rgb10a2Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGB10A2_UINT,
            format: TextureFormat::Rgb10a2Uint,
        }],
    },
    TextureFormatRow {
        row: "Rgb10a2Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGB10A2_UNORM,
            format: TextureFormat::Rgb10a2Unorm,
        }],
    },
    TextureFormatRow {
        row: "Rg11b10Ufloat",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG11B10_UFLOAT,
            format: TextureFormat::Rg11b10Ufloat,
        }],
    },
    TextureFormatRow {
        row: "R64Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_R64_UINT,
            format: TextureFormat::R64Uint,
        }],
    },
    TextureFormatRow {
        row: "Rg32Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG32_UINT,
            format: TextureFormat::Rg32Uint,
        }],
    },
    TextureFormatRow {
        row: "Rg32Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG32_SINT,
            format: TextureFormat::Rg32Sint,
        }],
    },
    TextureFormatRow {
        row: "Rg32Float",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RG32_FLOAT,
            format: TextureFormat::Rg32Float,
        }],
    },
    TextureFormatRow {
        row: "Rgba16Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA16_UINT,
            format: TextureFormat::Rgba16Uint,
        }],
    },
    TextureFormatRow {
        row: "Rgba16Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA16_SINT,
            format: TextureFormat::Rgba16Sint,
        }],
    },
    TextureFormatRow {
        row: "Rgba16Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA16_UNORM,
            format: TextureFormat::Rgba16Unorm,
        }],
    },
    TextureFormatRow {
        row: "Rgba16Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA16_SNORM,
            format: TextureFormat::Rgba16Snorm,
        }],
    },
    TextureFormatRow {
        row: "Rgba16Float",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA16_FLOAT,
            format: TextureFormat::Rgba16Float,
        }],
    },
    TextureFormatRow {
        row: "Rgba32Uint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA32_UINT,
            format: TextureFormat::Rgba32Uint,
        }],
    },
    TextureFormatRow {
        row: "Rgba32Sint",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA32_SINT,
            format: TextureFormat::Rgba32Sint,
        }],
    },
    TextureFormatRow {
        row: "Rgba32Float",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_RGBA32_FLOAT,
            format: TextureFormat::Rgba32Float,
        }],
    },
    TextureFormatRow {
        row: "Stencil8",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_STENCIL8,
            format: TextureFormat::Stencil8,
        }],
    },
    TextureFormatRow {
        row: "Depth16Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_DEPTH16_UNORM,
            format: TextureFormat::Depth16Unorm,
        }],
    },
    TextureFormatRow {
        row: "Depth24Plus",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_DEPTH24_PLUS,
            format: TextureFormat::Depth24Plus,
        }],
    },
    TextureFormatRow {
        row: "Depth24PlusStencil8",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_DEPTH24_PLUS_STENCIL8,
            format: TextureFormat::Depth24PlusStencil8,
        }],
    },
    TextureFormatRow {
        row: "Depth32Float",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_DEPTH32_FLOAT,
            format: TextureFormat::Depth32Float,
        }],
    },
    TextureFormatRow {
        row: "Depth32FloatStencil8",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_DEPTH32_FLOAT_STENCIL8,
            format: TextureFormat::Depth32FloatStencil8,
        }],
    },
    TextureFormatRow {
        row: "NV12",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_NV12,
            format: TextureFormat::NV12,
        }],
    },
    TextureFormatRow {
        row: "P010",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_P010,
            format: TextureFormat::P010,
        }],
    },
    TextureFormatRow {
        row: "Bc1RgbaUnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC1_RGBA_UNORM,
            format: TextureFormat::Bc1RgbaUnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc1RgbaUnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC1_RGBA_UNORM_SRGB,
            format: TextureFormat::Bc1RgbaUnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Bc2RgbaUnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC2_RGBA_UNORM,
            format: TextureFormat::Bc2RgbaUnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc2RgbaUnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC2_RGBA_UNORM_SRGB,
            format: TextureFormat::Bc2RgbaUnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Bc3RgbaUnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC3_RGBA_UNORM,
            format: TextureFormat::Bc3RgbaUnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc3RgbaUnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC3_RGBA_UNORM_SRGB,
            format: TextureFormat::Bc3RgbaUnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Bc4RUnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC4_R_UNORM,
            format: TextureFormat::Bc4RUnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc4RSnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC4_R_SNORM,
            format: TextureFormat::Bc4RSnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc5RgUnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC5_RG_UNORM,
            format: TextureFormat::Bc5RgUnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc5RgSnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC5_RG_SNORM,
            format: TextureFormat::Bc5RgSnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc6hRgbUfloat",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC6H_RGB_UFLOAT,
            format: TextureFormat::Bc6hRgbUfloat,
        }],
    },
    TextureFormatRow {
        row: "Bc6hRgbFloat",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC6H_RGB_FLOAT,
            format: TextureFormat::Bc6hRgbFloat,
        }],
    },
    TextureFormatRow {
        row: "Bc7RgbaUnorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC7_RGBA_UNORM,
            format: TextureFormat::Bc7RgbaUnorm,
        }],
    },
    TextureFormatRow {
        row: "Bc7RgbaUnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_BC7_RGBA_UNORM_SRGB,
            format: TextureFormat::Bc7RgbaUnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Etc2Rgb8Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_ETC2_RGB8_UNORM,
            format: TextureFormat::Etc2Rgb8Unorm,
        }],
    },
    TextureFormatRow {
        row: "Etc2Rgb8UnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_ETC2_RGB8_UNORM_SRGB,
            format: TextureFormat::Etc2Rgb8UnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Etc2Rgb8A1Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_ETC2_RGB8_A1_UNORM,
            format: TextureFormat::Etc2Rgb8A1Unorm,
        }],
    },
    TextureFormatRow {
        row: "Etc2Rgb8A1UnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_ETC2_RGB8_A1_UNORM_SRGB,
            format: TextureFormat::Etc2Rgb8A1UnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "Etc2Rgba8Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_ETC2_RGBA8_UNORM,
            format: TextureFormat::Etc2Rgba8Unorm,
        }],
    },
    TextureFormatRow {
        row: "Etc2Rgba8UnormSrgb",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_ETC2_RGBA8_UNORM_SRGB,
            format: TextureFormat::Etc2Rgba8UnormSrgb,
        }],
    },
    TextureFormatRow {
        row: "EacR11Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_EAC_R11_UNORM,
            format: TextureFormat::EacR11Unorm,
        }],
    },
    TextureFormatRow {
        row: "EacR11Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_EAC_R11_SNORM,
            format: TextureFormat::EacR11Snorm,
        }],
    },
    TextureFormatRow {
        row: "EacRg11Unorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_EAC_RG11_UNORM,
            format: TextureFormat::EacRg11Unorm,
        }],
    },
    TextureFormatRow {
        row: "EacRg11Snorm",
        keys: &[TextureFormatKey {
            key: TEXFORMAT_EAC_RG11_SNORM,
            format: TextureFormat::EacRg11Snorm,
        }],
    },
    TextureFormatRow {
        row: "Astc",
        keys: &ASTC_KEYS,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// `TextureFormat` の値から鍵の定数を引く。`_` の腕が無いので variant を足すとコンパイルで落ちる。
    fn key_of(f: TextureFormat) -> FeatureId {
        match f {
            TextureFormat::R8Unorm => TEXFORMAT_R8_UNORM,
            TextureFormat::R8Snorm => TEXFORMAT_R8_SNORM,
            TextureFormat::R8Uint => TEXFORMAT_R8_UINT,
            TextureFormat::R8Sint => TEXFORMAT_R8_SINT,
            TextureFormat::R16Uint => TEXFORMAT_R16_UINT,
            TextureFormat::R16Sint => TEXFORMAT_R16_SINT,
            TextureFormat::R16Unorm => TEXFORMAT_R16_UNORM,
            TextureFormat::R16Snorm => TEXFORMAT_R16_SNORM,
            TextureFormat::R16Float => TEXFORMAT_R16_FLOAT,
            TextureFormat::Rg8Unorm => TEXFORMAT_RG8_UNORM,
            TextureFormat::Rg8Snorm => TEXFORMAT_RG8_SNORM,
            TextureFormat::Rg8Uint => TEXFORMAT_RG8_UINT,
            TextureFormat::Rg8Sint => TEXFORMAT_RG8_SINT,
            TextureFormat::R32Uint => TEXFORMAT_R32_UINT,
            TextureFormat::R32Sint => TEXFORMAT_R32_SINT,
            TextureFormat::R32Float => TEXFORMAT_R32_FLOAT,
            TextureFormat::Rg16Uint => TEXFORMAT_RG16_UINT,
            TextureFormat::Rg16Sint => TEXFORMAT_RG16_SINT,
            TextureFormat::Rg16Unorm => TEXFORMAT_RG16_UNORM,
            TextureFormat::Rg16Snorm => TEXFORMAT_RG16_SNORM,
            TextureFormat::Rg16Float => TEXFORMAT_RG16_FLOAT,
            TextureFormat::Rgba8Unorm => TEXFORMAT_RGBA8_UNORM,
            TextureFormat::Rgba8UnormSrgb => TEXFORMAT_RGBA8_UNORM_SRGB,
            TextureFormat::Rgba8Snorm => TEXFORMAT_RGBA8_SNORM,
            TextureFormat::Rgba8Uint => TEXFORMAT_RGBA8_UINT,
            TextureFormat::Rgba8Sint => TEXFORMAT_RGBA8_SINT,
            TextureFormat::Bgra8Unorm => TEXFORMAT_BGRA8_UNORM,
            TextureFormat::Bgra8UnormSrgb => TEXFORMAT_BGRA8_UNORM_SRGB,
            TextureFormat::Rgb9e5Ufloat => TEXFORMAT_RGB9E5_UFLOAT,
            TextureFormat::Rgb10a2Uint => TEXFORMAT_RGB10A2_UINT,
            TextureFormat::Rgb10a2Unorm => TEXFORMAT_RGB10A2_UNORM,
            TextureFormat::Rg11b10Ufloat => TEXFORMAT_RG11B10_UFLOAT,
            TextureFormat::R64Uint => TEXFORMAT_R64_UINT,
            TextureFormat::Rg32Uint => TEXFORMAT_RG32_UINT,
            TextureFormat::Rg32Sint => TEXFORMAT_RG32_SINT,
            TextureFormat::Rg32Float => TEXFORMAT_RG32_FLOAT,
            TextureFormat::Rgba16Uint => TEXFORMAT_RGBA16_UINT,
            TextureFormat::Rgba16Sint => TEXFORMAT_RGBA16_SINT,
            TextureFormat::Rgba16Unorm => TEXFORMAT_RGBA16_UNORM,
            TextureFormat::Rgba16Snorm => TEXFORMAT_RGBA16_SNORM,
            TextureFormat::Rgba16Float => TEXFORMAT_RGBA16_FLOAT,
            TextureFormat::Rgba32Uint => TEXFORMAT_RGBA32_UINT,
            TextureFormat::Rgba32Sint => TEXFORMAT_RGBA32_SINT,
            TextureFormat::Rgba32Float => TEXFORMAT_RGBA32_FLOAT,
            TextureFormat::Stencil8 => TEXFORMAT_STENCIL8,
            TextureFormat::Depth16Unorm => TEXFORMAT_DEPTH16_UNORM,
            TextureFormat::Depth24Plus => TEXFORMAT_DEPTH24_PLUS,
            TextureFormat::Depth24PlusStencil8 => TEXFORMAT_DEPTH24_PLUS_STENCIL8,
            TextureFormat::Depth32Float => TEXFORMAT_DEPTH32_FLOAT,
            TextureFormat::Depth32FloatStencil8 => TEXFORMAT_DEPTH32_FLOAT_STENCIL8,
            TextureFormat::NV12 => TEXFORMAT_NV12,
            TextureFormat::P010 => TEXFORMAT_P010,
            TextureFormat::Bc1RgbaUnorm => TEXFORMAT_BC1_RGBA_UNORM,
            TextureFormat::Bc1RgbaUnormSrgb => TEXFORMAT_BC1_RGBA_UNORM_SRGB,
            TextureFormat::Bc2RgbaUnorm => TEXFORMAT_BC2_RGBA_UNORM,
            TextureFormat::Bc2RgbaUnormSrgb => TEXFORMAT_BC2_RGBA_UNORM_SRGB,
            TextureFormat::Bc3RgbaUnorm => TEXFORMAT_BC3_RGBA_UNORM,
            TextureFormat::Bc3RgbaUnormSrgb => TEXFORMAT_BC3_RGBA_UNORM_SRGB,
            TextureFormat::Bc4RUnorm => TEXFORMAT_BC4_R_UNORM,
            TextureFormat::Bc4RSnorm => TEXFORMAT_BC4_R_SNORM,
            TextureFormat::Bc5RgUnorm => TEXFORMAT_BC5_RG_UNORM,
            TextureFormat::Bc5RgSnorm => TEXFORMAT_BC5_RG_SNORM,
            TextureFormat::Bc6hRgbUfloat => TEXFORMAT_BC6H_RGB_UFLOAT,
            TextureFormat::Bc6hRgbFloat => TEXFORMAT_BC6H_RGB_FLOAT,
            TextureFormat::Bc7RgbaUnorm => TEXFORMAT_BC7_RGBA_UNORM,
            TextureFormat::Bc7RgbaUnormSrgb => TEXFORMAT_BC7_RGBA_UNORM_SRGB,
            TextureFormat::Etc2Rgb8Unorm => TEXFORMAT_ETC2_RGB8_UNORM,
            TextureFormat::Etc2Rgb8UnormSrgb => TEXFORMAT_ETC2_RGB8_UNORM_SRGB,
            TextureFormat::Etc2Rgb8A1Unorm => TEXFORMAT_ETC2_RGB8_A1_UNORM,
            TextureFormat::Etc2Rgb8A1UnormSrgb => TEXFORMAT_ETC2_RGB8_A1_UNORM_SRGB,
            TextureFormat::Etc2Rgba8Unorm => TEXFORMAT_ETC2_RGBA8_UNORM,
            TextureFormat::Etc2Rgba8UnormSrgb => TEXFORMAT_ETC2_RGBA8_UNORM_SRGB,
            TextureFormat::EacR11Unorm => TEXFORMAT_EAC_R11_UNORM,
            TextureFormat::EacR11Snorm => TEXFORMAT_EAC_R11_SNORM,
            TextureFormat::EacRg11Unorm => TEXFORMAT_EAC_RG11_UNORM,
            TextureFormat::EacRg11Snorm => TEXFORMAT_EAC_RG11_SNORM,
            TextureFormat::Astc { block, channel } => match (block, channel) {
                (AstcBlock::B4x4, AstcChannel::Unorm) => TEXFORMAT_ASTC_B4X4_UNORM,
                (AstcBlock::B4x4, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B4X4_UNORM_SRGB,
                (AstcBlock::B4x4, AstcChannel::Hdr) => TEXFORMAT_ASTC_B4X4_HDR,
                (AstcBlock::B5x4, AstcChannel::Unorm) => TEXFORMAT_ASTC_B5X4_UNORM,
                (AstcBlock::B5x4, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B5X4_UNORM_SRGB,
                (AstcBlock::B5x4, AstcChannel::Hdr) => TEXFORMAT_ASTC_B5X4_HDR,
                (AstcBlock::B5x5, AstcChannel::Unorm) => TEXFORMAT_ASTC_B5X5_UNORM,
                (AstcBlock::B5x5, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B5X5_UNORM_SRGB,
                (AstcBlock::B5x5, AstcChannel::Hdr) => TEXFORMAT_ASTC_B5X5_HDR,
                (AstcBlock::B6x5, AstcChannel::Unorm) => TEXFORMAT_ASTC_B6X5_UNORM,
                (AstcBlock::B6x5, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B6X5_UNORM_SRGB,
                (AstcBlock::B6x5, AstcChannel::Hdr) => TEXFORMAT_ASTC_B6X5_HDR,
                (AstcBlock::B6x6, AstcChannel::Unorm) => TEXFORMAT_ASTC_B6X6_UNORM,
                (AstcBlock::B6x6, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B6X6_UNORM_SRGB,
                (AstcBlock::B6x6, AstcChannel::Hdr) => TEXFORMAT_ASTC_B6X6_HDR,
                (AstcBlock::B8x5, AstcChannel::Unorm) => TEXFORMAT_ASTC_B8X5_UNORM,
                (AstcBlock::B8x5, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B8X5_UNORM_SRGB,
                (AstcBlock::B8x5, AstcChannel::Hdr) => TEXFORMAT_ASTC_B8X5_HDR,
                (AstcBlock::B8x6, AstcChannel::Unorm) => TEXFORMAT_ASTC_B8X6_UNORM,
                (AstcBlock::B8x6, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B8X6_UNORM_SRGB,
                (AstcBlock::B8x6, AstcChannel::Hdr) => TEXFORMAT_ASTC_B8X6_HDR,
                (AstcBlock::B8x8, AstcChannel::Unorm) => TEXFORMAT_ASTC_B8X8_UNORM,
                (AstcBlock::B8x8, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B8X8_UNORM_SRGB,
                (AstcBlock::B8x8, AstcChannel::Hdr) => TEXFORMAT_ASTC_B8X8_HDR,
                (AstcBlock::B10x5, AstcChannel::Unorm) => TEXFORMAT_ASTC_B10X5_UNORM,
                (AstcBlock::B10x5, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B10X5_UNORM_SRGB,
                (AstcBlock::B10x5, AstcChannel::Hdr) => TEXFORMAT_ASTC_B10X5_HDR,
                (AstcBlock::B10x6, AstcChannel::Unorm) => TEXFORMAT_ASTC_B10X6_UNORM,
                (AstcBlock::B10x6, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B10X6_UNORM_SRGB,
                (AstcBlock::B10x6, AstcChannel::Hdr) => TEXFORMAT_ASTC_B10X6_HDR,
                (AstcBlock::B10x8, AstcChannel::Unorm) => TEXFORMAT_ASTC_B10X8_UNORM,
                (AstcBlock::B10x8, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B10X8_UNORM_SRGB,
                (AstcBlock::B10x8, AstcChannel::Hdr) => TEXFORMAT_ASTC_B10X8_HDR,
                (AstcBlock::B10x10, AstcChannel::Unorm) => TEXFORMAT_ASTC_B10X10_UNORM,
                (AstcBlock::B10x10, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B10X10_UNORM_SRGB,
                (AstcBlock::B10x10, AstcChannel::Hdr) => TEXFORMAT_ASTC_B10X10_HDR,
                (AstcBlock::B12x10, AstcChannel::Unorm) => TEXFORMAT_ASTC_B12X10_UNORM,
                (AstcBlock::B12x10, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B12X10_UNORM_SRGB,
                (AstcBlock::B12x10, AstcChannel::Hdr) => TEXFORMAT_ASTC_B12X10_HDR,
                (AstcBlock::B12x12, AstcChannel::Unorm) => TEXFORMAT_ASTC_B12X12_UNORM,
                (AstcBlock::B12x12, AstcChannel::UnormSrgb) => TEXFORMAT_ASTC_B12X12_UNORM_SRGB,
                (AstcBlock::B12x12, AstcChannel::Hdr) => TEXFORMAT_ASTC_B12X12_HDR,
            },
        }
    }

    #[test]
    fn query_keys_follow_all_formats() {
        let all = TextureFormat::all();
        assert_eq!(TEXTURE_FORMAT_QUERY_KEYS.len(), all.len());
        for (i, f) in all.iter().enumerate() {
            assert_eq!(TEXTURE_FORMAT_QUERY_KEYS[i], key_of(*f), "{f:?}");
        }
    }

    #[test]
    fn every_format_is_in_exactly_one_row_entry() {
        for f in TextureFormat::all() {
            let hits: Vec<&TextureFormatKey> = TEXTURE_FORMAT_ROWS
                .iter()
                .flat_map(|r| r.keys.iter())
                .filter(|k| k.format == *f)
                .collect();
            assert_eq!(hits.len(), 1, "{f:?}");
            assert_eq!(hits[0].key, key_of(*f), "{f:?}");
        }
    }

    #[test]
    fn key_strings_follow_naming_rule() {
        for f in TextureFormat::all() {
            let expected = match f {
                TextureFormat::Astc { block, channel } => {
                    format!("texformat:Astc.{block:?}.{channel:?}")
                }
                other => format!("texformat:{other:?}"),
            };
            assert_eq!(key_of(*f).0, expected, "{f:?}");
        }
    }
}
