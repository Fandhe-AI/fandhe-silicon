//! 試験ごとの記録の部品（`Outcome::record_parts`）の形の確認（spec TASK-23）。
//! プロセスの環境と作業ツリーは書き換えず、設定は `RunSetting::from_lookup` へ渡す。
//! 置き場と範囲の説明は docs/test-frame.md の「試験ごとの記録の部品」。

use fandhe_silicon_core::test_frame::{
    BODY_KIND, COMBINATION_ENV, Declaration, IndeterminateReason, MachineCombination, Outcome,
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

fn decl(perms: &[&str]) -> Declaration {
    Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeB,
            MachineCombination::VulkanNodeA,
        ],
        perms,
    )
    .unwrap()
}

fn run(behaviors: &[&str], thresholds: &[&str], comb: Option<&str>, r: RunResult) -> Outcome {
    judge(
        &Subject::from_parts("record_case", behaviors, thresholds).unwrap(),
        &decl(&[]),
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

fn only_json(o: &Outcome) -> String {
    let parts = o.record_parts();
    assert_eq!(parts.len(), 1);
    parts[0].to_json()
}

#[test]
fn ok_part_is_one_with_the_combination_and_no_reason() {
    let o = run(&["MEM-1"], &[], Some("metal-mac-mini"), matched());
    assert_eq!(
        only_json(&o),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"ok","body_kind":"contract-test","body":{"test_id":"record_case"}}"#
    );
}

#[test]
fn ng_part_is_one_with_the_combination_and_no_reason() {
    let o = run(&["MEM-1"], &[], Some("cuda-node-b"), mismatched());
    assert_eq!(
        only_json(&o),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"cuda-node-b","machine":"node-b","label":"ng","body_kind":"contract-test","body":{"test_id":"record_case"}}"#
    );
}

#[test]
fn pending_part_has_the_fixed_reason_and_the_threshold_id_in_the_body() {
    let o = run(&["MEM-1"], &["t-999"], Some("metal-mac-mini"), matched());
    assert_eq!(
        only_json(&o),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"pending","label_reason":"threshold-unapproved","body_kind":"contract-test","body":{"test_id":"record_case","threshold_id":"t-999"}}"#
    );
}

#[test]
fn only_pending_parts_carry_a_threshold_id() {
    let labels = [
        run(&[], &[], Some("metal-mac-mini"), matched()),
        run(&[], &[], Some("metal-mac-mini"), mismatched()),
        run(&[], &[], None, matched()),
        run(
            &[],
            &[],
            Some("metal-mac-mini"),
            RunResult::Indeterminate(IndeterminateReason::DeviceMismatch),
        ),
    ];
    for o in labels {
        for p in o.record_parts() {
            assert!(!p.to_json().contains("threshold_id"));
        }
    }
    let pending = run(&[], &["t-999"], Some("metal-mac-mini"), matched());
    assert!(only_json(&pending).contains("threshold_id"));
}

#[test]
fn indeterminate_part_has_the_reason() {
    let o = run(
        &["MEM-1"],
        &[],
        Some("vulkan-node-a"),
        RunResult::Indeterminate(IndeterminateReason::MachineUnreachable),
    );
    assert_eq!(
        only_json(&o),
        r#"{"behavior_ids":["MEM-1"],"machine_combination":"vulkan-node-a","machine":"node-a","label":"indeterminate","label_reason":"machine-unreachable","body_kind":"contract-test","body":{"test_id":"record_case"}}"#
    );
}

#[test]
fn not_run_gives_one_part_per_declared_combination_in_declared_order() {
    let o = run(&["MEM-1", "MEM-2"], &[], None, matched());
    let parts = o.record_parts();
    assert_eq!(parts.len(), 3);
    let expected = [
        (
            MachineCombination::MetalMacMini,
            r#"{"behavior_ids":["MEM-1","MEM-2"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"not-run","label_reason":"not-configured","body_kind":"contract-test","body":{"test_id":"record_case"}}"#,
        ),
        (
            MachineCombination::CudaNodeB,
            r#"{"behavior_ids":["MEM-1","MEM-2"],"machine_combination":"cuda-node-b","machine":"node-b","label":"not-run","label_reason":"not-configured","body_kind":"contract-test","body":{"test_id":"record_case"}}"#,
        ),
        (
            MachineCombination::VulkanNodeA,
            r#"{"behavior_ids":["MEM-1","MEM-2"],"machine_combination":"vulkan-node-a","machine":"node-a","label":"not-run","label_reason":"not-configured","body_kind":"contract-test","body":{"test_id":"record_case"}}"#,
        ),
    ];
    for (part, (c, json)) in parts.iter().zip(expected) {
        assert_eq!(part.combination(), c);
        assert_eq!(part.test_id().as_str(), "record_case");
        assert_eq!(part.to_json(), json);
    }
}

#[test]
fn a_run_on_one_combination_gives_exactly_one_part_even_with_several_declared() {
    let o = run(&[], &[], Some("cuda-node-b"), matched());
    assert_eq!(o.declared_combinations().len(), 3);
    let parts = o.record_parts();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].combination(), MachineCombination::CudaNodeB);
}

#[test]
fn not_run_for_an_undeclared_combination_uses_its_own_reason() {
    let o = run(&[], &[], Some("vulkan-amd"), matched());
    let parts = o.record_parts();
    assert_eq!(parts.len(), 3);
    for p in &parts {
        assert!(p.to_json().contains(
            r#""label":"not-run","label_reason":"combination-not-declared","body_kind""#
        ));
    }
}

#[test]
fn behavior_ids_keep_declared_order_without_duplicates_and_may_be_empty() {
    let o = run(
        &["B-2", "A-1", "B-2"],
        &[],
        Some("metal-mac-mini"),
        matched(),
    );
    assert!(only_json(&o).starts_with(r#"{"behavior_ids":["B-2","A-1"],"machine_combination""#));
    let empty = run(&[], &[], Some("metal-mac-mini"), matched());
    assert!(only_json(&empty).starts_with(r#"{"behavior_ids":[],"machine_combination""#));
}

#[test]
fn fields_are_an_ordered_list_that_agrees_with_the_one_line_json() {
    let o = run(&["MEM-1"], &[], None, matched());
    for p in o.record_parts() {
        let names: Vec<&str> = p.fields().iter().map(|(k, _)| *k).collect();
        assert_eq!(
            names,
            [
                "behavior_ids",
                "machine_combination",
                "machine",
                "label",
                "label_reason",
                "body_kind",
                "body"
            ]
        );
        let rebuilt = format!(
            "{{{}}}",
            p.fields()
                .iter()
                .map(|(k, v)| format!("\"{k}\":{v}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(rebuilt, p.to_json());
        assert!(!p.to_json().contains('\n'));
        assert!(p.to_json().is_ascii());
    }
    let ok = run(&[], &[], Some("metal-mac-mini"), matched());
    let parts = ok.record_parts();
    let names: Vec<&str> = parts[0].fields().iter().map(|(k, _)| *k).collect();
    assert!(!names.contains(&"label_reason"));
    assert_eq!(
        parts[0].fields()[4],
        ("body_kind", "\"contract-test\"".to_owned())
    );
    assert_eq!(BODY_KIND, "contract-test");
}

#[test]
fn every_label_kind_and_combination_gives_a_valid_shape() {
    let results = [matched(), mismatched()];
    for c in MachineCombination::ALL {
        let d = Declaration::from_parts(&[c], &[]).unwrap();
        for r in results {
            let o = judge(
                &Subject::from_parts("shape", &["X-1"], &[]).unwrap(),
                &d,
                &setting(Some(c.name()), None),
                |_| r,
            );
            let parts = o.record_parts();
            assert_eq!(parts.len(), 1);
            assert!(parts[0].to_json().contains(c.machine().name()));
        }
    }
}

#[test]
fn parts_do_not_show_environment_values_or_permission_names() {
    let d = decl(&["secret-permission"]);
    let s = Subject::from_parts("leak_check", &["MEM-1"], &[]).unwrap();
    let granted = setting(
        Some("metal-mac-mini"),
        Some("secret-permission,other-granted"),
    );
    let ran = judge(&s, &d, &granted, |_| matched());
    let missing = judge(&s, &d, &setting(Some("metal-mac-mini"), None), |_| {
        matched()
    });
    for o in [ran, missing] {
        for p in o.record_parts() {
            let j = p.to_json();
            assert!(!j.contains("secret-permission"));
            assert!(!j.contains("other-granted"));
            assert!(!j.contains("FANDHE_SILICON"));
        }
    }
}
