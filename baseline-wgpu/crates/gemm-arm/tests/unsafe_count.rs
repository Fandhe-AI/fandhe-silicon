//! この crate の `unsafe` の個数の試験（spec TASK-1010 のポインタ）。
//!
//! `src/` の下の `.rs` を走査して、`unsafe` の種別ごとの個数を表 `EXPECTED` と照合する。
//! 数え方は `docs/unsafe-policy.md` の「個数テストの枠の形」に従い、この crate の中に自前で持つ
//! （製品の crate の試験の道具には依存しない。外部依存 0）。表に無いファイルは全種別 0 を期待し、
//! 表の行のファイルが無いのも失敗にする。`#[ignore]`・`cfg` では外さない。

use std::fs;
use std::path::Path;
/// `unsafe` の種別ごとの個数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsafeCounts {
    /// `unsafe { }` ブロック。
    pub block: usize,
    /// `unsafe impl`。
    pub impl_: usize,
    /// `unsafe fn`（`unsafe extern "…" fn`・関数ポインタ型を含む）。
    pub func: usize,
    /// `unsafe extern "…" { }` ブロック。
    pub extern_block: usize,
    /// 上のどれでもないもの（`unsafe trait`・`#[unsafe(...)]` など）。
    pub other: usize,
}

impl UnsafeCounts {
    /// 全種別 0。現状の全背後実装の期待値。
    pub const ZERO: UnsafeCounts = UnsafeCounts {
        block: 0,
        impl_: 0,
        func: 0,
        extern_block: 0,
        other: 0,
    };

    /// 全種別の合計。
    pub fn total(&self) -> usize {
        self.block + self.impl_ + self.func + self.extern_block + self.other
    }

    fn fields(&self) -> [(&'static str, usize); 5] {
        [
            ("block", self.block),
            ("impl", self.impl_),
            ("fn", self.func),
            ("extern block", self.extern_block),
            ("other", self.other),
        ]
    }
}

/// 識別子を構成する文字か。Rust の `XID_Continue` を標準ライブラリだけで近似する。
///
/// 非 ASCII の文字は、コメント・リテラルの外では識別子にしか現れないので、空白（Rust が空白と
/// みなす `U+200E` / `U+200F` を含む）以外はすべて識別子の一部として扱う。これで結合文字・連結文字
/// （例: `unsafe` + `U+0301`）を含む識別子が `unsafe` キーワードの途中で切れない。
fn is_ident(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric() || c == '_'
    } else {
        !c.is_whitespace() && !matches!(c, '\u{200E}' | '\u{200F}')
    }
}

/// 文字・バイト文字リテラルの本体（開きの `'` の次から）を読み飛ばす。閉じの次の位置を返す。
///
/// エスケープ（`\'`・`\\`・`\x..`・`\u{..}` 等）は全体を読み飛ばしてから、エスケープされていない
/// 閉じの `'` を探す。閉じが見つからない不正な形は閉じを消費しない。
fn skip_char_body(cs: &[char], mut i: usize) -> usize {
    if cs.get(i) == Some(&'\\') {
        i += 1;
        match cs.get(i) {
            Some('x') => i += 3,
            Some('u') => {
                while i < cs.len() && cs[i] != '}' && cs[i] != '\'' && cs[i] != '\n' {
                    i += 1;
                }
                i += 1;
            }
            Some(_) => i += 1,
            None => return i,
        }
    } else {
        i += 1;
    }
    if cs.get(i) == Some(&'\'') { i + 1 } else { i }
}

/// 文字列リテラルの本体（開きの `"` の次から）を読み飛ばす。閉じの次の位置を返す。
fn skip_string(cs: &[char], mut i: usize, out: &mut String) -> usize {
    while i < cs.len() {
        match cs[i] {
            '\\' => {
                if cs.get(i + 1) == Some(&'\n') {
                    out.push('\n');
                }
                i += 2;
            }
            '"' => return i + 1,
            '\n' => {
                out.push('\n');
                i += 1;
            }
            _ => i += 1,
        }
    }
    i
}

/// raw 文字列の本体（開きの `"` の次から）を、`#` を `hashes` 個そろえた閉じまで読み飛ばす。
fn skip_raw_string(cs: &[char], mut i: usize, hashes: usize, out: &mut String) -> usize {
    while i < cs.len() {
        if cs[i] == '"' && (0..hashes).all(|k| cs.get(i + 1 + k) == Some(&'#')) {
            return i + 1 + hashes;
        }
        if cs[i] == '\n' {
            out.push('\n');
        }
        i += 1;
    }
    i
}

/// コメントと文字列・文字リテラルを空白へ置き換える（改行は残す）。
///
/// 数える前の前処理。raw 識別子 `r#unsafe` は `r_unsafe` に直し、キーワードと取り違えない。
pub fn strip_comments_and_literals(src: &str) -> String {
    let cs: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let next = cs.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
            out.push(' ');
        } else if c == '/' && next == Some('*') {
            let mut depth = 1;
            i += 2;
            while i < cs.len() && depth > 0 {
                if cs[i] == '/' && cs.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if cs[i] == '*' && cs.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    if cs[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            out.push(' ');
        } else if c == '"' {
            i = skip_string(&cs, i + 1, &mut out);
            out.push(' ');
        } else if c == '\'' {
            if next == Some('\\') {
                // 文字リテラル（エスケープあり）。エスケープ全体を飛ばして閉じの `'` へ進む。
                i = skip_char_body(&cs, i + 1);
                out.push(' ');
            } else if cs.get(i + 2) == Some(&'\'') {
                i += 3;
                out.push(' ');
            } else {
                // ライフタイム・ラベル。続く識別子は通常どおり読む。
                out.push('\'');
                i += 1;
            }
        } else if is_ident(c) {
            let start = i;
            while i < cs.len() && is_ident(cs[i]) {
                i += 1;
            }
            let word: String = cs[start..i].iter().collect();
            let after = cs.get(i).copied();
            let raw_prefix = matches!(word.as_str(), "r" | "br" | "cr");
            let str_prefix = matches!(word.as_str(), "b" | "c");
            if raw_prefix && matches!(after, Some('"') | Some('#')) {
                let mut j = i;
                while cs.get(j) == Some(&'#') {
                    j += 1;
                }
                let hashes = j - i;
                if cs.get(j) == Some(&'"') {
                    i = skip_raw_string(&cs, j + 1, hashes, &mut out);
                    out.push(' ');
                } else if word == "r" && hashes == 1 && cs.get(j).is_some_and(|&x| is_ident(x)) {
                    // raw 識別子 `r#name`。キーワードとして拾わないよう `r_name` にする。
                    out.push_str("r_");
                    i = j;
                } else {
                    out.push_str(&word);
                }
            } else if str_prefix && after == Some('"') {
                i = skip_string(&cs, i + 1, &mut out);
                out.push(' ');
            } else if word == "b" && after == Some('\'') {
                // バイト文字リテラル。`b'"'` で文字列の走査がずれないよう読み飛ばす。
                i = skip_char_body(&cs, i + 1);
                out.push(' ');
            } else {
                out.push_str(&word);
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// 前処理済みの文字列から、境界で切った識別子の列を位置つきで取り出す。
fn words(cs: &[char]) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if is_ident(cs[i]) {
            let start = i;
            while i < cs.len() && is_ident(cs[i]) {
                i += 1;
            }
            v.push((start, cs[start..i].iter().collect()));
        } else {
            i += 1;
        }
    }
    v
}

/// ソース文字列の `unsafe` を種別ごとに数える（前処理を含む）。
///
/// 全種別の合計が拾ったキーワードの総数と食い違う場合は panic する（どの種別にも入らないものを作らない）。
pub fn count_unsafe(src: &str) -> UnsafeCounts {
    let stripped = strip_comments_and_literals(src);
    let cs: Vec<char> = stripped.chars().collect();
    let ws = words(&cs);
    let mut counts = UnsafeCounts::ZERO;
    let mut found = 0;
    for (idx, (pos, w)) in ws.iter().enumerate() {
        if w != "unsafe" {
            continue;
        }
        found += 1;
        // キーワードの後ろで空白を飛ばした次の文字と、それが識別子ならその語。
        let mut k = pos + w.chars().count();
        let mut nth = idx + 1;
        let kind = loop {
            while k < cs.len() && cs[k].is_whitespace() {
                k += 1;
            }
            let word_here = ws
                .get(nth)
                .filter(|(p, _)| *p == k)
                .map(|(_, s)| s.as_str());
            match (cs.get(k), word_here) {
                (Some('{'), _) => break "block",
                (_, Some("impl")) => break "impl",
                (_, Some("fn")) => break "fn",
                (_, Some("extern")) => {
                    // 省略可能な ABI（前処理で空白になっている）を飛ばした先が `fn` か `{` か。
                    k += "extern".len();
                    nth += 1;
                    while k < cs.len() && cs[k].is_whitespace() {
                        k += 1;
                    }
                    let w2 = ws
                        .get(nth)
                        .filter(|(p, _)| *p == k)
                        .map(|(_, s)| s.as_str());
                    break match (cs.get(k), w2) {
                        (_, Some("fn")) => "fn",
                        (Some('{'), _) => "extern block",
                        _ => "other",
                    };
                }
                // `unsafe const fn` / `unsafe async fn` は修飾子を読み飛ばして先で決める。
                (_, Some(q @ ("const" | "async"))) => {
                    k += q.len();
                    nth += 1;
                }
                _ => break "other",
            }
        };
        match kind {
            "block" => counts.block += 1,
            "impl" => counts.impl_ += 1,
            "fn" => counts.func += 1,
            "extern block" => counts.extern_block += 1,
            _ => counts.other += 1,
        }
    }
    assert_eq!(
        counts.total(),
        found,
        "unsafe kinds must sum to the keyword total"
    );
    counts
}

/// 期待値と実際の個数を照合する。不一致は ファイル・種別・期待・実際 を並べたメッセージで返す。
pub fn compare_counts(
    path: &str,
    expected: &UnsafeCounts,
    actual: &UnsafeCounts,
) -> Result<(), String> {
    let diffs: Vec<String> = expected
        .fields()
        .iter()
        .zip(actual.fields())
        .filter(|((_, e), (_, a))| e != a)
        .map(|((kind, e), (_, a))| format!("{path}: {kind}: expected {e}, actual {a}"))
        .collect();
    if diffs.is_empty() {
        Ok(())
    } else {
        Err(diffs.join("\n"))
    }
}
/// 期待値の表。`src/` からの相対パスから、種別ごとの個数。表に無いファイルは全種別 0 を期待する。
const EXPECTED: &[(&str, UnsafeCounts)] = &[(
    "bridge.rs",
    UnsafeCounts {
        block: 2,
        impl_: 0,
        func: 1,
        extern_block: 0,
        other: 0,
    },
)];

/// `dir` 以下の `.rs` を再帰で集める。シンボリックリンクは追わずに失敗として積む。
fn collect_rs(root: &Path, dir: &Path, files: &mut Vec<String>, errors: &mut Vec<String>) {
    let rel = |p: &Path| {
        p.strip_prefix(root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default()
    };
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            errors.push(format!("{}: cannot read directory: {e}", rel(dir)));
            return;
        }
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                errors.push(format!("{}: symlink is not allowed under src/", rel(&p)));
            }
            Ok(m) if m.is_dir() => collect_rs(root, &p, files, errors),
            Ok(_) if p.extension().is_some_and(|x| x == "rs") => files.push(rel(&p)),
            Ok(_) => {}
            Err(err) => errors.push(format!("{}: cannot stat: {err}", rel(&p))),
        }
    }
}

/// `src_dir` の全ファイルを `expected` と照合する。失敗は集めて 1 回で返す。
fn check_dir(src_dir: &Path, expected: &[(&str, UnsafeCounts)]) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    collect_rs(src_dir, src_dir, &mut files, &mut errors);
    for (path, _) in expected {
        if !files.iter().any(|f| f == path) {
            errors.push(format!("{path}: listed in the expected table but missing"));
        }
    }
    for file in &files {
        let want = expected
            .iter()
            .find(|(p, _)| p == file)
            .map_or(UnsafeCounts::ZERO, |(_, c)| *c);
        match fs::read_to_string(src_dir.join(file)) {
            Ok(src) => {
                if let Err(msg) = compare_counts(file, &want, &count_unsafe(&src)) {
                    errors.push(msg);
                }
            }
            Err(e) => errors.push(format!("{file}: cannot read: {e}")),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

#[test]
fn unsafe_count_matches_expected() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    if let Err(msg) = check_dir(&src, EXPECTED) {
        panic!("{msg}");
    }
}

#[test]
fn shifted_table_fails() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let shifted = [(
        "bridge.rs",
        UnsafeCounts {
            block: 3,
            func: 1,
            ..UnsafeCounts::ZERO
        },
    )];
    let err = check_dir(&src, &shifted).expect_err("a shifted table must fail");
    assert!(
        err.contains("bridge.rs: block: expected 3, actual 2"),
        "{err}"
    );
}

#[test]
fn file_outside_the_table_expects_zero() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let err = check_dir(&src, &[]).expect_err("an empty table must fail on bridge.rs");
    assert!(
        err.contains("bridge.rs: block: expected 0, actual 2"),
        "{err}"
    );
}

#[test]
fn missing_table_row_fails() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let table = [("no_such_file.rs", UnsafeCounts::ZERO)];
    let err = check_dir(&src, &table).expect_err("a missing row must fail");
    assert!(err.contains("no_such_file.rs"), "{err}");
}

#[test]
fn comments_and_literals_are_not_counted() {
    let src = r##"
// unsafe { }
/* unsafe { /* nested unsafe */ } */
const A: &str = "unsafe { }";
const B: &str = r#"unsafe fn x() {}"#;
fn f() {}
"##;
    assert_eq!(count_unsafe(src), UnsafeCounts::ZERO);
}

#[test]
fn identifiers_containing_unsafe_are_not_counted() {
    let src = "#![deny(unsafe_code)]\n#[allow(unsafe_code)]\nfn r#unsafe() {}\n";
    assert_eq!(count_unsafe(src), UnsafeCounts::ZERO);
}

#[test]
fn kinds_are_separated() {
    let src = r#"
fn a() { unsafe { f() } }
unsafe impl Send for X {}
pub unsafe fn b() {}
unsafe extern "C" { fn g(); }
unsafe extern "C" fn c() {}
unsafe trait T {}
"#;
    let c = count_unsafe(src);
    assert_eq!(
        (c.block, c.impl_, c.func, c.extern_block, c.other),
        (1, 1, 2, 1, 1)
    );
}
