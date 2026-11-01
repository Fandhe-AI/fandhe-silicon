//! `graphics` の描画先からバッファへの写しの書き込み先の検査を、外の crate から境界の入力の全組で確かめる公開テスト（TASK-591。CMD-86 のポインタ）。
//! feature `graphics` の裏でだけ走る。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::graphics::{
    PixelFormat, offscreen_target_bytes, pixel_format_bytes, validate_copy_target_to_buffer,
};

/// 試験で回す 3 つの形式。
const FORMATS: [PixelFormat; 3] = [
    PixelFormat::Rgba8Unorm,
    PixelFormat::Bgra8Unorm,
    PixelFormat::Rgba32Float,
];

/// 試験で使う描画先の大きさ（幅、高さ）。
const SIZES: [(u32, u32); 2] = [(1280, 720), (1, 1)];

/// 0 でない `dst_offset`。
const K: u64 = 256;

/// 形式と大きさごとに、書き込みに要るバイト数のリテラルを返す。
fn need(f: PixelFormat, w: u32, h: u32) -> u64 {
    match (f, w, h) {
        (PixelFormat::Rgba8Unorm | PixelFormat::Bgra8Unorm, 1280, 720) => 3_686_400,
        (PixelFormat::Rgba32Float, 1280, 720) => 14_745_600,
        (PixelFormat::Rgba8Unorm | PixelFormat::Bgra8Unorm, 1, 1) => 4,
        (PixelFormat::Rgba32Float, 1, 1) => 16,
        _ => panic!("unexpected case: {f:?} {w}x{h}"),
    }
}

/// `InvalidArgument` の文言を取り出す。それ以外は失敗にする。
fn invalid_message(r: Result<(), ContractError>, ctx: &str) -> String {
    match r {
        Err(ContractError::InvalidArgument(m)) => m,
        other => panic!("{ctx}: unexpected {other:?}"),
    }
}

#[test]
fn exact_length_from_zero_offset_is_accepted() {
    for f in FORMATS {
        for (w, h) in SIZES {
            let n = need(f, w, h);
            assert_eq!(
                validate_copy_target_to_buffer(w, h, f, 0, n),
                Ok(()),
                "{f:?} {w}x{h} dst_offset=0 dst_len={n}"
            );
        }
    }
}

#[test]
fn exact_length_from_nonzero_offset_is_accepted() {
    for f in FORMATS {
        for (w, h) in SIZES {
            let n = need(f, w, h);
            for off in [K, 4096] {
                assert_eq!(
                    validate_copy_target_to_buffer(w, h, f, off, off + n),
                    Ok(()),
                    "{f:?} {w}x{h} dst_offset={off} dst_len={}",
                    off + n
                );
            }
        }
    }
}

#[test]
fn longer_destination_is_accepted() {
    for f in FORMATS {
        for (w, h) in SIZES {
            let n = need(f, w, h);
            for off in [0, K] {
                let len = off + n + 512;
                assert_eq!(
                    validate_copy_target_to_buffer(w, h, f, off, len),
                    Ok(()),
                    "{f:?} {w}x{h} dst_offset={off} dst_len={len}"
                );
            }
        }
    }
}

#[test]
fn one_byte_short_is_rejected() {
    for f in FORMATS {
        for (w, h) in SIZES {
            let n = need(f, w, h);
            for off in [0, K] {
                let len = off + n - 1;
                let ctx = format!("{f:?} {w}x{h} dst_offset={off} dst_len={len}");
                let m = invalid_message(validate_copy_target_to_buffer(w, h, f, off, len), &ctx);
                assert!(m.starts_with("copy_target_to_buffer: "), "{ctx}: {m}");
                assert!(m.contains("exceeds"), "{ctx}: {m}");
            }
        }
    }
}

#[test]
fn offset_past_end_is_rejected() {
    for f in FORMATS {
        for (w, h) in SIZES {
            let n = need(f, w, h);
            for off in [n + 1, n + K] {
                let ctx = format!("{f:?} {w}x{h} dst_offset={off} dst_len={n}");
                let m = invalid_message(validate_copy_target_to_buffer(w, h, f, off, n), &ctx);
                assert!(m.starts_with("copy_target_to_buffer: "), "{ctx}: {m}");
                assert!(m.contains("exceeds"), "{ctx}: {m}");
            }
        }
    }
}

#[test]
fn offset_plus_need_overflow_is_rejected() {
    for f in FORMATS {
        for (w, h) in SIZES {
            let n = need(f, w, h);
            for off in [u64::MAX - n + 1, u64::MAX] {
                let ctx = format!("{f:?} {w}x{h} dst_offset={off} dst_len=u64::MAX");
                let m =
                    invalid_message(validate_copy_target_to_buffer(w, h, f, off, u64::MAX), &ctx);
                assert!(m.starts_with("copy_target_to_buffer: "), "{ctx}: {m}");
                assert!(m.contains("overflows u64"), "{ctx}: {m}");
            }
        }
    }
}

#[test]
fn byte_count_overflow_is_rejected() {
    for f in FORMATS {
        let ctx = format!(
            "{f:?} {}x{} dst_offset=0 dst_len=u64::MAX",
            u32::MAX,
            u32::MAX
        );
        let m = invalid_message(
            validate_copy_target_to_buffer(u32::MAX, u32::MAX, f, 0, u64::MAX),
            &ctx,
        );
        assert!(m.starts_with("copy_target_to_buffer: "), "{ctx}: {m}");
        assert!(m.contains("overflows u64"), "{ctx}: {m}");
    }
}

#[test]
fn need_matches_offscreen_target_bytes() {
    for f in FORMATS {
        let n = offscreen_target_bytes(1280, 720, pixel_format_bytes(f)).unwrap();
        assert_eq!(n, need(f, 1280, 720), "{f:?}");
        assert_eq!(
            validate_copy_target_to_buffer(1280, 720, f, 0, n),
            Ok(()),
            "{f:?}"
        );
        assert!(
            matches!(
                validate_copy_target_to_buffer(1280, 720, f, 0, n - 1),
                Err(ContractError::InvalidArgument(_))
            ),
            "{f:?}"
        );
    }
}

#[test]
fn rejection_is_repeatable() {
    for f in FORMATS {
        let n = need(f, 1280, 720);
        let inputs: [(u32, u32, u64, u64); 4] = [
            (1280, 720, 0, n - 1),
            (1280, 720, n + 1, n),
            (1280, 720, u64::MAX - n + 1, u64::MAX),
            (u32::MAX, u32::MAX, 0, u64::MAX),
        ];
        for (w, h, off, len) in inputs {
            let a = validate_copy_target_to_buffer(w, h, f, off, len);
            let b = validate_copy_target_to_buffer(w, h, f, off, len);
            assert!(a.is_err(), "{f:?} {w}x{h} dst_offset={off} dst_len={len}");
            assert_eq!(a, b, "{f:?} {w}x{h} dst_offset={off} dst_len={len}");
        }
    }
}
