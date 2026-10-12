//! contract の feature `test-support` だけで、core の有限でない値と符号付きの零を扱う比べ方に届くことの確認（spec TASK-25 の 3/4）。
//! `make build-profiles` の `test-support` を含む各組で、背後実装の feature に依らず同じ結果になることも確かめる。
//! 置き場・feature の説明は docs/parity-non-finite.md の「コードの現状の形」と docs/parity-tool.md の「現状の形」。

use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, compare_f32_with_special_values, judge_element_with_special_values,
};
use fandhe_silicon_core::parity::tolerance::{CompareError, T1};

use SpecialVerdict::{BothNan, Exceeded, NonFiniteMismatch, SameInfinity, Within};

#[test]
fn contract_test_support_enables_core_special_values_tool() {
    assert_eq!(
        judge_element_with_special_values(f32::NAN, -f32::NAN, T1),
        BothNan
    );
}

#[test]
fn special_value_verdicts_are_fixed_under_any_backend_feature_set() {
    let r = [f32::NAN, f32::INFINITY, 1.0, 1.0, 0.0, 2.0];
    let a = [-f32::NAN, f32::INFINITY, 1.0, 5.0, -0.0, f32::NEG_INFINITY];
    let got: Vec<SpecialVerdict> = r
        .iter()
        .zip(&a)
        .map(|(&x, &y)| judge_element_with_special_values(x, y, T1))
        .collect();
    assert_eq!(
        got,
        [
            BothNan,
            SameInfinity,
            Within,
            Exceeded,
            Within,
            NonFiniteMismatch
        ]
    );
}

#[test]
fn special_value_sequence_summary_is_fixed_under_any_backend_feature_set() {
    let r = [f32::NAN, 1.0, 1.0, f32::INFINITY];
    let a = [f32::NAN, f32::INFINITY, 5.0, f32::INFINITY];
    let c = compare_f32_with_special_values(&r, &a, T1).unwrap();
    assert_eq!(
        (
            c.compared(),
            c.exceeded(),
            c.non_finite_mismatch(),
            c.first_mismatch(),
            c.is_match()
        ),
        (4, 1, 1, Some(1), false)
    );
}

#[test]
fn special_value_sequence_errors_are_reachable_from_outside_core() {
    assert_eq!(
        compare_f32_with_special_values(&[], &[1.0], T1),
        Err(CompareError::LengthMismatch {
            reference: 0,
            actual: 1
        })
    );
    assert_eq!(
        compare_f32_with_special_values(&[], &[], T1),
        Err(CompareError::EmptyInput)
    );
}
