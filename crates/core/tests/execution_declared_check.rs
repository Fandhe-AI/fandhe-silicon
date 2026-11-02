//! `execution` の C1 の検査を外の crate から公開のパスで呼ぶ統合テスト（TASK-420 のポインタ）。
//!
//! 宣言の件数が `BindingLayout` の `slots` の件数と違う入力が `InvalidArgument`（文言の先頭 `C1`）で拒否され、
//! 件数が一致する入力は通ることを確かめる。feature なしで動く。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    BindingLayout, DType, OperandSpec, validate_declared_operands,
};

/// `shape` と `stride` を受けて宣言を 1 件作る。dtype と alignment は検査に影響しない固定値。
fn spec<'a>(slot: u32, shape: &'a [u64], stride: &'a [i64]) -> OperandSpec<'a> {
    OperandSpec {
        slot,
        dtype: DType::F32,
        shape,
        stride,
        alignment: 0,
    }
}

/// 結果が `InvalidArgument` で、文言の先頭が `name` かを返す（`ContractError` は `#[non_exhaustive]` なので `matches!` で見る）。
fn is_rejected_by(r: &Result<(), ContractError>, name: &str) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(name))
}

/// 宣言 1 件に対し `slots` が 2 件だと C1 で拒否される（BIND-33）。
#[test]
fn c1_binding_count_mismatch_is_rejected() {
    let d = [spec(0, &[4], &[1])];
    let r = validate_declared_operands(&d, &BindingLayout::new(vec![0, 1]));
    assert!(is_rejected_by(&r, "C1"), "{r:?}");
}

/// 宣言 2 件と `slots` 2 件は件数が一致するので通る（BIND-33）。
#[test]
fn c1_binding_count_match_is_accepted() {
    let d = [spec(0, &[4], &[1]), spec(1, &[4], &[1])];
    let r = validate_declared_operands(&d, &BindingLayout::new(vec![0, 1]));
    assert!(r.is_ok(), "{r:?}");
}
