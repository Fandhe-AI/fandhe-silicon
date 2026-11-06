//! `cpu_ops` の要素ごと演算に NaN・±Inf・±0 を含む入力を当てる外の試験（TASK-2456 のポインタ）。
//! feature `cpu-ops` と `test-support` が有効な組でだけ走る。入力は `CaseId::E2`（TASK-977 のポインタ）、
//! 比べは `compare_f32_with_special_values`（TASK-25 のポインタ）。

use fandhe_silicon_contract::cpu_ops::{
    CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp,
};
use fandhe_silicon_core::parity::case_inputs::{CaseId, CaseInput};
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::T1;

/// 出力を埋める番兵のビット列。
const SENTINEL_BITS: u32 = 0x7fc0_beef;

/// `CaseId::E2` の入力の列。
fn input() -> Vec<f32> {
    let CaseInput::Relu(v) = CaseId::E2.input().unwrap() else {
        panic!("e-2 is not relu")
    };
    v
}

/// 出力を番兵で埋めてから `elementwise_scalar`（`simd` が偽）または `elementwise_simd` を呼ぶ。
fn run<T: CpuOps>(ops: &T, simd: bool, op: UnaryElementwiseOp, a: &[f32]) -> Vec<f32> {
    let mut out = vec![f32::from_bits(SENTINEL_BITS); a.len()];
    if simd {
        ops.elementwise_simd(op, a, &mut out).unwrap();
    } else {
        ops.elementwise_scalar(op, a, &mut out).unwrap();
    }
    out
}

/// 型 2 つ × メソッド 2 つの 4 通りの出力。
fn run_all(op: UnaryElementwiseOp, a: &[f32]) -> Vec<(&'static str, Vec<f32>)> {
    vec![
        ("default-scalar", run(&DefaultCpuOps, false, op, a)),
        ("default-simd", run(&DefaultCpuOps, true, op, a)),
        ("reference-scalar", run(&ReferenceCpuOps, false, op, a)),
        ("reference-simd", run(&ReferenceCpuOps, true, op, a)),
    ]
}

/// 入力の語の件数を NaN・+Inf・-Inf・+0・-0 の順に数える。
fn special_counts(a: &[f32]) -> [usize; 5] {
    let count = |f: fn(&f32) -> bool| a.iter().filter(|x| f(x)).count();
    [
        count(|x| x.is_nan()),
        count(|x| *x == f32::INFINITY),
        count(|x| *x == f32::NEG_INFINITY),
        count(|x| x.to_bits() == 0.0_f32.to_bits()),
        count(|x| x.to_bits() == (-0.0_f32).to_bits()),
    ]
}

#[test]
fn input_holds_special_values() {
    let a = input();
    assert_eq!(a.len(), 37);
    assert_eq!(special_counts(&a), [7, 6, 6, 6, 6]);
}

/// 2 型の `elementwise_scalar` で、入力が NaN の位置の出力が `+0.0` のビットで、出力に NaN が無い（CPU-21）。
#[test]
fn relu_scalar_maps_nan_to_positive_zero() {
    let a = input();
    for (label, out) in [
        (
            "default",
            run(&DefaultCpuOps, false, UnaryElementwiseOp::Relu, &a),
        ),
        (
            "reference",
            run(&ReferenceCpuOps, false, UnaryElementwiseOp::Relu, &a),
        ),
    ] {
        for (i, (x, y)) in a.iter().zip(&out).enumerate() {
            if x.is_nan() {
                assert_eq!(y.to_bits(), 0.0_f32.to_bits(), "{label}[{i}]");
            }
        }
        assert!(out.iter().all(|y| !y.is_nan()), "{label}");
    }
}

/// 4 通りとも、`Relu` の出力が `CaseId::E2.reference()` と規則で一致する。
#[test]
fn relu_matches_case_reference_by_rule() {
    let a = input();
    let reference = CaseId::E2.reference().unwrap();
    let want = reference.as_f32().unwrap();
    for (label, out) in run_all(UnaryElementwiseOp::Relu, &a) {
        let c = compare_f32_with_special_values(want, &out, T1).unwrap();
        assert_eq!(c.compared(), a.len(), "{label}");
        assert_eq!(c.exceeded(), 0, "{label}");
        assert_eq!(c.non_finite_mismatch(), 0, "{label}");
    }
}

/// `Relu`・`AddScalar(0.5)` の `elementwise_simd` の出力が、長さ 1〜37 の接頭辞で
/// `elementwise_scalar` の出力と規則で一致する（CPU-22）。
#[test]
fn simd_matches_scalar_by_rule() {
    let full = input();
    for op in [UnaryElementwiseOp::Relu, UnaryElementwiseOp::AddScalar(0.5)] {
        for len in 1..=full.len() {
            let a = &full[..len];
            let want = run(&DefaultCpuOps, false, op, a);
            for (label, got) in [
                ("default", run(&DefaultCpuOps, true, op, a)),
                ("reference", run(&ReferenceCpuOps, true, op, a)),
            ] {
                let c = compare_f32_with_special_values(&want, &got, T1).unwrap();
                assert_eq!(c.compared(), len, "{label} {op:?}");
                assert_eq!(c.exceeded(), 0, "{label} {op:?} len={len}");
                assert_eq!(c.non_finite_mismatch(), 0, "{label} {op:?} len={len}");
            }
        }
    }
}

/// 上の比べが、NaN を通した出力を不一致にし、`+0.0` と `-0.0` の違いを一致にする。
#[test]
fn rule_detects_nan_passed_through() {
    let a = input();
    let want = run(&DefaultCpuOps, false, UnaryElementwiseOp::Relu, &a);
    let mut passed = want.clone();
    let mut nans = 0;
    for (x, y) in a.iter().zip(&mut passed) {
        if x.is_nan() {
            *y = *x;
            nans += 1;
        }
    }
    let c = compare_f32_with_special_values(&want, &passed, T1).unwrap();
    assert_eq!(c.non_finite_mismatch(), nans);
    let flipped: Vec<f32> = want
        .iter()
        .map(|y| if *y == 0.0 { -*y } else { *y })
        .collect();
    assert!(
        compare_f32_with_special_values(&want, &flipped, T1)
            .unwrap()
            .is_match()
    );
}
