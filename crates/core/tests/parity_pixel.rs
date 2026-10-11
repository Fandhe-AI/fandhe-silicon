//! 画像の入力の受け口の振る舞いの確認（spec TASK-26 の 3/6）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::pixel::{CHANNELS_PER_PIXEL, PixelInputError, PixelView};

#[test]
fn valid_input_exposes_what_was_passed() {
    let data: Vec<u8> = (0..(2 * 3 * CHANNELS_PER_PIXEL) as u8).collect();
    let v = PixelView::new(&data, 2, 3).unwrap();
    assert_eq!(v.width(), 2);
    assert_eq!(v.height(), 3);
    assert_eq!(v.data(), data.as_slice());
}

#[test]
fn channel_count_is_readable() {
    let one = vec![0u8; CHANNELS_PER_PIXEL];
    assert!(PixelView::new(&one, 1, 1).is_ok());
    let short = vec![0u8; CHANNELS_PER_PIXEL - 1];
    assert!(PixelView::new(&short, 1, 1).is_err());
}

#[test]
fn length_mismatch_reports_expected_and_actual() {
    let expected = 2 * 2 * CHANNELS_PER_PIXEL;
    for actual in [expected - 1, expected + 1, 0] {
        let data = vec![0u8; actual];
        let err = PixelView::new(&data, 2, 2).unwrap_err();
        assert_eq!(err, PixelInputError::LengthMismatch { expected, actual });
    }
}

#[test]
fn zero_width_or_height_is_a_zero_size_error() {
    let data = vec![0u8; 16];
    for (w, h) in [(0, 2), (2, 0), (0, 0)] {
        assert_eq!(
            PixelView::new(&data, w, h).unwrap_err(),
            PixelInputError::ZeroSize {
                width: w,
                height: h
            }
        );
    }
}

#[test]
fn every_size_product_is_checked_for_overflow() {
    // 幅 × 高さで溢れる。
    assert_eq!(
        PixelView::new(&[], usize::MAX, 2).unwrap_err(),
        PixelInputError::SizeOverflow
    );
    // 幅 × 高さは収まるが、チャネル数を掛けると溢れる。
    assert_eq!(
        PixelView::new(&[], usize::MAX / CHANNELS_PER_PIXEL + 1, 1).unwrap_err(),
        PixelInputError::SizeOverflow
    );
}

#[test]
fn error_display_is_non_empty_ascii() {
    let errors = [
        PixelInputError::ZeroSize {
            width: 0,
            height: 1,
        },
        PixelInputError::SizeOverflow,
        PixelInputError::LengthMismatch {
            expected: 4,
            actual: 3,
        },
    ];
    for e in errors {
        let s = e.to_string();
        assert!(!s.is_empty() && s.is_ascii());
    }
}
