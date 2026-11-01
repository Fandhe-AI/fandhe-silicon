//! 描画先の形式の 1 画素のバイト数と、描画先のバイト数を求める純粋関数（TASK-374。TEX-8・TEX-9・TEX-10 のポインタ）。
//!
//! `graphics` モジュールの一部で、公開のパスは `fandhe_silicon_core::graphics::…`。
//! 背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。

use super::{DepthFormat, PixelFormat};
use crate::error::ContractError;

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
}
