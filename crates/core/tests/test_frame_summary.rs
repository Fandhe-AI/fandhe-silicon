//! 組み合わせごとの集計（`Summary`）の振る舞いの確認（spec TASK-23 の 6/8）。
//! プロセスの環境と作業ツリーは書き換えず、設定は `RunSetting::from_lookup` へ渡す。
//! 置き場と範囲の説明は docs/test-frame.md の「現状の形（6/8）」。

use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, IndeterminateReason, LabelKind, MachineCombination, Outcome,
    PERMISSIONS_ENV, RunResult, RunSetting, Subject, Summary, SummaryError, Verdict, judge,
};
use std::ffi::OsString;

fn setting(comb: Option<&str>, perms: Option<&str>) -> RunSetting {
    let comb = comb.map(OsString::from);
    let perms = perms.map(OsString::from);
    RunSetting::from_lookup(move |n| {
        if n == COMBINATION_ENV {
            comb.clone()
        } else if n == PERMISSIONS_ENV {
            perms.clone()
        } else {
            None
        }
    })
}

fn decl(cs: &[MachineCombination], perms: &[&str]) -> Declaration {
    Declaration::from_parts(cs, perms).unwrap()
}

fn subject(id: &str, thresholds: &[&str]) -> Subject {
    Subject::from_parts(id, &["MEM-1"], thresholds).unwrap()
}

fn run(
    d: &Declaration,
    comb: Option<&str>,
    perms: Option<&str>,
    result: RunResult,
    thresholds: &[&str],
) -> Outcome {
    judge(
        &subject("t_one", thresholds),
        d,
        &setting(comb, perms),
        |_| result,
    )
}

fn matched() -> RunResult {
    RunResult::Verdict(Verdict::Match)
}

fn mismatched() -> RunResult {
    RunResult::Verdict(Verdict::Mismatch)
}

fn counts(s: &Summary, c: MachineCombination) -> [usize; 5] {
    let row = s.combination(c);
    LabelKind::ALL.map(|k| row.get(k))
}

fn two() -> Declaration {
    decl(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeB,
        ],
        &[],
    )
}

#[test]
fn empty_summary_has_seven_rows_of_zero() {
    let s = Summary::new();
    for c in MachineCombination::ALL {
        assert_eq!(counts(&s, c), [0; 5]);
        assert_eq!(s.combination(c).total(), 0);
    }
    assert_eq!(s.totals().total(), 0);
    assert!(!s.has_ng());
    assert_eq!(s, Summary::default());
}

#[test]
fn ran_outcomes_count_under_the_combination_they_ran_on() {
    let d = decl(&[MachineCombination::MetalMacMini], &[]);
    let at = Some("metal-mac-mini");
    let outcomes = [
        run(&d, at, None, matched(), &[]),
        run(&d, at, None, mismatched(), &[]),
        run(&d, at, None, matched(), &["t-999"]),
        run(
            &d,
            at,
            None,
            RunResult::Indeterminate(IndeterminateReason::DeviceMismatch),
            &[],
        ),
    ];
    let s = Summary::from_outcomes(&outcomes).unwrap();
    assert_eq!(
        counts(&s, MachineCombination::MetalMacMini),
        [1, 1, 1, 0, 1]
    );
    assert_eq!(counts(&s, MachineCombination::CudaNodeB), [0; 5]);
    assert_eq!(s.totals().get(LabelKind::Ok), 1);
    assert_eq!(s.totals().get(LabelKind::Pending), 1);
    assert_eq!(s.totals().total(), 4);
}

#[test]
fn not_run_outcomes_count_under_every_declared_combination() {
    let d = decl(&[MachineCombination::MetalMacMini], &["fault"]);
    let outcomes = [
        run(&d, None, None, matched(), &[]),
        run(&d, Some("bogus"), None, matched(), &[]),
        run(&d, Some("metal-mac-mini"), Some("!!"), matched(), &[]),
        run(&d, Some("cuda-node-a"), None, matched(), &[]),
        run(&d, Some("metal-mac-mini"), None, matched(), &[]),
    ];
    let s = Summary::from_outcomes(&outcomes).unwrap();
    assert_eq!(
        counts(&s, MachineCombination::MetalMacMini),
        [0, 0, 0, 5, 0]
    );
    assert_eq!(s.totals().get(LabelKind::NotRun), 5);
}

#[test]
fn declared_but_not_run_combinations_count_as_not_run() {
    let d = two();
    let o = run(&d, Some("metal-mac-mini"), None, matched(), &[]);
    let s = Summary::from_outcomes(&[o]).unwrap();
    assert_eq!(
        counts(&s, MachineCombination::MetalMacMini),
        [1, 0, 0, 0, 0]
    );
    assert_eq!(counts(&s, MachineCombination::CudaNodeB), [0, 0, 0, 1, 0]);
    assert_eq!(s.totals().total(), 1);
}

#[test]
fn undeclared_combinations_are_not_counted() {
    let d = two();
    let s = Summary::from_outcomes(&[run(&d, Some("metal-mac-mini"), None, mismatched(), &[])])
        .unwrap();
    for c in MachineCombination::ALL {
        if c != MachineCombination::MetalMacMini && c != MachineCombination::CudaNodeB {
            assert_eq!(counts(&s, c), [0; 5], "{}", c.name());
        }
    }
}

#[test]
fn row_total_equals_outcomes_declaring_the_combination() {
    let a = decl(&[MachineCombination::VulkanAmd], &[]);
    let b = two();
    let mut outcomes = Vec::new();
    for c in MachineCombination::ALL {
        outcomes.push(run(&a, Some(c.name()), None, matched(), &[]));
        outcomes.push(run(&b, Some(c.name()), None, mismatched(), &[]));
    }
    outcomes.push(run(&a, None, None, matched(), &[]));
    outcomes.push(run(&b, None, None, matched(), &[]));
    let s = Summary::from_outcomes(&outcomes).unwrap();
    for c in MachineCombination::ALL {
        let declared = outcomes
            .iter()
            .filter(|o| o.declared_combinations().contains(&c))
            .count();
        assert_eq!(s.combination(c).total(), declared, "{}", c.name());
    }
    assert_eq!(s.totals().total(), outcomes.len());
}

#[test]
fn totals_count_each_outcome_once_by_its_label() {
    let d = two();
    let outcomes = [
        run(&d, Some("metal-mac-mini"), None, matched(), &[]),
        run(&d, Some("cuda-node-b"), None, mismatched(), &[]),
        run(&d, None, None, matched(), &[]),
    ];
    let s = Summary::from_outcomes(&outcomes).unwrap();
    assert_eq!(LabelKind::ALL.map(|k| s.totals().get(k)), [1, 1, 0, 1, 0]);
}

#[test]
fn has_ng_is_true_only_when_an_outcome_is_ng() {
    let d = decl(&[MachineCombination::MetalMacMini], &[]);
    let at = Some("metal-mac-mini");
    let fine = [
        run(&d, at, None, matched(), &[]),
        run(&d, None, None, matched(), &[]),
        run(&d, at, None, matched(), &["t-999"]),
    ];
    assert!(!Summary::from_outcomes(&fine).unwrap().has_ng());
    let bad = [run(&d, at, None, mismatched(), &[])];
    assert!(Summary::from_outcomes(&bad).unwrap().has_ng());
}

#[test]
fn result_does_not_depend_on_order_and_record_agrees_with_from_outcomes() {
    let d = two();
    let outcomes = vec![
        run(&d, Some("metal-mac-mini"), None, matched(), &[]),
        run(&d, Some("cuda-node-b"), None, mismatched(), &[]),
        run(&d, None, None, matched(), &[]),
    ];
    let forward = Summary::from_outcomes(&outcomes).unwrap();
    let mut reversed = outcomes.clone();
    reversed.reverse();
    assert_eq!(forward, Summary::from_outcomes(&reversed).unwrap());
    let mut by_record = Summary::new();
    for o in &outcomes {
        by_record.record(o).unwrap();
    }
    assert_eq!(forward, by_record);
}

#[test]
fn table_has_fixed_header_and_seven_rows_in_all_order() {
    let table = Summary::new().to_string();
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 9);
    assert_eq!(
        lines[0],
        "| combination | ok | ng | pending | not-run | indeterminate |"
    );
    assert_eq!(lines[1], "| --- | ---: | ---: | ---: | ---: | ---: |");
    for (line, c) in lines[2..].iter().zip(MachineCombination::ALL) {
        assert_eq!(*line, format!("| {} | 0 | 0 | 0 | 0 | 0 |", c.name()));
    }
}

#[test]
fn table_shows_counts_and_keeps_non_ok_columns() {
    let d = decl(&[MachineCombination::MetalMacMini], &[]);
    let s = Summary::from_outcomes(&[
        run(&d, Some("metal-mac-mini"), None, mismatched(), &[]),
        run(&d, None, None, matched(), &[]),
    ])
    .unwrap();
    let table = s.to_string();
    assert!(table.contains("| metal-mac-mini | 0 | 1 | 0 | 1 | 0 |"));
}

#[test]
fn table_is_ascii_and_has_no_identifiers() {
    let d = decl(&[MachineCombination::MetalMacMini], &["secret-permission"]);
    let o = judge(
        &Subject::from_parts("very_unique_test_id", &["ZZZ-99"], &["t-77"]).unwrap(),
        &d,
        &setting(Some("metal-mac-mini"), Some("secret-permission")),
        |_| matched(),
    );
    let table = Summary::from_outcomes(&[o]).unwrap().to_string();
    assert!(table.is_ascii());
    for needle in ["very_unique_test_id", "ZZZ-99", "t-77", "secret-permission"] {
        assert!(!table.contains(needle), "{needle}");
    }
}

/// 7/8: 5 種と 2 桁の件数を含む表の完全一致（表を変える PR が期待値を一緒に直す前提の固定。置き場と範囲は docs/test-frame.md の「テストの現状（7/8）」）。
#[test]
fn table_with_every_kind_and_a_two_digit_count_is_fixed() {
    use MachineCombination::*;
    let one = |c: MachineCombination| decl(&[c], &[]);
    let mut outcomes = Vec::new();
    for _ in 0..12 {
        outcomes.push(run(
            &one(MetalMacMini),
            Some("metal-mac-mini"),
            None,
            matched(),
            &[],
        ));
    }
    outcomes.push(run(
        &one(VulkanNodeB),
        Some("vulkan-node-b"),
        None,
        mismatched(),
        &[],
    ));
    outcomes.push(run(
        &one(VulkanNodeA),
        Some("vulkan-node-a"),
        None,
        matched(),
        &["t-999"],
    ));
    outcomes.push(run(&one(VulkanAmd), None, None, matched(), &[]));
    outcomes.push(run(
        &one(VulkanIntel),
        Some("vulkan-intel"),
        None,
        RunResult::Indeterminate(IndeterminateReason::MachineUnreachable),
        &[],
    ));
    let cuda = one(CudaNodeA);
    outcomes.push(run(&cuda, Some("cuda-node-a"), None, matched(), &[]));
    outcomes.push(run(&cuda, Some("cuda-node-a"), None, mismatched(), &[]));
    assert_eq!(outcomes.len(), 18);

    let s = Summary::from_outcomes(&outcomes).unwrap();
    let expected = "\
| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 12 | 0 | 0 | 0 | 0 |
| vulkan-node-b | 0 | 1 | 0 | 0 | 0 |
| vulkan-node-a | 0 | 0 | 1 | 0 | 0 |
| vulkan-amd | 0 | 0 | 0 | 1 | 0 |
| vulkan-intel | 0 | 0 | 0 | 0 | 1 |
| cuda-node-b | 0 | 0 | 0 | 0 | 0 |
| cuda-node-a | 1 | 1 | 0 | 0 | 0 |";
    let table = s.to_string();
    assert_eq!(table, expected);
    assert_eq!(table.lines().count(), 9);
    assert!(!table.ends_with('\n'));
    assert_eq!(LabelKind::ALL.map(|k| s.totals().get(k)), [13, 2, 1, 1, 1]);
    for c in MachineCombination::ALL {
        let row = s.combination(c);
        assert_eq!(
            row.total(),
            LabelKind::ALL.iter().map(|k| row.get(*k)).sum()
        );
    }
}

#[test]
fn outcomes_with_the_same_test_id_are_each_counted() {
    let d = decl(&[MachineCombination::MetalMacMini], &[]);
    let at = Some("metal-mac-mini");
    let o = run(&d, at, None, matched(), &[]);
    let s = Summary::from_outcomes(&[o.clone(), o]).unwrap();
    assert_eq!(
        counts(&s, MachineCombination::MetalMacMini),
        [2, 0, 0, 0, 0]
    );
    assert_eq!(s.totals().total(), 2);
}

#[test]
fn summary_error_is_ascii() {
    let e = SummaryError::CountOverflow;
    assert!(e.as_str().is_ascii());
    assert_eq!(e.to_string(), e.as_str());
    let _: &dyn std::error::Error = &e;
}
