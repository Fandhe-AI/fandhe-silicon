//! 試験ごとの記録（`Outcome::to_record_json`）の形の確認。
//! プロセスの環境と作業ツリーは書き換えず、設定は `RunSetting::from_lookup` へ渡す。
//! 置き場と範囲の説明は docs/test-frame.md の「公開面」。

use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, IndeterminateReason, LabelKind, MachineCombination, Outcome,
    PERMISSIONS_ENV, RunResult, RunSetting, Subject, Verdict, judge,
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

fn subject(behaviors: &[&str], thresholds: &[&str]) -> Subject {
    Subject::from_parts("record_case", behaviors, thresholds).unwrap()
}

fn run(behaviors: &[&str], thresholds: &[&str], comb: Option<&str>, r: RunResult) -> Outcome {
    judge(
        &subject(behaviors, thresholds),
        &decl(),
        &setting(comb, None),
        |_| r,
    )
}

fn matched() -> RunResult {
    RunResult::Verdict(Verdict::Match)
}

fn mismatched() -> RunResult {
    RunResult::Verdict(Verdict::Mismatch)
}

#[test]
fn ok_record_has_the_combination_and_no_reason() {
    let o = run(&["MEM-1"], &[], Some("metal-mac-mini"), matched());
    assert_eq!(
        o.to_record_json(),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"ok"}"#
    );
}

#[test]
fn ng_record_has_the_combination_and_no_reason() {
    let o = run(&["MEM-1"], &[], Some("cuda-node-b"), mismatched());
    assert_eq!(
        o.to_record_json(),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"cuda-node-b","machine":"node-b","label":"ng"}"#
    );
}

#[test]
fn pending_record_carries_the_unapproved_threshold_id_as_reason() {
    let o = run(&["MEM-1"], &["t-999"], Some("metal-mac-mini"), matched());
    assert_eq!(o.label().kind(), LabelKind::Pending);
    assert_eq!(
        o.to_record_json(),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"pending","label_reason":"t-999"}"#
    );
}

#[test]
fn not_run_record_has_the_reason_and_no_combination() {
    let o = run(&["MEM-1"], &[], None, matched());
    assert_eq!(o.ran_on(), None);
    assert_eq!(
        o.to_record_json(),
        r#"{"behavior_ids":["MEM-1"],"label":"not-run","label_reason":"not-configured"}"#
    );
}

#[test]
fn indeterminate_record_has_the_combination_and_the_reason() {
    let o = run(
        &["MEM-1"],
        &[],
        Some("metal-mac-mini"),
        RunResult::Indeterminate(IndeterminateReason::DeviceMismatch),
    );
    assert_eq!(
        o.to_record_json(),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"indeterminate","label_reason":"device-mismatch"}"#
    );
}

#[test]
fn behavior_ids_keep_declared_order_without_duplicates_and_may_be_empty() {
    let o = run(
        &["B-2", "A-1", "B-2"],
        &[],
        Some("metal-mac-mini"),
        matched(),
    );
    assert!(
        o.to_record_json()
            .starts_with(r#"{"behavior_ids":["B-2","A-1"],"#)
    );
    let none = run(&[], &[], Some("metal-mac-mini"), matched());
    assert!(none.to_record_json().starts_with(r#"{"behavior_ids":[],"#));
}

#[test]
fn every_not_run_reason_appears_as_its_token_in_the_record() {
    let with_perm =
        Declaration::from_parts(&[MachineCombination::MetalMacMini], &["fault-injection"]).unwrap();
    let s = subject(&["MEM-1"], &[]);
    let cases = [
        (decl(), setting(None, None), "not-configured"),
        (
            decl(),
            setting(Some("nope"), None),
            "invalid-combination-setting",
        ),
        (
            decl(),
            setting(Some("metal-mac-mini"), Some("A")),
            "invalid-permission-setting",
        ),
        (
            decl(),
            setting(Some("vulkan-amd"), None),
            "combination-not-declared",
        ),
        (
            with_perm,
            setting(Some("metal-mac-mini"), None),
            "permission-missing",
        ),
    ];
    for (d, st, token) in cases {
        let json = judge(&s, &d, &st, |_| matched()).to_record_json();
        let expected =
            format!(r#"{{"behavior_ids":["MEM-1"],"label":"not-run","label_reason":"{token}"}}"#);
        assert_eq!(json, expected);
    }
}

#[test]
fn reason_token_covers_every_reasoned_label_and_only_those() {
    let ok = run(&[], &[], Some("metal-mac-mini"), matched());
    let ng = run(&[], &[], Some("metal-mac-mini"), mismatched());
    let pending = run(&[], &["t-999"], Some("metal-mac-mini"), matched());
    let not_run = run(&[], &[], None, matched());
    assert_eq!(ok.label().reason_token(), None);
    assert_eq!(ng.label().reason_token(), None);
    assert_eq!(pending.label().reason_token(), Some("t-999"));
    assert_eq!(not_run.label().reason_token(), Some("not-configured"));
    // 既存の理由のトークンの取り出しは、判定保留で `None` のまま。
    assert_eq!(pending.label().reason_name(), None);
}

#[test]
fn record_is_a_single_line_ascii_object_with_balanced_quoting() {
    for comb in [None, Some("metal-mac-mini"), Some("cuda-node-b")] {
        for r in [matched(), mismatched()] {
            let json = run(&["MEM-1", "BUILD-54"], &["t-999"], comb, r).to_record_json();
            assert!(json.is_ascii());
            assert!(json.starts_with('{') && json.ends_with('}'));
            assert!(!json.contains(['\n', '\r', '\t']));
            assert_eq!(json.matches('"').count() % 2, 0);
        }
    }
}

#[test]
fn the_same_outcome_gives_the_same_record() {
    let o = run(&["MEM-1"], &["t-999"], Some("metal-mac-mini"), matched());
    assert_eq!(o.to_record_json(), o.to_record_json());
    assert_eq!(o.clone().to_record_json(), o.to_record_json());
}

#[test]
fn record_does_not_leak_environment_values_or_permission_names() {
    let d = Declaration::from_parts(
        &[MachineCombination::MetalMacMini],
        &["secret-permission-name"],
    )
    .unwrap();
    let st = setting(
        Some("metal-mac-mini"),
        Some("secret-permission-name,another-secret"),
    );
    let s = Subject::from_parts("leak_case_id", &["MEM-1"], &[]).unwrap();
    let ran = judge(&s, &d, &st, |_| matched()).to_record_json();
    let skipped = judge(&s, &d, &setting(Some("metal-mac-mini"), None), |_| {
        matched()
    })
    .to_record_json();
    for json in [&ran, &skipped] {
        for forbidden in [
            "secret",
            "another",
            "FANDHE_SILICON",
            COMBINATION_ENV,
            PERMISSIONS_ENV,
        ] {
            assert!(!json.contains(forbidden), "{json} contains {forbidden}");
        }
    }
    assert!(skipped.contains(r#""label_reason":"permission-missing""#));
}
