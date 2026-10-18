//! 代わりの実行の比べ方の順序の前段とビット同一の条件・binary16 の順序の差の条件・f32 の和の相対の条件・非正規化数を 0 に落とす参照の振る舞いの確認と、境目の入力での判定の順序・閾値の固定の確認（spec TASK-928 の 2/8〜7/8）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。入力はこの場で作る。

use fandhe_silicon_core::parity::binary16::binary16_bits_to_f32;
use fandhe_silicon_core::parity::fallback_compare::{
    BINARY16_MAX_ORDER_DISTANCE, F32SubnormalHandling, FallbackComparison, FallbackVerdict,
    OrderVerdict, SUM_RELATIVE_BOUND, binary16_order_distance, compare_bits_f32, compare_bits_u32,
    compare_order_distance_binary16, compare_relative_sum_f32, count_bit_identical_binary16,
    flush_subnormal_f32, judge_bits_f32, judge_bits_u32, judge_order_binary16,
    judge_order_distance_binary16, judge_order_f32, judge_relative_sum_f32, reference_sum_f32,
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

const PRESERVE: F32SubnormalHandling = F32SubnormalHandling::Preserve;
const FLUSH: F32SubnormalHandling = F32SubnormalHandling::FlushToZero;

#[test]
fn flush_subnormal_boundaries() {
    for bits in [0x0000_0001u32, 0x007F_FFFF, 0x8000_0001, 0x807F_FFFF] {
        assert_eq!(flush_subnormal_f32(f32::from_bits(bits)).to_bits(), 0);
    }
    for bits in [0x0080_0000u32, 0x8080_0000] {
        assert_eq!(flush_subnormal_f32(f32::from_bits(bits)).to_bits(), bits);
    }
}

#[test]
fn flush_leaves_special_values() {
    for x in [
        0.0f32,
        -0.0,
        1.0,
        -2.5,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert_eq!(flush_subnormal_f32(x).to_bits(), x.to_bits());
    }
    assert!(flush_subnormal_f32(f32::NAN).is_nan());
}

#[test]
fn reference_preserve_is_plain_f64_sum() {
    let xs = [1.0f32, f32::from_bits(5), -0.5, f32::from_bits(0x007F_FFFF)];
    let r = reference_sum_f32(xs, PRESERVE);
    let want: f64 = xs.iter().map(|&x| f64::from(x)).sum();
    let abs: f64 = xs.iter().map(|&x| f64::from(x).abs()).sum();
    assert_eq!(r.value().to_bits(), (want as f32).to_bits());
    assert_eq!(r.abs_sum(), abs);
}

#[test]
fn reference_flush_replaces_inputs() {
    let xs = [
        f32::from_bits(1),
        -f32::from_bits(0x007F_FFFF),
        f32::from_bits(77),
    ];
    let f = reference_sum_f32(xs, FLUSH);
    assert_eq!(f.value().to_bits(), 0);
    assert_eq!(f.abs_sum(), 0.0);
    let p = reference_sum_f32(xs, PRESERVE);
    assert!(p.value().is_subnormal() || p.value() == 0.0);
    assert!(p.abs_sum() > 0.0);
    assert_ne!(p.value().to_bits(), 0);
}

#[test]
fn reference_flush_replaces_rounded_result() {
    let xs = [f32::MIN_POSITIVE * 1.5, -f32::MIN_POSITIVE];
    let p = reference_sum_f32(xs, PRESERVE);
    assert!(p.value().is_subnormal());
    let f = reference_sum_f32(xs, FLUSH);
    assert_eq!(f.value().to_bits(), 0);
    assert_eq!(f.abs_sum(), p.abs_sum());
}

#[test]
fn reference_empty_inputs() {
    for h in [PRESERVE, FLUSH] {
        let r = reference_sum_f32(std::iter::empty(), h);
        assert_eq!(r.value().to_bits(), 0);
        assert_eq!(r.abs_sum(), 0.0);
    }
}

#[test]
fn reference_accepts_strided_inputs() {
    let all = [1.0f32, 9.0, f32::from_bits(3), 9.0, 2.0, 9.0];
    let picked = [1.0f32, f32::from_bits(3), 2.0];
    for h in [PRESERVE, FLUSH] {
        let a = reference_sum_f32(all.iter().copied().step_by(2), h);
        assert_eq!(a, reference_sum_f32(picked, h));
    }
}

#[test]
fn reference_flush_changes_verdict() {
    let xs = [f32::MIN_POSITIVE * 1.5, -f32::MIN_POSITIVE];
    let flushed_actual = 0.0f32;
    let kept_actual = f32::MIN_POSITIVE * 0.5;
    for (actual, flush_ok, keep_ok) in [(flushed_actual, true, false), (kept_actual, false, true)] {
        let f = reference_sum_f32(xs, FLUSH);
        let p = reference_sum_f32(xs, PRESERVE);
        let vf = judge_relative_sum_f32(f.value(), actual, f.abs_sum());
        let vp = judge_relative_sum_f32(p.value(), actual, p.abs_sum());
        assert_eq!(vf.is_match(), flush_ok);
        assert_eq!(vp.is_match(), keep_ok);
    }
    // 置き換えの後の Σ|x| が小さくなって判定が変わる組（値は同じ +0）
    let s = f32::from_bits(0x007F_FFFF);
    let ys = [s, -s];
    let f = reference_sum_f32(ys, FLUSH);
    let p = reference_sum_f32(ys, PRESERVE);
    assert_eq!(f.value().to_bits(), p.value().to_bits());
    assert_eq!(f.abs_sum(), 0.0);
    assert!(p.abs_sum() > 0.0);
    let actual = f32::from_bits(1);
    assert!(!judge_relative_sum_f32(f.value(), actual, f.abs_sum()).is_match());
    assert!(judge_relative_sum_f32(p.value(), actual, p.abs_sum()).is_match());
    let mut m = [0.0f64];
    let c = compare_relative_sum_f32(&[p.value()], &[actual], &[p.abs_sum()], &mut m).unwrap();
    assert!(c.is_match());
}

#[test]
fn reference_non_finite_inputs_follow_order() {
    let nan = reference_sum_f32([1.0, f32::NAN], FLUSH);
    assert_eq!(
        judge_relative_sum_f32(nan.value(), f32::NAN, nan.abs_sum()),
        FallbackVerdict::BothNan
    );
    let inf = reference_sum_f32([1.0, f32::INFINITY], PRESERVE);
    assert_eq!(
        judge_relative_sum_f32(inf.value(), f32::INFINITY, inf.abs_sum()),
        FallbackVerdict::SameInfinity
    );
    let mixed = reference_sum_f32([f32::INFINITY, f32::NEG_INFINITY], FLUSH);
    assert!(mixed.value().is_nan());
    assert_eq!(
        judge_relative_sum_f32(mixed.value(), 1.0, mixed.abs_sum()),
        FallbackVerdict::NonFiniteMismatch
    );
}

#[test]
fn reference_connects_to_bit_identity() {
    let xs = [1.0f32, 2.0, 4.0];
    for h in [PRESERVE, FLUSH] {
        let r = reference_sum_f32(xs, h);
        assert_eq!(
            judge_bits_f32(r.value(), 7.0),
            FallbackVerdict::BitIdentical
        );
    }
}
// ---- 境目の入力（6/8）----
// 期待値は IEEE 754 のビット列から直接書く。前の段で決まる組は、後の段の条件が成り立つ値を与えても前の段の区分になる。

const INF: f32 = f32::INFINITY;
const NINF: f32 = f32::NEG_INFINITY;
const HUGE_SUM: f64 = f64::MAX;

/// f32 の 1 組を f32 を扱う入口（ビット同一・相対の条件・その余裕と、それぞれの列の入口）に通す。
/// `bits` はビット同一の入口の期待、`rel` は相対の条件の期待、`margin_zero` は余裕が 0.0 か +Inf か。
fn check_f32_pair(
    r: f32,
    a: f32,
    abs_sum: f64,
    bits: FallbackVerdict,
    rel: FallbackVerdict,
    margin_zero: bool,
) {
    assert_eq!(
        judge_bits_f32(r, a),
        bits,
        "bits {:#x} {:#x}",
        r.to_bits(),
        a.to_bits()
    );
    assert_eq!(judge_relative_sum_f32(r, a, abs_sum), rel);
    let m = relative_sum_margin(r, a, abs_sum);
    if margin_zero {
        assert_eq!(m, 0.0);
    } else {
        assert_eq!(m, f64::INFINITY);
    }
    let mut ms = [1.0f64];
    let c = compare_relative_sum_f32(&[r], &[a], &[abs_sum], &mut ms).unwrap();
    assert_eq!(c.mismatched(), usize::from(!rel.is_match()));
    assert_eq!(ms[0].to_bits(), m.to_bits());
    let cb = compare_bits_f32(&[r], &[a]).unwrap();
    assert_eq!(cb.mismatched(), usize::from(!bits.is_match()));
}

#[test]
fn boundary_nan_is_decided_first() {
    use FallbackVerdict::{BothNan, NonFiniteMismatch};
    for n in nans() {
        for x in [INF, NINF, 0.0, -0.0, 1.0, f32::MAX, f32::from_bits(1)] {
            for s in [0.0, 1.0, HUGE_SUM] {
                check_f32_pair(n, x, s, NonFiniteMismatch, NonFiniteMismatch, false);
                check_f32_pair(x, n, s, NonFiniteMismatch, NonFiniteMismatch, false);
            }
        }
        for m in nans() {
            check_f32_pair(n, m, 0.0, BothNan, BothNan, true);
        }
    }
    // binary16: 最小の NaN は +Inf とビット列が隣り合う（順序の差の鍵では 1）が、前段で不一致に決まる
    assert_eq!(binary16_order_distance(0x7C00, 0x7C01), 1);
    assert_eq!(
        judge_order_distance_binary16(0x7C01, 0x7C00),
        NonFiniteMismatch
    );
    assert_eq!(
        judge_order_distance_binary16(0x7C00, 0x7C01),
        NonFiniteMismatch
    );
    assert_eq!(
        judge_order_distance_binary16(0xFC00, 0xFC01),
        NonFiniteMismatch
    );
    for r in [0x7C01u16, 0x7E00, 0xFE00] {
        for a in [0x7C01u16, 0x7E00, 0xFE00] {
            assert_eq!(judge_order_distance_binary16(r, a), BothNan);
        }
    }
}

#[test]
fn boundary_infinity_before_numeric_f32() {
    use FallbackVerdict::{NonFiniteMismatch, SameInfinity};
    assert_eq!(f32::MAX.to_bits(), 0x7F7F_FFFF);
    assert_eq!(INF.to_bits(), 0x7F80_0000);
    // 最大の有限値と Inf はビット列が隣り合うが、数値の条件へは進まない
    for (r, a) in [
        (f32::MAX, INF),
        (INF, f32::MAX),
        (f32::MIN, NINF),
        (NINF, f32::MIN),
        (INF, NINF),
        (NINF, INF),
    ] {
        check_f32_pair(r, a, HUGE_SUM, NonFiniteMismatch, NonFiniteMismatch, false);
    }
    for x in [INF, NINF] {
        check_f32_pair(x, x, 0.0, SameInfinity, SameInfinity, true);
        // 同符号の Inf は Σ|x| が使えない値でも一致
        check_f32_pair(x, x, f64::NAN, SameInfinity, SameInfinity, true);
    }
}

#[test]
fn boundary_infinity_before_order_distance_binary16() {
    use FallbackVerdict::{NonFiniteMismatch, SameInfinity, WithinOrderDistance};
    for (r, a) in [
        (0x7BFFu16, 0x7C00u16),
        (0x7C00, 0x7BFF),
        (0xFBFF, 0xFC00),
        (0xFC00, 0xFBFF),
    ] {
        assert_eq!(binary16_order_distance(r, a), 1);
        assert_eq!(judge_order_distance_binary16(r, a), NonFiniteMismatch);
    }
    assert_eq!(
        judge_order_distance_binary16(0x7BFE, 0x7BFF),
        WithinOrderDistance
    );
    assert_eq!(
        judge_order_distance_binary16(0xFBFE, 0xFBFF),
        WithinOrderDistance
    );
    assert_eq!(judge_order_distance_binary16(0x7C00, 0x7C00), SameInfinity);
    assert_eq!(
        judge_order_distance_binary16(0x7C00, 0xFC00),
        NonFiniteMismatch
    );
    let c = compare_order_distance_binary16(&[0x7BFE, 0x7BFF, 0xFBFF], &[0x7BFF, 0x7C00, 0xFC00])
        .unwrap();
    assert_eq!(
        (c.compared(), c.mismatched(), c.first_mismatch()),
        (3, 2, Some(1))
    );
}

#[test]
fn boundary_signed_zero_before_numeric() {
    use FallbackVerdict::{BitsDiffer, BothZero, RelativeSumExceeded, WithinRelativeSum};
    // 符号つきの零はビット列が違っても全入口で一致
    for (r, a) in [(0.0f32, -0.0f32), (-0.0, 0.0), (0.0, 0.0), (-0.0, -0.0)] {
        for s in [0.0, 1.0, f64::NAN] {
            check_f32_pair(r, a, s, BothZero, BothZero, true);
        }
    }
    for (r, a) in [(0x0000u16, 0x8000u16), (0x8000, 0x0000), (0x0000, 0x0000)] {
        assert_eq!(judge_order_distance_binary16(r, a), BothZero);
    }
    // 零と最小の非正規化数は前段で決まらず、数値の条件へ進む
    for tiny in [f32::from_bits(1), f32::from_bits(0x8000_0001)] {
        for z in [0.0f32, -0.0] {
            for (r, a) in [(z, tiny), (tiny, z)] {
                assert_eq!(judge_order_f32(r, a), OrderVerdict::Numeric);
                check_f32_pair(r, a, 0.0, BitsDiffer, RelativeSumExceeded, false);
                assert_eq!(judge_relative_sum_f32(r, a, 1.0), WithinRelativeSum);
                assert_eq!(relative_sum_margin(r, a, 1.0), f64::from(tiny).abs());
            }
        }
    }
    for tiny in [0x0001u16, 0x8001] {
        for z in [0x0000u16, 0x8000] {
            assert_eq!(
                judge_order_distance_binary16(z, tiny),
                FallbackVerdict::WithinOrderDistance
            );
            assert_eq!(
                judge_order_distance_binary16(tiny, z),
                FallbackVerdict::WithinOrderDistance
            );
        }
    }
}

#[test]
fn boundary_adjacent_bits_binary16() {
    use FallbackVerdict::{OrderDistanceExceeded as Exceeded, WithinOrderDistance as Within};
    let rows: [(u16, u16, u32, FallbackVerdict); 9] = [
        (0x3C00, 0x3C01, 1, Within),
        (0x3C01, 0x3C00, 1, Within),
        (0x03FF, 0x0400, 1, Within),
        (0x83FF, 0x8400, 1, Within),
        (0x3C00, 0x3C02, 2, Exceeded),
        (0x8001, 0x0001, 2, Exceeded),
        (0x0002, 0x0000, 2, Exceeded),
        (0x8002, 0x0000, 2, Exceeded),
        (0x03FF, 0x0401, 2, Exceeded),
    ];
    for (r, a, d, v) in rows {
        assert_eq!(binary16_order_distance(r, a), d);
        assert_eq!(judge_order_distance_binary16(r, a), v);
    }
    let r: Vec<u16> = rows.iter().map(|x| x.0).collect();
    let a: Vec<u16> = rows.iter().map(|x| x.1).collect();
    let c = compare_order_distance_binary16(&r, &a).unwrap();
    assert_eq!(
        (c.compared(), c.mismatched(), c.first_mismatch()),
        (9, 5, Some(4))
    );
    // ビット同一の件数は記録だけで、判定の数とは別
    assert_eq!(count_bit_identical_binary16(&r, &a).unwrap(), 0);
}

#[test]
fn boundary_adjacent_bits_f32() {
    use FallbackVerdict::{BitIdentical, BitsDiffer};
    let one = 1.0f32;
    let next = f32::from_bits(one.to_bits() + 1);
    let prev = f32::from_bits(one.to_bits() - 1);
    let sub_max = f32::from_bits(0x007F_FFFF);
    let norm_min = f32::from_bits(0x0080_0000);
    assert_eq!(norm_min, f32::MIN_POSITIVE);
    for (r, a, v) in [
        (one, next, BitsDiffer),
        (one, prev, BitsDiffer),
        (sub_max, norm_min, BitsDiffer),
        (norm_min, sub_max, BitsDiffer),
        (-sub_max, -norm_min, BitsDiffer),
        (one, one, BitIdentical),
        (sub_max, sub_max, BitIdentical),
        (norm_min, norm_min, BitIdentical),
    ] {
        assert_eq!(judge_bits_f32(r, a), v);
    }
    assert_eq!(judge_bits_u32(1, 2), BitsDiffer);
    assert_eq!(judge_bits_u32(u32::MAX, u32::MAX), BitIdentical);
}

#[test]
fn boundary_relative_sum_edges() {
    use FallbackVerdict::{NonFiniteMismatch, RelativeSumExceeded, WithinRelativeSum};
    // 誤差 1.0 は Σ|x| = 1000 のちょうど SUM_RELATIVE_BOUND 倍（f64 で正確に 1.0）
    assert_eq!(SUM_RELATIVE_BOUND * 1000.0, 1.0);
    let up = f32::from_bits(1.0f32.to_bits() + 1);
    assert_eq!(judge_relative_sum_f32(0.0, 1.0, 1000.0), WithinRelativeSum);
    assert_eq!(judge_relative_sum_f32(0.0, -1.0, 1000.0), WithinRelativeSum);
    assert_eq!(relative_sum_margin(0.0, 1.0, 1000.0), 1e-3);
    // 境目を 1 ulp 超える誤差と、Σ|x| を 1 段小さくした組は不一致
    assert_eq!(judge_relative_sum_f32(0.0, up, 1000.0), RelativeSumExceeded);
    assert_eq!(judge_relative_sum_f32(0.0, 1.0, 999.0), RelativeSumExceeded);
    // Σ|x| = 0: 誤差 0 は一致、誤差が正なら不一致
    assert_eq!(judge_relative_sum_f32(1.0, 1.0, 0.0), WithinRelativeSum);
    assert_eq!(relative_sum_margin(1.0, 1.0, 0.0), 0.0);
    assert_eq!(judge_relative_sum_f32(1.0, up, 0.0), RelativeSumExceeded);
    assert_eq!(relative_sum_margin(1.0, up, 0.0), f64::INFINITY);
    // Σ|x| が使えない値は、前段で決まらない組だけ不一致
    for s in [f64::NAN, -1.0, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(judge_relative_sum_f32(1.0, 1.0, s), RelativeSumExceeded);
        assert_eq!(relative_sum_margin(1.0, 1.0, s), f64::INFINITY);
        assert_eq!(
            judge_relative_sum_f32(0.0, -0.0, s),
            FallbackVerdict::BothZero
        );
        assert_eq!(judge_relative_sum_f32(f32::NAN, 1.0, s), NonFiniteMismatch);
        assert_eq!(
            judge_relative_sum_f32(INF, INF, s),
            FallbackVerdict::SameInfinity
        );
    }
}

#[test]
fn boundary_flush_reference_edges() {
    // 最大の非正規化数は +0（ビット列 0）、最小の正規化数はそのまま
    assert_eq!(
        flush_subnormal_f32(f32::from_bits(0x007F_FFFF)).to_bits(),
        0
    );
    assert_eq!(
        flush_subnormal_f32(f32::from_bits(0x807F_FFFF)).to_bits(),
        0
    );
    assert_eq!(flush_subnormal_f32(f32::from_bits(1)).to_bits(), 0);
    assert_eq!(
        flush_subnormal_f32(f32::from_bits(0x8000_0001)).to_bits(),
        0
    );
    assert_eq!(
        flush_subnormal_f32(f32::MIN_POSITIVE).to_bits(),
        0x0080_0000
    );
    assert_eq!(
        flush_subnormal_f32(-f32::MIN_POSITIVE).to_bits(),
        0x8080_0000
    );
    assert_eq!(flush_subnormal_f32(-0.0).to_bits(), 0x8000_0000);

    let mp = f32::MIN_POSITIVE;
    // 和がちょうど最小の正規化数: 置き換えられない
    let at = [2.0 * mp, -mp];
    for h in [PRESERVE, FLUSH] {
        let r = reference_sum_f32(at, h);
        assert_eq!(r.value().to_bits(), 0x0080_0000);
        assert_eq!(r.abs_sum(), f64::from(3.0 * mp));
    }
    // その少し下（丸めた結果が非正規化数）: 落とす参照は +0、保つ参照は非正規化数のまま
    let below = [2.0 * mp, -1.5 * mp];
    let f = reference_sum_f32(below, FLUSH);
    let p = reference_sum_f32(below, PRESERVE);
    assert_eq!(f.value().to_bits(), 0);
    assert_eq!(p.value().to_bits(), 0x0040_0000);
    assert_eq!(f.abs_sum(), f64::from(3.5 * mp));
    assert_eq!(p.abs_sum(), f64::from(3.5 * mp));
    let neg = reference_sum_f32([-2.0 * mp, 1.5 * mp], FLUSH);
    assert_eq!(neg.value().to_bits(), 0);
    // Σ|x| は置き換えの後の入力で数える
    let sub = f32::from_bits(0x007F_FFFF);
    let mixed = [sub, mp, -sub];
    assert_eq!(reference_sum_f32(mixed, FLUSH).abs_sum(), f64::from(mp));
    assert_eq!(
        reference_sum_f32(mixed, PRESERVE).abs_sum(),
        2.0 * f64::from(sub) + f64::from(mp)
    );
    // 同じ実測で、落とす参照と保つ参照の判定が分かれる
    let actual = f32::from_bits(0x0040_0000);
    assert_eq!(
        judge_relative_sum_f32(f.value(), actual, 0.0),
        FallbackVerdict::RelativeSumExceeded
    );
    assert_eq!(
        judge_relative_sum_f32(p.value(), actual, 0.0),
        FallbackVerdict::WithinRelativeSum
    );
    let mut mf = [0.0f64];
    let mut mk = [0.0f64];
    let cf = compare_relative_sum_f32(&[f.value()], &[actual], &[0.0], &mut mf).unwrap();
    let ck = compare_relative_sum_f32(&[p.value()], &[actual], &[0.0], &mut mk).unwrap();
    assert_eq!((cf.mismatched(), ck.mismatched()), (1, 0));
    assert_eq!((mf[0], mk[0]), (f64::INFINITY, 0.0));
}

#[test]
fn boundary_binary16_ignores_flush() {
    // binary16 の非正規化数は 0 に落とさず零と区別する（入口は u16 のまま）
    assert_ne!(binary16_bits_to_f32(0x0002), 0.0);
    assert_eq!(judge_order_binary16(0x0002, 0x0000), OrderVerdict::Numeric);
    assert_eq!(
        judge_order_distance_binary16(0x0002, 0x0000),
        FallbackVerdict::OrderDistanceExceeded
    );
    assert_eq!(
        judge_order_distance_binary16(0x0001, 0x0000),
        FallbackVerdict::WithinOrderDistance
    );
}

#[test]
fn boundary_sequence_order_summary() {
    use FallbackVerdict::{
        BothNan, BothZero, NonFiniteMismatch, OrderDistanceExceeded, SameInfinity,
        WithinOrderDistance,
    };
    // binary16: 一致 / NaN / 零 / Inf 同 / 最大有限と Inf / 隣り合う / 差 2 / NaN と Inf
    let r16 = [
        0x3C00u16, 0x7E00, 0x0000, 0xFC00, 0x7BFF, 0x03FF, 0x8001, 0x7C01,
    ];
    let a16 = [
        0x3C00u16, 0xFE00, 0x8000, 0xFC00, 0x7C00, 0x0400, 0x0001, 0x7C00,
    ];
    let want16 = [
        WithinOrderDistance,
        BothNan,
        BothZero,
        SameInfinity,
        NonFiniteMismatch,
        WithinOrderDistance,
        OrderDistanceExceeded,
        NonFiniteMismatch,
    ];
    for i in 0..r16.len() {
        assert_eq!(judge_order_distance_binary16(r16[i], a16[i]), want16[i]);
    }
    let c = compare_order_distance_binary16(&r16, &a16).unwrap();
    assert_eq!(
        (c.compared(), c.mismatched(), c.first_mismatch()),
        (8, 3, Some(4))
    );

    // f32: ビット同一と相対の条件の集計
    let r = [1.0f32, f32::NAN, 0.0, NINF, f32::MAX, 0.0, 1.0];
    let a = [
        1.0f32,
        -f32::NAN,
        -0.0,
        NINF,
        INF,
        f32::from_bits(1),
        f32::from_bits(1.0f32.to_bits() + 1),
    ];
    let s = [0.0f64, 1.0, 0.0, 0.0, HUGE_SUM, 0.0, 0.0];
    let cb = compare_bits_f32(&r, &a).unwrap();
    assert_eq!(
        (cb.compared(), cb.mismatched(), cb.first_mismatch()),
        (7, 3, Some(4))
    );
    let mut m = [9.0f64; 7];
    let cr = compare_relative_sum_f32(&r, &a, &s, &mut m).unwrap();
    assert_eq!(
        (cr.compared(), cr.mismatched(), cr.first_mismatch()),
        (7, 3, Some(4))
    );
    let inf = f64::INFINITY;
    assert_eq!(m, [0.0, 0.0, 0.0, 0.0, inf, inf, inf]);
}

// ---- 閾値の固定（7/8）----

#[test]
fn threshold_constants_are_readable_and_fixed() {
    // 外の crate から読めて、値が固定されていること（緩めも締めもしない）
    assert_eq!(BINARY16_MAX_ORDER_DISTANCE, 1);
    assert_eq!(SUM_RELATIVE_BOUND.to_bits(), 1e-3_f64.to_bits());
}

#[test]
fn threshold_order_distance_applied_is_the_readable_one() {
    let d = u16::try_from(BINARY16_MAX_ORDER_DISTANCE).unwrap();
    // 正の正規化数・負の正規化数・零をまたがない非正規化数
    let bases = [0x3C00u16, 0xBC00, 0x0010, 0x8010];
    let (mut seen_within, mut seen_exceeded) = (false, false);
    let (mut r, mut a, mut want_mismatched) = (Vec::new(), Vec::new(), 0usize);
    for base in bases {
        // 符号ビットの向きによらず、ビット列の値が 1 増えると順序の差も 1 増える
        let at = base + d;
        let over = base + d + 1;
        assert_eq!(
            binary16_order_distance(base, at),
            BINARY16_MAX_ORDER_DISTANCE
        );
        assert_eq!(
            binary16_order_distance(base, over),
            BINARY16_MAX_ORDER_DISTANCE + 1
        );
        let v_at = judge_order_distance_binary16(base, at);
        let v_over = judge_order_distance_binary16(base, over);
        assert_eq!(v_at, FallbackVerdict::WithinOrderDistance);
        assert_eq!(v_over, FallbackVerdict::OrderDistanceExceeded);
        seen_within |= v_at == FallbackVerdict::WithinOrderDistance;
        seen_exceeded |= v_over == FallbackVerdict::OrderDistanceExceeded;
        r.extend([base, base]);
        a.extend([at, over]);
        want_mismatched += 1;
    }
    assert!(seen_within && seen_exceeded);
    // 列の入口でも同じ切り替わりが出る（超えた組だけが不一致）
    let c = compare_order_distance_binary16(&r, &a).unwrap();
    assert_eq!(c.compared(), r.len());
    assert_eq!(c.mismatched(), want_mismatched);
    assert_eq!(c.first_mismatch(), Some(1));
}

#[test]
fn threshold_relative_sum_applied_is_the_readable_one() {
    // 閾値 * Σ|x| が f64 で正確に 1.0 になる組を使う（先に確かめる）
    let abs_sum = 1000.0f64;
    let bound = SUM_RELATIVE_BOUND * abs_sum;
    assert_eq!(bound, 1.0);
    let r = 0.0f32;
    let at = 1.0f32; // 誤差がちょうど閾値 * Σ|x|
    let over = at.next_up(); // 1 ulp 上
    let under = at.next_down(); // 1 ulp 下
    assert_eq!(f64::from(at), bound);
    let (mut seen_within, mut seen_exceeded) = (false, false);

    let v_at = judge_relative_sum_f32(r, at, abs_sum);
    let v_over = judge_relative_sum_f32(r, over, abs_sum);
    let v_under = judge_relative_sum_f32(r, under, abs_sum);
    // 境目を含む。1 ulp 下も一致（締められていない）、1 ulp 上は不一致（緩められていない）
    assert_eq!(v_at, FallbackVerdict::WithinRelativeSum);
    assert_eq!(v_under, FallbackVerdict::WithinRelativeSum);
    assert_eq!(v_over, FallbackVerdict::RelativeSumExceeded);
    seen_within |= v_at == FallbackVerdict::WithinRelativeSum;
    seen_exceeded |= v_over == FallbackVerdict::RelativeSumExceeded;
    // 同じ誤差で Σ|x| をわずかに下げると不一致
    assert_eq!(
        judge_relative_sum_f32(r, at, abs_sum - 1e-6),
        FallbackVerdict::RelativeSumExceeded
    );
    assert!(seen_within && seen_exceeded);

    // 余裕の値も同じ閾値の側に出る
    let m_at = relative_sum_margin(r, at, abs_sum);
    let m_under = relative_sum_margin(r, under, abs_sum);
    let m_over = relative_sum_margin(r, over, abs_sum);
    assert!(m_at <= SUM_RELATIVE_BOUND && m_under <= SUM_RELATIVE_BOUND);
    assert!(m_over > SUM_RELATIVE_BOUND);

    // 列の入口でも同じ
    let mut margins = [0.0f64; 3];
    let c =
        compare_relative_sum_f32(&[r; 3], &[at, over, under], &[abs_sum; 3], &mut margins).unwrap();
    assert_eq!(
        (c.compared(), c.mismatched(), c.first_mismatch()),
        (3, 1, Some(1))
    );
    assert_eq!(margins, [m_at, m_over, m_under]);
}

#[test]
fn threshold_is_not_a_parameter() {
    // 閾値を引数に取らない形をコンパイルで固定する（呼び出し側で緩めも締めもできない）
    let _: fn(u16, u16) -> FallbackVerdict = judge_order_distance_binary16;
    let _: fn(u16, u16) -> u32 = binary16_order_distance;
    let _: fn(&[u16], &[u16]) -> Result<FallbackComparison, CompareError> =
        compare_order_distance_binary16;
    let _: fn(f32, f32, f64) -> FallbackVerdict = judge_relative_sum_f32;
    let _: fn(f32, f32, f64) -> f64 = relative_sum_margin;
    type SumSeq =
        fn(&[f32], &[f32], &[f64], &mut [f64]) -> Result<FallbackComparison, CompareError>;
    let _: SumSeq = compare_relative_sum_f32;
}
