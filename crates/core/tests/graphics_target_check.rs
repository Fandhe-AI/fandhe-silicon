//! `graphics` の描画先の大きさの検査とバイト数の関数を、外の crate から境界の入力で確かめる公開テスト（TASK-374。TEX-8・TEX-9・TEX-10 のポインタ）。
//! feature `graphics` の裏でだけ走る。

use fandhe_silicon_core::capability::LimitsTable;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::graphics::{
    DepthFormat, PixelFormat, depth_format_bytes, offscreen_target_bytes, pixel_format_bytes,
    validate_offscreen_target_size,
};

/// 試験の入力として使う上限の数。
const LIMIT: u32 = 16384;

fn is_invalid<T>(r: &Result<T, ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

#[test]
fn zero_width_or_height_is_rejected() {
    for (w, h) in [(0, 720), (1280, 0), (0, 0)] {
        assert!(is_invalid(&validate_offscreen_target_size(w, h, LIMIT)));
    }
}

#[test]
fn limit_exact_is_accepted() {
    for (w, h) in [(LIMIT, LIMIT), (LIMIT, 1), (1, LIMIT)] {
        assert_eq!(validate_offscreen_target_size(w, h, LIMIT), Ok(()));
    }
}

#[test]
fn limit_plus_one_is_rejected() {
    for (w, h) in [(LIMIT + 1, 1), (1, LIMIT + 1), (LIMIT + 1, LIMIT + 1)] {
        assert!(is_invalid(&validate_offscreen_target_size(w, h, LIMIT)));
    }
}

#[test]
fn limit_from_limits_table() {
    let t = LimitsTable {
        max_buffer_bytes: 1 << 20,
        max_dispatch_dims: [64, 64, 64],
        max_texture_size: 8,
    };
    assert_eq!(
        validate_offscreen_target_size(8, 8, t.max_texture_size),
        Ok(())
    );
    assert!(is_invalid(&validate_offscreen_target_size(
        9,
        8,
        t.max_texture_size
    )));
    assert!(is_invalid(&validate_offscreen_target_size(
        8,
        9,
        t.max_texture_size
    )));
}

#[test]
fn rgba32float_bytes_are_16_per_pixel() {
    let bpp = pixel_format_bytes(PixelFormat::Rgba32Float);
    assert_eq!(bpp, 16);
    assert_eq!(offscreen_target_bytes(1280, 720, bpp), Ok(14_745_600_u64));
    assert_eq!(
        offscreen_target_bytes(LIMIT, LIMIT, bpp),
        Ok(4_294_967_296_u64)
    );
    assert_eq!(
        offscreen_target_bytes(LIMIT, LIMIT, bpp),
        Ok(16 * u64::from(LIMIT) * u64::from(LIMIT))
    );
}

#[test]
fn color_and_depth_bytes() {
    assert_eq!(pixel_format_bytes(PixelFormat::Rgba8Unorm), 4);
    assert_eq!(pixel_format_bytes(PixelFormat::Bgra8Unorm), 4);
    assert_eq!(depth_format_bytes(DepthFormat::Depth32Float), 4);
    for bpp in [
        pixel_format_bytes(PixelFormat::Rgba8Unorm),
        pixel_format_bytes(PixelFormat::Bgra8Unorm),
        depth_format_bytes(DepthFormat::Depth32Float),
    ] {
        assert_eq!(offscreen_target_bytes(1280, 720, bpp), Ok(3_686_400_u64));
    }
}

#[test]
fn byte_count_overflow_is_rejected() {
    for bpp in [16, 4] {
        assert!(is_invalid(&offscreen_target_bytes(u32::MAX, u32::MAX, bpp)));
    }
}

#[test]
fn rejection_is_repeatable() {
    let size_cases = [
        (0, 720),
        (1280, 0),
        (0, 0),
        (LIMIT + 1, 1),
        (LIMIT, LIMIT),
        (1, LIMIT),
    ];
    for (w, h) in size_cases {
        assert_eq!(
            validate_offscreen_target_size(w, h, LIMIT),
            validate_offscreen_target_size(w, h, LIMIT)
        );
    }
    for (w, h, bpp) in [
        (u32::MAX, u32::MAX, 16),
        (u32::MAX, u32::MAX, 4),
        (1280, 720, 16),
    ] {
        assert_eq!(
            offscreen_target_bytes(w, h, bpp),
            offscreen_target_bytes(w, h, bpp)
        );
    }
}

#[test]
fn rejection_is_invalid_argument_only() {
    for (w, h) in [(0, 720), (1280, 0), (0, 0), (LIMIT + 1, 1), (1, LIMIT + 1)] {
        assert!(is_invalid(&validate_offscreen_target_size(w, h, LIMIT)));
    }
    for bpp in [16, 4] {
        assert!(is_invalid(&offscreen_target_bytes(u32::MAX, u32::MAX, bpp)));
    }
}
