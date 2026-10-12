//! 試験ごとの記録の部品の書き出し（`export`）の確認（spec TASK-23）。
//! 環境変数は書き換えず、ディレクトリは `*_from_lookup`・`export_parts` へ一時ディレクトリを引数で渡す。
//! 一時ディレクトリは `std::env::temp_dir()` の下に試験ごとの別の名前で作り、終わりに消す。
//! 置き場と範囲の説明は docs/test-frame.md の「記録の部品の書き出し」。

use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, ExportError, Label, MachineCombination, NotRunReason, Outcome,
    RECORD_DIR_ENV, RunResult, RunSetting, Subject, TestId, Verdict, export_from_lookup,
    export_parts, judge, judge_and_export_with, record_file_name,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// 試験ごとに別の一時ディレクトリ。落ちても残らないよう Drop で消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!(
            "fandhe-silicon-export-{}-{tag}-{n}",
            std::process::id()
        ));
        fs::create_dir(&p).unwrap();
        TempDir(p)
    }

    fn path(&self) -> &Path {
        &self.0
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

fn run_setting(comb: Option<&str>) -> RunSetting {
    let comb = comb.map(OsString::from);
    RunSetting::from_lookup(move |n| (n == COMBINATION_ENV).then(|| comb.clone()).flatten())
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

fn outcome(id: &str, comb: Option<&str>) -> Outcome {
    judge(
        &Subject::from_parts(id, &["MEM-1"], &[]).unwrap(),
        &decl(),
        &run_setting(comb),
        |_| RunResult::Verdict(Verdict::Match),
    )
}

fn lookup_dir(dir: &Path) -> impl Fn(&str) -> Option<OsString> {
    let dir = dir.as_os_str().to_owned();
    move |n| (n == RECORD_DIR_ENV).then(|| dir.clone())
}

#[test]
fn environment_name_is_fixed() {
    assert_eq!(RECORD_DIR_ENV, "FANDHE_SILICON_TEST_RECORD_DIR");
}

#[test]
fn a_run_writes_one_file_with_the_expected_name_and_content() {
    let t = TempDir::new("run");
    let o = outcome("export_case", Some("metal-mac-mini"));
    let names = export_from_lookup(lookup_dir(t.path()), &o).unwrap();
    assert_eq!(names, ["export_case.metal-mac-mini.json"]);
    assert_eq!(t.names(), ["export_case.metal-mac-mini.json"]);
    let content = fs::read_to_string(t.path().join("export_case.metal-mac-mini.json")).unwrap();
    assert_eq!(
        content,
        "{\"behavior_ids\":[\"MEM-1\"],\"machine_combination\":\"metal-mac-mini\",\"machine\":\"mac-mini\",\"label\":\"ok\",\"body_kind\":\"contract-test\",\"body\":{\"test_id\":\"export_case\"}}\n"
    );
}

#[test]
fn a_not_run_writes_one_file_per_declared_combination() {
    let t = TempDir::new("notrun");
    let o = outcome("export_case", None);
    let names = export_from_lookup(lookup_dir(t.path()), &o).unwrap();
    assert_eq!(
        names,
        [
            "export_case.metal-mac-mini.json",
            "export_case.cuda-node-b.json"
        ]
    );
    assert_eq!(
        t.names(),
        [
            "export_case.cuda-node-b.json",
            "export_case.metal-mac-mini.json"
        ]
    );
    for n in &names {
        let c = fs::read_to_string(t.path().join(n)).unwrap();
        assert!(c.ends_with("}\n"));
        assert_eq!(c.lines().count(), 1);
        assert!(c.contains(r#""label":"not-run","label_reason":"not-configured""#));
    }
}

#[test]
fn an_unset_directory_writes_nothing() {
    let t = TempDir::new("unset");
    let o = outcome("export_case", None);
    assert_eq!(export_from_lookup(|_| None, &o), Ok(Vec::new()));
    assert!(t.names().is_empty());
}

#[test]
fn the_lookup_is_asked_only_for_the_directory_name() {
    let asked = std::cell::RefCell::new(Vec::new());
    let o = outcome("export_case", None);
    let _ = export_from_lookup(
        |n| {
            asked.borrow_mut().push(n.to_owned());
            None
        },
        &o,
    );
    assert_eq!(*asked.borrow(), [RECORD_DIR_ENV]);
}

#[test]
fn an_existing_file_is_never_overwritten() {
    let t = TempDir::new("exists");
    let o = outcome("export_case", Some("metal-mac-mini"));
    export_from_lookup(lookup_dir(t.path()), &o).unwrap();
    let p = t.path().join("export_case.metal-mac-mini.json");
    let before = fs::read_to_string(&p).unwrap();
    let again = outcome("export_case", Some("metal-mac-mini"));
    assert_eq!(
        export_from_lookup(lookup_dir(t.path()), &again),
        Err(ExportError::AlreadyExists)
    );
    assert_eq!(fs::read_to_string(&p).unwrap(), before);
    fs::write(&p, "keep").unwrap();
    assert_eq!(
        export_from_lookup(lookup_dir(t.path()), &again),
        Err(ExportError::AlreadyExists)
    );
    assert_eq!(fs::read_to_string(&p).unwrap(), "keep");
}

#[test]
fn a_failure_part_way_removes_the_files_this_call_made_and_keeps_others() {
    let t = TempDir::new("rollback");
    let blocker = t.path().join("export_case.cuda-node-b.json");
    fs::write(&blocker, "pre-existing").unwrap();
    let o = outcome("export_case", None);
    assert_eq!(
        export_from_lookup(lookup_dir(t.path()), &o),
        Err(ExportError::AlreadyExists)
    );
    assert_eq!(t.names(), ["export_case.cuda-node-b.json"]);
    assert_eq!(fs::read_to_string(&blocker).unwrap(), "pre-existing");
}

#[test]
fn a_missing_directory_is_an_error_and_is_not_created() {
    let t = TempDir::new("missing");
    let sub = t.path().join("not-made");
    let o = outcome("export_case", None);
    assert_eq!(
        export_from_lookup(lookup_dir(&sub), &o),
        Err(ExportError::DirectoryMissing)
    );
    assert!(!sub.exists());
}

#[test]
fn a_file_is_not_accepted_as_the_directory() {
    let t = TempDir::new("file");
    let f = t.path().join("plain");
    fs::write(&f, "x").unwrap();
    let o = outcome("export_case", None);
    assert_eq!(
        export_from_lookup(lookup_dir(&f), &o),
        Err(ExportError::NotADirectory)
    );
}

#[test]
fn relative_and_empty_paths_are_rejected() {
    let o = outcome("export_case", None);
    for v in ["", "relative", "./relative", "../up"] {
        let v = OsString::from(v);
        let r = export_from_lookup(|n| (n == RECORD_DIR_ENV).then(|| v.clone()), &o);
        assert_eq!(r, Err(ExportError::NotAbsolute));
    }
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_directory_is_not_followed() {
    let t = TempDir::new("symdir");
    let real = t.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = t.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let o = outcome("export_case", None);
    assert_eq!(
        export_from_lookup(lookup_dir(&link), &o),
        Err(ExportError::DirectoryIsSymlink)
    );
    assert!(fs::read_dir(&real).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_at_the_file_name_is_not_followed() {
    let t = TempDir::new("symfile");
    let outside = TempDir::new("symtarget");
    let target = outside.path().join("target");
    std::os::unix::fs::symlink(&target, t.path().join("export_case.metal-mac-mini.json")).unwrap();
    let o = outcome("export_case", Some("metal-mac-mini"));
    assert_eq!(
        export_from_lookup(lookup_dir(t.path()), &o),
        Err(ExportError::AlreadyExists)
    );
    assert!(!target.exists());
}

#[test]
fn errors_do_not_show_the_path() {
    let t = TempDir::new("pathmarker");
    let o = outcome("export_case", None);
    let missing = t.path().join("pathmarker-missing");
    let e = export_from_lookup(lookup_dir(&missing), &o).unwrap_err();
    let text = format!("{e} {e:?}");
    assert!(!text.contains("pathmarker"));
    assert!(text.is_ascii());
    let e2 = ExportError::Io(std::io::ErrorKind::PermissionDenied);
    assert!(format!("{e2}").is_ascii());
    assert!(!format!("{e2}").contains('/'));
}

#[test]
fn file_names_stay_inside_the_directory_and_do_not_collide() {
    let longest = "a".repeat(128);
    let ids = ["x", "x-y", "x_y", "x-y_z", longest.as_str()];
    let mut seen = Vec::new();
    for id in ids {
        let id = TestId::new(id).unwrap();
        for c in MachineCombination::ALL {
            let n = record_file_name(&id, c);
            assert!(n.is_ascii());
            assert!(n.len() <= 255);
            assert!(!n.contains('/') && !n.contains('\\') && !n.contains(".."));
            assert!(n.ends_with(".json"));
            assert_eq!(n.matches('.').count(), 2);
            assert!(!n.starts_with('.'));
            assert!(!seen.contains(&n));
            seen.push(n);
        }
    }
}

#[test]
fn judge_and_export_writes_and_returns_the_same_outcome() {
    let t = TempDir::new("entry");
    let s = Subject::from_parts("entry_case", &["MEM-1"], &[]).unwrap();
    let called = std::cell::Cell::new(false);
    let o = judge_and_export_with(
        lookup_dir(t.path()),
        &s,
        &decl(),
        &run_setting(Some("cuda-node-b")),
        |c| {
            called.set(true);
            assert_eq!(c, MachineCombination::CudaNodeB);
            RunResult::Verdict(Verdict::Mismatch)
        },
    )
    .unwrap();
    assert!(called.get());
    assert_eq!(o.label(), &Label::Ng);
    assert_eq!(t.names(), ["entry_case.cuda-node-b.json"]);
}

#[test]
fn judge_and_export_does_not_call_the_body_when_not_run_and_writes_every_declared_one() {
    let t = TempDir::new("entry-notrun");
    let s = Subject::from_parts("entry_case", &[], &[]).unwrap();
    let o = judge_and_export_with(
        lookup_dir(t.path()),
        &s,
        &decl(),
        &run_setting(None),
        |_| panic!("body must not run"),
    )
    .unwrap();
    assert_eq!(o.label(), &Label::NotRun(NotRunReason::NotConfigured));
    assert_eq!(t.names().len(), 2);
}

#[test]
fn judge_and_export_writes_nothing_when_unset() {
    let t = TempDir::new("entry-unset");
    let s = Subject::from_parts("entry_case", &[], &[]).unwrap();
    let o = judge_and_export_with(
        |_| None,
        &s,
        &decl(),
        &run_setting(Some("metal-mac-mini")),
        |_| RunResult::Verdict(Verdict::Match),
    )
    .unwrap();
    assert_eq!(o.label(), &Label::Ok);
    assert!(t.names().is_empty());
}

#[test]
fn a_write_failure_is_returned_as_an_error_not_swallowed() {
    let t = TempDir::new("entry-fail");
    let s = Subject::from_parts("entry_case", &[], &[]).unwrap();
    let missing = t.path().join("absent");
    let r = judge_and_export_with(
        lookup_dir(&missing),
        &s,
        &decl(),
        &run_setting(Some("metal-mac-mini")),
        |_| RunResult::Verdict(Verdict::Match),
    );
    assert_eq!(r.unwrap_err(), ExportError::DirectoryMissing);
}

#[test]
fn a_hundred_concurrent_writes_of_different_tests_neither_collide_nor_go_missing() {
    let t = TempDir::new("concurrent");
    let dir = t.path().to_path_buf();
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                let o = outcome(&format!("concurrent_{i}"), Some("metal-mac-mini"));
                export_parts(&dir, &o.record_parts()).unwrap()
            })
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap().len(), 1);
    }
    let names = t.names();
    assert_eq!(names.len(), 100);
    for i in 0..100 {
        let n = format!("concurrent_{i}.metal-mac-mini.json");
        assert!(names.contains(&n));
        let c = fs::read_to_string(t.path().join(&n)).unwrap();
        assert!(c.contains(&format!("\"test_id\":\"concurrent_{i}\"")));
    }
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_directory_with_a_trailing_separator_or_dot_is_rejected() {
    let t = TempDir::new("symtrail");
    let real = t.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = t.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let o = outcome("export_case", None);
    for suffix in ["/", "/.", "//", "/./."] {
        let mut v = link.as_os_str().to_owned();
        v.push(suffix);
        let r = export_from_lookup(|n| (n == RECORD_DIR_ENV).then(|| v.clone()), &o);
        assert_eq!(r, Err(ExportError::DirectoryIsSymlink), "{suffix}");
    }
    assert!(fs::read_dir(&real).unwrap().next().is_none());
}

#[test]
fn trailing_separators_and_dots_on_a_real_directory_are_accepted() {
    let t = TempDir::new("trail-ok");
    let mut v = t.path().as_os_str().to_owned();
    v.push("/./");
    let o = outcome("export_case", Some("metal-mac-mini"));
    let r = export_from_lookup(|n| (n == RECORD_DIR_ENV).then(|| v.clone()), &o);
    assert_eq!(r.unwrap(), ["export_case.metal-mac-mini.json"]);
}

#[test]
fn parent_components_are_rejected() {
    let t = TempDir::new("parent");
    let sub = t.path().join("sub");
    fs::create_dir(&sub).unwrap();
    let o = outcome("export_case", None);
    let up = sub.join("..");
    assert_eq!(
        export_from_lookup(lookup_dir(&up), &o),
        Err(ExportError::ParentComponent)
    );
    let mid = sub.join("..").join("sub");
    assert_eq!(
        export_from_lookup(lookup_dir(&mid), &o),
        Err(ExportError::ParentComponent)
    );
    assert!(t.names() == ["sub"]);
    assert!(fs::read_dir(&sub).unwrap().next().is_none());
}

#[test]
fn concurrent_writes_of_the_same_outcome_succeed_exactly_once() {
    for round in 0..20 {
        let t = TempDir::new(&format!("same-{round}"));
        let dir = t.path().to_path_buf();
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    let o = outcome("same_case", None);
                    export_parts(&dir, &o.record_parts())
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        for r in results.iter().filter(|r| r.is_err()) {
            assert_eq!(r, &Err(ExportError::AlreadyExists));
        }
        assert_eq!(
            t.names(),
            [
                "same_case.cuda-node-b.json",
                "same_case.metal-mac-mini.json"
            ]
        );
        for n in t.names() {
            let c = fs::read_to_string(t.path().join(n)).unwrap();
            assert!(c.contains("\"test_id\":\"same_case\""));
            assert!(c.ends_with("}\n"));
        }
    }
}

#[test]
fn concurrent_writes_in_different_declared_orders_never_remove_the_others_files() {
    let metal_first = Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeB,
        ],
        &[],
    )
    .unwrap();
    let cuda_first = Declaration::from_parts(
        &[
            MachineCombination::CudaNodeB,
            MachineCombination::MetalMacMini,
        ],
        &[],
    )
    .unwrap();
    for round in 0..200 {
        let t = TempDir::new(&format!("order-{round}"));
        let dir = t.path().to_path_buf();
        let handles: Vec<_> = [metal_first.clone(), cuda_first.clone()]
            .into_iter()
            .map(|d| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    let o = judge(
                        &Subject::from_parts("order_case", &[], &[]).unwrap(),
                        &d,
                        &run_setting(None),
                        |_| RunResult::Verdict(Verdict::Match),
                    );
                    export_parts(&dir, &o.record_parts())
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let oks = results.iter().filter(|r| r.is_ok()).count();
        for r in results.iter().filter(|r| r.is_err()) {
            assert_eq!(r, &Err(ExportError::AlreadyExists));
        }
        // 両方が成功することはない。片方が成功したなら 2 件とも残り、両方が失敗したなら何も残らない。
        assert!(oks <= 1);
        if oks == 1 {
            assert_eq!(
                t.names(),
                [
                    "order_case.cuda-node-b.json",
                    "order_case.metal-mac-mini.json"
                ]
            );
        } else {
            assert!(t.names().is_empty());
        }
    }
}
