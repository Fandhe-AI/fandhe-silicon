//! 実行の設定の読み取りと、実行の可否（`decide`）の振る舞いの確認（spec TASK-23 の 3/8）。
//! プロセスの環境は書き換えず、値は `RunSetting::from_lookup` へ渡す。
//! 置き場と範囲の説明は docs/test-frame.md の「現状の形」。

use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Decision, Declaration, MachineCombination, NotRunReason, PERMISSIONS_ENV,
    RunSetting, decide,
};
use std::ffi::OsString;

#[cfg(unix)]
fn non_utf8() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(vec![0xff, 0xfe])
}

#[cfg(windows)]
fn non_utf8() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    OsString::from_wide(&[0xd800])
}

/// 2 つの環境変数の値を固定で返す引き口を作る（名前は定数経由で引かれることも確かめる）。
fn setting(comb: Option<OsString>, perms: Option<OsString>) -> RunSetting {
    RunSetting::from_lookup(move |name| {
        if name == COMBINATION_ENV {
            comb.clone()
        } else if name == PERMISSIONS_ENV {
            perms.clone()
        } else {
            None
        }
    })
}

fn s(v: &str) -> Option<OsString> {
    Some(OsString::from(v))
}

fn decl(perms: &[&str]) -> Declaration {
    Declaration::from_parts(
        &[
            MachineCombination::MetalMacMini,
            MachineCombination::CudaNodeB,
        ],
        perms,
    )
    .unwrap()
}

fn not_run(r: NotRunReason) -> Decision {
    Decision::NotRun(r)
}

#[test]
fn unset_combination_is_not_configured() {
    assert_eq!(
        decide(&decl(&[]), &setting(None, None)),
        not_run(NotRunReason::NotConfigured)
    );
}

#[test]
fn invalid_combination_values_are_distinguished_from_unset() {
    let long = "a".repeat(200);
    for v in [
        s(""),
        s("no-such"),
        s("METAL-MAC-MINI"),
        s(" metal-mac-mini"),
        s(&long),
        Some(non_utf8()),
    ] {
        assert_eq!(
            decide(&decl(&[]), &setting(v, None)),
            not_run(NotRunReason::InvalidCombinationSetting)
        );
    }
}

#[test]
fn invalid_permission_values_are_rejected() {
    let many = (0..40)
        .map(|i| format!("p{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let huge = "a,".repeat(2000);
    for v in [
        s("Bad"),
        s("a,,b"),
        s("a,"),
        s(",a"),
        s("a b"),
        s(&many),
        s(&huge),
        Some(non_utf8()),
    ] {
        assert_eq!(
            decide(&decl(&[]), &setting(s("metal-mac-mini"), v)),
            not_run(NotRunReason::InvalidPermissionSetting)
        );
    }
}

#[test]
fn unset_and_empty_permissions_mean_none_granted() {
    for v in [None, s("")] {
        assert_eq!(
            decide(&decl(&[]), &setting(s("metal-mac-mini"), v.clone())),
            Decision::Run(MachineCombination::MetalMacMini)
        );
        assert_eq!(
            decide(
                &decl(&["fault-injection"]),
                &setting(s("metal-mac-mini"), v)
            ),
            not_run(NotRunReason::PermissionMissing)
        );
    }
}

#[test]
fn undeclared_combination_is_not_run() {
    assert_eq!(
        decide(&decl(&[]), &setting(s("vulkan-amd"), None)),
        not_run(NotRunReason::CombinationNotDeclared)
    );
}

#[test]
fn run_when_declared_and_permissions_granted() {
    assert_eq!(
        decide(&decl(&["a", "b"]), &setting(s("cuda-node-b"), s("b,c,a"))),
        Decision::Run(MachineCombination::CudaNodeB)
    );
}

#[test]
fn one_missing_permission_among_several_is_not_run() {
    assert_eq!(
        decide(&decl(&["a", "b"]), &setting(s("cuda-node-b"), s("a"))),
        not_run(NotRunReason::PermissionMissing)
    );
}

#[test]
fn earlier_conditions_win_over_later_ones() {
    // 組み合わせが未設定なら、許可が不正でも「設定なし」。
    assert_eq!(
        decide(&decl(&["a"]), &setting(None, s("Bad"))),
        not_run(NotRunReason::NotConfigured)
    );
    // 組み合わせが不正なら、許可が不正でもこちら。
    assert_eq!(
        decide(&decl(&["a"]), &setting(s("zzz"), s("Bad"))),
        not_run(NotRunReason::InvalidCombinationSetting)
    );
    // 許可が不正なら、宣言の外でもこちら。
    assert_eq!(
        decide(&decl(&["a"]), &setting(s("vulkan-amd"), s("Bad"))),
        not_run(NotRunReason::InvalidPermissionSetting)
    );
    // 宣言の外なら、許可が足りなくてもこちら。
    assert_eq!(
        decide(&decl(&["a"]), &setting(s("vulkan-amd"), None)),
        not_run(NotRunReason::CombinationNotDeclared)
    );
}

#[test]
fn every_combination_runs_only_when_declared() {
    for target in MachineCombination::ALL {
        let st = setting(s(target.name()), None);
        for other in MachineCombination::ALL {
            let d = Declaration::from_parts(&[other], &[]).unwrap();
            let want = if other == target {
                Decision::Run(target)
            } else {
                not_run(NotRunReason::CombinationNotDeclared)
            };
            assert_eq!(decide(&d, &st), want);
        }
    }
}

#[test]
fn duplicate_declaration_decides_like_deduplicated_one() {
    let dup = Declaration::from_parts(
        &[MachineCombination::CudaNodeB, MachineCombination::CudaNodeB],
        &["a", "a"],
    )
    .unwrap();
    let one = Declaration::from_parts(&[MachineCombination::CudaNodeB], &["a"]).unwrap();
    for st in [
        setting(s("cuda-node-b"), s("a")),
        setting(s("cuda-node-b"), None),
        setting(None, None),
    ] {
        assert_eq!(decide(&dup, &st), decide(&one, &st));
    }
}

#[test]
fn reasons_and_decisions_have_ascii_text_without_input_values() {
    let reasons = [
        NotRunReason::NotConfigured,
        NotRunReason::InvalidCombinationSetting,
        NotRunReason::InvalidPermissionSetting,
        NotRunReason::CombinationNotDeclared,
        NotRunReason::PermissionMissing,
    ];
    for r in reasons {
        assert!(!r.as_str().is_empty());
        assert!(r.as_str().is_ascii());
        assert_eq!(r.to_string(), r.as_str());
        assert!(Decision::NotRun(r).to_string().is_ascii());
    }
    let d = decide(&decl(&[]), &setting(s("secret-value-xyz"), None));
    assert!(!d.to_string().contains("secret"));
    assert!(
        Decision::Run(MachineCombination::MetalMacMini)
            .to_string()
            .contains("metal-mac-mini")
    );
}

#[test]
fn environment_names_are_fixed_constants() {
    assert_eq!(COMBINATION_ENV, "FANDHE_SILICON_TEST_MACHINE_COMBINATION");
    assert_eq!(PERMISSIONS_ENV, "FANDHE_SILICON_TEST_PERMISSIONS");
}

#[test]
fn max_count_and_max_length_permissions_are_runnable() {
    let names: Vec<String> = (0..16).map(|i| format!("{i:0>64}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let joined = names.join(",");
    assert_eq!(joined.len(), 16 * 64 + 15);
    assert_eq!(
        decide(&decl(&refs), &setting(s("metal-mac-mini"), s(&joined))),
        Decision::Run(MachineCombination::MetalMacMini)
    );
}

// ---- 7/8: 許可の値の境界・引く名前（置き場と範囲は docs/test-frame.md の「テストの現状（7/8）」） ----

#[test]
fn permission_count_and_length_boundaries_in_the_setting() {
    let d = decl(&[]);
    let run = Decision::Run(MachineCombination::MetalMacMini);
    let invalid = not_run(NotRunReason::InvalidPermissionSetting);
    let at = |perms: &str| decide(&d, &setting(s("metal-mac-mini"), s(perms)));

    let sixteen: Vec<String> = (0..16).map(|i| format!("p{i}")).collect();
    assert_eq!(at(&sixteen.join(",")), run);
    let seventeen: Vec<String> = (0..17).map(|i| format!("p{i}")).collect();
    assert_eq!(at(&seventeen.join(",")), invalid);

    assert_eq!(at(&"a".repeat(64)), run);
    assert_eq!(at(&"a".repeat(65)), invalid);
}

#[test]
fn lookup_is_asked_only_for_the_two_fixed_names() {
    use std::cell::RefCell;
    let asked: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let _ = RunSetting::from_lookup(|name| {
        asked.borrow_mut().push(name.to_string());
        None
    });
    let mut got = asked.into_inner();
    got.sort();
    let mut want = vec![COMBINATION_ENV.to_string(), PERMISSIONS_ENV.to_string()];
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn unset_and_empty_permissions_give_equal_settings() {
    assert_eq!(
        setting(s("metal-mac-mini"), None),
        setting(s("metal-mac-mini"), s(""))
    );
}

#[test]
fn from_env_does_not_panic() {
    // 環境は書き換えない。値は何であれ、可否が返る（Run か NotRun のどちらか）。
    let _ = decide(&decl(&[]), &RunSetting::from_env());
}
