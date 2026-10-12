//! 宣言 → 実行の可否 → ラベル → 集計 → 表を 1 本でつなぐ経路の確認（spec TASK-23 の 7/8）。
//! 背後実装ごとの宣言を `MachineCombination::ALL` から作り、判定は `parity` の結果を `Verdict` へ写して渡す。
//! プロセスの環境と作業ツリーは書き換えず、設定は `RunSetting::from_lookup` へ渡す。
//! 置き場と範囲の説明は docs/test-frame.md の「テストの現状（7/8）」。

use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};
use fandhe_silicon_core::test_frame::{
    Backend, COMBINATION_ENV, Decision, Declaration, IndeterminateReason, Label, LabelKind,
    MachineCombination, NotRunReason, Outcome, RunResult, RunSetting, Subject, Summary, Verdict,
    decide, judge,
};
use std::cell::Cell;
use std::ffi::OsString;

fn setting(comb: Option<&str>) -> RunSetting {
    let comb = comb.map(OsString::from);
    RunSetting::from_lookup(move |n| {
        if n == COMBINATION_ENV {
            comb.clone()
        } else {
            None
        }
    })
}

fn declared_for(backend: Backend) -> Declaration {
    let cs: Vec<MachineCombination> = MachineCombination::ALL
        .into_iter()
        .filter(|c| c.backend() == backend)
        .collect();
    Declaration::from_parts(&cs, &[]).unwrap()
}

const BACKENDS: [Backend; 3] = [Backend::Metal, Backend::Vulkan, Backend::Cuda];

fn subject(id: &str, thresholds: &[&str]) -> Subject {
    Subject::from_parts(id, &["MEM-1"], thresholds).unwrap()
}

/// `parity` の比較結果を `Verdict` へ写した本体の結果。`same` が偽なら不一致になる入力を比べる。
fn parity_result(same: bool) -> RunResult {
    let reference = [1.0f32, 2.0, 3.0];
    let actual = if same {
        [1.0f32, 2.0, 3.0]
    } else {
        [1.0f32, 2.0, 9.0]
    };
    RunResult::Verdict(Verdict::from(
        &compare_f32(&reference, &actual, T1).unwrap(),
    ))
}

/// 3 つの背後実装の試験を同じ設定・同じ結果で流した `Outcome`。
fn run_all_backends(comb: Option<&str>, same: bool) -> Vec<Outcome> {
    BACKENDS
        .iter()
        .map(|b| {
            judge(
                &subject("flow", &[]),
                &declared_for(*b),
                &setting(comb),
                |_| parity_result(same),
            )
        })
        .collect()
}

#[test]
fn each_backend_declares_its_combinations_and_runs_only_there() {
    for backend in BACKENDS {
        let d = declared_for(backend);
        assert!(!d.combinations().is_empty());
        for c in MachineCombination::ALL {
            let seen = Cell::new(None);
            let o = judge(&subject("flow", &[]), &d, &setting(Some(c.name())), |x| {
                seen.set(Some(x));
                parity_result(true)
            });
            if c.backend() == backend {
                assert_eq!(seen.get(), Some(c));
                assert_eq!(o.ran_on(), Some(c));
                assert_eq!(o.label(), &Label::Ok);
            } else {
                assert_eq!(seen.get(), None);
                assert_eq!(o.ran_on(), None);
                assert_eq!(
                    o.label(),
                    &Label::NotRun(NotRunReason::CombinationNotDeclared)
                );
            }
        }
    }
}

#[test]
fn one_flow_yields_all_five_label_kinds() {
    let d = declared_for(Backend::Vulkan);
    let at = setting(Some("vulkan-amd"));
    let ok = judge(&subject("f_ok", &[]), &d, &at, |_| parity_result(true));
    let ng = judge(&subject("f_ng", &[]), &d, &at, |_| parity_result(false));
    let pending = judge(&subject("f_pending", &["t-1"]), &d, &at, |_| {
        parity_result(true)
    });
    let not_run = judge(&subject("f_nr", &[]), &d, &setting(None), |_| {
        parity_result(true)
    });
    let indet = judge(&subject("f_ind", &[]), &d, &at, |_| {
        RunResult::Indeterminate(IndeterminateReason::DeviceMismatch)
    });
    let all = [&ok, &ng, &pending, &not_run, &indet];
    let kinds: Vec<LabelKind> = all.iter().map(|o| o.label().kind()).collect();
    assert_eq!(kinds, LabelKind::ALL);
    let names: Vec<&str> = all.iter().map(|o| o.label().name()).collect();
    assert_eq!(names, ["ok", "ng", "pending", "not-run", "indeterminate"]);
    assert_eq!(not_run.label().reason_name(), Some("not-configured"));
    assert_eq!(indet.label().reason_name(), Some("device-mismatch"));
    assert_eq!(ok.label().reason_name(), None);
    for o in [&ok, &pending, &not_run, &indet] {
        assert!(!o.label().fails_test());
    }
    assert!(ng.label().fails_test());
}

#[test]
fn unconfigured_run_counts_every_declared_row_as_not_run() {
    let outcomes = run_all_backends(None, true);
    let s = Summary::from_outcomes(&outcomes).unwrap();
    let mut expected = String::from(
        "| combination | ok | ng | pending | not-run | indeterminate |\n| --- | ---: | ---: | ---: | ---: | ---: |",
    );
    for c in MachineCombination::ALL {
        let row = s.combination(c);
        assert_eq!(row.get(LabelKind::NotRun), 1);
        assert_eq!(row.total(), 1);
        expected.push_str(&format!("\n| {} | 0 | 0 | 0 | 1 | 0 |", c.name()));
    }
    assert_eq!(s.totals().get(LabelKind::NotRun), 3);
    assert_eq!(s.totals().total(), 3);
    assert!(!s.has_ng());
    assert_eq!(s.to_string(), expected);
}

const TABLE_A: &str = "\
| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-b | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-a | 0 | 1 | 0 | 0 | 0 |
| vulkan-amd | 0 | 0 | 0 | 1 | 0 |
| vulkan-intel | 0 | 0 | 0 | 1 | 0 |
| cuda-node-b | 0 | 0 | 0 | 1 | 0 |
| cuda-node-a | 0 | 0 | 0 | 1 | 0 |";

#[test]
fn run_on_one_combination_gives_the_fixed_table() {
    // 設定は vulkan-node-a。Vulkan の試験だけが走り、不一致なので NG。残る 2 つは宣言の外で未実施。
    let outcomes = run_all_backends(Some("vulkan-node-a"), false);
    let s = Summary::from_outcomes(&outcomes).unwrap();
    assert_eq!(s.to_string(), TABLE_A);
    assert_eq!(s.totals().get(LabelKind::Ng), 1);
    assert_eq!(s.totals().get(LabelKind::NotRun), 2);
    assert_eq!(s.totals().total(), 3);
    assert!(s.has_ng());
}

#[test]
fn seven_runs_give_one_ok_and_six_not_run_per_row() {
    let mut outcomes = Vec::new();
    for c in MachineCombination::ALL {
        outcomes.extend(run_all_backends(Some(c.name()), true));
    }
    assert_eq!(outcomes.len(), 21);
    let s = Summary::from_outcomes(&outcomes).unwrap();
    for c in MachineCombination::ALL {
        let row = s.combination(c);
        assert_eq!(row.get(LabelKind::Ok), 1);
        assert_eq!(row.get(LabelKind::NotRun), 6);
        assert_eq!(row.total(), 7);
    }
    assert_eq!(s.totals().get(LabelKind::Ok), 7);
    assert_eq!(s.totals().get(LabelKind::NotRun), 14);
    assert!(!s.has_ng());
}

#[test]
fn has_ng_agrees_with_fails_test_over_the_outcomes() {
    let scenes = [
        (run_all_backends(None, false), false),
        (run_all_backends(Some("cuda-node-a"), true), false),
        (run_all_backends(Some("cuda-node-b"), false), true),
        (run_all_backends(Some("metal-mac-mini"), false), true),
    ];
    for (outcomes, want) in &scenes {
        let s = Summary::from_outcomes(outcomes).unwrap();
        assert_eq!(s.has_ng(), outcomes.iter().any(|o| o.label().fails_test()));
        assert_eq!(s.has_ng(), *want);
    }
}

#[test]
fn flow_with_the_process_environment_never_yields_ng() {
    // 環境は書き換えない。値が何であれ、本体が一致を返す限りラベルは ok か not-run になり、可否と整合する。
    let d = Declaration::from_parts(&MachineCombination::ALL, &[]).unwrap();
    let st = RunSetting::from_env();
    let o = judge(&subject("env_flow", &[]), &d, &st, |_| parity_result(true));
    match decide(&d, &st) {
        Decision::Run(c) => {
            assert_eq!(o.label(), &Label::Ok);
            assert_eq!(o.ran_on(), Some(c));
        }
        Decision::NotRun(r) => {
            assert_eq!(o.label(), &Label::NotRun(r));
            assert_eq!(o.ran_on(), None);
        }
    }
}
