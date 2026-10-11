//! 許容誤差つきの要素ごとの比べ方と閾値の設定の振る舞いの確認（spec TASK-24 の 4/8）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::tolerance::{
    CompareError, ElementVerdict, T1, compare_f32, judge_element,
};

use ElementVerdict::{Exceeded, NonFinite, Within};

fn j(r: f32, a: f32) -> ElementVerdict {
    judge_element(r, a, T1)
}

#[test]
fn threshold_is_readable() {
    assert_eq!(T1.relative().to_bits(), 1e-3_f64.to_bits());
    assert_eq!(T1.absolute().to_bits(), 1e-5_f64.to_bits());
}

#[test]
fn relative_boundary_is_strict() {
    assert_eq!(j(1000.0, 1001.0), Exceeded);
    assert_eq!(j(1000.0, 999.0), Exceeded);
    assert_eq!(j(-1000.0, -1001.0), Exceeded);
    assert_eq!(j(1000.0, 1000.5), Within);
    assert_eq!(j(1000.0, 999.5), Within);
}

#[test]
fn absolute_rescues_small_values() {
    assert_eq!(j(1.0e-3, 1.005e-3), Within);
}

#[test]
fn zero_reference_uses_absolute_only() {
    assert_eq!(j(0.0, 5e-6), Within);
    assert_eq!(j(0.0, 2e-5), Exceeded);
    assert_eq!(j(0.0, -0.0), Within);
    assert_eq!(j(-0.0, 0.0), Within);
}

#[test]
fn absolute_boundary_is_computed_in_f64() {
    let x = 1e-5_f32;
    assert_eq!(j(0.0, x), Within);
    assert_eq!(j(0.0, f32::from_bits(x.to_bits() + 1)), Exceeded);
}

#[test]
fn non_finite_values_are_flagged() {
    let (nan, inf) = (f32::NAN, f32::INFINITY);
    for (r, a) in [
        (nan, nan),
        (inf, inf),
        (-inf, inf),
        (1.0, nan),
        (1.0, inf),
        (nan, 1.0),
    ] {
        assert_eq!(j(r, a), NonFinite);
    }
}

#[test]
fn sequence_summary() {
    let r = [1.0_f32, 2.0, 3.0, 4.0];
    let ok = compare_f32(&r, &r, T1).unwrap();
    assert!(ok.is_match());
    assert_eq!(ok.compared(), 4);
    assert_eq!(ok.first_mismatch(), None);

    let bad = compare_f32(&r, &[1.0, 2.0, 3.5, 4.0], T1).unwrap();
    assert!(!bad.is_match());
    assert_eq!(bad.exceeded(), 1);
    assert_eq!(bad.first_mismatch(), Some(2));

    let nan = compare_f32(&r, &[1.0, f32::NAN, 3.0, 4.0], T1).unwrap();
    assert!(!nan.is_match());
    assert_eq!(nan.non_finite(), 1);
    assert_eq!(nan.first_mismatch(), Some(1));
}

#[test]
fn errors() {
    let mismatch = CompareError::LengthMismatch {
        reference: 1,
        actual: 2,
    };
    assert_eq!(compare_f32(&[1.0], &[1.0, 2.0], T1), Err(mismatch.clone()));
    let empty = compare_f32(&[], &[], T1).unwrap_err();
    assert_eq!(empty, CompareError::EmptyInput);
    for err in [empty, mismatch] {
        let s = err.to_string();
        assert!(!s.is_empty() && s.is_ascii());
    }
}

#[test]
fn works_with_cpu_reference_output() {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let c = reference_gemm_f32(&a, &b, 2, 3, 2).unwrap();
    assert!(compare_f32(&c, &c, T1).unwrap().is_match());
    let mut shifted = c.clone();
    shifted[3] += 10.0;
    assert!(!compare_f32(&c, &shifted, T1).unwrap().is_match());
}

#[test]
fn readable_thresholds_are_the_ones_applied() {
    // 絶対側: 参照 0 のとき、getter で読んだ絶対誤差の境界で内外が切り替わる。
    let abs = T1.absolute() as f32;
    let mut seen = (false, false);
    for x in [abs.next_down(), abs, abs.next_up()] {
        let expected = if f64::from(x) < T1.absolute() {
            Within
        } else {
            Exceeded
        };
        assert_eq!(j(0.0, x), expected);
        seen.0 |= expected == Within;
        seen.1 |= expected == Exceeded;
    }
    assert!(seen.0 && seen.1);

    // 相対側: 参照 1024（2 の冪で f64 の割り算が正確）。
    let rel = (1024.0 * (1.0 + T1.relative())) as f32;
    let mut seen = (false, false);
    for x in [rel.next_down(), rel, rel.next_up()] {
        let err = (f64::from(x) - 1024.0).abs();
        assert!(err >= T1.absolute());
        let expected = if err / 1024.0 < T1.relative() {
            Within
        } else {
            Exceeded
        };
        assert_eq!(j(1024.0, x), expected);
        seen.0 |= expected == Within;
        seen.1 |= expected == Exceeded;
    }
    assert!(seen.0 && seen.1);
}

#[test]
fn relative_error_is_measured_against_the_reference() {
    assert_eq!(j(1000.0, 1001.0), Exceeded);
    assert_eq!(j(1001.0, 1000.0), Within);
}

#[test]
fn large_finite_difference_is_exceeded_not_non_finite() {
    assert_eq!(j(f32::MAX, -f32::MAX), Exceeded);
    assert_eq!(j(f32::MAX, f32::MAX), Within);
}

#[test]
fn mixed_mismatches_are_counted_separately() {
    let c = compare_f32(&[1.0, 2.0, 3.0, 4.0], &[1.0, f32::NAN, 3.5, 4.0], T1).unwrap();
    assert_eq!(
        (
            c.compared(),
            c.exceeded(),
            c.non_finite(),
            c.first_mismatch()
        ),
        (4, 1, 1, Some(1))
    );
    assert!(!c.is_match());
}

#[test]
fn one_empty_side_is_a_length_mismatch() {
    assert_eq!(
        compare_f32(&[], &[1.0], T1).unwrap_err(),
        CompareError::LengthMismatch {
            reference: 0,
            actual: 1
        }
    );
}
