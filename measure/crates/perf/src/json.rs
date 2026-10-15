//! run ごとの記録を JSON の文字列にするための小さな部品（標準ライブラリだけ）。
//!
//! `record` から呼ばれる。文字列は必ず [`push_string`] を通し、出力は ASCII だけにする。
//! 数は 10 進のままで、指数表記・NaN・無限大を出さない。

use crate::stats::MAX_EXACT_NS;
use std::fmt::Write;

/// JSON の文字列リテラル（引用符つき）を `out` に足す。
///
/// `"`・`\`・制御文字・非 ASCII をエスケープする。BMP の外の文字はサロゲート対で出す。
pub fn push_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    // String への書き込みは失敗しない。
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// 整数または `null`（取得不能）を `out` に足す。
pub fn push_opt_u64(out: &mut String, v: Option<u64>) {
    match v {
        Some(n) => {
            let _ = write!(out, "{n}");
        }
        None => out.push_str("null"),
    }
}

/// 中央値を `out` に足す。整数か「整数 + .5」だけを正確な 10 進で書く。
///
/// 負・有限でない・上限（[`MAX_EXACT_NS`]）超え・端数が 0 でも 0.5 でもない値は書かず `false` を返す。
pub fn push_median(out: &mut String, v: f64) -> bool {
    if !v.is_finite() || v < 0.0 || v > MAX_EXACT_NS as f64 {
        return false;
    }
    let whole = v.trunc();
    let half = match v - whole {
        0.0 => false,
        0.5 => true,
        _ => return false,
    };
    let _ = write!(out, "{}", whole as u64);
    if half {
        out.push_str(".5");
    }
    true
}

/// 整数の配列（要素ごとに `null` があり得る）を `out` に足す。
pub fn push_opt_array(out: &mut String, values: impl IntoIterator<Item = Option<u64>>) {
    out.push('[');
    for (i, v) in values.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_opt_u64(out, v);
    }
    out.push(']');
}
/// 真偽または `null`（取得不能）を `out` に足す。
pub fn push_opt_bool(out: &mut String, v: Option<bool>) {
    out.push_str(match v {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    });
}

/// 文字列の配列を `out` に足す。各要素は [`push_string`] を通る。
pub fn push_str_array<'a>(out: &mut String, values: impl IntoIterator<Item = &'a str>) {
    out.push('[');
    for (i, v) in values.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_string(out, v);
    }
    out.push(']');
}

/// 名前 → 文字列の表を `out` に足す。呼び手が渡す順（`BTreeMap` のキー順）のまま書く。
pub fn push_str_map<'a>(out: &mut String, entries: impl IntoIterator<Item = (&'a str, &'a str)>) {
    out.push('{');
    for (i, (k, v)) in entries.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_string(out, k);
        out.push(':');
        push_string(out, v);
    }
    out.push('}');
}

/// 名前 → 真偽の表を `out` に足す。呼び手が渡す順のまま書く。
pub fn push_bool_map<'a>(out: &mut String, entries: impl IntoIterator<Item = (&'a str, bool)>) {
    out.push('{');
    for (i, (k, v)) in entries.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_string(out, k);
        out.push(':');
        push_opt_bool(out, Some(v));
    }
    out.push('}');
}

/// 千分の一単位の整数を、固定 3 桁の小数の 10 進（例: `1.500`）で `out` に足す。
///
/// 浮動小数点を経由しないので、指数表記・NaN・無限大は出ない。
pub fn push_milli(out: &mut String, milli: u64) {
    let _ = write!(out, "{}.{:03}", milli / 1000, milli % 1000);
}
