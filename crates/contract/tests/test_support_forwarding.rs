//! contract の feature `test-support` だけで、core の検証用の道具に届くことの確認（spec TASK-24 の 3/8）。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;

#[test]
fn contract_test_support_enables_core_parity_tool() {
    let c = reference_gemm_f32(&[2.0], &[3.0], 1, 1, 1).unwrap();
    assert_eq!(c[0].to_bits(), 6.0_f32.to_bits());
}
