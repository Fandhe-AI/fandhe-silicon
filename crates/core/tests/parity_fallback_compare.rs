//! 代わりの実行の比べ方の順序の前段とビット同一の条件の振る舞いの確認（spec TASK-928 の 2/8）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。入力はこの場で作る。

use fandhe_silicon_core::parity::binary16::binary16_bits_to_f32;
use fandhe_silicon_core::parity::fallback_compare::{
    FallbackVerdict, OrderVerdict, compare_bits_f32, compare_bits_u32, judge_bits_f32,
    judge_bits_u32, judge_order_binary16, judge_order_f32,
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
