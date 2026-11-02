//! テクスチャの作成の記述子を照らす純粋関数（TASK-1942。TEX-59〜TEX-62・TEX-64 のポインタ）。
//!
//! `texture` モジュールの一部で、公開のパスは `fandhe_silicon_core::texture::…`。
//! 背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。

use super::{TextureDescriptor, TextureDimension};
use crate::error::ContractError;

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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::{TextureExtent, TextureUsage};
    use crate::texture_format::TextureFormat;

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
}
