//! GEMM の門の run の CPU 参照の振る舞いの確認（spec TASK-1005 の 2/6）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::binary16::{
    binary16_bits_to_f32, f32_to_binary16_bits, widen_binary16_slice,
};
use fandhe_silicon_core::parity::cpu_reference::{
    ReferenceError, reference_gemm_f16acc, reference_gemm_f32,
};
use fandhe_silicon_core::parity::gemm_inputs::{
    GemmGateRow, GemmInputDtype, generate_f32, reference_for,
};
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn f16acc_hand_computed() {
    // [1 2 3; 4 5 6] * [7 8; 9 10; 11 12] は binary16 で正確に表せる整数だけ。
    let a: Vec<u16> = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        .iter()
        .map(|&x| f32_to_binary16_bits(x))
        .collect();
    let b: Vec<u16> = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0]
        .iter()
        .map(|&x| f32_to_binary16_bits(x))
        .collect();
    let c = reference_gemm_f16acc(&a, &b, 2, 3, 2).unwrap();
    assert_eq!(bits(&c), bits(&[58.0, 64.0, 139.0, 154.0]));
}

#[test]
fn f16acc_equals_widen_then_f32() {
    let row = GemmGateRow::Odd37x41x53;
    let inputs = generate_f32(row).unwrap();
    let half = inputs.to_binary16().unwrap();
    let got = reference_gemm_f16acc(half.a(), half.b(), row.m(), row.k(), row.n()).unwrap();
    let a = widen_binary16_slice(half.a()).unwrap();
    let b = widen_binary16_slice(half.b()).unwrap();
    let want = reference_gemm_f32(&a, &b, row.m(), row.k(), row.n()).unwrap();
    assert_eq!(bits(&got), bits(&want));
}

#[test]
fn f16acc_errors_match_f32_shape() {
    assert_eq!(
        reference_gemm_f16acc(&[0; 5], &[0; 6], 2, 3, 2),
        Err(ReferenceError::LengthMismatch {
            input: "a",
            expected: 6,
            actual: 5
        })
    );
    assert_eq!(
        reference_gemm_f16acc(&[0; 6], &[0; 7], 2, 3, 2),
        Err(ReferenceError::LengthMismatch {
            input: "b",
            expected: 6,
            actual: 7
        })
    );
    assert_eq!(
        reference_gemm_f16acc(&[], &[], usize::MAX, 2, 1),
        Err(ReferenceError::SizeOverflow)
    );
}

#[test]
fn reference_for_covers_both_dtypes() {
    for row in [GemmGateRow::Odd37x41x53, GemmGateRow::Sq512] {
        let inputs = generate_f32(row).unwrap();
        let f32_ref = reference_for(row, GemmInputDtype::F32, &inputs).unwrap();
        let f16_ref = reference_for(row, GemmInputDtype::F16Acc, &inputs).unwrap();
        assert_eq!(f32_ref.values().len(), row.m() * row.n());
        assert_eq!(f16_ref.values().len(), row.m() * row.n());
        assert!(f32_ref.values().iter().all(|x| x.is_finite()));

        // f32 の参照は reference_gemm_f32 そのもの。
        if row == GemmGateRow::Odd37x41x53 {
            let direct =
                reference_gemm_f32(inputs.a(), inputs.b(), row.m(), row.k(), row.n()).unwrap();
            assert_eq!(bits(f32_ref.values()), bits(&direct));
        }

        // 丸めは 1 回だけ。
        for r in [&f32_ref, &f16_ref] {
            let rounded = r.rounded_to_binary16().unwrap();
            assert_eq!(rounded.len(), r.values().len());
            for (v, b) in r.values().iter().zip(&rounded) {
                assert_eq!(*b, f32_to_binary16_bits(*v));
            }
            let widened = r.rounded_to_binary16_widened().unwrap();
            for (b, w) in rounded.iter().zip(&widened) {
                assert_eq!(w.to_bits(), binary16_bits_to_f32(*b).to_bits());
            }
        }
    }
}

#[test]
fn reference_connects_to_the_judge() {
    let row = GemmGateRow::Odd37x41x53;
    let inputs = generate_f32(row).unwrap();
    for dtype in [GemmInputDtype::F32, GemmInputDtype::F16Acc] {
        let reference = reference_for(row, dtype, &inputs).unwrap();
        let mut actual = reference.values().to_vec();
        let ok = compare_f32(reference.values(), &actual, T1).unwrap();
        assert!(ok.is_match());
        actual[0] += 1.0;
        let bad = compare_f32(reference.values(), &actual, T1).unwrap();
        assert!(!bad.is_match());
        assert_eq!(bad.first_mismatch(), Some(0));
    }
}

#[test]
fn binary16_output_arm_compares_widened_columns() {
    let row = GemmGateRow::Odd37x41x53;
    let inputs = generate_f32(row).unwrap();
    let reference = reference_for(row, GemmInputDtype::F16Acc, &inputs).unwrap();
    let rounded = reference.rounded_to_binary16().unwrap();
    let widened = widen_binary16_slice(&rounded).unwrap();
    let expected = reference.rounded_to_binary16_widened().unwrap();
    assert_eq!(bits(&widened), bits(&expected));
    assert!(compare_f32(&expected, &widened, T1).unwrap().is_match());
}
