//! 有限でない値と符号付きの零を扱う比べ方の振る舞いの確認（spec TASK-25 の 2/4）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。入力はこの場で作る。

use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, compare_f32_with_special_values, judge_element_with_special_values,
};
use fandhe_silicon_core::parity::tolerance::{CompareError, ElementVerdict, T1, judge_element};

use SpecialVerdict::{BothNan, Exceeded, NonFiniteMismatch, SameInfinity, Within};

fn j(r: f32, a: f32) -> SpecialVerdict {
    judge_element_with_special_values(r, a, T1)
}

fn nans() -> [f32; 3] {
    [f32::NAN, -f32::NAN, f32::from_bits(f32::NAN.to_bits() | 1)]
}

#[test]
fn nan_pairs_match_regardless_of_payload_and_sign() {
    for r in nans() {
        for a in nans() {
            assert_eq!(j(r, a), BothNan);
        }
    }
}

#[test]
fn nan_reference_against_non_nan_is_mismatch() {
    for a in [0.0, 1.0, f32::MAX, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(j(f32::NAN, a), NonFiniteMismatch);
    }
}

#[test]
fn infinities_match_only_with_same_sign() {
    assert_eq!(j(f32::INFINITY, f32::INFINITY), SameInfinity);
    assert_eq!(j(f32::NEG_INFINITY, f32::NEG_INFINITY), SameInfinity);
    assert_eq!(j(f32::INFINITY, f32::NEG_INFINITY), NonFiniteMismatch);
    assert_eq!(j(f32::NEG_INFINITY, f32::INFINITY), NonFiniteMismatch);
    assert_eq!(j(f32::INFINITY, f32::NAN), NonFiniteMismatch);
    assert_eq!(j(f32::INFINITY, f32::MAX), NonFiniteMismatch);
    assert_eq!(j(f32::NEG_INFINITY, f32::MIN), NonFiniteMismatch);
}

#[test]
fn finite_reference_against_non_finite_actual_is_mismatch() {
    for r in [0.0, -0.0, 1.0, -1.0, f32::MAX] {
        for a in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(j(r, a), NonFiniteMismatch);
        }
    }
}

#[test]
fn signed_zeros_match_each_other() {
    assert_eq!(j(0.0, -0.0), Within);
    assert_eq!(j(-0.0, 0.0), Within);
    assert_eq!(j(0.0, 0.0), Within);
    assert_eq!(j(-0.0, -0.0), Within);
}

#[test]
fn zero_reference_switches_at_absolute_boundary() {
    let b = T1.absolute() as f32;
    assert_eq!(j(0.0, b * 0.5), Within);
    assert_eq!(j(-0.0, -b * 0.5), Within);
    assert_eq!(j(0.0, b * 2.0), Exceeded);
    assert_eq!(j(-0.0, -b * 2.0), Exceeded);
}

#[test]
fn finite_pairs_follow_judge_element() {
    // 相対誤差がちょうど上限になる組は外（厳密な `<`）。
    assert_eq!(j(1000.0, 1001.0), Exceeded);
    assert_eq!(j(1000.0, 1000.5), Within);
    let values = [
        0.0,
        -0.0,
        1e-7,
        -1e-7,
        1e-4,
        1.0,
        -1.0,
        1.0005,
        1.002,
        1000.0,
        1000.5,
        1001.0,
        f32::MAX,
        f32::MIN_POSITIVE,
    ];
    for &r in &values {
        for &a in &values {
            let expected = match judge_element(r, a, T1) {
                ElementVerdict::Within => Within,
                ElementVerdict::Exceeded => Exceeded,
                ElementVerdict::NonFinite => NonFiniteMismatch,
            };
            assert_ne!(expected, NonFiniteMismatch, "both finite: r={r} a={a}");
            assert_eq!(j(r, a), expected, "r={r} a={a}");
        }
    }
}

#[test]
fn is_match_covers_only_match_variants() {
    assert!(BothNan.is_match());
    assert!(SameInfinity.is_match());
    assert!(Within.is_match());
    assert!(!Exceeded.is_match());
    assert!(!NonFiniteMismatch.is_match());
}

#[test]
fn existing_judge_element_is_unchanged() {
    assert_eq!(
        judge_element(f32::NAN, f32::NAN, T1),
        ElementVerdict::NonFinite
    );
    assert_eq!(
        judge_element(f32::INFINITY, f32::INFINITY, T1),
        ElementVerdict::NonFinite
    );
}

#[test]
fn sequence_summary_counts_each_class() {
    let r = [1.0, f32::NAN, f32::INFINITY, 1.0, 0.0, f32::INFINITY, 0.0];
    let a = [
        1.0,
        -f32::NAN,
        f32::INFINITY,
        2.0,
        -0.0,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    let c = compare_f32_with_special_values(&r, &a, T1).expect("same length, non-empty");
    assert_eq!(c.compared(), 7);
    assert_eq!(c.exceeded(), 1);
    assert_eq!(c.non_finite_mismatch(), 2);
    assert_eq!(c.first_mismatch(), Some(3));
    assert!(!c.is_match());
}

#[test]
fn sequence_with_only_matching_special_values_is_match() {
    let r = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0];
    let a = [-f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.0];
    let c = compare_f32_with_special_values(&r, &a, T1).expect("same length, non-empty");
    assert!(c.is_match());
    assert_eq!(c.first_mismatch(), None);
}

#[test]
fn sequence_errors() {
    assert_eq!(
        compare_f32_with_special_values(&[1.0], &[1.0, 2.0], T1),
        Err(CompareError::LengthMismatch {
            reference: 1,
            actual: 2
        })
    );
    assert_eq!(
        compare_f32_with_special_values(&[], &[1.0], T1),
        Err(CompareError::LengthMismatch {
            reference: 0,
            actual: 1
        })
    );
    assert_eq!(
        compare_f32_with_special_values(&[], &[], T1),
        Err(CompareError::EmptyInput)
    );
}
