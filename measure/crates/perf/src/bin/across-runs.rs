//! run ごとの記録（`runs/<run の ID>.json`）を読み、run 間の集計を標準出力に 1 行の JSON で出すコマンド。
//!
//! 使い方: `across-runs <run の記録> [<run の記録> ...]`。渡されたファイルだけを集計し、
//! ディレクトリの走査はしない。判定に使えない run の除外は、先に
//! `scripts/perf-record-check.sh --session <回> <run>...` で確かめてから、使える run だけを渡す。
//! 集計は記録として書かない（ファイルを作らない）。時計・環境変数・ネットワークは使わない。
//!
//! 終了コード: 0 成功 / 1 入力の違反（JSON 不正・欄の不足・整合の失敗・混在・上限超え・通常ファイルでない）/
//! 2 誤用（引数なし・未知のオプション・ファイルが無い／読めない）。
//! 標準エラーには固定の文言と引数の番号だけを出し、入力の値やパスは出さない。

use measure_perf::jsonread::{self, MAX_BYTES};
use measure_perf::runs::{self, RunInput};
use std::fs;
use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "usage: across-runs <run-record.json> [<run-record.json> ...]";

/// 引数 1 件を読んで検証する。`Err` は（終了コード, 文言）。
fn load(index: usize, path: &str) -> Result<RunInput, (u8, String)> {
    let n = index + 1;
    let meta = fs::metadata(path).map_err(|_| (2, format!("error: arg {n}: cannot read")))?;
    if !meta.is_file() {
        return Err((1, format!("error: arg {n}: not a regular file")));
    }
    if meta.len() > MAX_BYTES as u64 {
        return Err((1, format!("error: arg {n}: input too large")));
    }
    let file = fs::File::open(path).map_err(|_| (2, format!("error: arg {n}: cannot read")))?;
    // 取得後に伸びたファイルも上限を 1 バイト超えた所で読むのをやめる。
    let mut buf = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| (2, format!("error: arg {n}: cannot read")))?;
    let value = jsonread::parse(&buf).map_err(|e| (1, format!("error: arg {n}: {e}")))?;
    runs::run_input(&value).map_err(|e| (1, format!("error: arg {n}: {e}")))
}

fn run(args: &[String]) -> Result<String, (u8, String)> {
    if args.is_empty() {
        return Err((2, USAGE.to_owned()));
    }
    if args.iter().any(|a| a.starts_with('-')) {
        return Err((2, USAGE.to_owned()));
    }
    let inputs = args
        .iter()
        .enumerate()
        .map(|(i, p)| load(i, p))
        .collect::<Result<Vec<_>, _>>()?;
    let out = runs::aggregate(&inputs).map_err(|e| (1, format!("error: {e}")))?;
    Ok(runs::render(&out))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
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
