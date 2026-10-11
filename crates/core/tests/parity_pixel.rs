//! 画像の入力の受け口・比べる関数・閾値の設定の振る舞いの確認（spec TASK-26 の 3/6・4/6・5/6）。
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

// ---- 4/6: 比べる関数と閾値の設定 ----

use fandhe_silicon_core::parity::pixel::{
    PixelCompareError, T3, compare_pixels, compare_pixels_exact,
};

/// 全要素が `v` の w×h 画像の列。
fn solid(w: usize, h: usize, v: u8) -> Vec<u8> {
    vec![v; w * h * CHANNELS_PER_PIXEL]
}

fn view(data: &[u8], w: usize, h: usize) -> PixelView<'_> {
    PixelView::new(data, w, h).unwrap()
}

#[test]
fn t3_setting_is_readable() {
    assert_eq!(T3.near_channel_diff(), 1);
    assert_eq!(T3.max_channel_diff(), 2);
    assert_eq!(T3.min_near_numerator(), 999);
    assert_eq!(T3.min_near_denominator(), 1000);
}

#[test]
fn identical_images_match_on_both_comparisons() {
    let d = solid(3, 2, 7);
    let e = compare_pixels_exact(view(&d, 3, 2), view(&d, 3, 2)).unwrap();
    assert!(e.is_match());
    assert_eq!(
        (e.compared(), e.mismatched(), e.first_mismatch()),
        (6, 0, None)
    );
    let c = compare_pixels(view(&d, 3, 2), view(&d, 3, 2), T3).unwrap();
    assert!(c.is_match());
    assert_eq!((c.compared(), c.near(), c.max_channel_diff()), (6, 6, 0));
    assert_eq!(c.first_beyond_near(), None);
}

#[test]
fn exact_comparison_counts_mismatched_pixels() {
    let a = solid(2, 2, 10);
    let mut b = a.clone();
    // 画素 1 の 1 要素だけ最小の差。
    b[CHANNELS_PER_PIXEL + 2] += 1;
    // 画素 3 は複数チャネルが違っても 1 画素と数える。
    b[3 * CHANNELS_PER_PIXEL] += 5;
    b[3 * CHANNELS_PER_PIXEL + 1] += 5;
    let e = compare_pixels_exact(view(&a, 2, 2), view(&b, 2, 2)).unwrap();
    assert!(!e.is_match());
    assert_eq!(
        (e.compared(), e.mismatched(), e.first_mismatch()),
        (4, 2, Some(1))
    );
}

#[test]
fn size_mismatch_is_an_error_even_with_equal_length() {
    let d = solid(2, 3, 0);
    let (r, a) = (view(&d, 2, 3), view(&d, 3, 2));
    let expected = PixelCompareError::SizeMismatch {
        reference: (2, 3),
        actual: (3, 2),
    };
    assert_eq!(compare_pixels_exact(r, a).unwrap_err(), expected);
    assert_eq!(compare_pixels(r, a, T3).unwrap_err(), expected);
}

#[test]
fn pixel_difference_is_the_largest_channel_difference() {
    let a = solid(1, 1, 100);
    for ch in 0..CHANNELS_PER_PIXEL {
        let mut b = a.clone();
        b[ch] = 100 + 2;
        let c = compare_pixels(view(&a, 1, 1), view(&b, 1, 1), T3).unwrap();
        assert_eq!(c.max_channel_diff(), 2, "channel {ch}");
    }
    // 差の最大が画素の差になる（複数チャネルの差は足さない）。
    let mut b = a.clone();
    b[0] = 101;
    b[3] = 103;
    let c = compare_pixels(view(&a, 1, 1), view(&b, 1, 1), T3).unwrap();
    assert_eq!(c.max_channel_diff(), 3);
}

#[test]
fn readable_bounds_are_the_ones_applied() {
    let near = T3.near_channel_diff();
    let a = solid(1, 1, 100);
    let mut inside = a.clone();
    inside[0] = 100 + near;
    let mut outside = a.clone();
    outside[0] = 100 + near + 1;
    let c_in = compare_pixels(view(&a, 1, 1), view(&inside, 1, 1), T3).unwrap();
    assert_eq!((c_in.near(), c_in.first_beyond_near()), (1, None));
    let c_out = compare_pixels(view(&a, 1, 1), view(&outside, 1, 1), T3).unwrap();
    assert_eq!((c_out.near(), c_out.first_beyond_near()), (0, Some(0)));

    // 全体の上限: ちょうどなら満たし、1 つ上は満たさない（割合は十分な大きさの画像で満たす）。
    let n = T3.min_near_denominator() as usize;
    let base = solid(n, 1, 100);
    for (diff, expect) in [
        (T3.max_channel_diff(), true),
        (T3.max_channel_diff() + 1, false),
    ] {
        let mut b = base.clone();
        b[0] = 100 + diff;
        let c = compare_pixels(view(&base, n, 1), view(&b, n, 1), T3).unwrap();
        assert_eq!(c.is_match(), expect, "diff {diff}");
    }
}

#[test]
fn ratio_boundary_is_inclusive() {
    let den = T3.min_near_denominator() as usize;
    let num = T3.min_near_numerator() as usize;
    let base = solid(den, 1, 100);
    // 「内」がちょうど下限になるよう、(den - num) 画素を内側の上限の外（全体の上限の内）にする。
    let far = T3.near_channel_diff() + 1;
    assert!(far <= T3.max_channel_diff());
    let mut b = base.clone();
    for p in 0..(den - num) {
        b[p * CHANNELS_PER_PIXEL] = 100 + far;
    }
    let c = compare_pixels(view(&base, den, 1), view(&b, den, 1), T3).unwrap();
    assert_eq!(c.near(), num);
    assert!(c.is_match());
    // もう 1 画素外すと下限を割る。
    b[(den - num) * CHANNELS_PER_PIXEL] = 100 + far;
    let c = compare_pixels(view(&base, den, 1), view(&b, den, 1), T3).unwrap();
    assert_eq!(c.near(), num - 1);
    assert!(!c.is_match());
}

#[test]
fn max_difference_alone_fails_the_match() {
    let n = (T3.min_near_denominator() * 10) as usize;
    let base = solid(n, 1, 0);
    let mut b = base.clone();
    // 割合は十分に満たすが、1 画素が全体の上限を超える。
    b[0] = T3.max_channel_diff() + 1;
    let c = compare_pixels(view(&base, n, 1), view(&b, n, 1), T3).unwrap();
    assert!(
        c.near() as u128 * u128::from(T3.min_near_denominator())
            >= c.compared() as u128 * u128::from(T3.min_near_numerator())
    );
    assert!(!c.is_match());
}

#[test]
fn comparison_is_symmetric() {
    let a: Vec<u8> = (0..(4 * CHANNELS_PER_PIXEL) as u8).collect();
    let b: Vec<u8> = a.iter().map(|v| v.wrapping_add(3)).collect();
    let ab = compare_pixels(view(&a, 2, 2), view(&b, 2, 2), T3).unwrap();
    let ba = compare_pixels(view(&b, 2, 2), view(&a, 2, 2), T3).unwrap();
    assert_eq!(ab, ba);
}

#[test]
fn extreme_values_do_not_overflow() {
    let lo = solid(2, 2, 0);
    let hi = solid(2, 2, 255);
    let c = compare_pixels(view(&lo, 2, 2), view(&hi, 2, 2), T3).unwrap();
    assert_eq!(c.max_channel_diff(), 255);
    assert!(!c.is_match());
}

#[test]
fn compare_error_display_is_non_empty_ascii() {
    let s = PixelCompareError::SizeMismatch {
        reference: (2, 3),
        actual: (3, 2),
    }
    .to_string();
    assert!(!s.is_empty() && s.is_ascii());
}

// ---- 5/6: 境界の補強 ----

#[test]
fn exact_comparison_sees_an_alpha_only_difference() {
    let a = solid(2, 1, 50);
    let mut b = a.clone();
    b[CHANNELS_PER_PIXEL + 3] += 1;
    let e = compare_pixels_exact(view(&a, 2, 1), view(&b, 2, 1)).unwrap();
    assert!(!e.is_match());
    assert_eq!((e.mismatched(), e.first_mismatch()), (1, Some(1)));
}

#[test]
fn exact_comparison_is_symmetric() {
    let a: Vec<u8> = (0..(4 * CHANNELS_PER_PIXEL) as u8).collect();
    let mut b = a.clone();
    b[5] = b[5].wrapping_add(9);
    let ab = compare_pixels_exact(view(&a, 2, 2), view(&b, 2, 2)).unwrap();
    let ba = compare_pixels_exact(view(&b, 2, 2), view(&a, 2, 2)).unwrap();
    assert_eq!(ab, ba);
}

#[test]
fn first_index_is_the_earliest_pixel_in_row_major_order() {
    let near = T3.near_channel_diff();
    let a = solid(3, 2, 100);
    let mut b = a.clone();
    // 添字 1 は内側の上限ちょうど、添字 4・5（2 行目）は上限の外。
    b[CHANNELS_PER_PIXEL] += near;
    b[4 * CHANNELS_PER_PIXEL] += near + 1;
    b[5 * CHANNELS_PER_PIXEL] += near + 1;
    let c = compare_pixels(view(&a, 3, 2), view(&b, 3, 2), T3).unwrap();
    assert_eq!((c.first_beyond_near(), c.near()), (Some(4), 4));
    let e = compare_pixels_exact(view(&a, 3, 2), view(&b, 3, 2)).unwrap();
    assert_eq!((e.first_mismatch(), e.mismatched()), (Some(1), 3));
}

#[test]
fn difference_within_near_bound_matches_with_tolerance_but_not_exactly() {
    let n = T3.min_near_denominator() as usize;
    let a = solid(n, 1, 100);
    let mut b = a.clone();
    b[0] += T3.near_channel_diff();
    let tol = compare_pixels(view(&a, n, 1), view(&b, n, 1), T3).unwrap();
    assert!(tol.is_match());
    let exact = compare_pixels_exact(view(&a, n, 1), view(&b, n, 1)).unwrap();
    assert!(!exact.is_match());
}

#[test]
fn ratio_is_applied_to_images_smaller_than_the_denominator() {
    assert!(T3.near_channel_diff() < T3.max_channel_diff());
    let a = solid(1, 1, 100);
    let mut b = a.clone();
    b[0] += T3.near_channel_diff() + 1;
    let c = compare_pixels(view(&a, 1, 1), view(&b, 1, 1), T3).unwrap();
    assert_eq!(c.near(), 0);
    assert!(!c.is_match());
}
