//! contract の feature `test-support` だけで、core の試験の枠（宣言と実行の可否、結果のラベルの決定）に届くことの確認（spec TASK-23 の 3/8・5/8）。
//! `make build-profiles` の `test-support` を含む各組で、背後実装の feature に依らず同じ結果になることも確かめる。
//! 置き場・feature の説明は docs/test-frame.md の「現状の形」。

use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Decision, Declaration, MachineCombination, NotRunReason, RunSetting, decide,
};
use std::ffi::OsString;

#[test]
fn contract_test_support_enables_core_test_frame() {
    let d = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    assert_eq!(
        decide(&d, &st),
        Decision::Run(MachineCombination::MetalMacMini)
    );
}

#[test]
fn decisions_are_fixed_under_any_backend_feature_set() {
    let d = Declaration::from_parts(&[MachineCombination::CudaNodeA], &["x"]).unwrap();
    let unset = RunSetting::from_lookup(|_| None);
    assert_eq!(
        decide(&d, &unset),
        Decision::NotRun(NotRunReason::NotConfigured)
    );
    let other =
        RunSetting::from_lookup(|n| (n == COMBINATION_ENV).then(|| OsString::from("vulkan-amd")));
    assert_eq!(
        decide(&d, &other),
        Decision::NotRun(NotRunReason::CombinationNotDeclared)
    );
}

#[test]
fn judge_is_reachable_and_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{Label, RunResult, Subject, Verdict, judge};
    let s = Subject::from_parts("reach_check", &["BUILD-54"], &[]).unwrap();
    let d = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    let ok = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Match));
    assert_eq!(ok.label(), &Label::Ok);
    let ng = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Mismatch));
    assert_eq!(ng.label(), &Label::Ng);
    let unset = RunSetting::from_lookup(|_| None);
    let nr = judge(&s, &d, &unset, |_| RunResult::Verdict(Verdict::Match));
    assert_eq!(nr.label(), &Label::NotRun(NotRunReason::NotConfigured));
}
