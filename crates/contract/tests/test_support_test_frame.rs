//! contract の feature `test-support` だけで、core の試験の枠（宣言と実行の可否、結果のラベルの決定、組み合わせごとの集計）に届くことの確認（spec TASK-23 の 3/8・5/8・6/8）。
//! `make build-profiles` の `test-support` を含む各組で、背後実装の feature に依らず同じ結果になることも確かめる。
//! 別 crate からのマクロ・端から端の表・背後実装の feature ごとの宣言も確かめる（7/8）。
//! 置き場・feature の説明は docs/test-frame.md の「現状の形」。

use fandhe_silicon_core::test_frame::{
    Backend, COMBINATION_ENV, Decision, Declaration, MachineCombination, NotRunReason, RunResult,
    RunSetting, decide,
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

#[test]
fn summary_is_reachable_and_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{LabelKind, RunResult, Subject, Summary, Verdict, judge};
    let s = Subject::from_parts("summary_reach", &["BUILD-54"], &[]).unwrap();
    let d = Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeA,
        ],
        &[],
    )
    .unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    let o = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Mismatch));
    let sum = Summary::from_outcomes(&[o]).unwrap();
    let metal = sum.combination(MachineCombination::MetalMacMini);
    assert_eq!(metal.get(LabelKind::Ng), 1);
    let cuda = sum.combination(MachineCombination::CudaNodeA);
    assert_eq!(cuda.get(LabelKind::NotRun), 1);
    assert!(sum.has_ng());
    assert_eq!(sum.totals().total(), 1);
}

#[test]
fn declaration_macro_is_reachable_from_another_crate() {
    let via_macro = fandhe_silicon_core::declaration!(
        [MachineCombination::VulkanAmd, MachineCombination::CudaNodeA,],
        ["fault",]
    );
    let via_fn = Declaration::from_parts(
        &[MachineCombination::VulkanAmd, MachineCombination::CudaNodeA],
        &["fault"],
    );
    assert!(via_macro.is_ok());
    assert_eq!(via_macro, via_fn);
}

#[test]
fn end_to_end_table_is_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{Subject, Summary, Verdict, judge};
    let s = Subject::from_parts("e2e_reach", &["BUILD-54"], &[]).unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("vulkan-node-a"))
    });
    let mut outcomes = Vec::new();
    for backend in [Backend::Metal, Backend::Vulkan, Backend::Cuda] {
        let cs: Vec<MachineCombination> = MachineCombination::ALL
            .into_iter()
            .filter(|c| c.backend() == backend)
            .collect();
        let d = Declaration::from_parts(&cs, &[]).unwrap();
        outcomes.push(judge(&s, &d, &st, |_| {
            RunResult::Verdict(Verdict::Mismatch)
        }));
    }
    let table = Summary::from_outcomes(&outcomes).unwrap().to_string();
    let expected = "\
| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-b | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-a | 0 | 1 | 0 | 0 | 0 |
| vulkan-amd | 0 | 0 | 0 | 1 | 0 |
| vulkan-intel | 0 | 0 | 0 | 1 | 0 |
| cuda-node-b | 0 | 0 | 0 | 1 | 0 |
| cuda-node-a | 0 | 0 | 0 | 1 | 0 |";
    assert_eq!(table, expected);
}

/// 背後実装の feature を持つ組でだけ使う補助。その背後実装の組み合わせだけを宣言し、設定がその組み合わせのときだけ走ることを確かめる。
#[cfg(any(
    feature = "backend-metal",
    feature = "backend-vulkan",
    feature = "backend-cuda"
))]
fn declares_and_runs_only_on(backend: Backend) {
    use fandhe_silicon_core::test_frame::{Label, Subject, Verdict, judge};
    let s = Subject::from_parts("backend_decl", &["BUILD-54"], &[]).unwrap();
    let cs: Vec<MachineCombination> = MachineCombination::ALL
        .into_iter()
        .filter(|c| c.backend() == backend)
        .collect();
    let d = Declaration::from_parts(&cs, &[]).unwrap();
    for c in MachineCombination::ALL {
        let st =
            RunSetting::from_lookup(|n| (n == COMBINATION_ENV).then(|| OsString::from(c.name())));
        let o = judge(&s, &d, &st, |_| RunResult::Verdict(Verdict::Match));
        if c.backend() == backend {
            assert_eq!(o.label(), &Label::Ok);
            assert_eq!(o.ran_on(), Some(c));
        } else {
            assert_eq!(o.ran_on(), None);
            assert_eq!(
                o.label(),
                &Label::NotRun(NotRunReason::CombinationNotDeclared)
            );
        }
    }
}

#[cfg(feature = "backend-metal")]
#[test]
fn metal_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Metal);
}

#[cfg(feature = "backend-vulkan")]
#[test]
fn vulkan_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Vulkan);
}

#[cfg(feature = "backend-cuda")]
#[test]
fn cuda_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Cuda);
}
