//! contract の feature `test-support` だけで、core の画素の一致を確かめる道具に届くことの確認（spec TASK-26 の 5/6）。
//! `make build-profiles` の `test-support` を含む各組で、背後実装の feature に依らず同じ結果になることも確かめる。
//! 置き場・feature の説明は docs/pixel-parity-tool.md の「所在」と「振る舞い」。

use fandhe_silicon_core::parity::pixel::{
    CHANNELS_PER_PIXEL, PixelCompareError, PixelInputError, PixelView, T3, compare_pixels,
    compare_pixels_exact,
};

fn solid(w: usize, h: usize, v: u8) -> Vec<u8> {
    vec![v; w * h * CHANNELS_PER_PIXEL]
}

#[test]
fn contract_test_support_enables_core_pixel_tool() {
    let d = solid(1, 1, 9);
    let v = PixelView::new(&d, 1, 1).unwrap();
    assert!(compare_pixels_exact(v, v).unwrap().is_match());
}

#[test]
fn pixel_threshold_setting_is_readable_from_outside_core() {
    assert!(T3.min_near_denominator() > 0);
    assert!(T3.min_near_numerator() <= T3.min_near_denominator());
    assert!(T3.near_channel_diff() <= T3.max_channel_diff());
}

#[test]
fn pixel_comparisons_give_fixed_results_under_any_backend_feature_set() {
    assert!(T3.near_channel_diff() > 0);
    let a = solid(2, 2, 10);
    let mut near = a.clone();
    // 画素 1 の 1 要素だけ内側の上限ぶんずらす。
    near[CHANNELS_PER_PIXEL + 1] += T3.near_channel_diff();
    let ra = PixelView::new(&a, 2, 2).unwrap();
    let rb = PixelView::new(&near, 2, 2).unwrap();
    let e = compare_pixels_exact(ra, rb).unwrap();
    assert!(!e.is_match());
    assert_eq!(
        (e.compared(), e.mismatched(), e.first_mismatch()),
        (4, 1, Some(1))
    );
    let c = compare_pixels(ra, rb, T3).unwrap();
    assert!(c.is_match());
    assert_eq!(
        (
            c.compared(),
            c.near(),
            c.max_channel_diff(),
            c.first_beyond_near()
        ),
        (4, 4, T3.near_channel_diff(), None)
    );

    let mut far = a.clone();
    far[CHANNELS_PER_PIXEL + 1] += T3.max_channel_diff() + 1;
    let rf = PixelView::new(&far, 2, 2).unwrap();
    let c = compare_pixels(ra, rf, T3).unwrap();
    assert!(!c.is_match());
    assert_eq!((c.near(), c.first_beyond_near()), (3, Some(1)));
}

#[test]
fn pixel_input_and_size_errors_are_reachable_from_outside_core() {
    assert_eq!(
        PixelView::new(&[], 0, 1).unwrap_err(),
        PixelInputError::ZeroSize {
            width: 0,
            height: 1
        }
    );
    let d = solid(2, 3, 0);
    let (r, a) = (
        PixelView::new(&d, 2, 3).unwrap(),
        PixelView::new(&d, 3, 2).unwrap(),
    );
    let expected = PixelCompareError::SizeMismatch {
        reference: (2, 3),
        actual: (3, 2),
    };
    assert_eq!(compare_pixels_exact(r, a).unwrap_err(), expected);
    assert_eq!(compare_pixels(r, a, T3).unwrap_err(), expected);
}
