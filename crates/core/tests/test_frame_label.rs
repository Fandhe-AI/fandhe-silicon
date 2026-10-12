//! 結果のラベルと理由のトークンの振る舞いの確認（spec TASK-23 の 5/8）。
//! 置き場と範囲の説明は docs/test-frame.md の「現状の形（5/8）」。

use fandhe_silicon_core::test_frame::{IndeterminateReason, Label, NotRunReason, ThresholdId};

const NOT_RUN: [NotRunReason; 5] = [
    NotRunReason::NotConfigured,
    NotRunReason::InvalidCombinationSetting,
    NotRunReason::InvalidPermissionSetting,
    NotRunReason::CombinationNotDeclared,
    NotRunReason::PermissionMissing,
];

const INDETERMINATE: [IndeterminateReason; 2] = [
    IndeterminateReason::DeviceMismatch,
    IndeterminateReason::MachineUnreachable,
];

fn token_ok(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn one_of_each() -> Vec<Label> {
    vec![
        Label::Ok,
        Label::Ng,
        Label::Pending(ThresholdId::new("t-12").unwrap()),
        Label::NotRun(NotRunReason::NotConfigured),
        Label::Indeterminate(IndeterminateReason::DeviceMismatch),
    ]
}

#[test]
fn kind_tokens_are_distinct_and_well_formed() {
    let names: Vec<&str> = one_of_each().iter().map(Label::name).collect();
    assert_eq!(names, ["ok", "ng", "pending", "not-run", "indeterminate"]);
    for (i, n) in names.iter().enumerate() {
        assert!(token_ok(n));
        assert!(!names[..i].contains(n));
    }
}

#[test]
fn reason_tokens_exist_only_for_kinds_with_a_reason() {
    for l in one_of_each() {
        let has = matches!(l, Label::NotRun(_) | Label::Indeterminate(_));
        assert_eq!(l.reason_name().is_some(), has, "{}", l.name());
    }
}

#[test]
fn not_run_reason_tokens_cover_every_variant_without_duplicates() {
    let names: Vec<&str> = NOT_RUN.iter().map(|r| r.name()).collect();
    for (i, n) in names.iter().enumerate() {
        assert!(token_ok(n));
        assert!(!names[..i].contains(n));
        assert_eq!(Label::NotRun(NOT_RUN[i]).reason_name(), Some(*n));
    }
}

#[test]
fn indeterminate_reason_tokens_cover_every_variant_without_duplicates() {
    let names: Vec<&str> = INDETERMINATE.iter().map(|r| r.name()).collect();
    for (i, n) in names.iter().enumerate() {
        assert!(token_ok(n));
        assert!(!names[..i].contains(n));
        assert_eq!(
            Label::Indeterminate(INDETERMINATE[i]).reason_name(),
            Some(*n)
        );
    }
}

#[test]
fn only_ng_fails_the_test() {
    for l in one_of_each() {
        assert_eq!(l.fails_test(), matches!(l, Label::Ng), "{}", l.name());
    }
    for r in NOT_RUN {
        assert!(!Label::NotRun(r).fails_test());
    }
    for r in INDETERMINATE {
        assert!(!Label::Indeterminate(r).fails_test());
    }
}

#[test]
fn display_is_ascii_english() {
    let mut all = one_of_each();
    all.extend(NOT_RUN.map(Label::NotRun));
    all.extend(INDETERMINATE.map(Label::Indeterminate));
    for l in all {
        let t = l.to_string();
        assert!(t.is_ascii(), "{t}");
        assert!(t.starts_with(l.name()), "{t}");
    }
    for r in INDETERMINATE {
        assert!(r.to_string().is_ascii());
    }
}
