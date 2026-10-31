//! `TextureFormat` の性質の関数の値を、比較基準の `wgpu-types` 30.0.1 の同じ名前の関数の値に照らす公開テスト（TASK-1312）。
//!
//! 期待値は `wgpu-types` 30.0.1 の `texture/format.rs` の関数の返す値から書いた定数で、実行時に wgpu は使わない。
//! 対象は既存の 4 値と、色・深度ステンシル・複数平面・BC・ETC2/EAC・ASTC の代表値。

use fandhe_silicon_core::capability::FeatureId;
use fandhe_silicon_core::texture_format::{AstcBlock, AstcChannel, TextureFormat};

/// 1 形式ぶんの期待値。`without_srgb`・`with_srgb` の `None` は「自身」を表す。
struct Expect {
    format: TextureFormat,
    color: bool,
    depth: bool,
    stencil: bool,
    depth_stencil: bool,
    combined: bool,
    planes: Option<u32>,
    block: (u32, u32),
    srgb: bool,
    without_srgb: Option<TextureFormat>,
    with_srgb: Option<TextureFormat>,
    feature: Option<&'static str>,
}

/// 非圧縮の色の形式（面は色だけ・ブロックは `(1, 1)`・機能は要らない）の既定値から作る。
fn color(format: TextureFormat) -> Expect {
    Expect {
        format,
        color: true,
        depth: false,
        stencil: false,
        depth_stencil: false,
        combined: false,
        planes: None,
        block: (1, 1),
        srgb: false,
        without_srgb: None,
        with_srgb: None,
        feature: None,
    }
}

/// 深度・ステンシルの形式の既定値から作る。
fn ds(format: TextureFormat, depth: bool, stencil: bool) -> Expect {
    Expect {
        color: false,
        depth,
        stencil,
        depth_stencil: true,
        combined: depth && stencil,
        ..color(format)
    }
}

/// 4x4 ブロックの圧縮形式の既定値から作る。
fn block4(format: TextureFormat, feature: &'static str) -> Expect {
    Expect {
        block: (4, 4),
        feature: Some(feature),
        ..color(format)
    }
}

/// ASTC の形式の既定値から作る。機能はチャネルが `Hdr` かどうかで分かれる。
fn astc(block: AstcBlock, channel: AstcChannel, dims: (u32, u32)) -> Expect {
    let feature = match channel {
        AstcChannel::Hdr => "feature:TEXTURE_COMPRESSION_ASTC_HDR",
        _ => "feature:TEXTURE_COMPRESSION_ASTC",
    };
    Expect {
        block: dims,
        srgb: channel == AstcChannel::UnormSrgb,
        feature: Some(feature),
        ..color(TextureFormat::Astc { block, channel })
    }
}

fn expectations() -> Vec<Expect> {
    use TextureFormat as F;
    vec![
        // 既存の 4 値
        Expect {
            with_srgb: Some(F::Rgba8UnormSrgb),
            ..color(F::Rgba8Unorm)
        },
        Expect {
            with_srgb: Some(F::Bgra8UnormSrgb),
            ..color(F::Bgra8Unorm)
        },
        color(F::Rgba32Float),
        ds(F::Depth32Float, true, false),
        // 色
        color(F::R8Unorm),
        Expect {
            srgb: true,
            without_srgb: Some(F::Rgba8Unorm),
            ..color(F::Rgba8UnormSrgb)
        },
        Expect {
            feature: Some("feature:TEXTURE_FORMAT_16BIT_NORM"),
            ..color(F::R16Unorm)
        },
        Expect {
            feature: Some("feature:TEXTURE_INT64_ATOMIC"),
            ..color(F::R64Uint)
        },
        // 深度ステンシル
        ds(F::Stencil8, false, true),
        ds(F::Depth16Unorm, true, false),
        ds(F::Depth24Plus, true, false),
        ds(F::Depth24PlusStencil8, true, true),
        Expect {
            feature: Some("feature:DEPTH32FLOAT_STENCIL8"),
            ..ds(F::Depth32FloatStencil8, true, true)
        },
        // 複数平面（色の面は持たない）
        Expect {
            color: false,
            planes: Some(2),
            feature: Some("feature:TEXTURE_FORMAT_NV12"),
            ..color(F::NV12)
        },
        Expect {
            color: false,
            planes: Some(2),
            feature: Some("feature:TEXTURE_FORMAT_P010"),
            ..color(F::P010)
        },
        // BC
        Expect {
            with_srgb: Some(F::Bc1RgbaUnormSrgb),
            ..block4(F::Bc1RgbaUnorm, "feature:TEXTURE_COMPRESSION_BC")
        },
        Expect {
            srgb: true,
            without_srgb: Some(F::Bc1RgbaUnorm),
            ..block4(F::Bc1RgbaUnormSrgb, "feature:TEXTURE_COMPRESSION_BC")
        },
        block4(F::Bc6hRgbUfloat, "feature:TEXTURE_COMPRESSION_BC"),
        // ETC2 / EAC
        Expect {
            with_srgb: Some(F::Etc2Rgb8UnormSrgb),
            ..block4(F::Etc2Rgb8Unorm, "feature:TEXTURE_COMPRESSION_ETC2")
        },
        Expect {
            srgb: true,
            without_srgb: Some(F::Etc2Rgb8Unorm),
            ..block4(F::Etc2Rgb8UnormSrgb, "feature:TEXTURE_COMPRESSION_ETC2")
        },
        block4(F::EacR11Unorm, "feature:TEXTURE_COMPRESSION_ETC2"),
        // ASTC
        Expect {
            with_srgb: Some(F::Astc {
                block: AstcBlock::B4x4,
                channel: AstcChannel::UnormSrgb,
            }),
            ..astc(AstcBlock::B4x4, AstcChannel::Unorm, (4, 4))
        },
        Expect {
            without_srgb: Some(F::Astc {
                block: AstcBlock::B8x5,
                channel: AstcChannel::Unorm,
            }),
            ..astc(AstcBlock::B8x5, AstcChannel::UnormSrgb, (8, 5))
        },
        Expect {
            with_srgb: Some(F::Astc {
                block: AstcBlock::B10x5,
                channel: AstcChannel::UnormSrgb,
            }),
            ..astc(AstcBlock::B10x5, AstcChannel::Unorm, (10, 5))
        },
        astc(AstcBlock::B12x12, AstcChannel::Hdr, (12, 12)),
    ]
}

#[test]
fn properties_match_wgpu_types_30_0_1() {
    for e in expectations() {
        let f = e.format;
        assert_eq!(f.has_color_aspect(), e.color, "{f:?} has_color_aspect");
        assert_eq!(f.has_depth_aspect(), e.depth, "{f:?} has_depth_aspect");
        assert_eq!(
            f.has_stencil_aspect(),
            e.stencil,
            "{f:?} has_stencil_aspect"
        );
        assert_eq!(
            f.is_depth_stencil_format(),
            e.depth_stencil,
            "{f:?} is_depth_stencil_format"
        );
        assert_eq!(
            f.is_combined_depth_stencil_format(),
            e.combined,
            "{f:?} is_combined_depth_stencil_format"
        );
        assert_eq!(f.planes(), e.planes, "{f:?} planes");
        assert_eq!(
            f.is_multi_planar_format(),
            e.planes.is_some(),
            "{f:?} is_multi_planar_format"
        );
        assert_eq!(f.block_dimensions(), e.block, "{f:?} block_dimensions");
        assert_eq!(f.is_compressed(), e.block != (1, 1), "{f:?} is_compressed");
        assert_eq!(f.is_srgb(), e.srgb, "{f:?} is_srgb");
        assert_eq!(
            f.remove_srgb_suffix(),
            e.without_srgb.unwrap_or(f),
            "{f:?} remove_srgb_suffix"
        );
        assert_eq!(
            f.add_srgb_suffix(),
            e.with_srgb.unwrap_or(f),
            "{f:?} add_srgb_suffix"
        );
        assert_eq!(
            f.required_feature(),
            e.feature.map(FeatureId),
            "{f:?} required_feature"
        );
    }
}
