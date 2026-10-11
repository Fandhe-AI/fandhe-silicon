//! contract の feature `test-support` だけで、core の検証用の道具に届くことの確認（spec TASK-24 の 3/8）。
//! あわせて、背後実装の feature の組に依らず道具の結果が同じになることも確かめる（TASK-24 の 7/8）。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};
use fandhe_silicon_core::parity::topk::{compare_topk, reference_topk_f32};

#[test]
fn contract_test_support_enables_core_parity_tool() {
    let c = reference_gemm_f32(&[2.0], &[3.0], 1, 1, 1).unwrap();
    assert_eq!(c[0].to_bits(), 6.0_f32.to_bits());
}

#[test]
fn threshold_setting_is_readable_from_outside_core() {
    assert!(T1.relative().is_finite() && T1.relative() > 0.0);
    assert!(T1.absolute().is_finite() && T1.absolute() > 0.0);
}

#[test]
fn parity_pipeline_gives_fixed_results_under_any_backend_feature_set() {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let c = reference_gemm_f32(&a, &b, 2, 3, 2).unwrap();
    let bits: Vec<u32> = c.iter().map(|x| x.to_bits()).collect();
    let want = [58.0_f32, 64.0, 139.0, 154.0];
    assert_eq!(bits, want.map(f32::to_bits));
    assert!(compare_f32(&want, &c, T1).unwrap().is_match());

    let top = reference_topk_f32(&c, 2).unwrap();
    let slots: Vec<u32> = top.iter().map(|t| t.slot()).collect();
    assert_eq!(slots, vec![3, 2]);
    assert!(compare_topk(&top, &top).unwrap().is_match());
}
