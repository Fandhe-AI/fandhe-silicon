//! この crate の `src/` の `unsafe` の個数を固定する試験（TASK-1008 のポインタ）。
//!
//! `unsafe` を書いてよいのは `src/metal.rs` だけ。ここで種別ごとの個数を表 `EXPECTED` に固定し、
//! 表に無いファイルは全種別 0 を期待する。数える道具はこのファイルの中に持ち、ほかの crate に依存しない。
//! どの OS でも流れ、`metal.rs` が macOS でしかコンパイルされなくてもソースの文字列として数える。

use std::fs;
use std::path::{Path, PathBuf};

/// 種別の並び。`Counts` の添字と対応する。
const KINDS: [&str; 5] = ["block", "impl", "fn", "extern-block", "other"];

type Counts = [usize; 5];

/// `src/` からの相対パス → 種別ごとの個数（block・impl・fn・extern-block・other）。
const EXPECTED: &[(&str, Counts)] = &[("metal.rs", [15, 0, 0, 0, 0])];

/// コメントと文字列リテラルの中身を空白にした文字列を返す（改行と長さは保つ）。
fn strip(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let blank = |ch: char| if ch == '\n' { '\n' } else { ' ' };
    while i < c.len() {
        let ch = c[i];
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(c[i]));
                    i += 1;
                }
            }
        } else if ch == '"' {
            i = skip_string(&c, i, &mut out);
        } else if ch == '\'' {
            if c.get(i + 1) == Some(&'\\') {
                // 文字リテラル（エスケープつき）。閉じの `'` まで空白にする。
                out.push('\'');
                i += 1;
                while i < c.len() && c[i] != '\'' {
                    if c[i] == '\\' {
                        out.push(' ');
                        i += 1;
                    }
                    if i < c.len() {
                        out.push(' ');
                        i += 1;
                    }
                }
                if i < c.len() {
                    out.push('\'');
                    i += 1;
                }
            } else if c.get(i + 2) == Some(&'\'') {
                out.push_str("' '");
                i += 3;
            } else {
                // ライフタイム。
                out.push('\'');
                i += 1;
            }
        } else if is_ident_start(ch) {
            let start = i;
            while i < c.len() && is_ident_continue(c[i]) {
                i += 1;
            }
            let word: String = c[start..i].iter().collect();
            let raw_prefix = matches!(word.as_str(), "r" | "br" | "cr");
            if raw_prefix && matches!(c.get(i), Some('"') | Some('#')) {
                let mut j = i;
                while c.get(j) == Some(&'#') {
                    j += 1;
                }
                if c.get(j) == Some(&'"') {
                    let hashes = j - i;
                    out.push_str(&word);
                    i = skip_raw_string(&c, i, hashes, &mut out);
                    continue;
                }
                if word == "r" && c.get(j).is_some_and(|x| is_ident_start(*x)) {
                    // 生の識別子 `r#name`。`unsafe` を数えないよう識別子の中身を伏せる。
                    out.push_str("r#");
                    i = j;
                    while i < c.len() && is_ident_continue(c[i]) {
                        out.push('_');
                        i += 1;
                    }
                    continue;
                }
            }
            out.push_str(&word);
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

/// 通常の文字列（`"` から始まる）を飛ばして、中身を空白にして出力する。次の位置を返す。
fn skip_string(c: &[char], mut i: usize, out: &mut String) -> usize {
    out.push('"');
    i += 1;
    while i < c.len() && c[i] != '"' {
        if c[i] == '\\' && i + 1 < c.len() {
            out.push(' ');
            i += 1;
        }
        out.push(if c[i] == '\n' { '\n' } else { ' ' });
        i += 1;
    }
    if i < c.len() {
        out.push('"');
        i += 1;
    }
    i
}

/// 生文字列（`#` の列と `"` から始まる）を飛ばす。次の位置を返す。
fn skip_raw_string(c: &[char], mut i: usize, hashes: usize, out: &mut String) -> usize {
    for _ in 0..hashes {
        out.push('#');
        i += 1;
    }
    out.push('"');
    i += 1;
    while i < c.len() {
        if c[i] == '"' && (0..hashes).all(|h| c.get(i + 1 + h) == Some(&'#')) {
            out.push('"');
            for _ in 0..hashes {
                out.push('#');
            }
            return i + 1 + hashes;
        }
        out.push(if c[i] == '\n' { '\n' } else { ' ' });
        i += 1;
    }
    i
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

/// 前処理済みの文字列を識別子と記号の並びにする。文字列の囲み `""` は 1 つの記号として残る。
fn tokens(clean: &str) -> Vec<String> {
    let c: Vec<char> = clean.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if is_ident_start(ch) {
            let start = i;
            while i < c.len() && is_ident_continue(c[i]) {
                i += 1;
            }
            out.push(c[start..i].iter().collect());
        } else if ch == '"' {
            i += 1;
            while i < c.len() && c[i] != '"' {
                i += 1;
            }
            i += 1;
            out.push("\"\"".to_string());
        } else {
            out.push(ch.to_string());
            i += 1;
        }
    }
    out
}

/// ソースの文字列の `unsafe` を種別ごとに数える。キーワードの総数も返す。
fn count_unsafe(src: &str) -> (Counts, usize) {
    let toks = tokens(&strip(src));
    let mut counts: Counts = [0; 5];
    let mut total = 0;
    for (idx, tok) in toks.iter().enumerate() {
        if tok != "unsafe" {
            continue;
        }
        total += 1;
        let next = toks.get(idx + 1).map(String::as_str);
        let kind = match next {
            Some("{") => 0,
            Some("impl") => 1,
            Some("fn") => 2,
            Some("extern") => {
                let mut j = idx + 2;
                if toks.get(j).map(String::as_str) == Some("\"\"") {
                    j += 1;
                }
                match toks.get(j).map(String::as_str) {
                    Some("fn") => 2,
                    Some("{") => 3,
                    _ => 4,
                }
            }
            _ => 4,
        };
        counts[kind] += 1;
    }
    (counts, total)
}

/// `dir` の下のすべての `.rs` を再帰で集める。シンボリックリンクは失敗にする。
fn collect(dir: &Path, root: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        let meta = fs::symlink_metadata(&path).expect("metadata");
        assert!(!meta.file_type().is_symlink(), "symlink under src");
        if meta.is_dir() {
            collect(&path, root, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path
                .strip_prefix(root)
                .expect("prefix")
                .to_string_lossy()
                .replace('\\', "/");
            files.push((rel, path));
        }
    }
}

/// 期待の表と実際の個数を突き合わせ、食い違いを全件集めて返す。
fn mismatches(actual: &[(String, Counts)], expected: &[(&str, Counts)]) -> Vec<String> {
    let mut out = Vec::new();
    for (path, counts) in actual {
        let want = expected
            .iter()
            .find(|(p, _)| p == path)
            .map_or([0; 5], |(_, c)| *c);
        for (k, name) in KINDS.iter().enumerate() {
            if counts[k] != want[k] {
                out.push(format!(
                    "{path}: {name}: expected {}, actual {}",
                    want[k], counts[k]
                ));
            }
        }
    }
    for (path, want) in expected {
        if !actual.iter().any(|(p, _)| p == path) && want.iter().any(|n| *n != 0) {
            out.push(format!("{path}: missing file"));
        }
    }
    out
}

#[test]
fn unsafe_counts_match_the_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    assert!(!files.is_empty());
    let mut actual = Vec::new();
    for (rel, path) in &files {
        let src = fs::read_to_string(path).expect("read");
        let (counts, total) = count_unsafe(&src);
        assert_eq!(counts.iter().sum::<usize>(), total, "{rel}: kind sum");
        actual.push((rel.clone(), counts));
    }
    let bad = mismatches(&actual, EXPECTED);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn comments_and_strings_are_not_counted() {
    let src = "// unsafe\n/* unsafe /* unsafe */ unsafe */\nfn f() { let _ = \"unsafe\"; let _ = r#\"unsafe \"x\" unsafe\"#; }\n";
    assert_eq!(count_unsafe(src), ([0; 5], 0));
}

#[test]
fn lint_names_and_raw_identifiers_are_not_counted() {
    let src = "#![allow(unsafe_code)]\n#[allow(unsafe_op_in_unsafe_fn)]\nfn f() { let r#unsafe = 1; let _c = 'u'; let _l: &'static str = \"\"; }\n";
    assert_eq!(count_unsafe(src), ([0; 5], 0));
}

#[test]
fn kinds_are_split() {
    let src = "unsafe extern \"C\" { fn g(); }\nfn f() { unsafe { g() } }\nunsafe fn h() {}\nunsafe extern \"C\" fn i() {}\nunsafe impl Send for T {}\n#[unsafe(no_mangle)]\nfn j() {}\nmacro_rules! m { () => { unsafe { 1 } } }\n";
    let (counts, total) = count_unsafe(src);
    assert_eq!(counts, [2, 1, 2, 1, 1]);
    assert_eq!(total, 7);
}

#[test]
fn shifting_the_table_fails() {
    let actual = vec![("a.rs".to_string(), [2, 0, 0, 0, 0])];
    assert!(mismatches(&actual, &[("a.rs", [2, 0, 0, 0, 0])]).is_empty());
    let bad = mismatches(&actual, &[("a.rs", [1, 0, 0, 0, 0])]);
    assert_eq!(bad, vec!["a.rs: block: expected 1, actual 2".to_string()]);
    // 表に無いファイルは全種別 0 を期待する。
    let bad = mismatches(&actual, &[]);
    assert_eq!(bad.len(), 1);
}
