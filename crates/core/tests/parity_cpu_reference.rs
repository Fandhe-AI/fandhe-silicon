//! CPU 参照（`reference_gemm_f32`）の振る舞いの確認（spec TASK-24 の 3/8）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::cpu_reference::{ReferenceError, reference_gemm_f32};

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn small_integer_matrices_match_hand_computed_result() {
    // [1 2 3; 4 5 6] * [7 8; 9 10; 11 12]
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let c = reference_gemm_f32(&a, &b, 2, 3, 2).unwrap();
    assert_eq!(bits(&c), bits(&[58.0, 64.0, 139.0, 154.0]));
}

#[test]
fn accumulates_with_fused_multiply_add_in_ascending_k() {
    // x = 1 + 2^-12 のとき x*x = 1 + 2^-11 + 2^-24。融合なしの積は 2^-24 を丸めて落とし、
    // 融合ありは -(1 + 2^-11) を足した残りの 2^-24 を保つ。
    let e = 2.0_f32.powi(-12);
    let x = 1.0 + e;
    let neg = -(1.0 + 2.0 * e);
    let a = [1.0, x];
    let b = [neg, x];
    let c = reference_gemm_f32(&a, &b, 1, 2, 1).unwrap();
    let expected = x.mul_add(x, 1.0_f32.mul_add(neg, 0.0));
    assert_eq!(bits(&c), bits(&[expected]));
    assert_eq!(c[0], 2.0_f32.powi(-24));
    // 融合なしの手順では 0 になる（このテストが積み方を区別できることの確認）。
    let unfused = x * x + neg;
    assert_ne!(c[0].to_bits(), unfused.to_bits());
}

#[test]
fn length_mismatch_names_the_offending_input() {
    let err = reference_gemm_f32(&[1.0; 5], &[1.0; 6], 2, 3, 2).unwrap_err();
    assert_eq!(
        err,
        ReferenceError::LengthMismatch {
            input: "a",
            expected: 6,
            actual: 5
        }
    );
    let err = reference_gemm_f32(&[1.0; 6], &[1.0; 7], 2, 3, 2).unwrap_err();
    assert_eq!(
        err,
        ReferenceError::LengthMismatch {
            input: "b",
            expected: 6,
            actual: 7
        }
    );
}

#[test]
fn dimension_overflow_is_an_error_without_allocation() {
    let err = reference_gemm_f32(&[], &[], usize::MAX, 2, 1).unwrap_err();
    assert_eq!(err, ReferenceError::SizeOverflow);
}

#[test]
fn error_display_is_non_empty_ascii() {
    let errors = [
        ReferenceError::SizeOverflow,
        ReferenceError::AllocationFailed,
        ReferenceError::LengthMismatch {
            input: "a",
            expected: 1,
            actual: 2,
        },
    ];
    for e in errors {
        let msg = e.to_string();
        assert!(!msg.is_empty() && msg.is_ascii());
    }
}

#[test]
fn every_dimension_product_is_checked_for_overflow() {
    for (m, k, n) in [(usize::MAX, 2, 1), (1, usize::MAX, 2), (usize::MAX, 0, 2)] {
        assert_eq!(
            reference_gemm_f32(&[], &[], m, k, n).unwrap_err(),
            ReferenceError::SizeOverflow,
            "({m}, {k}, {n})"
        );
    }
}

#[test]
fn oversized_output_is_an_allocation_error() {
    // k == 0 なので入力は空でよく、出力の容量の溢れで確保前に失敗する。
    let m = isize::MAX as usize;
    assert_eq!(
        reference_gemm_f32(&[], &[], m, 0, 1).unwrap_err(),
        ReferenceError::AllocationFailed
    );
}

#[test]
fn zero_dimensions_give_zeros_or_empty() {
    let c = reference_gemm_f32(&[], &[], 2, 0, 3).unwrap();
    assert_eq!(bits(&c), vec![0.0_f32.to_bits(); 6]);
    assert!(
        reference_gemm_f32(&[], &[1.0; 3], 0, 1, 3)
            .unwrap()
            .is_empty()
    );
    assert!(
        reference_gemm_f32(&[1.0; 3], &[], 3, 1, 0)
            .unwrap()
            .is_empty()
    );
}
