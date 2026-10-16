//! 流し直しの照合 1 件の型（`rerun::Check`・`CheckSet`）の振る舞いの確認（spec TASK-29）。
//! 値はすべてダミー。環境は読まず、設定は `RunSetting::from_lookup` へ渡す。記録は書き出さない。
//! 置き場と範囲の説明は docs/rerun-frame.md。

use fandhe_silicon_core::rerun::{Check, CheckSet, CheckSetError, MAX_CHECKS};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, IndeterminateReason, Label, MachineCombination, NotRunReason,
    RunResult, RunSetting, Subject, Verdict, judge,
};
use std::ffi::OsString;

/// 結果を固定で返すダミーの照合。`panics` が真なら本体が呼ばれた時点で落ちる。
struct Dummy {
    subject: Subject,
    declaration: Declaration,
    result: RunResult,
    panics: bool,
}

impl Dummy {
    fn new(id: &str, result: RunResult) -> Dummy {
        Dummy {
            subject: Subject::from_parts(id, &["DUM-1"], &[]).unwrap(),
            declaration: Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap(),
            result,
            panics: false,
        }
    }
}

impl Check for Dummy {
    fn subject(&self) -> &Subject {
        &self.subject
    }
    fn declaration(&self) -> &Declaration {
        &self.declaration
    }
    fn run(&self, _on: MachineCombination) -> RunResult {
        assert!(!self.panics, "run must not be called");
        self.result
    }
}

fn ok() -> RunResult {
    RunResult::Verdict(Verdict::Match)
}

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

fn ids(set: &CheckSet<'_>) -> Vec<String> {
    set.checks()
        .iter()
        .map(|c| c.subject().id().as_str().to_string())
        .collect()
}

#[test]
fn check_is_usable_as_trait_object() {
    let d = Dummy::new("dummy-a", ok());
    let c: &dyn Check = &d;
    assert_eq!(c.subject().id().as_str(), "dummy-a");
    assert_eq!(
        c.declaration().combinations(),
        &[MachineCombination::MetalMacMini]
    );
    assert_eq!(c.run(MachineCombination::MetalMacMini), ok());
}

#[test]
fn set_keeps_declaration_order() {
    let (a, b, c) = (
        Dummy::new("dummy-c", ok()),
        Dummy::new("dummy-a", ok()),
        Dummy::new("dummy-b", ok()),
    );
    let list: [&dyn Check; 3] = [&a, &b, &c];
    let set = CheckSet::new(&list).unwrap();
    assert_eq!(ids(&set), ["dummy-c", "dummy-a", "dummy-b"]);
    assert_eq!(set.len(), 3);
    assert!(!set.is_empty());
}

#[test]
fn empty_is_rejected() {
    let list: [&dyn Check; 0] = [];
    assert_eq!(CheckSet::new(&list).err(), Some(CheckSetError::Empty));
}

#[test]
fn count_limit_is_enforced() {
    let items: Vec<Dummy> = (0..=MAX_CHECKS)
        .map(|i| Dummy::new(&format!("dummy-{i}"), ok()))
        .collect();
    let refs: Vec<&dyn Check> = items.iter().map(|d| d as &dyn Check).collect();
    assert_eq!(CheckSet::new(&refs).err(), Some(CheckSetError::TooMany));
    assert!(CheckSet::new(&refs[..MAX_CHECKS]).is_ok());
}

#[test]
fn duplicate_id_is_rejected_even_if_other_parts_differ() {
    let a = Dummy::new("dummy-a", ok());
    let mut b = Dummy::new("dummy-a", RunResult::Verdict(Verdict::Mismatch));
    b.subject = Subject::from_parts("dummy-a", &["DUM-2"], &[]).unwrap();
    let list: [&dyn Check; 2] = [&a, &b];
    assert_eq!(CheckSet::new(&list).err(), Some(CheckSetError::DuplicateId));
}

#[test]
fn validation_order_is_empty_then_count_then_duplicate() {
    let items: Vec<Dummy> = (0..=MAX_CHECKS)
        .map(|_| Dummy::new("dummy-a", ok()))
        .collect();
    let refs: Vec<&dyn Check> = items.iter().map(|d| d as &dyn Check).collect();
    assert_eq!(CheckSet::new(&refs).err(), Some(CheckSetError::TooMany));
    assert_eq!(
        CheckSet::new(&refs[..2]).err(),
        Some(CheckSetError::DuplicateId)
    );
}

#[test]
fn error_text_is_ascii_and_has_no_input() {
    for e in [
        CheckSetError::Empty,
        CheckSetError::TooMany,
        CheckSetError::DuplicateId,
    ] {
        assert!(e.as_str().is_ascii());
        assert_eq!(e.to_string(), e.as_str());
        assert!(!e.to_string().contains("dummy"));
        let _: &dyn std::error::Error = &e;
    }
}

#[test]
fn unset_combination_does_not_call_run() {
    let mut d = Dummy::new("dummy-a", ok());
    d.panics = true;
    let out = judge(d.subject(), d.declaration(), &setting(None), |m| d.run(m));
    assert_eq!(out.label(), &Label::NotRun(NotRunReason::NotConfigured));
    assert_eq!(out.ran_on(), None);
}

#[test]
fn judge_maps_results_to_labels() {
    let s = setting(Some("metal-mac-mini"));
    let cases = [
        (ok(), Label::Ok),
        (RunResult::Verdict(Verdict::Mismatch), Label::Ng),
        (
            RunResult::Indeterminate(IndeterminateReason::MachineUnreachable),
            Label::Indeterminate(IndeterminateReason::MachineUnreachable),
        ),
    ];
    for (result, expected) in cases {
        let d = Dummy::new("dummy-a", result);
        let out = judge(d.subject(), d.declaration(), &s, |m| d.run(m));
        assert_eq!(out.label(), &expected);
        assert_eq!(out.id(), d.subject().id());
        assert_eq!(out.ran_on(), Some(MachineCombination::MetalMacMini));
    }
}
