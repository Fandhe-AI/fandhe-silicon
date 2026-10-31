//! テクスチャの形式の型と、機材に依らない性質を返す関数（TASK-1312）。
//!
//! 3 段（下-1・下-2・上）で受け渡す共通の型として、形式の型 [`TextureFormat`] と ASTC の補助の型
//! [`AstcBlock`]・[`AstcChannel`] を置く。値は資源を持たない純粋な値で、背後実装（Metal・Vulkan・CUDA）の
//! 型は含まない。形式ごとの提供の有無と使い方の可否は能力の問合せの側の持ち分で、この module の型は持たない。
//!
//! 性質の関数は `match` と定数だけで値を返し、機材を引数に取らず、`Result` を返さず、panic しない。
//! 面・深度ステンシル・平面・圧縮ブロック・sRGB の対・要る機能の ID を、形式の値だけから決める。

use crate::capability::FeatureId;

/// ASTC のブロックの大きさ。14 値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AstcBlock {
    /// 4x4 のブロック。
    B4x4,
    /// 5x4 のブロック。
    B5x4,
    /// 5x5 のブロック。
    B5x5,
    /// 6x5 のブロック。
    B6x5,
    /// 6x6 のブロック。
    B6x6,
    /// 8x5 のブロック。
    B8x5,
    /// 8x6 のブロック。
    B8x6,
    /// 8x8 のブロック。
    B8x8,
    /// 10x5 のブロック。
    B10x5,
    /// 10x6 のブロック。
    B10x6,
    /// 10x8 のブロック。
    B10x8,
    /// 10x10 のブロック。
    B10x10,
    /// 12x10 のブロック。
    B12x10,
    /// 12x12 のブロック。
    B12x12,
}

/// ASTC のチャネルの種類。3 値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AstcChannel {
    /// 8 ビット整数 RGBA の符号なし正規化（線形）。
    Unorm,
    /// 8 ビット整数 RGBA の符号なし正規化（sRGB）。
    UnormSrgb,
    /// 浮動小数点 RGBA（HDR）。
    Hdr,
}

/// テクスチャの形式。ASTC 以外の 76 値と、ブロックとチャネルを持つ `Astc` の 1 variant（計 77 variant）。
///
/// 具体形式の全件（ASTC を 42 通りに展開した 118 件）は [`TextureFormat::all`] が返す。
/// `#[repr]` は付けず、判別子の数値は公開しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureFormat {
    /// 赤・8 ビット/チャネルの符号なし正規化。
    R8Unorm,
    /// 赤・8 ビット/チャネルの符号付き正規化。
    R8Snorm,
    /// 赤・8 ビット/チャネルの符号なし整数。
    R8Uint,
    /// 赤・8 ビット/チャネルの符号付き整数。
    R8Sint,
    /// 赤・16 ビット/チャネルの符号なし整数。
    R16Uint,
    /// 赤・16 ビット/チャネルの符号付き整数。
    R16Sint,
    /// 赤・16 ビット/チャネルの符号なし正規化。
    R16Unorm,
    /// 赤・16 ビット/チャネルの符号付き正規化。
    R16Snorm,
    /// 赤・16 ビット/チャネルの浮動小数点。
    R16Float,
    /// 赤緑・8 ビット/チャネルの符号なし正規化。
    Rg8Unorm,
    /// 赤緑・8 ビット/チャネルの符号付き正規化。
    Rg8Snorm,
    /// 赤緑・8 ビット/チャネルの符号なし整数。
    Rg8Uint,
    /// 赤緑・8 ビット/チャネルの符号付き整数。
    Rg8Sint,
    /// 赤・32 ビット/チャネルの符号なし整数。
    R32Uint,
    /// 赤・32 ビット/チャネルの符号付き整数。
    R32Sint,
    /// 赤・32 ビット/チャネルの浮動小数点。
    R32Float,
    /// 赤緑・16 ビット/チャネルの符号なし整数。
    Rg16Uint,
    /// 赤緑・16 ビット/チャネルの符号付き整数。
    Rg16Sint,
    /// 赤緑・16 ビット/チャネルの符号なし正規化。
    Rg16Unorm,
    /// 赤緑・16 ビット/チャネルの符号付き正規化。
    Rg16Snorm,
    /// 赤緑・16 ビット/チャネルの浮動小数点。
    Rg16Float,
    /// 赤緑青アルファ・8 ビット/チャネルの符号なし正規化。
    Rgba8Unorm,
    /// 赤緑青アルファ・8 ビット/チャネルの符号なし正規化の sRGB。
    Rgba8UnormSrgb,
    /// 赤緑青アルファ・8 ビット/チャネルの符号付き正規化。
    Rgba8Snorm,
    /// 赤緑青アルファ・8 ビット/チャネルの符号なし整数。
    Rgba8Uint,
    /// 赤緑青アルファ・8 ビット/チャネルの符号付き整数。
    Rgba8Sint,
    /// 青緑赤アルファ・8 ビット/チャネルの符号なし正規化。
    Bgra8Unorm,
    /// 青緑赤アルファ・8 ビット/チャネルの符号なし正規化の sRGB。
    Bgra8UnormSrgb,
    /// 赤緑青の 9 ビット仮数・共有指数 5 ビットの符号なし浮動小数点。
    Rgb9e5Ufloat,
    /// 赤緑青 10 ビット・アルファ 2 ビットの符号なし整数。
    Rgb10a2Uint,
    /// 赤緑青 10 ビット・アルファ 2 ビットの符号なし正規化。
    Rgb10a2Unorm,
    /// 赤緑 11 ビット・青 10 ビットの符号なし浮動小数点。
    Rg11b10Ufloat,
    /// 赤・64 ビット/チャネルの符号なし整数。
    R64Uint,
    /// 赤緑・32 ビット/チャネルの符号なし整数。
    Rg32Uint,
    /// 赤緑・32 ビット/チャネルの符号付き整数。
    Rg32Sint,
    /// 赤緑・32 ビット/チャネルの浮動小数点。
    Rg32Float,
    /// 赤緑青アルファ・16 ビット/チャネルの符号なし整数。
    Rgba16Uint,
    /// 赤緑青アルファ・16 ビット/チャネルの符号付き整数。
    Rgba16Sint,
    /// 赤緑青アルファ・16 ビット/チャネルの符号なし正規化。
    Rgba16Unorm,
    /// 赤緑青アルファ・16 ビット/チャネルの符号付き正規化。
    Rgba16Snorm,
    /// 赤緑青アルファ・16 ビット/チャネルの浮動小数点。
    Rgba16Float,
    /// 赤緑青アルファ・32 ビット/チャネルの符号なし整数。
    Rgba32Uint,
    /// 赤緑青アルファ・32 ビット/チャネルの符号付き整数。
    Rgba32Sint,
    /// 赤緑青アルファ・32 ビット/チャネルの浮動小数点。
    Rgba32Float,
    /// ステンシルのみ・8 ビット。
    Stencil8,
    /// 深度のみ・16 ビットの符号なし正規化。
    Depth16Unorm,
    /// 深度のみ・24 ビット以上（実装が幅を選ぶ）。
    Depth24Plus,
    /// 深度 24 ビット以上とステンシル 8 ビットの併用。
    Depth24PlusStencil8,
    /// 深度のみ・32 ビット浮動小数点。
    Depth32Float,
    /// 深度 32 ビット浮動小数点とステンシル 8 ビットの併用。
    Depth32FloatStencil8,
    /// 2 平面の 8 ビット YUV 4:2:0。
    NV12,
    /// 2 平面の 10 ビット YUV 4:2:0。
    P010,
    /// BC1 ブロック圧縮（RGBA・符号なし正規化）。
    Bc1RgbaUnorm,
    /// BC1 ブロック圧縮（RGBA・符号なし正規化の sRGB）。
    Bc1RgbaUnormSrgb,
    /// BC2 ブロック圧縮（RGBA・符号なし正規化）。
    Bc2RgbaUnorm,
    /// BC2 ブロック圧縮（RGBA・符号なし正規化の sRGB）。
    Bc2RgbaUnormSrgb,
    /// BC3 ブロック圧縮（RGBA・符号なし正規化）。
    Bc3RgbaUnorm,
    /// BC3 ブロック圧縮（RGBA・符号なし正規化の sRGB）。
    Bc3RgbaUnormSrgb,
    /// BC4 ブロック圧縮（R・符号なし正規化）。
    Bc4RUnorm,
    /// BC4 ブロック圧縮（R・符号付き正規化）。
    Bc4RSnorm,
    /// BC5 ブロック圧縮（RG・符号なし正規化）。
    Bc5RgUnorm,
    /// BC5 ブロック圧縮（RG・符号付き正規化）。
    Bc5RgSnorm,
    /// BC6 ブロック圧縮（RGB・符号なし浮動小数点）。
    Bc6hRgbUfloat,
    /// BC6 ブロック圧縮（RGB・符号付き浮動小数点）。
    Bc6hRgbFloat,
    /// BC7 ブロック圧縮（RGBA・符号なし正規化）。
    Bc7RgbaUnorm,
    /// BC7 ブロック圧縮（RGBA・符号なし正規化の sRGB）。
    Bc7RgbaUnormSrgb,
    /// ETC2 ブロック圧縮（RGB8・符号なし正規化）。
    Etc2Rgb8Unorm,
    /// ETC2 ブロック圧縮（RGB8・符号なし正規化の sRGB）。
    Etc2Rgb8UnormSrgb,
    /// ETC2 ブロック圧縮（RGB8A1・符号なし正規化）。
    Etc2Rgb8A1Unorm,
    /// ETC2 ブロック圧縮（RGB8A1・符号なし正規化の sRGB）。
    Etc2Rgb8A1UnormSrgb,
    /// ETC2 ブロック圧縮（RGBA8・符号なし正規化）。
    Etc2Rgba8Unorm,
    /// ETC2 ブロック圧縮（RGBA8・符号なし正規化の sRGB）。
    Etc2Rgba8UnormSrgb,
    /// EAC ブロック圧縮（R11・符号なし正規化）。
    EacR11Unorm,
    /// EAC ブロック圧縮（R11・符号付き正規化）。
    EacR11Snorm,
    /// EAC ブロック圧縮（RG11・符号なし正規化）。
    EacRg11Unorm,
    /// EAC ブロック圧縮（RG11・符号付き正規化）。
    EacRg11Snorm,
    /// ASTC ブロック圧縮。ブロックの大きさとチャネルの種類で 42 通りの具体形式になる。
    Astc {
        /// ブロックの大きさ。
        block: AstcBlock,
        /// チャネルの種類。
        channel: AstcChannel,
    },
}

static ASTC_BLOCKS: [AstcBlock; 14] = [
    AstcBlock::B4x4,
    AstcBlock::B5x4,
    AstcBlock::B5x5,
    AstcBlock::B6x5,
    AstcBlock::B6x6,
    AstcBlock::B8x5,
    AstcBlock::B8x6,
    AstcBlock::B8x8,
    AstcBlock::B10x5,
    AstcBlock::B10x6,
    AstcBlock::B10x8,
    AstcBlock::B10x10,
    AstcBlock::B12x10,
    AstcBlock::B12x12,
];

static ASTC_CHANNELS: [AstcChannel; 3] =
    [AstcChannel::Unorm, AstcChannel::UnormSrgb, AstcChannel::Hdr];

static ALL_FORMATS: [TextureFormat; 118] = [
    TextureFormat::R8Unorm,
    TextureFormat::R8Snorm,
    TextureFormat::R8Uint,
    TextureFormat::R8Sint,
    TextureFormat::R16Uint,
    TextureFormat::R16Sint,
    TextureFormat::R16Unorm,
    TextureFormat::R16Snorm,
    TextureFormat::R16Float,
    TextureFormat::Rg8Unorm,
    TextureFormat::Rg8Snorm,
    TextureFormat::Rg8Uint,
    TextureFormat::Rg8Sint,
    TextureFormat::R32Uint,
    TextureFormat::R32Sint,
    TextureFormat::R32Float,
    TextureFormat::Rg16Uint,
    TextureFormat::Rg16Sint,
    TextureFormat::Rg16Unorm,
    TextureFormat::Rg16Snorm,
    TextureFormat::Rg16Float,
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba8UnormSrgb,
    TextureFormat::Rgba8Snorm,
    TextureFormat::Rgba8Uint,
    TextureFormat::Rgba8Sint,
    TextureFormat::Bgra8Unorm,
    TextureFormat::Bgra8UnormSrgb,
    TextureFormat::Rgb9e5Ufloat,
    TextureFormat::Rgb10a2Uint,
    TextureFormat::Rgb10a2Unorm,
    TextureFormat::Rg11b10Ufloat,
    TextureFormat::R64Uint,
    TextureFormat::Rg32Uint,
    TextureFormat::Rg32Sint,
    TextureFormat::Rg32Float,
    TextureFormat::Rgba16Uint,
    TextureFormat::Rgba16Sint,
    TextureFormat::Rgba16Unorm,
    TextureFormat::Rgba16Snorm,
    TextureFormat::Rgba16Float,
    TextureFormat::Rgba32Uint,
    TextureFormat::Rgba32Sint,
    TextureFormat::Rgba32Float,
    TextureFormat::Stencil8,
    TextureFormat::Depth16Unorm,
    TextureFormat::Depth24Plus,
    TextureFormat::Depth24PlusStencil8,
    TextureFormat::Depth32Float,
    TextureFormat::Depth32FloatStencil8,
    TextureFormat::NV12,
    TextureFormat::P010,
    TextureFormat::Bc1RgbaUnorm,
    TextureFormat::Bc1RgbaUnormSrgb,
    TextureFormat::Bc2RgbaUnorm,
    TextureFormat::Bc2RgbaUnormSrgb,
    TextureFormat::Bc3RgbaUnorm,
    TextureFormat::Bc3RgbaUnormSrgb,
    TextureFormat::Bc4RUnorm,
    TextureFormat::Bc4RSnorm,
    TextureFormat::Bc5RgUnorm,
    TextureFormat::Bc5RgSnorm,
    TextureFormat::Bc6hRgbUfloat,
    TextureFormat::Bc6hRgbFloat,
    TextureFormat::Bc7RgbaUnorm,
    TextureFormat::Bc7RgbaUnormSrgb,
    TextureFormat::Etc2Rgb8Unorm,
    TextureFormat::Etc2Rgb8UnormSrgb,
    TextureFormat::Etc2Rgb8A1Unorm,
    TextureFormat::Etc2Rgb8A1UnormSrgb,
    TextureFormat::Etc2Rgba8Unorm,
    TextureFormat::Etc2Rgba8UnormSrgb,
    TextureFormat::EacR11Unorm,
    TextureFormat::EacR11Snorm,
    TextureFormat::EacRg11Unorm,
    TextureFormat::EacRg11Snorm,
    TextureFormat::Astc {
        block: AstcBlock::B4x4,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B4x4,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B4x4,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B5x4,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B5x4,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B5x4,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B5x5,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B5x5,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B5x5,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B6x5,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B6x5,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B6x5,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B6x6,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B6x6,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B6x6,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x5,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x5,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x5,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x6,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x6,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x6,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x8,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x8,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B8x8,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x5,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x5,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x5,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x6,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x6,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x6,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x8,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x8,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x8,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x10,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x10,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B10x10,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B12x10,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B12x10,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B12x10,
        channel: AstcChannel::Hdr,
    },
    TextureFormat::Astc {
        block: AstcBlock::B12x12,
        channel: AstcChannel::Unorm,
    },
    TextureFormat::Astc {
        block: AstcBlock::B12x12,
        channel: AstcChannel::UnormSrgb,
    },
    TextureFormat::Astc {
        block: AstcBlock::B12x12,
        channel: AstcChannel::Hdr,
    },
];

impl AstcBlock {
    /// 全 14 値を宣言順に返す。
    #[must_use]
    pub fn all() -> &'static [AstcBlock] {
        &ASTC_BLOCKS
    }

    /// ブロックの幅と高さ（テクセル）を返す。
    fn dimensions(self) -> (u32, u32) {
        match self {
            AstcBlock::B4x4 => (4, 4),
            AstcBlock::B5x4 => (5, 4),
            AstcBlock::B5x5 => (5, 5),
            AstcBlock::B6x5 => (6, 5),
            AstcBlock::B6x6 => (6, 6),
            AstcBlock::B8x5 => (8, 5),
            AstcBlock::B8x6 => (8, 6),
            AstcBlock::B8x8 => (8, 8),
            AstcBlock::B10x5 => (10, 5),
            AstcBlock::B10x6 => (10, 6),
            AstcBlock::B10x8 => (10, 8),
            AstcBlock::B10x10 => (10, 10),
            AstcBlock::B12x10 => (12, 10),
            AstcBlock::B12x12 => (12, 12),
        }
    }
}

impl AstcChannel {
    /// 全 3 値を宣言順に返す。
    #[must_use]
    pub fn all() -> &'static [AstcChannel] {
        &ASTC_CHANNELS
    }
}

impl TextureFormat {
    /// 具体形式の全 118 件を返す。
    ///
    /// 順は、ASTC 以外の 76 値を宣言順に並べ、続けて ASTC をブロックの順（外）× チャネルの順（内）の 42 件にしたもの。
    #[must_use]
    pub fn all() -> &'static [TextureFormat] {
        &ALL_FORMATS
    }

    /// 形式が持つ面を `(色, 深度, ステンシル)` の順で返す。面の各判定の共通の元になる。
    fn aspects(&self) -> (bool, bool, bool) {
        use TextureFormat as F;
        match self {
            F::R8Unorm
            | F::R8Snorm
            | F::R8Uint
            | F::R8Sint
            | F::R16Uint
            | F::R16Sint
            | F::R16Unorm
            | F::R16Snorm
            | F::R16Float
            | F::Rg8Unorm
            | F::Rg8Snorm
            | F::Rg8Uint
            | F::Rg8Sint
            | F::R32Uint
            | F::R32Sint
            | F::R32Float
            | F::Rg16Uint
            | F::Rg16Sint
            | F::Rg16Unorm
            | F::Rg16Snorm
            | F::Rg16Float
            | F::Rgba8Unorm
            | F::Rgba8UnormSrgb
            | F::Rgba8Snorm
            | F::Rgba8Uint
            | F::Rgba8Sint
            | F::Bgra8Unorm
            | F::Bgra8UnormSrgb
            | F::Rgb9e5Ufloat
            | F::Rgb10a2Uint
            | F::Rgb10a2Unorm
            | F::Rg11b10Ufloat
            | F::R64Uint
            | F::Rg32Uint
            | F::Rg32Sint
            | F::Rg32Float
            | F::Rgba16Uint
            | F::Rgba16Sint
            | F::Rgba16Unorm
            | F::Rgba16Snorm
            | F::Rgba16Float
            | F::Rgba32Uint
            | F::Rgba32Sint
            | F::Rgba32Float
            | F::Bc1RgbaUnorm
            | F::Bc1RgbaUnormSrgb
            | F::Bc2RgbaUnorm
            | F::Bc2RgbaUnormSrgb
            | F::Bc3RgbaUnorm
            | F::Bc3RgbaUnormSrgb
            | F::Bc4RUnorm
            | F::Bc4RSnorm
            | F::Bc5RgUnorm
            | F::Bc5RgSnorm
            | F::Bc6hRgbUfloat
            | F::Bc6hRgbFloat
            | F::Bc7RgbaUnorm
            | F::Bc7RgbaUnormSrgb
            | F::Etc2Rgb8Unorm
            | F::Etc2Rgb8UnormSrgb
            | F::Etc2Rgb8A1Unorm
            | F::Etc2Rgb8A1UnormSrgb
            | F::Etc2Rgba8Unorm
            | F::Etc2Rgba8UnormSrgb
            | F::EacR11Unorm
            | F::EacR11Snorm
            | F::EacRg11Unorm
            | F::EacRg11Snorm
            | F::Astc { .. } => (true, false, false),
            F::Stencil8 => (false, false, true),
            F::Depth16Unorm | F::Depth24Plus | F::Depth32Float => (false, true, false),
            F::Depth24PlusStencil8 | F::Depth32FloatStencil8 => (false, true, true),
            F::NV12 | F::P010 => (false, false, false),
        }
    }

    /// 色の面を持つとき `true`。`NV12`・`P010` は輝度・色差のチャネルで、色の面を持たない。
    #[must_use]
    pub fn has_color_aspect(&self) -> bool {
        self.aspects().0
    }

    /// 深度の面を持つとき `true`。
    #[must_use]
    pub fn has_depth_aspect(&self) -> bool {
        self.aspects().1
    }

    /// ステンシルの面を持つとき `true`。
    #[must_use]
    pub fn has_stencil_aspect(&self) -> bool {
        self.aspects().2
    }

    /// 深度またはステンシルの面を持つとき `true`。
    #[must_use]
    pub fn is_depth_stencil_format(&self) -> bool {
        let (_, depth, stencil) = self.aspects();
        depth || stencil
    }

    /// 深度とステンシルの両方の面を持つとき `true`。
    #[must_use]
    pub fn is_combined_depth_stencil_format(&self) -> bool {
        let (_, depth, stencil) = self.aspects();
        depth && stencil
    }

    /// 複数の平面を持つ形式のとき `true`。
    #[must_use]
    pub fn is_multi_planar_format(&self) -> bool {
        self.planes().is_some()
    }

    /// 平面の数を返す。`NV12`・`P010` は `Some(2)`、ほかは `None`。
    #[must_use]
    pub fn planes(&self) -> Option<u32> {
        match self {
            TextureFormat::NV12 | TextureFormat::P010 => Some(2),
            _ => None,
        }
    }

    /// ブロックの幅と高さ（テクセル）を返す。非圧縮は `(1, 1)`、BC・ETC2・EAC は `(4, 4)`、ASTC はブロックの大きさ。
    #[must_use]
    pub fn block_dimensions(&self) -> (u32, u32) {
        use TextureFormat as F;
        match self {
            F::R8Unorm
            | F::R8Snorm
            | F::R8Uint
            | F::R8Sint
            | F::R16Uint
            | F::R16Sint
            | F::R16Unorm
            | F::R16Snorm
            | F::R16Float
            | F::Rg8Unorm
            | F::Rg8Snorm
            | F::Rg8Uint
            | F::Rg8Sint
            | F::R32Uint
            | F::R32Sint
            | F::R32Float
            | F::Rg16Uint
            | F::Rg16Sint
            | F::Rg16Unorm
            | F::Rg16Snorm
            | F::Rg16Float
            | F::Rgba8Unorm
            | F::Rgba8UnormSrgb
            | F::Rgba8Snorm
            | F::Rgba8Uint
            | F::Rgba8Sint
            | F::Bgra8Unorm
            | F::Bgra8UnormSrgb
            | F::Rgb9e5Ufloat
            | F::Rgb10a2Uint
            | F::Rgb10a2Unorm
            | F::Rg11b10Ufloat
            | F::R64Uint
            | F::Rg32Uint
            | F::Rg32Sint
            | F::Rg32Float
            | F::Rgba16Uint
            | F::Rgba16Sint
            | F::Rgba16Unorm
            | F::Rgba16Snorm
            | F::Rgba16Float
            | F::Rgba32Uint
            | F::Rgba32Sint
            | F::Rgba32Float
            | F::Stencil8
            | F::Depth16Unorm
            | F::Depth24Plus
            | F::Depth24PlusStencil8
            | F::Depth32Float
            | F::Depth32FloatStencil8
            | F::NV12
            | F::P010 => (1, 1),
            F::Bc1RgbaUnorm
            | F::Bc1RgbaUnormSrgb
            | F::Bc2RgbaUnorm
            | F::Bc2RgbaUnormSrgb
            | F::Bc3RgbaUnorm
            | F::Bc3RgbaUnormSrgb
            | F::Bc4RUnorm
            | F::Bc4RSnorm
            | F::Bc5RgUnorm
            | F::Bc5RgSnorm
            | F::Bc6hRgbUfloat
            | F::Bc6hRgbFloat
            | F::Bc7RgbaUnorm
            | F::Bc7RgbaUnormSrgb
            | F::Etc2Rgb8Unorm
            | F::Etc2Rgb8UnormSrgb
            | F::Etc2Rgb8A1Unorm
            | F::Etc2Rgb8A1UnormSrgb
            | F::Etc2Rgba8Unorm
            | F::Etc2Rgba8UnormSrgb
            | F::EacR11Unorm
            | F::EacR11Snorm
            | F::EacRg11Unorm
            | F::EacRg11Snorm => (4, 4),
            F::Astc { block, .. } => block.dimensions(),
        }
    }

    /// 圧縮形式（ブロックの大きさが `(1, 1)` でない形式）のとき `true`。
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.block_dimensions() != (1, 1)
    }

    /// sRGB の接尾を外した形式を返す。対の無い形式は自身を返す。
    #[must_use]
    pub fn remove_srgb_suffix(&self) -> TextureFormat {
        use TextureFormat as F;
        match *self {
            F::Rgba8UnormSrgb => F::Rgba8Unorm,
            F::Bgra8UnormSrgb => F::Bgra8Unorm,
            F::Bc1RgbaUnormSrgb => F::Bc1RgbaUnorm,
            F::Bc2RgbaUnormSrgb => F::Bc2RgbaUnorm,
            F::Bc3RgbaUnormSrgb => F::Bc3RgbaUnorm,
            F::Bc7RgbaUnormSrgb => F::Bc7RgbaUnorm,
            F::Etc2Rgb8UnormSrgb => F::Etc2Rgb8Unorm,
            F::Etc2Rgb8A1UnormSrgb => F::Etc2Rgb8A1Unorm,
            F::Etc2Rgba8UnormSrgb => F::Etc2Rgba8Unorm,
            F::Astc {
                block,
                channel: AstcChannel::UnormSrgb,
            } => F::Astc {
                block,
                channel: AstcChannel::Unorm,
            },
            _ => *self,
        }
    }

    /// sRGB の接尾を付けた形式を返す。対の無い形式は自身を返す。
    #[must_use]
    pub fn add_srgb_suffix(&self) -> TextureFormat {
        use TextureFormat as F;
        match *self {
            F::Rgba8Unorm => F::Rgba8UnormSrgb,
            F::Bgra8Unorm => F::Bgra8UnormSrgb,
            F::Bc1RgbaUnorm => F::Bc1RgbaUnormSrgb,
            F::Bc2RgbaUnorm => F::Bc2RgbaUnormSrgb,
            F::Bc3RgbaUnorm => F::Bc3RgbaUnormSrgb,
            F::Bc7RgbaUnorm => F::Bc7RgbaUnormSrgb,
            F::Etc2Rgb8Unorm => F::Etc2Rgb8UnormSrgb,
            F::Etc2Rgb8A1Unorm => F::Etc2Rgb8A1UnormSrgb,
            F::Etc2Rgba8Unorm => F::Etc2Rgba8UnormSrgb,
            F::Astc {
                block,
                channel: AstcChannel::Unorm,
            } => F::Astc {
                block,
                channel: AstcChannel::UnormSrgb,
            },
            _ => *self,
        }
    }

    /// sRGB の形式のとき `true`。
    #[must_use]
    pub fn is_srgb(&self) -> bool {
        *self != self.remove_srgb_suffix()
    }

    /// 使うのに要る機能を返す。要らない形式は `None`。
    ///
    /// 値は `FeatureId("feature:<名前>")` の形。ASTC はチャネルが `Hdr` かどうかで機能が分かれる。
    #[must_use]
    pub fn required_feature(&self) -> Option<FeatureId> {
        use TextureFormat as F;
        match self {
            F::R8Unorm
            | F::R8Snorm
            | F::R8Uint
            | F::R8Sint
            | F::R16Uint
            | F::R16Sint
            | F::R16Float
            | F::Rg8Unorm
            | F::Rg8Snorm
            | F::Rg8Uint
            | F::Rg8Sint
            | F::R32Uint
            | F::R32Sint
            | F::R32Float
            | F::Rg16Uint
            | F::Rg16Sint
            | F::Rg16Float
            | F::Rgba8Unorm
            | F::Rgba8UnormSrgb
            | F::Rgba8Snorm
            | F::Rgba8Uint
            | F::Rgba8Sint
            | F::Bgra8Unorm
            | F::Bgra8UnormSrgb
            | F::Rgb9e5Ufloat
            | F::Rgb10a2Uint
            | F::Rgb10a2Unorm
            | F::Rg11b10Ufloat
            | F::Rg32Uint
            | F::Rg32Sint
            | F::Rg32Float
            | F::Rgba16Uint
            | F::Rgba16Sint
            | F::Rgba16Float
            | F::Rgba32Uint
            | F::Rgba32Sint
            | F::Rgba32Float
            | F::Stencil8
            | F::Depth16Unorm
            | F::Depth24Plus
            | F::Depth24PlusStencil8
            | F::Depth32Float => None,
            F::R64Uint => Some(FeatureId("feature:TEXTURE_INT64_ATOMIC")),
            F::Depth32FloatStencil8 => Some(FeatureId("feature:DEPTH32FLOAT_STENCIL8")),
            F::NV12 => Some(FeatureId("feature:TEXTURE_FORMAT_NV12")),
            F::P010 => Some(FeatureId("feature:TEXTURE_FORMAT_P010")),
            F::R16Unorm
            | F::R16Snorm
            | F::Rg16Unorm
            | F::Rg16Snorm
            | F::Rgba16Unorm
            | F::Rgba16Snorm => Some(FeatureId("feature:TEXTURE_FORMAT_16BIT_NORM")),
            F::Bc1RgbaUnorm
            | F::Bc1RgbaUnormSrgb
            | F::Bc2RgbaUnorm
            | F::Bc2RgbaUnormSrgb
            | F::Bc3RgbaUnorm
            | F::Bc3RgbaUnormSrgb
            | F::Bc4RUnorm
            | F::Bc4RSnorm
            | F::Bc5RgUnorm
            | F::Bc5RgSnorm
            | F::Bc6hRgbUfloat
            | F::Bc6hRgbFloat
            | F::Bc7RgbaUnorm
            | F::Bc7RgbaUnormSrgb => Some(FeatureId("feature:TEXTURE_COMPRESSION_BC")),
            F::Etc2Rgb8Unorm
            | F::Etc2Rgb8UnormSrgb
            | F::Etc2Rgb8A1Unorm
            | F::Etc2Rgb8A1UnormSrgb
            | F::Etc2Rgba8Unorm
            | F::Etc2Rgba8UnormSrgb
            | F::EacR11Unorm
            | F::EacR11Snorm
            | F::EacRg11Unorm
            | F::EacRg11Snorm => Some(FeatureId("feature:TEXTURE_COMPRESSION_ETC2")),
            F::Astc {
                channel: AstcChannel::Hdr,
                ..
            } => Some(FeatureId("feature:TEXTURE_COMPRESSION_ASTC_HDR")),
            F::Astc {
                channel: AstcChannel::Unorm | AstcChannel::UnormSrgb,
                ..
            } => Some(FeatureId("feature:TEXTURE_COMPRESSION_ASTC")),
        }
    }
}

/// 値の一覧（`ALL_FORMATS`・`ASTC_BLOCKS`・`ASTC_CHANNELS`）への足し忘れを検出する試験。
///
/// 番号を返す `match` は `_` の腕を持たないので、variant を足すとコンパイルが落ちて番号を足させる。
/// 一覧への足し忘れは、各番号がちょうど期待の件数だけ現れるかを数える試験が落とす。
#[cfg(test)]
mod tests {
    use super::*;

    const VARIANTS: usize = 77;

    fn ordinal(f: TextureFormat) -> usize {
        use TextureFormat as F;
        match f {
            F::R8Unorm => 0,
            F::R8Snorm => 1,
            F::R8Uint => 2,
            F::R8Sint => 3,
            F::R16Uint => 4,
            F::R16Sint => 5,
            F::R16Unorm => 6,
            F::R16Snorm => 7,
            F::R16Float => 8,
            F::Rg8Unorm => 9,
            F::Rg8Snorm => 10,
            F::Rg8Uint => 11,
            F::Rg8Sint => 12,
            F::R32Uint => 13,
            F::R32Sint => 14,
            F::R32Float => 15,
            F::Rg16Uint => 16,
            F::Rg16Sint => 17,
            F::Rg16Unorm => 18,
            F::Rg16Snorm => 19,
            F::Rg16Float => 20,
            F::Rgba8Unorm => 21,
            F::Rgba8UnormSrgb => 22,
            F::Rgba8Snorm => 23,
            F::Rgba8Uint => 24,
            F::Rgba8Sint => 25,
            F::Bgra8Unorm => 26,
            F::Bgra8UnormSrgb => 27,
            F::Rgb9e5Ufloat => 28,
            F::Rgb10a2Uint => 29,
            F::Rgb10a2Unorm => 30,
            F::Rg11b10Ufloat => 31,
            F::R64Uint => 32,
            F::Rg32Uint => 33,
            F::Rg32Sint => 34,
            F::Rg32Float => 35,
            F::Rgba16Uint => 36,
            F::Rgba16Sint => 37,
            F::Rgba16Unorm => 38,
            F::Rgba16Snorm => 39,
            F::Rgba16Float => 40,
            F::Rgba32Uint => 41,
            F::Rgba32Sint => 42,
            F::Rgba32Float => 43,
            F::Stencil8 => 44,
            F::Depth16Unorm => 45,
            F::Depth24Plus => 46,
            F::Depth24PlusStencil8 => 47,
            F::Depth32Float => 48,
            F::Depth32FloatStencil8 => 49,
            F::NV12 => 50,
            F::P010 => 51,
            F::Bc1RgbaUnorm => 52,
            F::Bc1RgbaUnormSrgb => 53,
            F::Bc2RgbaUnorm => 54,
            F::Bc2RgbaUnormSrgb => 55,
            F::Bc3RgbaUnorm => 56,
            F::Bc3RgbaUnormSrgb => 57,
            F::Bc4RUnorm => 58,
            F::Bc4RSnorm => 59,
            F::Bc5RgUnorm => 60,
            F::Bc5RgSnorm => 61,
            F::Bc6hRgbUfloat => 62,
            F::Bc6hRgbFloat => 63,
            F::Bc7RgbaUnorm => 64,
            F::Bc7RgbaUnormSrgb => 65,
            F::Etc2Rgb8Unorm => 66,
            F::Etc2Rgb8UnormSrgb => 67,
            F::Etc2Rgb8A1Unorm => 68,
            F::Etc2Rgb8A1UnormSrgb => 69,
            F::Etc2Rgba8Unorm => 70,
            F::Etc2Rgba8UnormSrgb => 71,
            F::EacR11Unorm => 72,
            F::EacR11Snorm => 73,
            F::EacRg11Unorm => 74,
            F::EacRg11Snorm => 75,
            F::Astc { .. } => 76,
        }
    }

    fn block_ordinal(b: AstcBlock) -> usize {
        match b {
            AstcBlock::B4x4 => 0,
            AstcBlock::B5x4 => 1,
            AstcBlock::B5x5 => 2,
            AstcBlock::B6x5 => 3,
            AstcBlock::B6x6 => 4,
            AstcBlock::B8x5 => 5,
            AstcBlock::B8x6 => 6,
            AstcBlock::B8x8 => 7,
            AstcBlock::B10x5 => 8,
            AstcBlock::B10x6 => 9,
            AstcBlock::B10x8 => 10,
            AstcBlock::B10x10 => 11,
            AstcBlock::B12x10 => 12,
            AstcBlock::B12x12 => 13,
        }
    }

    fn channel_ordinal(c: AstcChannel) -> usize {
        match c {
            AstcChannel::Unorm => 0,
            AstcChannel::UnormSrgb => 1,
            AstcChannel::Hdr => 2,
        }
    }

    #[test]
    fn all_formats_cover_every_variant() {
        let mut counts = [0usize; VARIANTS];
        for f in TextureFormat::all() {
            counts[ordinal(*f)] += 1;
        }
        for (i, c) in counts.iter().enumerate() {
            let expected = if i == VARIANTS - 1 { 42 } else { 1 };
            assert_eq!(*c, expected, "variant ordinal {i}");
        }
    }

    #[test]
    fn all_formats_follow_declaration_order() {
        let all = TextureFormat::all();
        assert_eq!(all.len(), 118);
        for (i, f) in all.iter().take(VARIANTS - 1).enumerate() {
            assert_eq!(ordinal(*f), i);
        }
        let mut k = 0;
        for b in 0..14 {
            for c in 0..3 {
                match all[VARIANTS - 1 + k] {
                    TextureFormat::Astc { block, channel } => {
                        assert_eq!(block_ordinal(block), b);
                        assert_eq!(channel_ordinal(channel), c);
                    }
                    other => panic!("ASTC の位置に別の形式: {other:?}"),
                }
                k += 1;
            }
        }
        assert_eq!(k, 42);
    }

    #[test]
    fn astc_blocks_cover_every_variant() {
        let got: Vec<usize> = AstcBlock::all().iter().map(|b| block_ordinal(*b)).collect();
        assert_eq!(got, (0..14).collect::<Vec<_>>());
    }

    #[test]
    fn astc_channels_cover_every_variant() {
        let got: Vec<usize> = AstcChannel::all()
            .iter()
            .map(|c| channel_ordinal(*c))
            .collect();
        assert_eq!(got, (0..3).collect::<Vec<_>>());
    }
}
