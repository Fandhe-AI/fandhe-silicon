//! Node での照合の行を流し直しの照合へ写す部品（`rerun::reference`）の振る舞いの確認（spec TASK-29。ビヘイビアは BUILD-41）。
//! 値はすべてダミー。環境は読まず、設定は `RunSetting::from_lookup` へ渡す。宣言の組み合わせもダミーの扱い。
//! 書き出し先は試験ごとの一時ディレクトリで、`FANDHE_SILICON_TEST_RECORD_DIR` には書かない。
//! 置き場と範囲の説明は docs/rerun-frame.md。

use fandhe_silicon_core::rerun::{
    Check, CheckSet, CheckSetError, ReferenceSource, RowCheck, RowState, reference_gap,
    run_and_export, run_checks,
};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, Label, MachineCombination, NotRunReason, Outcome, ReferenceGap,
    RunSetting, Subject, record_file_name,
};
use std::ffi::OsString;
use std::fs;

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

fn row(id: &str, state: RowState) -> RowCheck {
    RowCheck::new(
        Subject::from_parts(id, &["BUILD-41"], &[]).unwrap(),
        Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap(),
        state,
    )
}

fn labels(rows: &[&RowCheck], comb: Option<&str>) -> Vec<Outcome> {
    let list: Vec<&dyn Check> = rows.iter().map(|r| *r as &dyn Check).collect();
    let set = CheckSet::new(&list).unwrap();
    run_checks(&set, &setting(comb))
}

#[test]
fn source_tokens_round_trip() {
    for s in ReferenceSource::ALL {
        assert_eq!(ReferenceSource::from_name(s.name()), Some(s));
    }
    assert_eq!(ReferenceSource::ALL[0].name(), "fixed");
    assert_eq!(ReferenceSource::ALL[1].name(), "native-neon");
    for bad in [
        "",
        "Fixed",
        "FIXED",
        " fixed",
        "fixed ",
        "native_neon",
        "neon",
    ] {
        assert_eq!(ReferenceSource::from_name(bad), None, "{bad:?}");
    }
}

#[test]
fn row_tokens_parse_exactly() {
    assert_eq!(RowState::from_tokens("match", None), Some(RowState::Match));
    assert_eq!(
        RowState::from_tokens("mismatch", None),
        Some(RowState::Mismatch)
    );
    assert_eq!(
        RowState::from_tokens("not-run", Some("reference-missing")),
        Some(RowState::NotRun(ReferenceGap::Missing))
    );
    assert_eq!(
        RowState::from_tokens("not-run", Some("reference-unavailable")),
        Some(RowState::NotRun(ReferenceGap::Unavailable))
    );
    for (s, r) in [
        ("not-run", None),
        ("not-run", Some("not-configured")),
        ("not-run", Some("")),
        ("match", Some("reference-missing")),
        ("mismatch", Some("reference-unavailable")),
        ("Match", None),
        ("ok", None),
        ("", None),
    ] {
        assert_eq!(RowState::from_tokens(s, r), None, "{s:?} {r:?}");
    }
}

#[test]
fn gap_prefers_unavailable_source() {
    let only_fixed = [ReferenceSource::Fixed];
    for has in [true, false] {
        assert_eq!(
            reference_gap(ReferenceSource::NativeNeon, has, &only_fixed),
            Some(ReferenceGap::Unavailable)
        );
    }
    assert_eq!(
        reference_gap(ReferenceSource::Fixed, false, &only_fixed),
        Some(ReferenceGap::Missing)
    );
    assert_eq!(
        reference_gap(ReferenceSource::Fixed, true, &only_fixed),
        None
    );
    assert_eq!(
        reference_gap(ReferenceSource::Fixed, true, &[]),
        Some(ReferenceGap::Unavailable)
    );
    assert_eq!(
        reference_gap(ReferenceSource::NativeNeon, true, &ReferenceSource::ALL),
        None
    );
}

#[test]
fn row_states_map_to_task23_labels() {
    let a = row("row-a", RowState::Match);
    let b = row("row-b", RowState::Mismatch);
    let c = row("row-c", RowState::NotRun(ReferenceGap::Missing));
    let d = row("row-d", RowState::NotRun(ReferenceGap::Unavailable));
    let outcomes = labels(&[&a, &b, &c, &d], Some("metal-mac-mini"));
    assert_eq!(*outcomes[0].label(), Label::Ok);
    assert_eq!(*outcomes[1].label(), Label::Ng);
    assert_eq!(
        *outcomes[2].label(),
        Label::NotRun(NotRunReason::ReferenceMissing)
    );
    assert_eq!(
        *outcomes[3].label(),
        Label::NotRun(NotRunReason::ReferenceUnavailable)
    );
    assert_eq!(a.state(), RowState::Match);
}

#[test]
fn device_free_setting_is_not_run_without_calling_rows() {
    let a = row("row-a", RowState::Match);
    let b = row("row-b", RowState::NotRun(ReferenceGap::Missing));
    let outcomes = labels(&[&a, &b], None);
    for o in &outcomes {
        assert_eq!(*o.label(), Label::NotRun(NotRunReason::NotConfigured));
        assert_eq!(o.ran_on(), None);
    }
}

#[test]
fn reference_gap_rows_export_reason() {
    let a = row("row-a", RowState::NotRun(ReferenceGap::Unavailable));
    let b = row("row-b", RowState::Match);
    let list: [&dyn Check; 2] = [&a, &b];
    let set = CheckSet::new(&list).unwrap();
    let dir = std::env::temp_dir().join(format!("fandhe-rerun-reference-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    let outcomes = run_and_export(Some(&dir), &set, &setting(Some("metal-mac-mini"))).unwrap();
    let parts = outcomes[0].record_parts();
    assert_eq!(parts.len(), 1);
    let json = fs::read_to_string(dir.join(record_file_name(
        outcomes[0].id(),
        MachineCombination::MetalMacMini,
    )))
    .unwrap();
    assert_eq!(json, format!("{}\n", parts[0].to_json()));
    assert!(json.contains("reference-unavailable"));
    assert!(json.contains("not-run"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn row_checks_reject_duplicate_ids() {
    let a = row("row-a", RowState::Match);
    let b = row("row-a", RowState::Mismatch);
    let list: [&dyn Check; 2] = [&a, &b];
    assert!(matches!(
        CheckSet::new(&list),
        Err(CheckSetError::DuplicateId)
    ));
}
