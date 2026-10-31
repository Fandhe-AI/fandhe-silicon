//! `first_instance_decision` の入力と判定の対応を格子で確かめ、`FirstInstanceDecision` が閉じた 2 値であることを固定する（TASK-1878 のポインタ）。
//!
//! `_` の腕なしの `match` が通るので、値が増える・`#[non_exhaustive]` が付くとコンパイルで落ちる。

use fandhe_silicon_core::execution::{FirstInstanceDecision, first_instance_decision};

fn expected(enabled: bool, first_instance: u32) -> FirstInstanceDecision {
    if first_instance == 0 || enabled {
        FirstInstanceDecision::Apply
    } else {
        FirstInstanceDecision::Reject
    }
}

#[test]
fn boundary_values_match_reference() {
    for enabled in [false, true] {
        for fi in [0u32, 1, 2, 4, u32::MAX - 1, u32::MAX] {
            assert_eq!(
                first_instance_decision(enabled, fi),
                expected(enabled, fi),
                "(enabled={enabled}, first_instance={fi})"
            );
        }
    }
}

#[test]
fn named_claims() {
    assert_eq!(
        first_instance_decision(false, 0),
        FirstInstanceDecision::Apply
    );
    assert_eq!(
        first_instance_decision(true, 0),
        FirstInstanceDecision::Apply
    );
    for fi in [1u32, 2, u32::MAX] {
        assert_eq!(
            first_instance_decision(false, fi),
            FirstInstanceDecision::Reject
        );
        assert_eq!(
            first_instance_decision(true, fi),
            FirstInstanceDecision::Apply
        );
    }
}

#[test]
fn stride_sweep_over_u32_matches_reference() {
    let mut count = 0u32;
    let mut v = 0u64;
    while v <= u64::from(u32::MAX) {
        let fi = u32::try_from(v).expect("v is within u32");
        for enabled in [false, true] {
            assert_eq!(
                first_instance_decision(enabled, fi),
                expected(enabled, fi),
                "(enabled={enabled}, first_instance={fi})"
            );
        }
        count += 1;
        v += 65_537;
    }
    assert!(count > 60_000, "sweep too small: {count}");
}

fn name(d: FirstInstanceDecision) -> &'static str {
    match d {
        FirstInstanceDecision::Apply => "apply",
        FirstInstanceDecision::Reject => "reject",
    }
}

#[test]
fn decision_is_closed_two_values() {
    let names = [
        name(FirstInstanceDecision::Apply),
        name(FirstInstanceDecision::Reject),
    ];
    assert_eq!(names, ["apply", "reject"]);
}
