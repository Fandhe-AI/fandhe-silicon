//! Node での照合の結果を記録の部品へ書き出すスクリプト（`scripts/wasm-node-check.sh`）の、
//! 行の状態 → 枠のラベルの写しと、組み合わせ → 機材の表が、`RowState` と `MachineCombination` と
//! 食い違わないことの確認（spec TASK-29。範囲は BUILD-41）。
//! スクリプトの中の目印（`BEGIN row-labels`・`BEGIN combinations`）の間の表を読むだけで、スクリプトは起動せず、
//! ファイルも書かない（`FANDHE_SILICON_TEST_RECORD_DIR` にも触れない）。
//! 目印が無い・行の形が崩れているときは skip せず失敗にする。端から端までの確かめは `scripts/test-wasm-node-check.sh`。

use fandhe_silicon_core::rerun::{
    Check, CheckSet, ReferenceSource, RowCheck, RowState, run_checks,
};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, MachineCombination, ReferenceGap, RunSetting, Subject,
};
use std::ffi::OsString;
use std::fs;
use std::path::Path;

/// 表の 1 行: (状態, 理由, ラベル, ラベルの理由)。
type LabelRow = (String, Option<String>, String, Option<String>);

fn script_text() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/wasm-node-check.sh");
    fs::read_to_string(path).expect("the script must be readable")
}

/// 目印の間の行を返す。目印が 1 組ずつ無ければ失敗。
fn between(text: &str, name: &str) -> Vec<String> {
    let begin = format!("# BEGIN {name}");
    let end = format!("# END {name}");
    assert_eq!(text.matches(&begin).count(), 1, "one BEGIN marker: {name}");
    assert_eq!(text.matches(&end).count(), 1, "one END marker: {name}");
    let mut inside = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t == end {
            assert!(inside, "END before BEGIN: {name}");
            return out;
        }
        if inside {
            out.push(t.to_string());
        }
        if t == begin {
            inside = true;
        }
    }
    panic!("END marker not reached: {name}");
}

/// `("a", None, "b"),` の形の 1 行を要素に分ける。要素は引用符つきの文字列か `None`。
fn fields(line: &str) -> Vec<Option<String>> {
    let inner = line
        .strip_prefix('(')
        .and_then(|l| l.strip_suffix("),"))
        .unwrap_or_else(|| panic!("malformed table row: {line}"));
    inner
        .split(", ")
        .map(|f| {
            if f == "None" {
                None
            } else {
                let s = f
                    .strip_prefix('"')
                    .and_then(|f| f.strip_suffix('"'))
                    .unwrap_or_else(|| panic!("malformed field: {f}"));
                assert!(
                    s.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                    "token spelling: {s}"
                );
                Some(s.to_string())
            }
        })
        .collect()
}

fn label_rows() -> Vec<LabelRow> {
    between(&script_text(), "row-labels")
        .iter()
        .map(|l| {
            let f = fields(l);
            assert_eq!(f.len(), 4, "row-labels has four fields: {l}");
            (
                f[0].clone().expect("state"),
                f[1].clone(),
                f[2].clone().expect("label"),
                f[3].clone(),
            )
        })
        .collect()
}

fn setting(comb: &str) -> RunSetting {
    let comb = OsString::from(comb);
    RunSetting::from_lookup(move |n| {
        if n == COMBINATION_ENV {
            Some(comb.clone())
        } else {
            None
        }
    })
}

#[test]
fn script_row_labels_match_row_state() {
    let rows = label_rows();
    assert!(!rows.is_empty());
    for (state, reason, label, label_reason) in &rows {
        let rs = RowState::from_tokens(state, reason.as_deref())
            .unwrap_or_else(|| panic!("state not accepted by RowState: {state} {reason:?}"));
        let comb = MachineCombination::MetalMacMini;
        let check = RowCheck::new(
            Subject::from_parts("row-x", &["BUILD-41"], &[]).unwrap(),
            Declaration::from_parts(&[comb], &[]).unwrap(),
            rs,
        );
        let list: Vec<&dyn Check> = vec![&check];
        let set = CheckSet::new(&list).unwrap();
        let outcomes = run_checks(&set, &setting(comb.name()));
        assert_eq!(outcomes.len(), 1);
        let l = outcomes[0].label();
        assert_eq!(l.name(), label, "label for {state} {reason:?}");
        assert_eq!(
            l.reason_name(),
            label_reason.as_deref(),
            "reason for {state}"
        );
    }
}

#[test]
fn script_row_labels_cover_every_row_state() {
    // 網羅的な match: RowState か ReferenceGap に値が増えるとここがコンパイルで落ちる。
    let all: Vec<RowState> = [
        RowState::Match,
        RowState::Mismatch,
        RowState::NotRun(ReferenceGap::Missing),
        RowState::NotRun(ReferenceGap::Unavailable),
    ]
    .into_iter()
    .inspect(|s| match s {
        RowState::Match | RowState::Mismatch => {}
        RowState::NotRun(g) => match g {
            ReferenceGap::Missing | ReferenceGap::Unavailable => {}
        },
    })
    .collect();
    let mut from_table: Vec<RowState> = label_rows()
        .iter()
        .map(|(s, r, _, _)| RowState::from_tokens(s, r.as_deref()).expect("accepted"))
        .collect();
    assert_eq!(
        from_table.len(),
        all.len(),
        "no missing, extra, or duplicate row"
    );
    for s in &all {
        let n = from_table.iter().filter(|t| *t == s).count();
        assert_eq!(n, 1, "exactly one table row for {s:?}");
    }
    from_table.dedup();
    assert_eq!(from_table.len(), all.len());
}

#[test]
fn script_combinations_match_machine_combination() {
    let rows: Vec<(String, String)> = between(&script_text(), "combinations")
        .iter()
        .map(|l| {
            let f = fields(l);
            assert_eq!(f.len(), 2, "combinations has two fields: {l}");
            (f[0].clone().unwrap(), f[1].clone().unwrap())
        })
        .collect();
    let want: Vec<(String, String)> = MachineCombination::ALL
        .iter()
        .map(|c| (c.name().to_string(), c.machine().name().to_string()))
        .collect();
    assert_eq!(rows, want);
}

#[test]
fn script_behavior_ids_are_valid() {
    let text = script_text();
    let line = text
        .lines()
        .find(|l| l.starts_with("BEHAVIOR_IDS = ["))
        .expect("BEHAVIOR_IDS line");
    let id = line
        .strip_prefix("BEHAVIOR_IDS = [\"")
        .and_then(|l| l.split('"').next())
        .expect("one id");
    assert!(Subject::from_parts("row-x", &[id], &[]).is_ok());
}

#[test]
fn script_source_tokens_are_known() {
    let text = script_text();
    for s in ReferenceSource::ALL {
        assert!(text.contains(&format!("\"{}\"", s.name())));
    }
}
