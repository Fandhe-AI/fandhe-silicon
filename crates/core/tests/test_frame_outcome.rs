//! 結果の決め方（`judge`）・`Outcome`・失敗にするかの判定・`parity` の結果からの写像の振る舞いの確認（spec TASK-23 の 5/8）。
//! プロセスの環境と作業ツリーは書き換えず、設定は `RunSetting::from_lookup` へ渡す。
//! 置き場と範囲の説明は docs/test-frame.md の「現状の形（5/8）」。

use fandhe_silicon_core::parity::pixel::{PixelView, T3, compare_pixels, compare_pixels_exact};
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};
use fandhe_silicon_core::parity::topk::{TopKEntry, compare_topk};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, IndeterminateReason, Label, MAX_BEHAVIORS, MAX_THRESHOLDS,
    MachineCombination, NotRunReason, PERMISSIONS_ENV, RunResult, RunSetting, Subject,
    SubjectError, Verdict, judge, judge_with_approval,
};
use std::cell::Cell;
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

fn decl() -> Declaration {
    Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeB,
        ],
        &[],
    )
    .unwrap()
}

fn subject(thresholds: &[&str]) -> Subject {
    Subject::from_parts("alloc_roundtrip", &["MEM-1"], thresholds).unwrap()
}

/// 承認の判定は試験の側から渡す（公開側の表は空）。
fn approve(t: &fandhe_silicon_core::test_frame::ThresholdId) -> bool {
    matches!(t.as_str(), "t-1" | "t-2")
}

fn matched() -> RunResult {
    RunResult::Verdict(Verdict::Match)
}

fn mismatched() -> RunResult {
    RunResult::Verdict(Verdict::Mismatch)
}

#[test]
fn subject_validates_and_deduplicates() {
    let s = Subject::from_parts("t", &["A-1", "A-1", "B-2"], &["t-1", "t-1", "t-2"]).unwrap();
    assert_eq!(s.id().as_str(), "t");
    assert_eq!(s.behaviors().len(), 2);
    assert_eq!(s.thresholds().len(), 2);
    assert!(matches!(
        Subject::from_parts("T", &[], &[]),
        Err(SubjectError::TestId(_))
    ));
    assert!(matches!(
        Subject::from_parts("t", &["a"], &[]),
        Err(SubjectError::BehaviorId(_))
    ));
    assert!(matches!(
        Subject::from_parts("t", &[], &["t-01"]),
        Err(SubjectError::ThresholdId(_))
    ));
}

#[test]
fn subject_limits_counts() {
    let b: Vec<&str> = vec!["A"; MAX_BEHAVIORS + 1];
    let t: Vec<&str> = vec!["t-1"; MAX_THRESHOLDS + 1];
    assert_eq!(
        Subject::from_parts("t", &b, &[]),
        Err(SubjectError::TooManyBehaviors)
    );
    assert_eq!(
        Subject::from_parts("t", &[], &t),
        Err(SubjectError::TooManyThresholds)
    );
    assert!(Subject::from_parts("t", &b[..MAX_BEHAVIORS], &t[..MAX_THRESHOLDS]).is_ok());
}

#[test]
fn subject_errors_are_ascii_and_do_not_echo_input() {
    let e = Subject::from_parts("t", &["SECRET/x"], &[]).unwrap_err();
    assert!(e.to_string().is_ascii());
    assert!(!e.to_string().contains("SECRET"));
    assert!(!format!("{e:?}").contains("SECRET"));
}

#[test]
fn not_run_reasons_pass_through_and_the_body_is_not_called() {
    let calls = Cell::new(0u32);
    let run = |d: &Declaration, st: &RunSetting| {
        judge(&subject(&[]), d, st, |_| {
            calls.set(calls.get() + 1);
            matched()
        })
    };
    let with_perm =
        Declaration::from_parts(&[MachineCombination::MetalMacMini], &["fault"]).unwrap();
    let cases = [
        (decl(), setting(None, None), NotRunReason::NotConfigured),
        (
            decl(),
            setting(Some("nope"), None),
            NotRunReason::InvalidCombinationSetting,
        ),
        (
            decl(),
            setting(Some("metal-mac-mini"), Some("A")),
            NotRunReason::InvalidPermissionSetting,
        ),
        (
            decl(),
            setting(Some("vulkan-amd"), None),
            NotRunReason::CombinationNotDeclared,
        ),
        (
            with_perm,
            setting(Some("metal-mac-mini"), None),
            NotRunReason::PermissionMissing,
        ),
    ];
    for (d, st, reason) in cases {
        let o = run(&d, &st);
        assert_eq!(o.label(), &Label::NotRun(reason));
        assert_eq!(o.ran_on(), None);
    }
    assert_eq!(calls.get(), 0);
}

#[test]
fn approved_thresholds_map_verdicts_to_ok_and_ng() {
    let st = setting(Some("metal-mac-mini"), None);
    let s = subject(&["t-1", "t-2"]);
    let ok = judge_with_approval(&s, &decl(), &st, approve, |_| matched());
    assert_eq!(ok.label(), &Label::Ok);
    let ng = judge_with_approval(&s, &decl(), &st, approve, |_| mismatched());
    assert_eq!(ng.label(), &Label::Ng);
    let none = judge(&subject(&[]), &decl(), &st, |_| matched());
    assert_eq!(none.label(), &Label::Ok);
}

#[test]
fn unapproved_threshold_gives_pending_with_the_first_one_in_declared_order() {
    let st = setting(Some("metal-mac-mini"), None);
    let s = subject(&["t-1", "t-13", "t-12"]);
    for body in [matched(), mismatched()] {
        let o = judge_with_approval(&s, &decl(), &st, approve, |_| body);
        match o.label() {
            Label::Pending(id) => assert_eq!(id.as_str(), "t-13"),
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn public_table_is_empty_so_any_declared_threshold_gives_pending() {
    let st = setting(Some("metal-mac-mini"), None);
    let o = judge(&subject(&["t-1"]), &decl(), &st, |_| matched());
    assert_eq!(o.label().name(), "pending");
}

#[test]
fn indeterminate_wins_over_pending_and_verdicts() {
    let st = setting(Some("metal-mac-mini"), None);
    let s = subject(&["t-12"]);
    for r in [
        IndeterminateReason::DeviceMismatch,
        IndeterminateReason::MachineUnreachable,
    ] {
        let o = judge(&s, &decl(), &st, |_| RunResult::Indeterminate(r));
        assert_eq!(o.label(), &Label::Indeterminate(r));
    }
}

#[test]
fn outcome_getters_report_what_was_declared_and_run() {
    let s = Subject::from_parts("x", &["A-1", "B-2"], &[]).unwrap();
    let seen = Cell::new(None);
    let o = judge(&s, &decl(), &setting(Some("cuda-node-b"), None), |c| {
        seen.set(Some(c));
        matched()
    });
    assert_eq!(seen.get(), Some(MachineCombination::CudaNodeB));
    assert_eq!(o.ran_on(), Some(MachineCombination::CudaNodeB));
    assert_eq!(o.id().as_str(), "x");
    assert_eq!(o.behaviors().len(), 2);
    assert_eq!(o.declared_combinations(), decl().combinations());
}

#[test]
fn only_declared_combinations_run_over_all_seven() {
    let s = subject(&[]);
    for c in MachineCombination::ALL {
        let o = judge(&s, &decl(), &setting(Some(c.name()), None), |_| matched());
        let declared = decl().combinations().contains(&c);
        assert_eq!(o.ran_on().is_some(), declared, "{}", c.name());
        assert_eq!(matches!(o.label(), Label::Ok), declared, "{}", c.name());
    }
}

/// 失敗にするかは `fails_test` で呼び出し側が明示的に決める。`Ng` のときだけ失敗で、
/// ほかの 4 種は失敗にも成功の確認にも読み替えない。
#[test]
fn fails_test_is_true_only_for_ng() {
    let st = setting(Some("metal-mac-mini"), None);
    let ok = judge(&subject(&[]), &decl(), &st, |_| matched());
    let ng = judge(&subject(&[]), &decl(), &st, |_| mismatched());
    let pending = judge(&subject(&["t-12"]), &decl(), &st, |_| matched());
    let not_run = judge(&subject(&[]), &decl(), &setting(None, None), |_| matched());
    let indet = judge(&subject(&[]), &decl(), &st, |_| {
        RunResult::Indeterminate(IndeterminateReason::MachineUnreachable)
    });
    assert!(!ok.label().fails_test());
    assert!(ng.label().fails_test());
    assert!(!pending.label().fails_test());
    assert!(!not_run.label().fails_test());
    assert!(!indet.label().fails_test());
}

#[test]
fn from_match_maps_booleans() {
    assert_eq!(Verdict::from_match(true), Verdict::Match);
    assert_eq!(Verdict::from_match(false), Verdict::Mismatch);
}

#[test]
fn parity_comparisons_map_to_verdicts() {
    let r = [1.0f32, 2.0, 3.0];
    let bad = [1.0f32, 2.0, 9.0];
    assert_eq!(
        Verdict::from(&compare_f32(&r, &r, T1).unwrap()),
        Verdict::Match
    );
    assert_eq!(
        Verdict::from(&compare_f32(&r, &bad, T1).unwrap()),
        Verdict::Mismatch
    );
    assert_eq!(
        Verdict::from(&compare_f32_with_special_values(&r, &r, T1).unwrap()),
        Verdict::Match
    );
    assert_eq!(
        Verdict::from(&compare_f32_with_special_values(&r, &bad, T1).unwrap()),
        Verdict::Mismatch
    );

    let a = [TopKEntry::new(0, 3.0), TopKEntry::new(1, 2.0)];
    let b = [TopKEntry::new(0, 3.0), TopKEntry::new(2, 2.0)];
    assert_eq!(
        Verdict::from(&compare_topk(&a, &a).unwrap()),
        Verdict::Match
    );
    assert_eq!(
        Verdict::from(&compare_topk(&a, &b).unwrap()),
        Verdict::Mismatch
    );

    let p = [10u8, 20, 30, 255, 40, 50, 60, 255];
    let mut q = p;
    q[0] = 200;
    let pv = PixelView::new(&p, 2, 1).unwrap();
    let qv = PixelView::new(&q, 2, 1).unwrap();
    assert_eq!(
        Verdict::from(&compare_pixels_exact(pv, pv).unwrap()),
        Verdict::Match
    );
    assert_eq!(
        Verdict::from(&compare_pixels_exact(pv, qv).unwrap()),
        Verdict::Mismatch
    );
    assert_eq!(
        Verdict::from(&compare_pixels(pv, pv, T3).unwrap()),
        Verdict::Match
    );
    assert_eq!(
        Verdict::from(&compare_pixels(pv, qv, T3).unwrap()),
        Verdict::Mismatch
    );
}
