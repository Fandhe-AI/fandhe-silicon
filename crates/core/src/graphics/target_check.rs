//! 描画先の大きさの入力の検査と、描画先の形式の 1 画素のバイト数・描画先のバイト数を求める純粋関数（TASK-374。TEX-8・TEX-9・TEX-10 のポインタ）。
//!
//! `graphics` モジュールの一部で、公開のパスは `fandhe_silicon_core::graphics::…`。
//! 背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。

use super::{DepthFormat, PixelFormat};
use crate::error::ContractError;
use crate::memory::validate_buffer_range;

/// 色の形式の 1 画素のバイト数を返す。失敗しない。
///
/// ```
/// use fandhe_silicon_core::graphics::{pixel_format_bytes, PixelFormat};
/// assert_eq!(pixel_format_bytes(PixelFormat::Rgba32Float), 16);
/// ```
#[must_use]
pub fn pixel_format_bytes(format: PixelFormat) -> u32 {
    match format {
        PixelFormat::Rgba8Unorm | PixelFormat::Bgra8Unorm => 4,
        PixelFormat::Rgba32Float => 16,
    }
}

/// 深度の形式の 1 画素のバイト数を返す。失敗しない。
///
/// ```
/// use fandhe_silicon_core::graphics::{depth_format_bytes, DepthFormat};
/// assert_eq!(depth_format_bytes(DepthFormat::Depth32Float), 4);
/// ```
#[must_use]
pub fn depth_format_bytes(format: DepthFormat) -> u32 {
    match format {
        DepthFormat::Depth32Float => 4,
    }
}

/// 幅 × 高さ × 1 画素のバイト数を `u64` で求める。0 や上限は見ない（0 を渡せば 0 を返す）。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::graphics::offscreen_target_bytes;
/// assert_eq!(offscreen_target_bytes(1280, 720, 4), Ok(3_686_400));
/// assert!(matches!(
///     offscreen_target_bytes(u32::MAX, u32::MAX, 16),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 積が `u64` であふれるとき `ContractError::InvalidArgument`。
pub fn offscreen_target_bytes(
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
) -> Result<u64, ContractError> {
    u64::from(bytes_per_pixel)
        .checked_mul(u64::from(width))
        .and_then(|v| v.checked_mul(u64::from(height)))
        .ok_or_else(|| {
            ContractError::InvalidArgument(format!(
                "offscreen target byte count overflows u64 (width {width}, height {height}, bytes per pixel {bytes_per_pixel})"
            ))
        })
}

/// 描画先の幅・高さが 0 のとき、または `max_texture_size` を超えるときに拒む。上限ちょうどは通す。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::graphics::validate_offscreen_target_size;
/// assert_eq!(validate_offscreen_target_size(1280, 720, 16384), Ok(()));
/// assert_eq!(validate_offscreen_target_size(16384, 16384, 16384), Ok(()));
/// assert!(matches!(
///     validate_offscreen_target_size(0, 720, 16384),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// assert!(matches!(
///     validate_offscreen_target_size(16385, 1, 16384),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 幅または高さが 0 のとき、または幅か高さが `max_texture_size` を超えるとき
/// `ContractError::InvalidArgument`。
pub fn validate_offscreen_target_size(
    width: u32,
    height: u32,
    max_texture_size: u32,
) -> Result<(), ContractError> {
    if width == 0 || height == 0 {
        return Err(ContractError::InvalidArgument(format!(
            "offscreen target size must be non-zero (width {width}, height {height})"
        )));
    }
    if width > max_texture_size || height > max_texture_size {
        return Err(ContractError::InvalidArgument(format!(
            "offscreen target size exceeds max_texture_size {max_texture_size} (width {width}, height {height})"
        )));
    }
    Ok(())
}

/// `copy_target_to_buffer` の拒否の文言の先頭に操作名を付ける。`InvalidArgument` 以外はそのまま返す。
fn with_copy_prefix(e: ContractError) -> ContractError {
    match e {
        ContractError::InvalidArgument(m) => {
            ContractError::InvalidArgument(format!("copy_target_to_buffer: {m}"))
        }
        other => other,
    }
}

/// 描画先からバッファへの写しの書き込み先を、背後実装を呼ばずに検査する。
/// 必要なバイト数は幅 × 高さ × 形式の 1 画素のバイト数で、`dst_offset` から数えて `dst_len` に収まるかを見る。ちょうど収まる入力は通す。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::graphics::{validate_copy_target_to_buffer, PixelFormat};
/// let need = 1280 * 720 * 4;
/// assert_eq!(
///     validate_copy_target_to_buffer(1280, 720, PixelFormat::Rgba8Unorm, 0, need),
///     Ok(())
/// );
/// assert!(matches!(
///     validate_copy_target_to_buffer(1280, 720, PixelFormat::Rgba8Unorm, 0, need - 1),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// assert_eq!(
///     validate_copy_target_to_buffer(1280, 720, PixelFormat::Rgba8Unorm, 256, 256 + need),
///     Ok(())
/// );
/// ```
///
/// # Errors
///
/// 必要なバイト数の積のあふれ、`dst_offset` との和のあふれ、書き込み先の長さの不足のいずれも
/// `ContractError::InvalidArgument`。文言の先頭は `copy_target_to_buffer:`。
pub fn validate_copy_target_to_buffer(
    width: u32,
    height: u32,
    format: PixelFormat,
    dst_offset: u64,
    dst_len: u64,
) -> Result<(), ContractError> {
    let need = offscreen_target_bytes(width, height, pixel_format_bytes(format))
        .map_err(with_copy_prefix)?;
    validate_buffer_range(dst_offset, need, dst_len).map_err(with_copy_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_check_pixel_format_bytes() {
        assert_eq!(pixel_format_bytes(PixelFormat::Rgba8Unorm), 4);
        assert_eq!(pixel_format_bytes(PixelFormat::Bgra8Unorm), 4);
        assert_eq!(pixel_format_bytes(PixelFormat::Rgba32Float), 16);
    }

    #[test]
    fn target_check_depth_format_bytes() {
        assert_eq!(depth_format_bytes(DepthFormat::Depth32Float), 4);
    }

    #[test]
    fn target_check_bytes_1280x720() {
        let bytes = |bpp| offscreen_target_bytes(1280, 720, bpp);
        assert_eq!(
            bytes(pixel_format_bytes(PixelFormat::Rgba8Unorm)),
            Ok(3_686_400)
        );
        assert_eq!(
            bytes(pixel_format_bytes(PixelFormat::Bgra8Unorm)),
            Ok(3_686_400)
        );
        assert_eq!(
            bytes(pixel_format_bytes(PixelFormat::Rgba32Float)),
            Ok(14_745_600)
        );
        assert_eq!(
            bytes(depth_format_bytes(DepthFormat::Depth32Float)),
            Ok(3_686_400)
        );
    }

    #[test]
    fn target_check_bytes_overflow_is_rejected() {
        for bpp in [4, 16] {
            match offscreen_target_bytes(u32::MAX, u32::MAX, bpp) {
                Err(ContractError::InvalidArgument(m)) => assert!(m.contains("overflows u64")),
                other => panic!("unexpected: {other:?}"),
            }
        }
    }

    #[test]
    fn target_check_bytes_max_without_overflow() {
        assert_eq!(
            offscreen_target_bytes(u32::MAX, u32::MAX, 1),
            Ok(u64::from(u32::MAX) * u64::from(u32::MAX))
        );
    }

    fn is_invalid(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn target_check_size_zero_is_rejected() {
        for (w, h) in [(0, 720), (1280, 0), (0, 0)] {
            assert!(is_invalid(validate_offscreen_target_size(w, h, 16384)));
        }
    }

    #[test]
    fn target_check_size_over_limit_is_rejected() {
        assert!(is_invalid(validate_offscreen_target_size(16385, 1, 16384)));
        assert!(is_invalid(validate_offscreen_target_size(1, 16385, 16384)));
    }

    #[test]
    fn target_check_size_limit_exact_passes() {
        assert_eq!(validate_offscreen_target_size(16384, 16384, 16384), Ok(()));
        assert_eq!(validate_offscreen_target_size(1280, 720, 16384), Ok(()));
    }

    #[test]
    fn target_check_size_u32_max_limit_passes() {
        assert_eq!(
            validate_offscreen_target_size(u32::MAX, u32::MAX, u32::MAX),
            Ok(())
        );
    }

    #[test]
    fn target_check_size_zero_limit_rejects() {
        assert!(is_invalid(validate_offscreen_target_size(1, 1, 0)));
    }

    const COPY_NEED: u64 = 3_686_400;

    fn copy_check(offset: u64, len: u64) -> Result<(), ContractError> {
        validate_copy_target_to_buffer(1280, 720, PixelFormat::Rgba8Unorm, offset, len)
    }

    fn invalid_message(r: Result<(), ContractError>) -> String {
        match r {
            Err(ContractError::InvalidArgument(m)) => m,
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn target_check_copy_exact_passes() {
        assert_eq!(copy_check(0, COPY_NEED), Ok(()));
        assert_eq!(copy_check(256, 256 + COPY_NEED), Ok(()));
        assert_eq!(copy_check(0, COPY_NEED + 1), Ok(()));
    }

    #[test]
    fn target_check_copy_short_is_rejected() {
        assert!(invalid_message(copy_check(0, COPY_NEED - 1)).contains("exceeds"));
        assert!(invalid_message(copy_check(256, COPY_NEED)).contains("exceeds"));
    }

    #[test]
    fn target_check_copy_product_overflow_is_rejected() {
        let r = validate_copy_target_to_buffer(
            u32::MAX,
            u32::MAX,
            PixelFormat::Rgba32Float,
            0,
            u64::MAX,
        );
        assert!(invalid_message(r).contains("overflows u64"));
    }

    #[test]
    fn target_check_copy_sum_overflow_is_rejected() {
        let m = invalid_message(copy_check(u64::MAX - COPY_NEED + 1, u64::MAX));
        assert!(m.contains("overflows u64"));
    }

    #[test]
    fn target_check_copy_message_has_prefix() {
        let all = [
            copy_check(0, COPY_NEED - 1),
            copy_check(u64::MAX - COPY_NEED + 1, u64::MAX),
            validate_copy_target_to_buffer(u32::MAX, u32::MAX, PixelFormat::Rgba8Unorm, 0, 0),
        ];
        for r in all {
            assert!(invalid_message(r).starts_with("copy_target_to_buffer: "));
        }
    }
}
