//! 累積の順を `k` の降順にした CPU の対照の確認（spec TASK-1005 の 4/6）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。
//!
//! 対照は判定には使わない。T-1 で一致しても同じ順で積み上げた証拠にはならないことを、
//! 本リポのテスト（`t1_match_does_not_imply_same_order`）で示す。大きな行は行 × dtype ごとに
//! 別の `#[ignore]` の試験で、`--release` で 1 つずつ流す。大きな行は外れの件数を出力して記録する
//! （既定の行と違い、外れ 0 件は要求しない）。

use fandhe_silicon_core::parity::cpu_reference::{
    ReferenceError, reference_gemm_f32, reference_gemm_f32_descending_k,
};
use fandhe_silicon_core::parity::gemm_inputs::{
    GemmGateRow, GemmInputDtype, generate_f32, reference_for, reference_for_descending_k,
};
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};

#[test]
fn descending_hand_computed() {
    // a = [[1,2,3],[4,5,6]]、b = [[1,2],[3,4],[5,6]]
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let c = reference_gemm_f32_descending_k(&a, &b, 2, 3, 2).unwrap();
    assert_eq!(c, vec![22.0, 28.0, 49.0, 64.0]);
}

#[test]
fn descending_differs_from_ascending_when_order_matters() {
    // 昇順: 1 + 2^-24 は 1 に丸まり、もう一度足しても 1。降順: 2^-24 + 2^-24 = 2^-23 を先に作ってから 1 に足す。
    let e = 2.0_f32.powi(-24);
    let a = [1.0, e, e];
    let b = [1.0, 1.0, 1.0];
    let asc = reference_gemm_f32(&a, &b, 1, 3, 1).unwrap();
    let desc = reference_gemm_f32_descending_k(&a, &b, 1, 3, 1).unwrap();
    assert_eq!(asc[0].to_bits(), 1.0_f32.to_bits());
    assert_eq!(desc[0].to_bits(), (1.0_f32 + 2.0_f32.powi(-23)).to_bits());
    assert_ne!(asc[0].to_bits(), desc[0].to_bits());
}

#[test]
fn descending_errors_match_ascending() {
    let cases: [(usize, usize, usize, usize, usize); 4] = [
        (5, 6, 2, 3, 2),
        (6, 5, 2, 3, 2),
        (0, 0, usize::MAX, 2, 1),
        (0, 0, 1, usize::MAX, 2),
    ];
    for (al, bl, m, k, n) in cases {
        let a = vec![0.0_f32; al];
        let b = vec![0.0_f32; bl];
        let asc = reference_gemm_f32(&a, &b, m, k, n);
        let desc = reference_gemm_f32_descending_k(&a, &b, m, k, n);
        assert!(asc.is_err());
        assert_eq!(asc, desc);
    }
    assert!(matches!(
        reference_gemm_f32_descending_k(&[0.0; 5], &[0.0; 6], 2, 3, 2),
        Err(ReferenceError::LengthMismatch { input: "a", .. })
    ));
}

/// 行・dtype で昇順と降順を T-1 で比べ、ビット列が違う要素の数を返す。
///
/// `require_t1` が真のときだけ、外れ 0 件を要求する。偽のとき（大きな行）は外れの件数を出力するだけで、
/// 有限でない値が 0 件であることと比べた件数だけを要求する（実測の事実は `docs/gemm-test-inputs.md`）。
fn check_row(row: GemmGateRow, dtype: GemmInputDtype, require_t1: bool) -> usize {
    let inputs = generate_f32(row).unwrap();
    let asc = reference_for(row, dtype, &inputs).unwrap();
    let desc = reference_for_descending_k(row, dtype, &inputs).unwrap();
    let cmp = compare_f32(asc.values(), desc.values(), T1).unwrap();
    assert_eq!(cmp.compared(), row.m() * row.n());
    assert_eq!(cmp.non_finite(), 0);
    if require_t1 {
        assert_eq!(cmp.exceeded(), 0);
        assert!(cmp.is_match());
    }

    let differing = asc
        .values()
        .iter()
        .zip(desc.values())
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count();
    // 最大の絶対差は出力するだけで、期待値としては固定しない。
    let max_abs = asc
        .values()
        .iter()
        .zip(desc.values())
        .map(|(&x, &y)| (f64::from(x) - f64::from(y)).abs())
        .fold(0.0_f64, f64::max);
    println!(
        "{} {}: exceeded={} differing_bits={differing} max_abs_diff={max_abs:e}",
        row.name(),
        dtype.name(),
        cmp.exceeded()
    );
    differing
}

#[test]
fn descending_is_within_t1_on_default_rows() {
    for row in [GemmGateRow::Odd37x41x53, GemmGateRow::Sq512] {
        for dtype in [GemmInputDtype::F32, GemmInputDtype::F16Acc] {
            check_row(row, dtype, true);
        }
    }
}

#[test]
fn t1_match_does_not_imply_same_order() {
    let mut total = 0;
    for row in [GemmGateRow::Odd37x41x53, GemmGateRow::Sq512] {
        for dtype in [GemmInputDtype::F32, GemmInputDtype::F16Acc] {
            total += check_row(row, dtype, true);
        }
    }
    assert!(total > 0, "expected some bitwise differences");
}

macro_rules! large_row_test {
    ($name:ident, $row:expr, $dtype:expr) => {
        #[test]
        #[ignore = "large inputs; run with --release"]
        fn $name() {
            check_row($row, $dtype, false);
        }
    };
}

large_row_test!(sq2048_f32, GemmGateRow::Sq2048, GemmInputDtype::F32);
large_row_test!(sq2048_f16acc, GemmGateRow::Sq2048, GemmInputDtype::F16Acc);
large_row_test!(sq4096_f32, GemmGateRow::Sq4096, GemmInputDtype::F32);
large_row_test!(sq4096_f16acc, GemmGateRow::Sq4096, GemmInputDtype::F16Acc);
large_row_test!(
    rect4096x1024x4096_f32,
    GemmGateRow::Rect4096x1024x4096,
    GemmInputDtype::F32
);
large_row_test!(
    rect4096x1024x4096_f16acc,
    GemmGateRow::Rect4096x1024x4096,
    GemmInputDtype::F16Acc
);
