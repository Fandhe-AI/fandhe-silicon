//! 代わりの実行の比べ方の順序の前段とビット同一の条件・binary16 の順序の差の条件・f32 の和の相対の条件の振る舞いの確認（spec TASK-928 の 2/8・3/8・4/8）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。入力はこの場で作る。

use fandhe_silicon_core::parity::binary16::binary16_bits_to_f32;
use fandhe_silicon_core::parity::fallback_compare::{
    BINARY16_MAX_ORDER_DISTANCE, FallbackVerdict, OrderVerdict, SUM_RELATIVE_BOUND,
    binary16_order_distance, compare_bits_f32, compare_bits_u32, compare_order_distance_binary16,
    compare_relative_sum_f32, count_bit_identical_binary16, judge_bits_f32, judge_bits_u32,
    judge_order_binary16, judge_order_distance_binary16, judge_order_f32, judge_relative_sum_f32,
    relative_sum_margin,
};
use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, judge_element_with_special_values,
};
use fandhe_silicon_core::parity::tolerance::{CompareError, T1};

fn nans() -> [f32; 3] {
    [f32::NAN, -f32::NAN, f32::from_bits(f32::NAN.to_bits() | 1)]
}

#[test]
fn order_nan_pairs_are_both_nan() {
    for r in nans() {
        for a in nans() {
            assert_eq!(judge_order_f32(r, a), OrderVerdict::BothNan);
        }
    }
}

#[test]
fn order_single_nan_is_a_mismatch() {
    for x in [0.0, -0.0, 1.0, f32::MAX, f32::INFINITY, f32::NEG_INFINITY] {
        for n in nans() {
            assert_eq!(judge_order_f32(n, x), OrderVerdict::NonFiniteMismatch);
            assert_eq!(judge_order_f32(x, n), OrderVerdict::NonFiniteMismatch);
        }
    }
}

#[test]
fn order_infinity_requires_the_same_sign() {
    let (p, n) = (f32::INFINITY, f32::NEG_INFINITY);
    assert_eq!(judge_order_f32(p, p), OrderVerdict::SameInfinity);
    assert_eq!(judge_order_f32(n, n), OrderVerdict::SameInfinity);
    for (r, a) in [
        (p, n),
        (n, p),
        (p, f32::MAX),
        (f32::MAX, p),
        (n, 0.0),
        (0.0, n),
    ] {
        assert_eq!(judge_order_f32(r, a), OrderVerdict::NonFiniteMismatch);
    }
}

#[test]
fn order_signed_zeros_are_both_zero() {
    for r in [0.0f32, -0.0] {
        for a in [0.0f32, -0.0] {
            assert_eq!(judge_order_f32(r, a), OrderVerdict::BothZero);
        }
    }
    let tiny = f32::from_bits(1);
    assert_eq!(judge_order_f32(0.0, tiny), OrderVerdict::Numeric);
    assert_eq!(judge_order_f32(tiny, -0.0), OrderVerdict::Numeric);
}

#[test]
fn order_finite_non_zero_pairs_go_to_numeric() {
    for (r, a) in [(1.0, 1.0), (1.0, 2.0), (-1.0, 1.0), (f32::MAX, f32::MIN)] {
        let v = judge_order_f32(r, a);
        assert_eq!(v, OrderVerdict::Numeric);
        assert!(!v.is_decided());
        assert!(!v.is_match());
    }
    assert!(OrderVerdict::BothZero.is_decided() && OrderVerdict::BothZero.is_match());
    assert!(OrderVerdict::NonFiniteMismatch.is_decided());
    assert!(!OrderVerdict::NonFiniteMismatch.is_match());
}

#[test]
fn order_agrees_with_special_values_on_non_finite() {
    let grid = [
        f32::NAN,
        -f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        0.0,
        -0.0,
        1.0,
        -1.0,
        f32::from_bits(1),
        f32::MAX,
    ];
    for r in grid {
        for a in grid {
            let expected = match judge_element_with_special_values(r, a, T1) {
                SpecialVerdict::BothNan => Some(OrderVerdict::BothNan),
                SpecialVerdict::SameInfinity => Some(OrderVerdict::SameInfinity),
                SpecialVerdict::NonFiniteMismatch => Some(OrderVerdict::NonFiniteMismatch),
                _ => None,
            };
            if let Some(e) = expected {
                assert_eq!(judge_order_f32(r, a), e);
            } else {
                assert!(matches!(
                    judge_order_f32(r, a),
                    OrderVerdict::BothZero | OrderVerdict::Numeric
                ));
            }
        }
    }
}

#[test]
fn binary16_overflow_neighbour_is_decided_before_numeric() {
    for (max, inf) in [(0x7BFFu16, 0x7C00u16), (0xFBFF, 0xFC00)] {
        assert_eq!(
            judge_order_binary16(max, inf),
            OrderVerdict::NonFiniteMismatch
        );
        assert_eq!(
            judge_order_binary16(inf, max),
            OrderVerdict::NonFiniteMismatch
        );
    }
}

#[test]
fn binary16_order_kinds() {
    // 指数がすべて 1 で仮数が 0 でない全ビット列は NaN。
    for sign in [0u16, 0x8000] {
        for frac in 1..0x400u16 {
            let a = sign | 0x7C00 | frac;
            assert_eq!(judge_order_binary16(a, 0x7E00), OrderVerdict::BothNan);
        }
    }
    assert_eq!(judge_order_binary16(0x0000, 0x8000), OrderVerdict::BothZero);
    assert_eq!(judge_order_binary16(0x8000, 0x0000), OrderVerdict::BothZero);
    assert_eq!(judge_order_binary16(0x0001, 0x0000), OrderVerdict::Numeric);
    assert_eq!(
        judge_order_binary16(0x7C00, 0x7C00),
        OrderVerdict::SameInfinity
    );
}

#[test]
fn binary16_same_bits_never_mismatch_in_order() {
    for b in 0..=u16::MAX {
        let v = judge_order_binary16(b, b);
        assert!(v.is_match() || v == OrderVerdict::Numeric, "bits {b:#06x}");
        let wide = binary16_bits_to_f32(b);
        assert_eq!(v, judge_order_f32(wide, wide));
    }
}

#[test]
fn bits_u32_identity() {
    for x in [0u32, 1, 0x8000_0000, u32::MAX] {
        assert_eq!(judge_bits_u32(x, x), FallbackVerdict::BitIdentical);
    }
    assert_eq!(judge_bits_u32(0, 1), FallbackVerdict::BitsDiffer);
    assert_eq!(judge_bits_u32(u32::MAX, 0), FallbackVerdict::BitsDiffer);
    assert!(FallbackVerdict::BitIdentical.is_match());
    assert!(!FallbackVerdict::BitsDiffer.is_match());
    assert!(!FallbackVerdict::NonFiniteMismatch.is_match());
}

#[test]
fn bits_f32_after_order() {
    assert_eq!(judge_bits_f32(-0.0, 0.0), FallbackVerdict::BothZero);
    assert_eq!(judge_bits_f32(0.0, -0.0), FallbackVerdict::BothZero);
    assert_eq!(judge_bits_f32(2.0, 2.0), FallbackVerdict::BitIdentical);
    let next = f32::from_bits(1.0f32.to_bits() + 1);
    assert_eq!(judge_bits_f32(1.0, next), FallbackVerdict::BitsDiffer);
    assert_eq!(
        judge_bits_f32(f32::NAN, -f32::NAN),
        FallbackVerdict::BothNan
    );
    assert_eq!(
        judge_bits_f32(f32::INFINITY, f32::INFINITY),
        FallbackVerdict::SameInfinity
    );
    assert_eq!(
        judge_bits_f32(f32::INFINITY, f32::NEG_INFINITY),
        FallbackVerdict::NonFiniteMismatch
    );
}

#[test]
fn compare_bits_errors_and_summary() {
    assert_eq!(
        compare_bits_u32(&[], &[1]),
        Err(CompareError::LengthMismatch {
            reference: 0,
            actual: 1
        })
    );
    assert_eq!(compare_bits_u32(&[], &[]), Err(CompareError::EmptyInput));
    assert_eq!(compare_bits_f32(&[], &[]), Err(CompareError::EmptyInput));
    assert_eq!(
        compare_bits_f32(&[1.0], &[1.0, 2.0]),
        Err(CompareError::LengthMismatch {
            reference: 1,
            actual: 2
        })
    );

    let u = compare_bits_u32(&[1, 2, 3, 4], &[1, 9, 3, 8]).unwrap();
    assert_eq!(
        (u.compared(), u.mismatched(), u.first_mismatch()),
        (4, 2, Some(1))
    );
    assert!(!u.is_match());

    let r = [1.0, -0.0, f32::NAN, 5.0];
    let a = [1.0, 0.0, f32::NAN, 5.0];
    let ok = compare_bits_f32(&r, &a).unwrap();
    assert!(ok.is_match());
    assert_eq!(ok.first_mismatch(), None);

    let a2 = [1.0, 0.0, 1.0, 6.0];
    let bad = compare_bits_f32(&r, &a2).unwrap();
    assert_eq!(
        (bad.compared(), bad.mismatched(), bad.first_mismatch()),
        (4, 2, Some(2))
    );
}

#[test]
fn order_distance_mapping() {
    assert_eq!(BINARY16_MAX_ORDER_DISTANCE, 1);
    for (r, a, d) in [
        (0x0000u16, 0x8000u16, 0u32),
        (0x0001, 0x0000, 1),
        (0x0001, 0x8001, 2),
        (0x3C00, 0x3C01, 1),
        (0xBC00, 0xBC01, 1),
        (0x7BFF, 0xFBFF, 0xF7FE),
    ] {
        assert_eq!(binary16_order_distance(r, a), d);
        assert_eq!(binary16_order_distance(a, r), d);
    }
}

#[test]
fn order_distance_same_bits_all_patterns() {
    for b in 0..=u16::MAX {
        let expected = match judge_order_binary16(b, b) {
            OrderVerdict::BothNan => FallbackVerdict::BothNan,
            OrderVerdict::SameInfinity => FallbackVerdict::SameInfinity,
            OrderVerdict::BothZero => FallbackVerdict::BothZero,
            OrderVerdict::Numeric => FallbackVerdict::WithinOrderDistance,
            OrderVerdict::NonFiniteMismatch => panic!("same bits must not mismatch: {b:#06x}"),
        };
        let got = judge_order_distance_binary16(b, b);
        assert_eq!(got, expected, "bits {b:#06x}");
        assert!(got.is_match());
    }
}

#[test]
fn order_distance_neighbours() {
    for (r, a) in [(0x3C00u16, 0x3C01u16), (0xBC00, 0xBC01), (0x3C01, 0x3C00)] {
        assert_eq!(
            judge_order_distance_binary16(r, a),
            FallbackVerdict::WithinOrderDistance
        );
    }
    for (r, a) in [(0x3C00u16, 0x3C02u16), (0xBC00, 0xBC02), (0x3C02, 0x3C00)] {
        let v = judge_order_distance_binary16(r, a);
        assert_eq!(v, FallbackVerdict::OrderDistanceExceeded);
        assert!(!v.is_match());
    }
}

#[test]
fn order_distance_subnormals() {
    for (r, a) in [(0x0001u16, 0x0002u16), (0x03FF, 0x0400), (0x8001, 0x8002)] {
        assert_eq!(
            judge_order_distance_binary16(r, a),
            FallbackVerdict::WithinOrderDistance
        );
    }
    assert_eq!(
        judge_order_distance_binary16(0x0000, 0x0002),
        FallbackVerdict::OrderDistanceExceeded
    );
}

#[test]
fn order_distance_across_zero() {
    for (r, a) in [(0x0000u16, 0x8001u16), (0x8000, 0x0001)] {
        assert_eq!(
            judge_order_distance_binary16(r, a),
            FallbackVerdict::WithinOrderDistance
        );
    }
    assert_eq!(
        judge_order_distance_binary16(0x0001, 0x8001),
        FallbackVerdict::OrderDistanceExceeded
    );
}

#[test]
fn order_distance_overflow_neighbour_is_not_within() {
    assert_eq!(binary16_order_distance(0x7BFF, 0x7C00), 1);
    for (r, a) in [
        (0x7BFFu16, 0x7C00u16),
        (0x7C00, 0x7BFF),
        (0xFBFF, 0xFC00),
        (0xFC00, 0xFBFF),
    ] {
        let v = judge_order_distance_binary16(r, a);
        assert_eq!(v, FallbackVerdict::NonFiniteMismatch);
        assert!(!v.is_match());
    }
}

#[test]
fn order_distance_non_finite_follows_order() {
    let grid = [
        0x0000u16, 0x8000, 0x0001, 0x8001, 0x03FF, 0x3C00, 0x3C01, 0x7BFF, 0xFBFF, 0x7C00, 0xFC00,
        0x7C01, 0xFE00, 0x7E00,
    ];
    for r in grid {
        for a in grid {
            let got = judge_order_distance_binary16(r, a);
            match judge_order_binary16(r, a) {
                OrderVerdict::BothNan => assert_eq!(got, FallbackVerdict::BothNan),
                OrderVerdict::SameInfinity => assert_eq!(got, FallbackVerdict::SameInfinity),
                OrderVerdict::NonFiniteMismatch => {
                    assert_eq!(got, FallbackVerdict::NonFiniteMismatch)
                }
                OrderVerdict::BothZero => assert_eq!(got, FallbackVerdict::BothZero),
                OrderVerdict::Numeric => assert!(matches!(
                    got,
                    FallbackVerdict::WithinOrderDistance | FallbackVerdict::OrderDistanceExceeded
                )),
            }
        }
    }
}

#[test]
fn compare_order_distance_errors_and_summary() {
    assert_eq!(
        compare_order_distance_binary16(&[], &[1]),
        Err(CompareError::LengthMismatch {
            reference: 0,
            actual: 1
        })
    );
    assert_eq!(
        compare_order_distance_binary16(&[], &[]),
        Err(CompareError::EmptyInput)
    );
    let r = [0x3C00u16, 0x3C00, 0x7BFF, 0x0000, 0x7E00];
    let a = [0x3C01u16, 0x3C02, 0x7C00, 0x8000, 0x7C01];
    let c = compare_order_distance_binary16(&r, &a).unwrap();
    assert_eq!(c.compared(), 5);
    assert_eq!(c.mismatched(), 2);
    assert_eq!(c.first_mismatch(), Some(1));
    assert!(!c.is_match());
    let ok = compare_order_distance_binary16(&[0x3C00, 0x0001], &[0x3C01, 0x8000]).unwrap();
    assert!(ok.is_match());
    assert_eq!(ok.first_mismatch(), None);
}

#[test]
fn count_bit_identical_records_only() {
    let r = [0x3C00u16, 0x3C00, 0x0000, 0x7E00];
    let a = [0x3C00u16, 0x3C01, 0x8000, 0x7E00];
    assert_eq!(count_bit_identical_binary16(&r, &a), Ok(2));
    assert!(compare_order_distance_binary16(&r, &a).unwrap().is_match());
    assert_eq!(
        count_bit_identical_binary16(&[], &[1]),
        Err(CompareError::LengthMismatch {
            reference: 0,
            actual: 1
        })
    );
    assert_eq!(
        count_bit_identical_binary16(&[], &[]),
        Err(CompareError::EmptyInput)
    );
}

fn next_up(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

#[test]
fn relative_sum_order_first() {
    let sums = [0.0, -1.0, f64::NAN, f64::INFINITY, 1000.0];
    let cases = [
        (f32::NAN, -f32::NAN, FallbackVerdict::BothNan),
        (f32::INFINITY, f32::INFINITY, FallbackVerdict::SameInfinity),
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            FallbackVerdict::NonFiniteMismatch,
        ),
        (f32::NAN, 1.0, FallbackVerdict::NonFiniteMismatch),
        (0.0, -0.0, FallbackVerdict::BothZero),
    ];
    for s in sums {
        for (r, a, v) in cases {
            assert_eq!(judge_relative_sum_f32(r, a, s), v);
        }
    }
}

#[test]
fn relative_sum_boundary_is_inclusive() {
    assert_eq!(SUM_RELATIVE_BOUND, 1e-3);
    assert_eq!(
        judge_relative_sum_f32(1.0, 2.0, 1000.0),
        FallbackVerdict::WithinRelativeSum
    );
    assert_eq!(
        judge_relative_sum_f32(2.0, 1.0, 1000.0),
        FallbackVerdict::WithinRelativeSum
    );
    assert_eq!(
        judge_relative_sum_f32(1.0, next_up(2.0), 1000.0),
        FallbackVerdict::RelativeSumExceeded
    );
    assert_eq!(
        judge_relative_sum_f32(next_up(2.0), 1.0, 1000.0),
        FallbackVerdict::RelativeSumExceeded
    );
}

#[test]
fn relative_sum_within_and_exceeded() {
    let w = judge_relative_sum_f32(10.0, 10.25, 1000.0);
    assert_eq!(w, FallbackVerdict::WithinRelativeSum);
    assert!(w.is_match());
    assert_eq!(
        judge_relative_sum_f32(3.5, 3.5, 1.0),
        FallbackVerdict::WithinRelativeSum
    );
    let e = judge_relative_sum_f32(10.0, 12.0, 1000.0);
    assert_eq!(e, FallbackVerdict::RelativeSumExceeded);
    assert!(!e.is_match());
}

#[test]
fn relative_sum_zero_abs_sum() {
    let tiny = f32::from_bits(1);
    assert_eq!(
        judge_relative_sum_f32(0.0, tiny, 0.0),
        FallbackVerdict::RelativeSumExceeded
    );
    assert_eq!(relative_sum_margin(0.0, tiny, 0.0), f64::INFINITY);
    assert_eq!(
        judge_relative_sum_f32(1.5, 1.5, 0.0),
        FallbackVerdict::WithinRelativeSum
    );
    assert_eq!(relative_sum_margin(1.5, 1.5, 0.0), 0.0);
}

#[test]
fn relative_sum_invalid_abs_sum_fails_closed() {
    for s in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            judge_relative_sum_f32(1.0, 1.0, s),
            FallbackVerdict::RelativeSumExceeded
        );
        assert_eq!(relative_sum_margin(1.0, 1.0, s), f64::INFINITY);
    }
}

#[test]
fn relative_sum_margin_values() {
    assert_eq!(relative_sum_margin(1.0, 1.5, 1000.0), 5e-4);
    assert_eq!(relative_sum_margin(f32::NAN, f32::NAN, 1000.0), 0.0);
    assert_eq!(relative_sum_margin(0.0, -0.0, f64::NAN), 0.0);
    assert_eq!(
        relative_sum_margin(f32::INFINITY, 1.0, 1000.0),
        f64::INFINITY
    );
    for (r, a) in [(1.0f32, 1.5f32), (1.0, 3.0), (5.0, 5.0)] {
        let within = judge_relative_sum_f32(r, a, 1000.0).is_match();
        assert_eq!(
            within,
            relative_sum_margin(r, a, 1000.0) <= SUM_RELATIVE_BOUND
        );
    }
}

#[test]
fn compare_relative_sum_errors() {
    let mut m = [-7.0f64; 3];
    assert_eq!(
        compare_relative_sum_f32(&[1.0], &[1.0, 2.0], &[1.0], &mut m),
        Err(CompareError::LengthMismatch {
            reference: 1,
            actual: 2
        })
    );
    assert_eq!(
        compare_relative_sum_f32(&[1.0, 2.0], &[1.0, 2.0], &[1.0], &mut m),
        Err(CompareError::LengthMismatch {
            reference: 2,
            actual: 1
        })
    );
    assert_eq!(
        compare_relative_sum_f32(&[1.0, 2.0], &[1.0, 2.0], &[1.0, 1.0], &mut m),
        Err(CompareError::LengthMismatch {
            reference: 2,
            actual: 3
        })
    );
    assert_eq!(
        compare_relative_sum_f32(&[], &[], &[], &mut []),
        Err(CompareError::EmptyInput)
    );
    assert_eq!(m, [-7.0; 3]);
}

#[test]
fn compare_relative_sum_summary_and_margins() {
    let r = [1.0f32, 10.0, f32::NAN, 0.0, 5.0];
    let a = [2.0f32, 12.0, f32::NAN, -0.0, 5.0];
    let s = [1000.0f64, 1000.0, f64::NAN, 0.0, 1.0];
    let mut m = [-7.0f64; 5];
    let c = compare_relative_sum_f32(&r, &a, &s, &mut m).unwrap();
    assert_eq!(c.compared(), 5);
    assert_eq!(c.mismatched(), 1);
    assert_eq!(c.first_mismatch(), Some(1));
    for i in 0..5 {
        assert_eq!(m[i], relative_sum_margin(r[i], a[i], s[i]));
    }
}
