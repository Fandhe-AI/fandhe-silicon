//! 記録済みの run 間の値から行ごとの判定のラベルを再現し、期待と突き合わせて 1 行の JSON で出すコマンド。
//!
//! 使い方: `variation-replay <入力の JSON> [<入力の JSON> ...]`。渡されたファイルだけを順に処理し、
//! 1 ファイルにつき標準出力へ 1 行の JSON を出す。ディレクトリの走査はしない。
//! 記録済みの行のファイルは本リポに置かず、手元でパスを引数に渡して流す（公開側の試験・CI は合成の入力で行う）。
//! ファイルは作らない。時計・環境変数・ネットワークは使わない。入力の形は `measure_perf::replay` を参照。
//!
//! 終了コード: 0 すべて一致 / 1 不一致がある、または入力の違反（JSON 不正・欄の不足・未知のキーや規則・
//! 判定側の拒否・上限超え・通常ファイルでない。違反のときは JSON を出さず、不一致のときは JSON を出す）/
//! 2 誤用（引数なし・未知のオプション・ファイルが無い／読めない）。
//! 標準エラーには固定の文言と引数・行の番号だけを出し、入力の値やパスは出さない。

use measure_perf::jsonread::{self, MAX_BYTES};
use measure_perf::replay;
use std::fs;
use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "usage: variation-replay <input.json> [<input.json> ...]";

/// 引数 1 件を読んで再現する。`Err` は（終了コード, 文言）。`Ok` は（出力の 1 行, 全て一致か）。
fn load(index: usize, path: &str) -> Result<(String, bool), (u8, String)> {
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
    let input = replay::parse_input(&value).map_err(|e| (1, format!("error: arg {n}: {e}")))?;
    let out = replay::replay(&input).map_err(|e| (1, format!("error: arg {n}: {e}")))?;
    Ok((out.line().to_owned(), out.all_matched()))
}

fn run(args: &[String]) -> Result<(Vec<String>, bool), (u8, String)> {
    if args.is_empty() || args.iter().any(|a| a.starts_with('-')) {
        return Err((2, USAGE.to_owned()));
    }
    let mut lines = Vec::new();
    let mut all = true;
    for (i, p) in args.iter().enumerate() {
        let (line, ok) = load(i, p)?;
        lines.push(line);
        all &= ok;
    }
    Ok((lines, all))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match run(&args) {
        Ok((lines, all)) => {
            for l in lines {
                println!("{l}");
            }
            if all {
                ExitCode::SUCCESS
            } else {
                eprintln!("mismatch: some rows differ from expected");
                ExitCode::from(1)
            }
        }
        Err((code, msg)) => {
            eprintln!("{msg}");
            ExitCode::from(code)
        }
    }
}
