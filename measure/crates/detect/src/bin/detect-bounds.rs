//! 検出・見逃し・判定不能の件数から下限・上限とラベルを 1 行の JSON で標準出力に出すコマンド。
//!
//! 使い方は `detect-bounds <detected> <missed> <indeterminate>`（10 進の非負整数 3 つ）。
//! `measure_detect::rate::bounds` と `measure_detect::rate::label` を呼ぶだけの薄い入口で、
//! セルの規則（最小件数・未接続）は当てない。
//! 出力のキーは `detected`・`missed`・`indeterminate`・`denominator`・`lower`・`upper`・`threshold`・
//! `label`（`"ok"`・`"ng"`・`"indeterminate"`）・`reason`（判定不能のときだけ固定の文字列、ほかは `null`）。
//! 値は整数と固定の ASCII の文字列だけなので JSON は自前で組み立てる。小数・丸めの値は出さない。
//!
//! 終了コード: 0 出力した（`--help` を含む） / 1 入力の違反（分母 0・あふれ。JSON は出さない） /
//! 2 誤用（引数の数が 3 でない・非負整数でない・未知のオプション）。
//! 標準エラーには固定の英語の文言と引数の番号だけを出し、入力の値は出さない。
//! ファイル・環境変数・時計・ネットワークは使わず、ファイルを作らない。
//! 手元でも CI でも同じ引数で流せる。

use std::process::ExitCode;

use measure_detect::rate::{DETECTION_THRESHOLD, RateLabel, bounds, label};

const USAGE: &str = "usage: detect-bounds <detected> <missed> <indeterminate>";

/// 引数 1 つを非負整数として読む。ASCII の数字だけを受け、`+`・空・小数・`u64` 超えは `None`。
fn parse_count(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<u64>().ok()
}

/// 引数から出力の文字列を作る。`Err` は (終了コード, 標準エラーの文言)。
fn run(args: &[String]) -> Result<String, (u8, String)> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        return Ok(USAGE.to_string());
    }
    if args.iter().any(|a| a.starts_with('-')) {
        return Err((2, format!("error: unknown option\n{USAGE}")));
    }
    if args.len() != 3 {
        return Err((2, format!("error: expected 3 arguments\n{USAGE}")));
    }
    let mut counts = [0u64; 3];
    for (i, a) in args.iter().enumerate() {
        counts[i] = parse_count(a).ok_or_else(|| {
            (
                2,
                format!("error: arg {}: not a non-negative integer\n{USAGE}", i + 1),
            )
        })?;
    }
    let [d, m, u] = counts;
    let b = bounds(d, m, u).map_err(|e| (1, format!("error: {e}")))?;
    let l = label(&b).map_err(|e| (1, format!("error: {e}")))?;
    let (name, reason) = match l {
        RateLabel::Ok => ("ok", "null".to_string()),
        RateLabel::Ng => ("ng", "null".to_string()),
        RateLabel::Indeterminate { reason } => ("indeterminate", format!("\"{reason}\"")),
    };
    let t = DETECTION_THRESHOLD;
    Ok(format!(
        "{{\"detected\":{d},\"missed\":{m},\"indeterminate\":{u},\"denominator\":{},\"lower\":{{\"numerator\":{},\"denominator\":{}}},\"upper\":{{\"numerator\":{},\"denominator\":{}}},\"threshold\":{{\"numerator\":{},\"denominator\":{}}},\"label\":\"{name}\",\"reason\":{reason}}}",
        b.lower.denominator,
        b.lower.numerator,
        b.lower.denominator,
        b.upper.numerator,
        b.upper.denominator,
        t.numerator,
        t.denominator,
    ))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err((code, msg)) => {
            eprintln!("{msg}");
            ExitCode::from(code)
        }
    }
}
