//! MSL のソースから `kernel void <名前>(` の入口名を出現の順に抜き出す純粋な関数（TASK-485 のポインタ）。

/// 空白（Unicode の White_Space）を読み飛ばす。`min` 文字に満たなければ `None`。
fn skip_ws(s: &str, min: usize) -> Option<&str> {
    let rest = s.trim_start_matches(char::is_whitespace);
    let skipped = s[..s.len() - rest.len()].chars().count();
    (skipped >= min).then_some(rest)
}

/// 位置 `s` から `kernel void <名前> (` が合えば、名前と `(` の後ろの残りを返す。
fn match_at(s: &str) -> Option<(&str, &str)> {
    let s = s.strip_prefix("kernel")?;
    let s = skip_ws(s, 1)?;
    let s = s.strip_prefix("void")?;
    let s = skip_ws(s, 1)?;
    let first = s.chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    let (name, after) = s.split_at(end);
    let after = skip_ws(after, 0)?;
    let after = after.strip_prefix('(')?;
    Some((name, after))
}

/// 正規表現 `kernel\s+void\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(` と同じ規則で、先頭から重ならずに入口名を拾う。
///
/// コメントは除かない。語の境界も求めない。
pub fn scan_entries(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        if let Some((name, after)) = match_at(rest) {
            out.push(name);
            rest = after;
        } else {
            let step = rest.chars().next().map_or(1, char::len_utf8);
            rest = &rest[step..];
        }
    }
    out
}
