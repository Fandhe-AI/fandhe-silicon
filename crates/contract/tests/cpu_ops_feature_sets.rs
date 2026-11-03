//! `cpu-ops` を含まない組の成立を、設定の文字列から固定する試験（spec CPU-28、TASK-2457 のポインタ）。
//!
//! - `Cargo.toml` の `[features]` で `contract-core` から含意を辿った先に `cpu-ops` が無い。
//! - `scripts/build-profiles.sh` の `NATIVE_SETS` に `contract-core` だけの組の行がちょうど 1 行ある。
//!
//! 外部 crate もコマンドの起動も使わず、ファイルの文字列だけを読む。
//! 判定は純粋関数に分け、実物を 1 か所だけ変えた入力を渡す陽性対照を常設する。
//! 条件のない試験なので、feature の組に依らず同じ結果になる。

use std::collections::BTreeSet;

/// `[features]` の区画から `name = ["a", "b"]` の行を読み、`start` から含意を辿った feature の集合を返す。
fn closure(manifest: &str, start: &str) -> Result<BTreeSet<String>, String> {
    let mut in_features = false;
    let mut edges: Vec<(String, Vec<String>)> = Vec::new();
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_features = t == "[features]";
            continue;
        }
        if !in_features || t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (name, rest) = t.split_once('=').ok_or(format!("no '=' in: {t}"))?;
        let body = rest.split('#').next().unwrap_or("");
        let inner = body
            .trim()
            .strip_prefix('[')
            .and_then(|b| b.rsplit_once(']'))
            .ok_or(format!("not a list: {t}"))?
            .0;
        let items = inner
            .split(',')
            .map(|i| i.trim().trim_matches('"').to_string())
            .filter(|i| !i.is_empty())
            .collect();
        edges.push((name.trim().to_string(), items));
    }
    if !edges.iter().any(|(n, _)| n == start) {
        return Err(format!("feature {start} not found"));
    }
    let mut seen = BTreeSet::new();
    let mut stack = vec![start.to_string()];
    while let Some(f) = stack.pop() {
        if seen.insert(f.clone()) {
            for (n, items) in &edges {
                if *n == f {
                    stack.extend(items.iter().cloned());
                }
            }
        }
    }
    Ok(seen)
}

fn manifest_problems(manifest: &str) -> Vec<String> {
    match closure(manifest, "contract-core") {
        Err(e) => vec![e],
        Ok(set) if set.contains("cpu-ops") => {
            vec!["contract-core implies cpu-ops".to_string()]
        }
        Ok(_) => Vec::new(),
    }
}

/// `NATIVE_SETS=(` から `)` までの引用符の中の組を返す。
fn native_sets(text: &str) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let opens: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim() == "NATIVE_SETS=(")
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
        let rest = t.split_once('"').ok_or(format!("no quote in: {t}"))?.1;
        let set = rest.split_once('"').ok_or(format!("unterminated: {t}"))?.0;
        rows.push(set.to_string());
    }
    Err("NATIVE_SETS is not closed".to_string())
}

fn script_problems(text: &str) -> Vec<String> {
    match native_sets(text) {
        Err(e) => vec![e],
        Ok(rows) => {
            let n = rows
                .iter()
                .filter(|r| r.as_str() == "contract-core")
                .count();
            if n == 1 {
                Vec::new()
            } else {
                vec![format!("contract-core only row appears {n} times")]
            }
        }
    }
}

fn read(rel: &str) -> String {
    let path = format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

#[test]
fn real_manifest_contract_core_does_not_imply_cpu_ops() {
    let p = manifest_problems(&read("Cargo.toml"));
    assert!(p.is_empty(), "unexpected problems: {p:?}");
}

#[test]
fn manifest_mutations_are_caught() {
    let t = read("Cargo.toml");
    let direct = t.replacen("contract-core = []", "contract-core = [\"cpu-ops\"]", 1);
    assert_ne!(t, direct);
    assert!(!manifest_problems(&direct).is_empty());
    // 別の feature を経由する含意も辿る。
    let via = direct.replacen(
        "contract-core = [\"cpu-ops\"]",
        "contract-core = [\"cpu-isa\"]",
        1,
    );
    let via = via.replacen("cpu-isa = []", "cpu-isa = [\"cpu-ops\"]", 1);
    assert_ne!(t, via);
    assert!(!manifest_problems(&via).is_empty());
    assert!(!manifest_problems("[features]\ncpu-ops = []\n").is_empty());
}

#[test]
fn real_script_has_the_contract_core_only_row() {
    let p = script_problems(&read("../../scripts/build-profiles.sh"));
    assert!(p.is_empty(), "unexpected problems: {p:?}");
}

#[test]
fn script_mutations_are_caught() {
    let t = read("../../scripts/build-profiles.sh");
    let line = t
        .lines()
        .find(|l| l.trim_start().starts_with("\"contract-core\""))
        .expect("contract-core only row")
        .to_string();
    let removed = t.replacen(&format!("{line}\n"), "", 1);
    assert_ne!(t, removed);
    assert!(!script_problems(&removed).is_empty());
    let dup = t.replacen(&line, &format!("{line}\n{line}"), 1);
    assert_ne!(t, dup);
    assert!(!script_problems(&dup).is_empty());
    let commented = t.replacen(&line, &format!("  #{}", line.trim_start()), 1);
    assert_ne!(t, commented);
    assert!(!script_problems(&commented).is_empty());
    let widened = t.replacen(
        &line,
        &line.replacen("\"contract-core\"", "\"contract-core,cpu-isa\"", 1),
        1,
    );
    assert_ne!(t, widened);
    assert!(!script_problems(&widened).is_empty());
    assert!(native_sets("nothing\n").is_err());
}
