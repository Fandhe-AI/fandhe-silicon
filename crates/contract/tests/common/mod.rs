//! 背後実装ごとの `unsafe` の個数テストが共有する、数える道具と検査の本体。
//!
//! `tests/backend_<名前>_unsafe_count.rs`（Metal / Vulkan / CUDA）が `mod common;` で取り込み、
//! 期待値の表を渡して [`check_backend`] を呼ぶ。見本の文字列で道具そのものを確かめるテストは
//! 別の Issue（TASK-9 の 3/5）が同じ入口へ足す。方針は `docs/unsafe-policy.md`（案）を参照。
//!
//! 数え方: コメントと文字列リテラルを空白へ置き換えたあと、識別子の境界で切った `unsafe`
//! キーワードを拾い、直後のトークンで種別に分ける。外部依存は持たず、標準ライブラリだけを使う。
//!
//! 既知の限界（本枠では広げない）:
//! - 数えるのはソースの文字列。外部 crate のマクロ展開が生む `unsafe` は見えない（依存は現状 0）。
//! - 走査は `src/` 配下の `.rs` だけ。`build.rs` や `#[path]` で `src/` の外を指すモジュールは対象外。
//! - マクロ定義・呼び出しの中の `unsafe` もトークンとして数える（多めに数える側）。

use std::fs;
use std::path::Path;

/// 枠が知っている背後実装の名前。背後実装を足すときの登録点（テストファイルの存在も検査する）。
pub const BACKENDS: &[&str] = &["metal", "vulkan", "cuda"];

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

/// 期待値の表の 1 行。
pub struct ExpectedFile {
    /// crate ルートからの相対パス（例: `src/backend_metal.rs`）。
    pub path: &'static str,
    /// `include_str!` で埋めたそのファイルの中身。`path` との取り違えを検出するために使う。
    pub source: &'static str,
    /// そのファイルの `unsafe` の期待個数。
    pub expected: UnsafeCounts,
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
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
                // 文字リテラル（エスケープあり）。閉じの `'` まで進む。
                i += 2;
                while i < cs.len() && cs[i] != '\'' {
                    i += 1;
                }
                i += 1;
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
                i += 1;
                if cs.get(i) == Some(&'\\') {
                    i += 1;
                }
                while i < cs.len() && cs[i] != '\'' {
                    i += 1;
                }
                i += 1;
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

/// `src/` 配下の相対パスから、持ち主の背後実装の名前を返す（backend 以外は `None`）。
fn owner_of(rel_in_src: &str) -> Option<&'static str> {
    let top = rel_in_src.split('/').next().unwrap_or("");
    BACKENDS
        .iter()
        .copied()
        .find(|n| top == format!("backend_{n}.rs") || top == format!("backend_{n}"))
}

/// `dir` 以下の `.rs` を再帰的に集める。シンボリックリンクは追わずエラーにする。
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

/// 背後実装 1 つ分の検査の全体。不一致・取りこぼしを集めて 1 つの `Err` で返す（fail-closed）。
///
/// 検査: 登録済みの名前か／表の各行がその背後実装の持ち場か・重複が無いか／`include_str!` の中身が
/// 実ファイルと一致するか／個数の照合／持ち場の `.rs` が表に載っているか／backend 以外に `unsafe` が
/// 無いか／登録済みの全背後実装にテストファイルがあるか。
pub fn check_backend(backend: &str, table: &[ExpectedFile]) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();
    if !BACKENDS.contains(&backend) {
        return Err(format!("backend '{backend}' is not registered in BACKENDS"));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut seen: Vec<&str> = Vec::new();
    for row in table {
        if seen.contains(&row.path) {
            errors.push(format!("{}: duplicated in the expected table", row.path));
            continue;
        }
        seen.push(row.path);
        let owner = row.path.strip_prefix("src/").and_then(owner_of);
        if owner != Some(backend) {
            errors.push(format!("{}: not a file of backend '{backend}'", row.path));
            continue;
        }
        match fs::read_to_string(root.join(row.path)) {
            Ok(on_disk) if on_disk == row.source => {}
            Ok(_) => errors.push(format!(
                "{}: include_str! content differs from the file",
                row.path
            )),
            Err(e) => errors.push(format!("{}: cannot read: {e}", row.path)),
        }
        if let Err(m) = compare_counts(row.path, &row.expected, &count_unsafe(row.source)) {
            errors.push(m);
        }
    }

    let mut files = Vec::new();
    collect_rs(root, &root.join("src"), &mut files, &mut errors);
    for f in &files {
        let in_src = f.strip_prefix("src/").unwrap_or(f);
        match owner_of(in_src) {
            Some(o) if o == backend => {
                if !seen.contains(&f.as_str()) {
                    errors.push(format!("{f}: not listed in the expected table"));
                }
            }
            Some(_) => {}
            None => match fs::read_to_string(root.join(f)) {
                Ok(s) => {
                    let n = count_unsafe(&s).total();
                    if n > 0 {
                        errors.push(format!(
                            "{f}: {n} unsafe keyword(s) outside backend modules"
                        ));
                    }
                }
                Err(e) => errors.push(format!("{f}: cannot read: {e}")),
            },
        }
    }

    for n in BACKENDS {
        let t = format!("tests/backend_{n}_unsafe_count.rs");
        if !root.join(&t).is_file() {
            errors.push(format!(
                "{t}: missing test file for registered backend '{n}'"
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}
