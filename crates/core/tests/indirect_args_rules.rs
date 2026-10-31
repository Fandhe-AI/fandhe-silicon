//! 間接実行の引数の範囲・整列の検査と `first_instance` の判定の各分岐を 1 件ずつ確かめる（TASK-1878 のポインタ）。
//!
//! 配置や入力の格子は別の試験が受け持つ。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    FirstInstanceDecision, first_instance_decision, validate_indirect_args_range,
};

fn message(r: Result<(), ContractError>) -> String {
    match r {
        Err(ContractError::InvalidArgument(m)) => m,
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[test]
fn range_rejects_misaligned_offset() {
    let m = message(validate_indirect_args_range(1, 16, 64));
    assert!(m.starts_with("indirect args offset"), "{m}");
}

#[test]
fn range_rejects_end_overflow() {
    let m = message(validate_indirect_args_range(u64::MAX - 3, 16, 64));
    assert!(m.starts_with("buffer range end overflows"), "{m}");
}

#[test]
fn range_rejects_end_beyond_length() {
    let m = message(validate_indirect_args_range(4, 12, 12));
    assert!(m.starts_with("buffer range end ("), "{m}");
    assert!(m.contains("exceeds"), "{m}");
}

#[test]
fn range_accepts_end_equal_to_length() {
    assert_eq!(validate_indirect_args_range(0, 20, 20), Ok(()));
}

#[test]
fn first_instance_decision_cases() {
    assert_eq!(
        first_instance_decision(false, 0),
        FirstInstanceDecision::Apply
    );
    assert_eq!(
        first_instance_decision(true, 7),
        FirstInstanceDecision::Apply
    );
    assert_eq!(
        first_instance_decision(false, 1),
        FirstInstanceDecision::Reject
    );
}
