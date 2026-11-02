//! テクスチャの作成の記述子を照らす純粋関数（TASK-1942。TEX-56・TEX-59〜TEX-65 のポインタ）。
//!
//! `texture` モジュールの一部で、公開のパスは `fandhe_silicon_core::texture::…`。
//! 背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。

use super::{TextureDescriptor, TextureDimension, TextureExtent, TextureUsage};
use crate::error::ContractError;
use crate::texture_format::TextureFormat;

/// 次元と大きさから mip の段数の上限を返す。
///
/// `D1` は 1。`D2` は幅と高さの大きい方、`D3` は 3 辺の最大を `m` として
/// `32 - m.leading_zeros()`。その値と 16 の小さい方を返す。
///
/// ```
/// use fandhe_silicon_core::texture::{max_mip_level_count, TextureDimension, TextureExtent};
///
/// let ext = |width, height, depth_or_array_layers| TextureExtent {
///     width,
///     height,
///     depth_or_array_layers,
/// };
/// assert_eq!(max_mip_level_count(TextureDimension::D2, ext(1280, 720, 1)), 11);
/// assert_eq!(max_mip_level_count(TextureDimension::D2, ext(65536, 1, 1)), 16);
/// assert_eq!(max_mip_level_count(TextureDimension::D3, ext(4, 4, 32)), 6);
/// assert_eq!(max_mip_level_count(TextureDimension::D1, ext(4096, 1, 1)), 1);
/// ```
#[must_use]
pub fn max_mip_level_count(dimension: TextureDimension, size: TextureExtent) -> u32 {
    let longest = match dimension {
        TextureDimension::D1 => return 1,
        TextureDimension::D2 => size.width.max(size.height),
        TextureDimension::D3 => size.width.max(size.height).max(size.depth_or_array_layers),
    };
    (32 - longest.leading_zeros()).min(16)
}

/// 形式が BC 系か ASTC かを返す（3 次元で許す圧縮形式の見分けに使う）。
fn is_bc_or_astc(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::Bc1RgbaUnorm
            | TextureFormat::Bc1RgbaUnormSrgb
            | TextureFormat::Bc2RgbaUnorm
            | TextureFormat::Bc2RgbaUnormSrgb
            | TextureFormat::Bc3RgbaUnorm
            | TextureFormat::Bc3RgbaUnormSrgb
            | TextureFormat::Bc4RUnorm
            | TextureFormat::Bc4RSnorm
            | TextureFormat::Bc5RgUnorm
            | TextureFormat::Bc5RgSnorm
            | TextureFormat::Bc6hRgbUfloat
            | TextureFormat::Bc6hRgbFloat
            | TextureFormat::Bc7RgbaUnorm
            | TextureFormat::Bc7RgbaUnormSrgb
            | TextureFormat::Astc { .. }
    )
}

/// 記述子を上から順に照らし、最初に当たった 1 件を `InvalidArgument` で返す。
///
/// 上限ちょうどは通す。上限は呼び出し側が 4 つの `u32` で渡す。`label` は読まない。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::texture::{
///     validate_texture_descriptor, TextureDescriptor, TextureDimension, TextureExtent,
///     TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let size = TextureExtent { width: 1280, height: 720, depth_or_array_layers: 1 };
/// let desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     size,
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::TEXTURE_BINDING,
/// );
/// assert_eq!(validate_texture_descriptor(&desc, 4096, 8192, 1024, 256), Ok(()));
///
/// let mut empty = desc.clone();
/// empty.usage = TextureUsage::empty();
/// assert!(matches!(
///     validate_texture_descriptor(&empty, 4096, 8192, 1024, 256),
///     Err(ContractError::InvalidArgument(_))
/// ));
///
/// let mut wide = desc.clone();
/// wide.size.width = 8193;
/// assert!(matches!(
///     validate_texture_descriptor(&wide, 4096, 8192, 1024, 256),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 次のどれかに当たるとき `ContractError::InvalidArgument`（上から順に照らす）。
///
/// - 用途が空
/// - 幅・高さ・`depth_or_array_layers` のどれかが 0
/// - 次元ごとの上限の超過。`D1` は幅が `max_texture_dimension_1d` を超えるか、高さと
///   `depth_or_array_layers` が 1 でない。`D2` は幅・高さが `max_texture_dimension_2d` を、
///   `depth_or_array_layers` が `max_texture_array_layers` を超える。`D3` は 3 辺のどれかが
///   `max_texture_dimension_3d` を超える
/// - サンプル数が 0、2 の冪でない、または次元ごとの上限（`D1`・`D3` は 1、`D2` は 32）を超える
/// - 次元が `D2` でないのに、形式が深度・ステンシルを持つか、用途に `TRANSIENT_ATTACHMENT` を含む
/// - 次元が `D1` で、形式が圧縮形式か用途に `RENDER_ATTACHMENT` を含む。または次元が `D3` で、
///   形式が BC・ASTC 以外の圧縮形式
/// - 形式が圧縮形式で、幅か高さがブロックの大きさの倍数でない
/// - `mip_level_count` が 0 か、`max_mip_level_count` を超える
/// - 用途に `TRANSIENT_ATTACHMENT` を含み、用途が `TRANSIENT_ATTACHMENT | RENDER_ATTACHMENT` と
///   同じでないか、`mip_level_count`・`depth_or_array_layers` が 1 でないか、`view_formats` が空でない
/// - `sample_count` が 1 より大きく、`mip_level_count` が 1 でないか、用途に `STORAGE_BINDING` を含むか、
///   `RENDER_ATTACHMENT` を含まない
/// - `view_formats` の要素が元の形式と同じでなく、`remove_srgb_suffix()` も元の形式のそれと同じでない
pub fn validate_texture_descriptor(
    desc: &TextureDescriptor<'_>,
    max_texture_dimension_1d: u32,
    max_texture_dimension_2d: u32,
    max_texture_dimension_3d: u32,
    max_texture_array_layers: u32,
) -> Result<(), ContractError> {
    if desc.usage.is_empty() {
        return Err(ContractError::InvalidArgument(
            "texture usage must not be empty".to_string(),
        ));
    }
    let (w, h, d) = (
        desc.size.width,
        desc.size.height,
        desc.size.depth_or_array_layers,
    );
    if w == 0 || h == 0 || d == 0 {
        return Err(ContractError::InvalidArgument(format!(
            "texture size must be non-zero (width {w}, height {h}, depth_or_array_layers {d})"
        )));
    }
    let dimension_ok = match desc.dimension {
        TextureDimension::D1 => w <= max_texture_dimension_1d && h == 1 && d == 1,
        TextureDimension::D2 => {
            w <= max_texture_dimension_2d
                && h <= max_texture_dimension_2d
                && d <= max_texture_array_layers
        }
        TextureDimension::D3 => {
            w <= max_texture_dimension_3d
                && h <= max_texture_dimension_3d
                && d <= max_texture_dimension_3d
        }
    };
    if !dimension_ok {
        return Err(ContractError::InvalidArgument(format!(
            "texture dimension {:?} size out of range (width {w}, height {h}, depth_or_array_layers {d}; limits 1d {max_texture_dimension_1d}, 2d {max_texture_dimension_2d}, 3d {max_texture_dimension_3d}, array_layers {max_texture_array_layers})",
            desc.dimension
        )));
    }
    let samples = desc.sample_count;
    let max_samples = match desc.dimension {
        TextureDimension::D1 | TextureDimension::D3 => 1,
        TextureDimension::D2 => 32,
    };
    if samples == 0 || !samples.is_power_of_two() || samples > max_samples {
        return Err(ContractError::InvalidArgument(format!(
            "texture sample_count {samples} is invalid for dimension {:?} (max {max_samples})",
            desc.dimension
        )));
    }
    let (format, usage) = (desc.format, desc.usage);
    if desc.dimension != TextureDimension::D2
        && (format.is_depth_stencil_format() || usage.contains(TextureUsage::TRANSIENT_ATTACHMENT))
    {
        return Err(ContractError::InvalidArgument(format!(
            "texture format {format:?} or usage {usage:?} requires dimension D2 (got {:?})",
            desc.dimension
        )));
    }
    if desc.dimension == TextureDimension::D1
        && (format.is_compressed() || usage.contains(TextureUsage::RENDER_ATTACHMENT))
    {
        return Err(ContractError::InvalidArgument(format!(
            "texture dimension D1 does not support compressed format {format:?} or RENDER_ATTACHMENT usage {usage:?}"
        )));
    }
    if desc.dimension == TextureDimension::D3 && format.is_compressed() && !is_bc_or_astc(format) {
        return Err(ContractError::InvalidArgument(format!(
            "texture dimension D3 does not support compressed format {format:?}"
        )));
    }
    let (bw, bh) = format.block_dimensions();
    if format.is_compressed() && (w % bw != 0 || h % bh != 0) {
        return Err(ContractError::InvalidArgument(format!(
            "texture size {w}x{h} is not a multiple of the block {bw}x{bh} of format {format:?}"
        )));
    }
    let max_mips = max_mip_level_count(desc.dimension, desc.size);
    let mips = desc.mip_level_count;
    if mips == 0 || mips > max_mips {
        return Err(ContractError::InvalidArgument(format!(
            "texture mip_level_count {mips} is out of range (1..={max_mips})"
        )));
    }
    let transient = TextureUsage::TRANSIENT_ATTACHMENT | TextureUsage::RENDER_ATTACHMENT;
    if usage.contains(TextureUsage::TRANSIENT_ATTACHMENT)
        && (usage != transient || mips != 1 || d != 1 || !desc.view_formats.is_empty())
    {
        return Err(ContractError::InvalidArgument(format!(
            "transient attachment texture requires usage {transient:?}, mip_level_count 1, depth_or_array_layers 1 and no view_formats (got usage {usage:?}, mips {mips}, layers {d}, view_formats {})",
            desc.view_formats.len()
        )));
    }
    if samples > 1
        && (mips != 1
            || usage.contains(TextureUsage::STORAGE_BINDING)
            || !usage.contains(TextureUsage::RENDER_ATTACHMENT))
    {
        return Err(ContractError::InvalidArgument(format!(
            "multisampled texture (sample_count {samples}) requires mip_level_count 1, RENDER_ATTACHMENT usage and no STORAGE_BINDING (got mips {mips}, usage {usage:?})"
        )));
    }
    let base = format.remove_srgb_suffix();
    if let Some(v) = desc
        .view_formats
        .iter()
        .find(|v| **v != format && v.remove_srgb_suffix() != base)
    {
        return Err(ContractError::InvalidArgument(format!(
            "view_formats entry {v:?} is not compatible with texture format {format:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture_format::{AstcBlock, AstcChannel};

    const L1: u32 = 4096;
    const L2: u32 = 8192;
    const L3: u32 = 1024;
    const LA: u32 = 256;

    fn desc(dim: TextureDimension, w: u32, h: u32, d: u32) -> TextureDescriptor<'static> {
        TextureDescriptor::new(
            dim,
            TextureExtent {
                width: w,
                height: h,
                depth_or_array_layers: d,
            },
            TextureFormat::Rgba8Unorm,
            TextureUsage::TEXTURE_BINDING,
        )
    }

    fn check(d: &TextureDescriptor<'_>) -> Result<(), ContractError> {
        validate_texture_descriptor(d, L1, L2, L3, LA)
    }

    fn is_invalid(d: &TextureDescriptor<'_>) -> bool {
        matches!(check(d), Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn descriptor_check_empty_usage_is_rejected() {
        for dim in [
            TextureDimension::D1,
            TextureDimension::D2,
            TextureDimension::D3,
        ] {
            let mut d = desc(dim, 1, 1, 1);
            assert_eq!(check(&d), Ok(()));
            d.usage = TextureUsage::empty();
            assert!(is_invalid(&d));
        }
    }

    #[test]
    fn descriptor_check_zero_extent_is_rejected() {
        for dim in [
            TextureDimension::D1,
            TextureDimension::D2,
            TextureDimension::D3,
        ] {
            assert!(is_invalid(&desc(dim, 0, 1, 1)));
            assert!(is_invalid(&desc(dim, 1, 0, 1)));
            assert!(is_invalid(&desc(dim, 1, 1, 0)));
        }
    }

    #[test]
    fn descriptor_check_d1_limits() {
        let d1 = TextureDimension::D1;
        assert_eq!(check(&desc(d1, 4096, 1, 1)), Ok(()));
        assert!(is_invalid(&desc(d1, 4097, 1, 1)));
        assert!(is_invalid(&desc(d1, 1, 2, 1)));
        assert!(is_invalid(&desc(d1, 1, 1, 2)));
    }

    #[test]
    fn descriptor_check_d2_limits() {
        let d2 = TextureDimension::D2;
        assert_eq!(check(&desc(d2, 8192, 8192, 1)), Ok(()));
        assert!(is_invalid(&desc(d2, 8193, 1, 1)));
        assert!(is_invalid(&desc(d2, 1, 8193, 1)));
        assert_eq!(check(&desc(d2, 1, 1, 256)), Ok(()));
        assert!(is_invalid(&desc(d2, 1, 1, 257)));
    }

    #[test]
    fn descriptor_check_d3_limits() {
        let d3 = TextureDimension::D3;
        assert_eq!(check(&desc(d3, 1024, 1024, 1024)), Ok(()));
        assert!(is_invalid(&desc(d3, 1025, 1, 1)));
        assert!(is_invalid(&desc(d3, 1, 1025, 1)));
        assert!(is_invalid(&desc(d3, 1, 1, 1025)));
    }

    #[test]
    fn descriptor_check_limits_are_not_swapped() {
        // 他の次元の上限より小さい値でも、自分の次元の上限を超えれば拒まれる。
        assert!(is_invalid(&desc(TextureDimension::D1, 4097, 1, 1)));
        assert!(is_invalid(&desc(TextureDimension::D2, 1, 1, 257)));
        assert!(is_invalid(&desc(TextureDimension::D3, 1025, 1025, 1025)));
        assert_eq!(check(&desc(TextureDimension::D2, 4097, 1, 1)), Ok(()));
    }

    fn ext(width: u32, height: u32, depth_or_array_layers: u32) -> TextureExtent {
        TextureExtent {
            width,
            height,
            depth_or_array_layers,
        }
    }

    fn with(
        dim: TextureDimension,
        size: TextureExtent,
        format: TextureFormat,
        usage: TextureUsage,
    ) -> TextureDescriptor<'static> {
        TextureDescriptor::new(dim, size, format, usage)
    }

    #[test]
    fn descriptor_check_max_mip_level_count_values() {
        use TextureDimension::{D1, D2, D3};
        assert_eq!(max_mip_level_count(D2, ext(1280, 720, 1)), 11);
        assert_eq!(max_mip_level_count(D2, ext(16384, 16384, 1)), 15);
        assert_eq!(max_mip_level_count(D2, ext(32768, 1, 1)), 16);
        assert_eq!(max_mip_level_count(D2, ext(65536, 1, 1)), 16);
        assert_eq!(max_mip_level_count(D2, ext(u32::MAX, 1, 1)), 16);
        assert_eq!(max_mip_level_count(D2, ext(4, 4, 32)), 3);
        assert_eq!(max_mip_level_count(D3, ext(4, 4, 32)), 6);
        assert_eq!(max_mip_level_count(D1, ext(4096, 1, 1)), 1);
    }

    #[test]
    fn descriptor_check_depth_stencil_and_transient_need_d2() {
        use TextureDimension::{D1, D2, D3};
        for dim in [D1, D3] {
            for f in [TextureFormat::Depth32Float, TextureFormat::Stencil8] {
                let d = with(dim, ext(4, 1, 1), f, TextureUsage::TEXTURE_BINDING);
                assert!(is_invalid(&d), "{dim:?} {f:?}");
            }
            for u in [
                TextureUsage::TRANSIENT_ATTACHMENT,
                TextureUsage::TRANSIENT_ATTACHMENT | TextureUsage::RENDER_ATTACHMENT,
            ] {
                let d = with(dim, ext(4, 1, 1), TextureFormat::Rgba8Unorm, u);
                assert!(is_invalid(&d), "{dim:?} {u:?}");
            }
        }
        let d = with(
            D2,
            ext(4, 4, 1),
            TextureFormat::Depth32Float,
            TextureUsage::TEXTURE_BINDING,
        );
        assert_eq!(check(&d), Ok(()));
    }

    #[test]
    fn descriptor_check_d1_rejects_compressed_and_render_attachment() {
        let d1 = TextureDimension::D1;
        let bc = with(
            d1,
            ext(8, 1, 1),
            TextureFormat::Bc1RgbaUnorm,
            TextureUsage::TEXTURE_BINDING,
        );
        assert!(is_invalid(&bc));
        let ra = with(
            d1,
            ext(8, 1, 1),
            TextureFormat::Rgba8Unorm,
            TextureUsage::RENDER_ATTACHMENT,
        );
        assert!(is_invalid(&ra));
    }

    #[test]
    fn descriptor_check_d3_compressed_formats() {
        let d3 = TextureDimension::D3;
        let u = TextureUsage::TEXTURE_BINDING;
        let bc = with(d3, ext(8, 8, 2), TextureFormat::Bc1RgbaUnorm, u);
        assert_eq!(check(&bc), Ok(()));
        let astc = TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Unorm,
        };
        assert_eq!(check(&with(d3, ext(12, 12, 2), astc, u)), Ok(()));
        let etc2 = with(d3, ext(8, 8, 2), TextureFormat::Etc2Rgb8Unorm, u);
        assert!(is_invalid(&etc2));
        let ra = with(
            d3,
            ext(8, 8, 2),
            TextureFormat::Rgba8Unorm,
            TextureUsage::RENDER_ATTACHMENT,
        );
        assert_eq!(check(&ra), Ok(()));
    }

    #[test]
    fn descriptor_check_compressed_block_multiple() {
        let d2 = TextureDimension::D2;
        let u = TextureUsage::TEXTURE_BINDING;
        let bc = TextureFormat::Bc1RgbaUnorm;
        assert_eq!(check(&with(d2, ext(8, 8, 1), bc, u)), Ok(()));
        assert!(is_invalid(&with(d2, ext(6, 8, 1), bc, u)));
        assert!(is_invalid(&with(d2, ext(8, 6, 1), bc, u)));
        let astc = TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::Unorm,
        };
        assert_eq!(check(&with(d2, ext(12, 12, 1), astc, u)), Ok(()));
        assert!(is_invalid(&with(d2, ext(8, 8, 1), astc, u)));
        let rgba = TextureFormat::Rgba8Unorm;
        assert_eq!(check(&with(d2, ext(7, 5, 1), rgba, u)), Ok(()));
    }

    #[test]
    fn descriptor_check_mip_level_count() {
        use TextureDimension::{D1, D2, D3};
        let cases = [
            (D2, ext(1280, 720, 1), 11),
            (D3, ext(4, 4, 32), 6),
            (D1, ext(8, 1, 1), 1),
        ];
        for (dim, size, max) in cases {
            let mut d = desc(dim, size.width, size.height, size.depth_or_array_layers);
            d.mip_level_count = max;
            assert_eq!(check(&d), Ok(()), "{dim:?} mip {max}");
            d.mip_level_count = max + 1;
            assert!(is_invalid(&d), "{dim:?} mip {}", max + 1);
            d.mip_level_count = 0;
            assert!(is_invalid(&d), "{dim:?} mip 0");
        }
    }

    #[test]
    fn descriptor_check_sample_count() {
        let mut d = desc(TextureDimension::D2, 16, 16, 1);
        d.usage = TextureUsage::RENDER_ATTACHMENT | TextureUsage::TEXTURE_BINDING;
        for ok in [1, 2, 4, 8, 16, 32] {
            d.sample_count = ok;
            assert_eq!(check(&d), Ok(()), "sample_count {ok}");
        }
        for bad in [0, 3, 64] {
            d.sample_count = bad;
            assert!(is_invalid(&d), "sample_count {bad}");
        }
        for dim in [TextureDimension::D1, TextureDimension::D3] {
            let mut d = desc(dim, 1, 1, 1);
            d.sample_count = 1;
            assert_eq!(check(&d), Ok(()));
            d.sample_count = 2;
            assert!(is_invalid(&d));
        }
    }

    #[test]
    fn descriptor_check_transient_attachment() {
        let tr = TextureUsage::TRANSIENT_ATTACHMENT | TextureUsage::RENDER_ATTACHMENT;
        let mk = |u| {
            with(
                TextureDimension::D2,
                ext(16, 16, 1),
                TextureFormat::Rgba8Unorm,
                u,
            )
        };
        let mut d = mk(tr);
        assert_eq!(check(&d), Ok(()));
        d.sample_count = 4;
        assert_eq!(check(&d), Ok(()));
        assert!(is_invalid(&mk(tr | TextureUsage::COPY_SRC)));
        assert!(is_invalid(&mk(TextureUsage::TRANSIENT_ATTACHMENT)));
        let mut d = mk(tr);
        d.mip_level_count = 2;
        assert!(is_invalid(&d));
        let d = with(
            TextureDimension::D2,
            ext(16, 16, 2),
            TextureFormat::Rgba8Unorm,
            tr,
        );
        assert!(is_invalid(&d));
        let mut d = mk(tr);
        d.view_formats = &[TextureFormat::Rgba8UnormSrgb];
        assert!(is_invalid(&d));
    }

    #[test]
    fn descriptor_check_multisample() {
        let mut d = desc(TextureDimension::D2, 16, 16, 1);
        d.usage = TextureUsage::RENDER_ATTACHMENT;
        d.sample_count = 4;
        assert_eq!(check(&d), Ok(()));
        let mut m = d.clone();
        m.mip_level_count = 2;
        assert!(is_invalid(&m));
        let mut s = d.clone();
        s.usage = TextureUsage::RENDER_ATTACHMENT | TextureUsage::STORAGE_BINDING;
        assert!(is_invalid(&s));
        let mut n = d.clone();
        n.usage = TextureUsage::TEXTURE_BINDING;
        assert!(is_invalid(&n));
        let mut one = n.clone();
        one.sample_count = 1;
        one.mip_level_count = 2;
        one.usage = TextureUsage::STORAGE_BINDING;
        assert_eq!(check(&one), Ok(()));
    }

    #[test]
    fn descriptor_check_view_formats_compatibility() {
        use TextureFormat::{Bgra8Unorm, Bgra8UnormSrgb, Rgba8Unorm, Rgba8UnormSrgb};
        let u = TextureUsage::TEXTURE_BINDING;
        let mk = |size, f| with(TextureDimension::D2, size, f, u);
        let mut d = mk(ext(16, 16, 1), Rgba8Unorm);
        d.view_formats = &[Rgba8UnormSrgb];
        assert_eq!(check(&d), Ok(()));
        d.view_formats = &[Rgba8Unorm];
        assert_eq!(check(&d), Ok(()));
        d.view_formats = &[Bgra8Unorm];
        assert!(is_invalid(&d));
        d.view_formats = &[Rgba8UnormSrgb, Bgra8Unorm];
        assert!(is_invalid(&d));
        let mut d = mk(ext(16, 16, 1), Bgra8UnormSrgb);
        d.view_formats = &[Bgra8Unorm];
        assert_eq!(check(&d), Ok(()));
        const fn astc(channel: AstcChannel) -> TextureFormat {
            TextureFormat::Astc {
                block: AstcBlock::B6x6,
                channel,
            }
        }
        const UNORM: TextureFormat = astc(AstcChannel::Unorm);
        const SRGB: TextureFormat = astc(AstcChannel::UnormSrgb);
        const HDR: TextureFormat = astc(AstcChannel::Hdr);
        let mut d = mk(ext(12, 12, 1), UNORM);
        d.view_formats = &[SRGB];
        assert_eq!(check(&d), Ok(()));
        d.view_formats = &[UNORM];
        assert_eq!(check(&d), Ok(()));
        d.view_formats = &[HDR];
        assert!(is_invalid(&d));
    }
}
