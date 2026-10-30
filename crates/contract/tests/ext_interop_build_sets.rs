//! `scripts/build-profiles.sh` の `NATIVE_SETS` にある、片方の背後実装だけに `ext-interop` を足した
//! 3 組の行の存在を固定する試験（spec TASK-231、BUILD-14 のポインタ）。
//!
//! 両方の背後実装を含む本番の組（BUILD-4）の行は `feature_default_scope.rs` が受け持つので、
//! ここでは `backend-vulkan,ext-interop`・`backend-cuda,ext-interop`・`backend-metal,ext-interop` の 3 行だけを守る。
//! 対象を `NATIVE_SETS` の区画に限るのは、行が `WASM_SETS` へ移っても通ってしまわないようにするため
//! （両背後実装のモジュールは wasm32 では有効にならず、そこでは確かめにならない）。
//! 判定は純粋関数 `problems` に分け、実物の本文を 1 か所だけ変えた入力を渡す陽性対照を常設する。
//! 条件のない試験なので、`make verify` の `cargo test` と `build-profiles.sh` のどの組でも結果は同じになる。
//! 想定外の形は黙って通さず、問題として積んで落とす。

use std::collections::BTreeSet;

const TAG: &str = "# spec TASK-231";
const EXPECTED: [&[&str]; 3] = [
    &["backend-vulkan", "ext-interop"],
    &["backend-cuda", "ext-interop"],
    &["backend-metal", "ext-interop"],
];
const BACKENDS: [&str; 3] = ["backend-vulkan", "backend-cuda", "backend-metal"];

/// 組と注記（`#` 以降。無ければ空）。
type Row = (String, String);

/// `NATIVE_SETS=(` から閉じの `)` までの行を読む。空行とコメント行は飛ばす。
fn native_sets(text: &str) -> Result<Vec<Row>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let opens: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == "NATIVE_SETS=(")
        .map(|(i, _)| i)
        .collect();
    if opens.len() != 1 {
        return Err(format!(
            "expected one NATIVE_SETS opener, found {}",
            opens.len()
        ));
    }
    let mut rows = Vec::new();
    for line in &lines[opens[0] + 1..] {
        let t = line.trim();
        if t == ")" {
            return Ok(rows);
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let start = t
            .find('"')
            .ok_or_else(|| format!("no quoted set in line: {t}"))?;
        let rest = &t[start + 1..];
        let end = rest
            .find('"')
            .ok_or_else(|| format!("unterminated quote in line: {t}"))?;
        let after = &rest[end + 1..];
        let note = after.find('#').map_or("", |i| &after[i..]);
        rows.push((rest[..end].to_string(), note.to_string()));
    }
    Err("NATIVE_SETS is not closed".to_string())
}

fn as_set(s: &str) -> BTreeSet<&str> {
    s.split(',').collect()
}

/// 違反を文字列で返す。空なら 3 行は期待どおり。
fn problems(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for exp in EXPECTED {
        let backends = exp.iter().filter(|f| BACKENDS.contains(f)).count();
        if backends != 1 || !exp.contains(&"ext-interop") {
            out.push(format!("expected table entry is malformed: {exp:?}"));
        }
    }
    let rows = match native_sets(text) {
        Ok(r) => r,
        Err(e) => {
            out.push(e);
            return out;
        }
    };
    let tagged = rows.iter().filter(|(_, n)| n.contains(TAG)).count();
    if tagged != EXPECTED.len() {
        out.push(format!(
            "expected {} rows tagged {TAG}, found {tagged}",
            EXPECTED.len()
        ));
    }
    for exp in EXPECTED {
        let want: BTreeSet<&str> = exp.iter().copied().collect();
        let hits: Vec<&Row> = rows.iter().filter(|(s, _)| as_set(s) == want).collect();
        if hits.len() != 1 {
            out.push(format!(
                "set {exp:?} appears {} times in NATIVE_SETS",
                hits.len()
            ));
        } else if !hits[0].1.contains(TAG) {
            out.push(format!("set {exp:?} lacks the {TAG} note"));
        }
    }
    out
}

fn real_script() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/build-profiles.sh"
    ))
    .expect("build-profiles.sh must be readable")
}

const LINE_STARTS: [&str; 3] = [
    "  \"backend-vulkan,ext-interop\"",
    "  \"backend-cuda,ext-interop\"",
    "  \"backend-metal,ext-interop\"",
];

/// 指定の行頭で始まる実物の 1 行を返す。
fn line_of(text: &str, start: &str) -> String {
    text.lines()
        .find(|l| l.starts_with(start))
        .unwrap_or_else(|| panic!("line not found: {start}"))
        .to_string()
}

/// 本文が実際に変わったことを確かめてから返す（対照が空振りしないように）。
fn changed(orig: &str, new: String) -> String {
    assert_ne!(orig, new, "mutation must change the text");
    new
}

#[test]
fn real_script_has_no_problems() {
    let p = problems(&real_script());
    assert!(p.is_empty(), "unexpected problems: {p:?}");
}

#[test]
fn reader_skips_comments_and_blank_lines_and_extracts_notes() {
    let text = "x\nNATIVE_SETS=(\n\n  # c\n  \"a,b\"   # note\n  \"c\"\n)\n";
    let rows = native_sets(text).expect("must parse");
    assert_eq!(
        rows,
        vec![
            ("a,b".to_string(), "# note".to_string()),
            ("c".to_string(), String::new())
        ]
    );
}

#[test]
fn reader_rejects_malformed_inputs() {
    assert!(native_sets("nothing\n").is_err());
    assert!(native_sets("NATIVE_SETS=(\nNATIVE_SETS=(\n)\n").is_err());
    assert!(native_sets("NATIVE_SETS=(\n  \"a\"\n").is_err());
    assert!(native_sets("NATIVE_SETS=(\n  bare\n)\n").is_err());
}

#[test]
fn removing_either_row_is_caught() {
    let t = real_script();
    for start in LINE_STARTS {
        let line = line_of(&t, start);
        let m = changed(&t, t.replacen(&format!("{line}\n"), "", 1));
        assert!(!problems(&m).is_empty(), "removal of {start} not caught");
    }
}

#[test]
fn duplicating_either_row_is_caught() {
    let t = real_script();
    for start in LINE_STARTS {
        let line = line_of(&t, start);
        let m = changed(&t, t.replacen(&line, &format!("{line}\n{line}"), 1));
        assert!(!problems(&m).is_empty(), "duplicate of {start} not caught");
    }
}

#[test]
fn commenting_out_either_row_is_caught() {
    let t = real_script();
    for start in LINE_STARTS {
        let line = line_of(&t, start);
        let m = changed(
            &t,
            t.replacen(&line, &format!("  #{}", line.trim_start()), 1),
        );
        assert!(
            !problems(&m).is_empty(),
            "comment-out of {start} not caught"
        );
    }
}

#[test]
fn dropping_ext_interop_from_either_row_is_caught() {
    let t = real_script();
    for start in LINE_STARTS {
        let line = line_of(&t, start);
        let m = changed(
            &t,
            t.replacen(&line, &line.replacen(",ext-interop", "", 1), 1),
        );
        assert!(
            !problems(&m).is_empty(),
            "dropping ext-interop in {start} not caught"
        );
    }
}

#[test]
fn dropping_the_tag_from_either_row_is_caught() {
    let t = real_script();
    for start in LINE_STARTS {
        let line = line_of(&t, start);
        let m = changed(&t, t.replacen(&line, &line.replacen(TAG, "# spec", 1), 1));
        assert!(
            !problems(&m).is_empty(),
            "dropping the tag in {start} not caught"
        );
    }
}

#[test]
fn moving_either_row_to_wasm_sets_is_caught() {
    let t = real_script();
    for start in LINE_STARTS {
        let line = line_of(&t, start);
        let removed = changed(&t, t.replacen(&format!("{line}\n"), "", 1));
        let m = changed(
            &removed,
            removed.replacen("WASM_SETS=(\n", &format!("WASM_SETS=(\n{line}\n"), 1),
        );
        assert!(!problems(&m).is_empty(), "move of {start} not caught");
    }
}
