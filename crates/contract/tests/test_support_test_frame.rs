//! contract の feature `test-support` だけで、core の試験の枠（宣言と実行の可否、結果のラベルの決定、組み合わせごとの集計）に届くことの確認（spec TASK-23）。
//! `make build-profiles` の `test-support` を含む各組で、背後実装の feature に依らず同じ結果になることも確かめる。
//! 別 crate からのマクロ・端から端の表・背後実装の feature ごとの宣言も確かめる。
//! 置き場・feature の説明は docs/test-frame.md の「置き場と有効になる条件」。
//! 背後実装の feature ごとの試験は、結果の決定のあとに記録の部品を書き出す入口（`judge_and_export`）を通す。
//! 環境変数 `FANDHE_SILICON_TEST_RECORD_DIR` が未設定なら何も書かず、設定されていれば（`make verify` が渡す）、
//! 機材の無い環境では宣言した組み合わせごとの「未実施」の部品が書かれる。

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

/// 書き出しつきの入口を通す試験の ID（背後実装ごとに 1 つ）。
///
/// 部品のファイル名は試験の ID と組み合わせの名前で決まり、既にあるファイルは上書きされない。
/// 同じ ID を workspace の別の試験が使うと、同じディレクトリへの 2 回目の書き出しが `AlreadyExists` で失敗し、
/// その試験が落ちる（`make verify` は毎回新しいディレクトリを渡すので、重複はそこで見つかる）。
/// このファイルの中での重複は、下の `export_test_ids_are_distinct` が確かめる。
const METAL_EXPORT_ID: &str = "contract_metal_declaration";
const VULKAN_EXPORT_ID: &str = "contract_vulkan_declaration";
const CUDA_EXPORT_ID: &str = "contract_cuda_declaration";

/// 背後実装の feature を持つ組でだけ使う補助。その背後実装の組み合わせだけを宣言して、書き出しつきの入口を 1 回通す。
///
/// 実行の組み合わせはプロセスの環境から決まる。機材の無い環境では宣言した組み合わせごとの「未実施」になり、
/// 宣言した組み合わせで走ったときは、本体が渡された組み合わせの背後実装を確かめて結果を返す。
/// 書き出しの失敗は握りつぶさず試験の失敗にし、NG も試験の失敗にする（未実施・判定保留は失敗にしない）。
#[cfg(any(
    feature = "backend-metal",
    feature = "backend-vulkan",
    feature = "backend-cuda"
))]
fn declares_and_exports(test_id: &str, backend: Backend) {
    use fandhe_silicon_core::test_frame::{Subject, Verdict, judge_and_export};
    let s = Subject::from_parts(test_id, &["BUILD-54"], &[]).unwrap();
    let cs: Vec<MachineCombination> = MachineCombination::ALL
        .into_iter()
        .filter(|c| c.backend() == backend)
        .collect();
    let d = Declaration::from_parts(&cs, &[]).unwrap();
    let outcome = judge_and_export(&s, &d, &RunSetting::from_env(), |c| {
        RunResult::Verdict(Verdict::from_match(c.backend() == backend))
    })
    .expect("exporting the record parts must succeed");
    assert!(!outcome.label().fails_test());
}

#[cfg(feature = "backend-metal")]
#[test]
fn metal_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Metal);
    declares_and_exports(METAL_EXPORT_ID, Backend::Metal);
}

#[cfg(feature = "backend-vulkan")]
#[test]
fn vulkan_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Vulkan);
    declares_and_exports(VULKAN_EXPORT_ID, Backend::Vulkan);
}

#[cfg(feature = "backend-cuda")]
#[test]
fn cuda_test_declares_on_the_frame() {
    declares_and_runs_only_on(Backend::Cuda);
    declares_and_exports(CUDA_EXPORT_ID, Backend::Cuda);
}

#[test]
fn export_test_ids_are_distinct() {
    let ids = [METAL_EXPORT_ID, VULKAN_EXPORT_ID, CUDA_EXPORT_ID];
    for (i, a) in ids.iter().enumerate() {
        assert!(fandhe_silicon_core::test_frame::TestId::new(a).is_ok());
        for b in &ids[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn record_parts_are_reachable_and_fixed_under_any_backend_feature_set() {
    use fandhe_silicon_core::test_frame::{Subject, Verdict, judge};
    let s = Subject::from_parts("record_reach", &["BUILD-54"], &[]).unwrap();
    let d = Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeA,
        ],
        &[],
    )
    .unwrap();
    let unset = RunSetting::from_lookup(|_| None);
    let o = judge(&s, &d, &unset, |_| RunResult::Verdict(Verdict::Match));
    let parts = o.record_parts();
    assert_eq!(parts.len(), 2);
    assert_eq!(
        parts[1].to_json(),
        r#"{"behavior_ids":["BUILD-54"],"machine_combination":"cuda-node-a","machine":"node-a","label":"not-run","label_reason":"not-configured","body_kind":"contract-test","body":{"test_id":"record_reach"}}"#
    );
}
