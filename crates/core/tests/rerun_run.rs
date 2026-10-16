//! 流し直しの実行の口（`rerun::run_checks`・`export_outcomes`・`run_and_export`）の振る舞いの確認（spec TASK-29）。
//! 値はすべてダミー。環境は読まず、設定は `RunSetting::from_lookup` へ渡す。
//! 書き出し先は試験ごとの一時ディレクトリで、`FANDHE_SILICON_TEST_RECORD_DIR` には書かない。
//! 置き場と範囲の説明は docs/rerun-frame.md。

use fandhe_silicon_core::rerun::{Check, CheckSet, export_outcomes, run_and_export, run_checks};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, ExportError, IndeterminateReason, Label, MachineCombination,
    NotRunReason, Outcome, RunResult, RunSetting, Subject, Summary, Verdict, record_file_name,
};
use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

/// 結果を固定で返し、本体が呼ばれた回数と組み合わせを数えるダミーの照合。
struct Dummy {
    subject: Subject,
    declaration: Declaration,
    result: RunResult,
    panics: bool,
    calls: Cell<usize>,
    last_on: RefCell<Option<MachineCombination>>,
}

impl Dummy {
    fn new(id: &str, result: RunResult) -> Dummy {
        Dummy {
            subject: Subject::from_parts(id, &["DUM-1"], &[]).unwrap(),
            declaration: Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap(),
            result,
            panics: false,
            calls: Cell::new(0),
            last_on: RefCell::new(None),
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
    fn run(&self, on: MachineCombination) -> RunResult {
        assert!(!self.panics, "run must not be called");
        self.calls.set(self.calls.get() + 1);
        *self.last_on.borrow_mut() = Some(on);
        self.result
    }
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

/// 試験ごとの一時ディレクトリ。先に消してから作り、終わったら消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let p =
            std::env::temp_dir().join(format!("fandhe-rerun-run-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir(&p).unwrap();
        TempDir(p)
    }
    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        v.sort();
        v
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ok() -> RunResult {
    RunResult::Verdict(Verdict::Match)
}

#[test]
fn unset_combination_runs_nothing_and_every_check_is_not_run() {
    let mut a = Dummy::new("dummy-a", ok());
    let mut b = Dummy::new("dummy-b", ok());
    a.panics = true;
    b.panics = true;
    let list: [&dyn Check; 2] = [&a, &b];
    let set = CheckSet::new(&list).unwrap();
    let outcomes = run_checks(&set, &setting(None));
    assert_eq!(outcomes.len(), 2);
    for o in &outcomes {
        assert_eq!(*o.label(), Label::NotRun(NotRunReason::NotConfigured));
        assert_eq!(o.ran_on(), None);
    }
    assert_eq!(outcomes[0].id().as_str(), "dummy-a");
    assert_eq!(outcomes[1].id().as_str(), "dummy-b");
}

#[test]
fn labels_follow_the_frame_on_a_configured_combination() {
    let m = Dummy::new("dummy-ok", ok());
    let n = Dummy::new("dummy-ng", RunResult::Verdict(Verdict::Mismatch));
    let i = Dummy::new(
        "dummy-ind",
        RunResult::Indeterminate(IndeterminateReason::MachineUnreachable),
    );
    let list: [&dyn Check; 3] = [&m, &n, &i];
    let set = CheckSet::new(&list).unwrap();
    let outcomes = run_checks(&set, &setting(Some("metal-mac-mini")));
    assert_eq!(*outcomes[0].label(), Label::Ok);
    assert_eq!(*outcomes[1].label(), Label::Ng);
    assert_eq!(
        *outcomes[2].label(),
        Label::Indeterminate(IndeterminateReason::MachineUnreachable)
    );
    for d in [&m, &n, &i] {
        assert_eq!(d.calls.get(), 1);
        assert_eq!(*d.last_on.borrow(), Some(MachineCombination::MetalMacMini));
    }
}

#[test]
fn undeclared_combination_does_not_call_the_body() {
    let x = Dummy::new("dummy-out", ok());
    let list: [&dyn Check; 1] = [&x];
    let set = CheckSet::new(&list).unwrap();
    let outcomes = run_checks(&set, &setting(Some("vulkan-node-b")));
    assert_eq!(
        *outcomes[0].label(),
        Label::NotRun(NotRunReason::CombinationNotDeclared)
    );
    assert_eq!(x.calls.get(), 0);
}

#[test]
fn outcomes_keep_declaration_order() {
    let (c, a, b) = (
        Dummy::new("dummy-c", ok()),
        Dummy::new("dummy-a", ok()),
        Dummy::new("dummy-b", ok()),
    );
    let list: [&dyn Check; 3] = [&c, &a, &b];
    let set = CheckSet::new(&list).unwrap();
    let ids: Vec<String> = run_checks(&set, &setting(None))
        .iter()
        .map(|o| o.id().as_str().to_string())
        .collect();
    assert_eq!(ids, ["dummy-c", "dummy-a", "dummy-b"]);
}

#[test]
fn no_directory_writes_nothing() {
    let a = Dummy::new("dummy-a", ok());
    let list: [&dyn Check; 1] = [&a];
    let set = CheckSet::new(&list).unwrap();
    let t = TempDir::new("none");
    let outcomes = run_and_export(None, &set, &setting(None)).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(t.names().is_empty());
}

#[test]
fn export_writes_one_file_per_part_with_the_frame_names_and_content() {
    let a = Dummy::new("dummy-a", ok());
    let b = Dummy::new("dummy-b", RunResult::Verdict(Verdict::Mismatch));
    let list: [&dyn Check; 2] = [&a, &b];
    let set = CheckSet::new(&list).unwrap();
    let t = TempDir::new("export");
    let outcomes = run_and_export(Some(&t.0), &set, &setting(None)).unwrap();

    let parts: Vec<_> = outcomes.iter().flat_map(Outcome::record_parts).collect();
    assert_eq!(t.names().len(), parts.len());
    let expected: Vec<String> = outcomes
        .iter()
        .flat_map(|o| {
            o.declared_combinations()
                .iter()
                .map(|c| record_file_name(o.id(), *c))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut sorted = expected.clone();
    sorted.sort();
    assert_eq!(t.names(), sorted);
    for (name, part) in expected.iter().zip(&parts) {
        let body = fs::read_to_string(t.0.join(name)).unwrap();
        assert_eq!(body, format!("{}\n", part.to_json()));
    }
}

#[test]
fn export_outcomes_returns_names_in_part_order() {
    let a = Dummy::new("dummy-a", ok());
    let b = Dummy::new("dummy-b", ok());
    let list: [&dyn Check; 2] = [&b, &a];
    let set = CheckSet::new(&list).unwrap();
    let outcomes = run_checks(&set, &setting(None));
    let t = TempDir::new("order");
    let names = export_outcomes(&t.0, &outcomes).unwrap();
    assert_eq!(
        names,
        [
            "dummy-b.metal-mac-mini.json".to_string(),
            "dummy-a.metal-mac-mini.json".to_string()
        ]
    );
}

#[test]
fn a_failure_removes_every_file_this_call_made_and_keeps_others() {
    let a = Dummy::new("dummy-a", ok());
    let b = Dummy::new("dummy-b", ok());
    let list: [&dyn Check; 2] = [&a, &b];
    let set = CheckSet::new(&list).unwrap();
    let t = TempDir::new("fail");
    let pre = "dummy-b.metal-mac-mini.json";
    fs::write(t.0.join(pre), "keep").unwrap();
    let r = run_and_export(Some(&t.0), &set, &setting(None));
    assert_eq!(r.err(), Some(ExportError::AlreadyExists));
    assert_eq!(t.names(), [pre.to_string()]);
    assert_eq!(fs::read_to_string(t.0.join(pre)).unwrap(), "keep");
}

#[test]
fn invalid_directories_are_errors() {
    let a = Dummy::new("dummy-a", ok());
    let list: [&dyn Check; 1] = [&a];
    let set = CheckSet::new(&list).unwrap();
    let rel = std::path::Path::new("relative-dummy-dir");
    let r = run_and_export(Some(rel), &set, &setting(None));
    assert_eq!(r.err(), Some(ExportError::NotAbsolute));
    assert!(!rel.exists());

    let missing =
        std::env::temp_dir().join(format!("fandhe-rerun-run-{}-missing", std::process::id()));
    let e = run_and_export(Some(&missing), &set, &setting(None))
        .err()
        .unwrap();
    assert_eq!(e, ExportError::DirectoryMissing);
    assert!(!missing.exists());
    assert!(!e.to_string().contains("fandhe-rerun-run"));
}

#[test]
fn labels_do_not_depend_on_export() {
    let a = Dummy::new("dummy-a", ok());
    let b = Dummy::new("dummy-b", RunResult::Verdict(Verdict::Mismatch));
    let list: [&dyn Check; 2] = [&a, &b];
    let set = CheckSet::new(&list).unwrap();
    let s = setting(Some("metal-mac-mini"));
    let plain = run_checks(&set, &s);
    let t = TempDir::new("labels");
    let exported = run_and_export(Some(&t.0), &set, &s).unwrap();
    assert_eq!(plain, exported);
}

#[test]
fn summary_from_the_outcomes_matches_the_frame() {
    let a = Dummy::new("dummy-a", ok());
    let b = Dummy::new("dummy-b", RunResult::Verdict(Verdict::Mismatch));
    let list: [&dyn Check; 2] = [&a, &b];
    let set = CheckSet::new(&list).unwrap();
    let outcomes = run_checks(&set, &setting(Some("metal-mac-mini")));
    let summary = Summary::from_outcomes(&outcomes).unwrap();
    let mut by_hand = Summary::new();
    for o in &outcomes {
        by_hand.record(o).unwrap();
    }
    assert_eq!(summary, by_hand);
    assert!(summary.has_ng());
}
