//! 再現の試験（`replay` とコマンド `variation-replay`）の公開テスト（TASK-1027 のポインタ）。
//!
//! 入力はすべて合成の値で、規則の係数や記録済みの比は写さない（規則ごとの違いは `VariationRule::setting` を
//! 経由して読む）。書き込みは一時ディレクトリだけで、終わりに消す。作業ツリーと環境変数は変えない。

use measure_perf::jsonread;
use measure_perf::replay::{self, ReplayErrorKind};
use measure_perf::variation::{IndeterminateScope, VariationRule};
use std::path::PathBuf;
use std::process::{Command, Output};

/// 幅が無い 6 run。
const N: [u64; 6] = [100_000; 6];
/// 幅が大きい 6 run（中央値は `N` と同じ。どの規則でも当たる大きさ）。
const W: [u64; 6] = [10_000, 100_000, 100_000, 100_000, 100_000, 190_000];
/// 幅が無く、中央値が `N` と違う 6 run。
const M4: [u64; 6] = [400_000; 6];

fn arr(v: &[u64]) -> String {
    let s: Vec<String> = v.iter().map(u64::to_string).collect();
    format!("[{}]", s.join(","))
}

fn attempt(arms: &[(&str, &[u64])]) -> String {
    let s: Vec<String> = arms
        .iter()
        .map(|(n, v)| format!("{{\"arm\":\"{n}\",\"runs\":{}}}", arr(v)))
        .collect();
    format!("[{}]", s.join(","))
}

/// 比較: (id, target, 相手の腕 or 定数, 上限の分子, 分母)。
type Cmp = (
    &'static str,
    &'static str,
    Result<&'static str, u64>,
    u64,
    u64,
);

fn cmps(list: &[Cmp]) -> String {
    let s: Vec<String> = list
        .iter()
        .map(|(id, t, cp, n, d)| {
            let cp = match cp {
                Ok(a) => format!("\"counterpart_arm\":\"{a}\""),
                Err(c) => format!("\"counterpart_const\":{c}"),
            };
            format!(
                "{{\"id\":\"{id}\",\"target\":\"{t}\",{cp},\"bound_num\":{n},\"bound_den\":{d}}}"
            )
        })
        .collect();
    format!("[{}]", s.join(","))
}

fn labels(l: &[(&str, &str)]) -> String {
    let s: Vec<String> = l.iter().map(|(i, x)| format!("\"{i}\":\"{x}\"")).collect();
    format!("{{\"labels\":{{{}}}}}", s.join(","))
}

const NEEDS: &str = "{\"needs_remeasure\":true}";

fn row(id: &str, attempts: &[String], comparisons: &str, expected: &str) -> String {
    format!(
        "{{\"row\":\"{id}\",\"attempts\":[{}],\"comparisons\":{comparisons},\"expected\":{expected}}}",
        attempts.join(",")
    )
}

fn input(rule: &str, rows: &[String]) -> String {
    format!(
        "{{\"rule\":\"{rule}\",\"resolution\":{{\"cpu_ps\":41666,\"gpu_ps\":1000}},\"rows\":[{}]}}",
        rows.join(",")
    )
}

fn run_lib(text: &str) -> Result<replay::ReplayOutcome, replay::ReplayError> {
    let v = jsonread::parse(text.as_bytes()).unwrap();
    replay::replay(&replay::parse_input(&v)?)
}

/// 行の比較（腕 a が target、b が相手）。境目ちょうど・直上・定数の相手を含む。
const ROW_CMPS: [Cmp; 5] = [
    ("c1", "a", Ok("b"), 3, 2),
    ("c2", "a", Err(300_000), 2, 5),
    ("c3", "b", Err(300_000), 1, 4),
    ("c4", "a", Ok("b"), 1, 1),
    ("c5", "a", Ok("b"), 99_999, 100_000),
];

/// N の値での判定（c3 は NG、c5 は境目の直上で NG、c4 は等しくて OK）。
const SETTLED_N: [(&str, &str); 5] = [
    ("c1", "ok"),
    ("c2", "ok"),
    ("c3", "ng"),
    ("c4", "ok"),
    ("c5", "ng"),
];

/// M4（a のみ 4 倍）の値での判定。
const SETTLED_M4: [(&str, &str); 5] = [
    ("c1", "ng"),
    ("c2", "ng"),
    ("c3", "ng"),
    ("c4", "ng"),
    ("c5", "ng"),
];

fn involves_a(id: &str) -> bool {
    id != "c3"
}

fn spread_rules() -> impl Iterator<Item = VariationRule> {
    VariationRule::ALL
        .into_iter()
        .filter(|r| *r != VariationRule::Poc11)
}

#[test]
fn scopes_are_all_exercised() {
    let scopes: Vec<_> = VariationRule::ALL
        .iter()
        .map(|r| r.setting().scope)
        .collect();
    for s in [
        IndeterminateScope::Row,
        IndeterminateScope::ComparisonsWithExceededArm,
        IndeterminateScope::Comparison,
    ] {
        assert!(scopes.contains(&s));
    }
}

#[test]
fn first_attempt_without_trigger_settles() {
    for rule in spread_rules() {
        let r = row(
            "r1",
            &[attempt(&[("a", &N), ("b", &N)])],
            &cmps(&ROW_CMPS),
            &labels(&SETTLED_N),
        );
        let out = run_lib(&input(rule.name(), &[r])).unwrap();
        assert!(out.all_matched(), "{}", rule.name());
        assert!(out.line().contains("\"attempt\":\"first\""));
        assert!(out.line().contains("\"rows_total\":1,\"rows_matched\":1"));
    }
}

#[test]
fn first_attempt_with_trigger_needs_remeasure() {
    for rule in spread_rules() {
        let r = row(
            "r1",
            &[attempt(&[("a", &W), ("b", &N)])],
            &cmps(&ROW_CMPS),
            NEEDS,
        );
        let out = run_lib(&input(rule.name(), &[r])).unwrap();
        assert!(out.all_matched(), "{}", rule.name());
        assert!(
            out.line()
                .contains("\"result\":\"needs-remeasure\",\"triggered\":[\"a\"]")
        );
    }
}

#[test]
fn trigger_remaining_after_remeasure_is_indeterminate_per_scope() {
    for rule in spread_rules() {
        let scope = rule.setting().scope;
        let expected: Vec<(&str, &str)> = SETTLED_N
            .iter()
            .map(|(id, l)| match scope {
                IndeterminateScope::Row => (*id, "indeterminate"),
                _ if involves_a(id) => (*id, "indeterminate"),
                _ => (*id, *l),
            })
            .collect();
        let r = row(
            "r1",
            &[
                attempt(&[("a", &W), ("b", &N)]),
                attempt(&[("a", &W), ("b", &N)]),
            ],
            &cmps(&ROW_CMPS),
            &labels(&expected),
        );
        let out = run_lib(&input(rule.name(), &[r])).unwrap();
        assert!(out.all_matched(), "{}", rule.name());
        assert!(out.line().contains("\"attempt\":\"remeasured\""));
        assert!(out.line().contains("\"reason\":\"variation-exceeded\""));
    }
}

#[test]
fn remeasured_values_alone_decide() {
    for rule in spread_rules() {
        // 1 回目は当たり（a の中央値は N と同じ）。2 回目は当たりが消え、a だけ中央値が違う。
        let r = row(
            "r1",
            &[
                attempt(&[("a", &W), ("b", &N)]),
                attempt(&[("a", &M4), ("b", &N)]),
            ],
            &cmps(&ROW_CMPS),
            &labels(&SETTLED_M4),
        );
        let out = run_lib(&input(rule.name(), &[r])).unwrap();
        assert!(out.all_matched(), "{}", rule.name());
        assert!(!out.line().contains("indeterminate\","));
    }
}

fn poc11_cmps() -> String {
    cmps(&[
        ("c1", "a", Ok("b"), 3, 2),
        ("c2", "a", Err(1_000_000), 1, 1),
    ])
}

/// 差が幅の中にある a と b（c1 が当たる）。
const A_WIDE: [u64; 6] = [90_000, 100_000, 100_000, 100_000, 100_000, 110_000];
const B_NEAR: [u64; 6] = [105_000; 6];

#[test]
fn poc11_comparison_scope_splits_comparisons() {
    // 当たりなし: 差が幅より大きい。
    let r = row(
        "r1",
        &[attempt(&[("a", &N), ("b", &M4)])],
        &poc11_cmps(),
        &labels(&[("c1", "ok"), ("c2", "ok")]),
    );
    assert!(run_lib(&input("poc11", &[r])).unwrap().all_matched());
    // 1 回目で c1 が当たる。
    let r = row(
        "r1",
        &[attempt(&[("a", &A_WIDE), ("b", &B_NEAR)])],
        &poc11_cmps(),
        NEEDS,
    );
    let out = run_lib(&input("poc11", &[r])).unwrap();
    assert!(out.all_matched());
    assert!(out.line().contains("\"triggered\":[\"c1\"]"));
    // 測り直しても c1 だけが判定不能。
    let r = row(
        "r1",
        &[
            attempt(&[("a", &A_WIDE), ("b", &B_NEAR)]),
            attempt(&[("a", &A_WIDE), ("b", &B_NEAR)]),
        ],
        &poc11_cmps(),
        &labels(&[("c1", "indeterminate"), ("c2", "ok")]),
    );
    assert!(run_lib(&input("poc11", &[r])).unwrap().all_matched());
    // 測り直しで当たりが消えれば 2 回目の値で判定する。
    let r = row(
        "r1",
        &[
            attempt(&[("a", &A_WIDE), ("b", &B_NEAR)]),
            attempt(&[("a", &N), ("b", &M4)]),
        ],
        &poc11_cmps(),
        &labels(&[("c1", "ok"), ("c2", "ok")]),
    );
    assert!(run_lib(&input("poc11", &[r])).unwrap().all_matched());
}

#[test]
fn mixed_rows_in_one_file_are_in_order_and_input_order_is_irrelevant() {
    let r1 = row(
        "r-b",
        &[attempt(&[("a", &N), ("b", &N)])],
        &cmps(&ROW_CMPS),
        &labels(&SETTLED_N),
    );
    let r2 = row(
        "r-a",
        &[attempt(&[("a", &W), ("b", &N)])],
        &cmps(&ROW_CMPS),
        NEEDS,
    );
    let r3 = row(
        "r-c",
        &[
            attempt(&[("a", &W), ("b", &N)]),
            attempt(&[("a", &W), ("b", &N)]),
        ],
        &cmps(&ROW_CMPS),
        &labels(&[
            ("c1", "indeterminate"),
            ("c2", "indeterminate"),
            ("c3", "indeterminate"),
            ("c4", "indeterminate"),
            ("c5", "indeterminate"),
        ]),
    );
    let one = run_lib(&input("poc10-2", &[r1.clone(), r2.clone(), r3.clone()])).unwrap();
    let two = run_lib(&input("poc10-2", &[r3, r1, r2])).unwrap();
    assert_eq!(one.line(), two.line());
    assert!(one.all_matched());
    let l = one.line();
    assert!(l.find("\"row\":\"r-a\"").unwrap() < l.find("\"row\":\"r-b\"").unwrap());
    assert!(l.find("\"row\":\"r-b\"").unwrap() < l.find("\"row\":\"r-c\"").unwrap());
    assert!(l.contains("\"rows_total\":3,\"rows_matched\":3"));
}

#[test]
fn arm_and_comparison_order_is_irrelevant() {
    let mk = |arms: &[(&str, &[u64])], list: &[Cmp]| {
        row("r1", &[attempt(arms)], &cmps(list), &labels(&SETTLED_N))
    };
    let mut rev = ROW_CMPS;
    rev.reverse();
    let a = run_lib(&input("poc9", &[mk(&[("a", &N), ("b", &N)], &ROW_CMPS)])).unwrap();
    let b = run_lib(&input("poc9", &[mk(&[("b", &N), ("a", &N)], &rev)])).unwrap();
    assert_eq!(a.line(), b.line());
}

#[test]
fn changed_expected_label_is_a_mismatch_with_output() {
    let mut l = SETTLED_N;
    l[0] = ("c1", "ng");
    let r = row(
        "r1",
        &[attempt(&[("a", &N), ("b", &N)])],
        &cmps(&ROW_CMPS),
        &labels(&l),
    );
    let out = run_lib(&input("poc10-2", &[r])).unwrap();
    assert!(!out.all_matched());
    assert!(out.line().contains("\"rows_total\":1,\"rows_matched\":0"));
    assert!(
        out.line()
            .contains("\"label\":\"ok\",\"reason\":null,\"expected\":\"ng\",\"match\":false")
    );
}

#[test]
fn expected_needs_remeasure_but_settled_is_a_mismatch() {
    let r = row(
        "r1",
        &[attempt(&[("a", &N), ("b", &N)])],
        &cmps(&ROW_CMPS),
        NEEDS,
    );
    assert!(!run_lib(&input("poc10-2", &[r])).unwrap().all_matched());
    let r = row(
        "r1",
        &[attempt(&[("a", &W), ("b", &N)])],
        &cmps(&ROW_CMPS),
        &labels(&SETTLED_N),
    );
    assert!(!run_lib(&input("poc10-2", &[r])).unwrap().all_matched());
}

#[test]
fn resolution_passes_through() {
    let r = row(
        "r1",
        &[attempt(&[("a", &N), ("b", &N)])],
        &cmps(&ROW_CMPS),
        &labels(&SETTLED_N),
    );
    let both = run_lib(&input("poc9", std::slice::from_ref(&r))).unwrap();
    assert!(
        both.line()
            .contains("\"resolution\":{\"cpu_ps\":41666,\"gpu_ps\":1000}")
    );
    let text = format!("{{\"rule\":\"poc9\",\"resolution\":{{\"cpu_ps\":41666}},\"rows\":[{r}]}}");
    let cpu = run_lib(&text).unwrap();
    assert!(
        cpu.line()
            .contains("\"resolution\":{\"cpu_ps\":41666,\"gpu_ps\":null}")
    );
}

fn kind(text: &str) -> ReplayErrorKind {
    run_lib(text).unwrap_err().kind
}

fn good_row() -> String {
    row(
        "r1",
        &[attempt(&[("a", &N), ("b", &N)])],
        &cmps(&ROW_CMPS),
        &labels(&SETTLED_N),
    )
}

#[test]
fn violations_are_errors() {
    let g = good_row();
    let rule = "poc10-2";
    assert_eq!(
        kind(&input("poc-unknown", std::slice::from_ref(&g))),
        ReplayErrorKind::UnknownRule
    );
    // 未知のキー。
    let t = input(rule, std::slice::from_ref(&g)).replacen("\"rows\"", "\"extra\":1,\"rows\"", 1);
    assert_eq!(kind(&t), ReplayErrorKind::Shape);
    // 欄の不足。
    let t = format!("{{\"rule\":\"{rule}\",\"rows\":[{g}]}}");
    assert_eq!(kind(&t), ReplayErrorKind::Shape);
    // 小数の値。
    let t = input(rule, &[g.replacen("100000", "100000.5", 1)]);
    assert_eq!(kind(&t), ReplayErrorKind::BadNumber);
    // run の数の不一致。
    let t = input(
        rule,
        &[row(
            "r1",
            &[attempt(&[("a", &N[..5]), ("b", &N)])],
            &cmps(&ROW_CMPS),
            &labels(&SETTLED_N),
        )],
    );
    assert!(matches!(kind(&t), ReplayErrorKind::Judge(_)));
    // 回数の上限超え（判定側）。
    let three = attempt(&[("a", &W), ("b", &N)]);
    let t = input(
        rule,
        &[row(
            "r1",
            &[three.clone(), three.clone(), three],
            &cmps(&ROW_CMPS),
            NEEDS,
        )],
    );
    assert!(matches!(kind(&t), ReplayErrorKind::Judge(_)));
    // 不要な測り直し。
    let ok = attempt(&[("a", &N), ("b", &N)]);
    let t = input(
        rule,
        &[row(
            "r1",
            &[ok.clone(), ok],
            &cmps(&ROW_CMPS),
            &labels(&SETTLED_N),
        )],
    );
    assert!(matches!(kind(&t), ReplayErrorKind::Judge(_)));
    // 未知の腕。
    let t = input(
        rule,
        &[row(
            "r1",
            &[attempt(&[("a", &N), ("b", &N)])],
            &cmps(&[("c1", "zz", Ok("b"), 1, 1)]),
            &labels(&[("c1", "ok")]),
        )],
    );
    assert!(matches!(kind(&t), ReplayErrorKind::Judge(_)));
    // 期待の欠け・余り・未知のラベル。
    let t = input(
        rule,
        &[row(
            "r1",
            &[attempt(&[("a", &N), ("b", &N)])],
            &cmps(&ROW_CMPS),
            &labels(&SETTLED_N[..4]),
        )],
    );
    assert_eq!(kind(&t), ReplayErrorKind::BadExpected);
    let mut extra = SETTLED_N.to_vec();
    extra.push(("c9", "ok"));
    let t = input(
        rule,
        &[row(
            "r1",
            &[attempt(&[("a", &N), ("b", &N)])],
            &cmps(&ROW_CMPS),
            &labels(&extra),
        )],
    );
    assert_eq!(kind(&t), ReplayErrorKind::BadExpected);
    let t = input(rule, &[g.replacen("\"c1\":\"ok\"", "\"c1\":\"maybe\"", 1)]);
    assert_eq!(kind(&t), ReplayErrorKind::BadExpected);
    // 上限の分母 0・相手が 0。
    let t = input(
        rule,
        &[row(
            "r1",
            &[attempt(&[("a", &N), ("b", &N)])],
            &cmps(&[("c1", "a", Ok("b"), 1, 0)]),
            &labels(&[("c1", "ok")]),
        )],
    );
    assert!(matches!(kind(&t), ReplayErrorKind::Judge(_)));
    let t = input(
        rule,
        &[row(
            "r1",
            &[attempt(&[("a", &N), ("b", &N)])],
            &cmps(&[("c1", "a", Err(0), 1, 1)]),
            &labels(&[("c1", "ok")]),
        )],
    );
    assert!(matches!(kind(&t), ReplayErrorKind::Judge(_)));
    // 行の重複・名前の不正・件数の上限超え。
    assert_eq!(
        kind(&input(rule, &[g.clone(), g.clone()])),
        ReplayErrorKind::BadName
    );
    assert_eq!(
        kind(&input(rule, &[g.replacen("\"r1\"", "\"r 1\"", 1)])),
        ReplayErrorKind::BadName
    );
    let many = vec![g; replay::MAX_ROWS + 1];
    assert_eq!(kind(&input(rule, &many)), ReplayErrorKind::TooMany);
}

#[test]
fn error_display_has_no_input_values() {
    let g = good_row().replacen("\"c1\":\"ok\"", "\"c1\":\"secret-marker\"", 1);
    let e = run_lib(&input("poc10-2", &[g])).unwrap_err();
    let s = e.to_string();
    assert!(!s.contains("secret-marker"));
    assert!(s.contains("row 1"));
}

// ---- コマンド ----

struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("variation-replay-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }

    fn file(&self, name: &str, text: &str) -> String {
        let p = self.0.join(name);
        std::fs::write(&p, text).unwrap();
        p.to_string_lossy().into_owned()
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cmd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_variation-replay"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn command_exit_codes() {
    let t = Tmp::new("exit");
    let ok = t.file("ok.json", &input("poc10-2", &[good_row()]));
    let out = cmd(&[&ok]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.contains("\"rows_matched\":1"));

    // 不一致は 1 で、JSON は出す。
    let ng = t.file(
        "ng.json",
        &input(
            "poc10-2",
            &[good_row().replacen("\"c1\":\"ok\"", "\"c1\":\"ng\"", 1)],
        ),
    );
    let out = cmd(&[&ng]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("\"match\":false")
    );

    // 入力の違反は 1 で、JSON は出さない。
    let bad = t.file("bad.json", "{\"rule\":\"x\"");
    let out = cmd(&[&bad]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let dup = t.file("dup.json", "{\"rule\":\"a\",\"rule\":\"b\"}");
    assert_eq!(cmd(&[&dup]).status.code(), Some(1));

    // 複数ファイルは 1 ファイル 1 行。
    let out = cmd(&[&ok, &ok]);
    assert_eq!(String::from_utf8(out.stdout).unwrap().lines().count(), 2);

    // 通常ファイルでない。
    let dir = t.0.to_string_lossy().into_owned();
    assert_eq!(cmd(&[&dir]).status.code(), Some(1));
}

#[test]
fn command_misuse_is_two() {
    assert_eq!(cmd(&[]).status.code(), Some(2));
    assert_eq!(cmd(&["--bogus"]).status.code(), Some(2));
    let t = Tmp::new("misuse");
    let missing = t.0.join("none.json").to_string_lossy().into_owned();
    assert_eq!(cmd(&[&missing]).status.code(), Some(2));
}

#[test]
fn command_help_exits_zero() {
    let out = cmd(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("usage: variation-replay")
    );
}

#[test]
fn command_stderr_has_no_values_or_paths() {
    let t = Tmp::new("stderr");
    let g = good_row().replacen("\"c1\":\"ok\"", "\"c1\":\"secret-marker\"", 1);
    let p = t.file("secret-name.json", &input("poc10-2", &[g]));
    let out = cmd(&[&p]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(!err.contains("secret-marker"));
    assert!(!err.contains("secret-name"));
    assert!(err.contains("arg 1"));
}
