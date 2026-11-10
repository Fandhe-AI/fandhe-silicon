//! コマンド `fetch-kernel-source`: ローカルの clone からカーネルのソースを取り出し、
//! sha256 で照らしたうえで repo の外のファイルに書く。
//!
//! 使い方は 2 通り（TASK-1009 のポインタ）。
//! - `fetch-kernel-source <clone> <output>`: crate に固定した rev・パス・sha256 で取り出す。
//! - `fetch-kernel-source <clone> <output> <rev> <path> <sha256>`: 3 値を実行時に必須の引数で受ける。
//!   rev は 40 桁の小文字の 16 進、sha256 は 64 桁の小文字の 16 進、パスは素直な相対パスに限り、
//!   検査は git を起動する前に行う。
//!
//! 取得したファイルを repo の中に置かない。
//!
//! 終了コード: 0 書いた / 1 取得または照合の失敗 / 2 誤用（引数の数・引数の形式・既にある書き出し先・書き出し先を作れない）。
//! 書き出しは照合を通った後に新規作成だけで行い、既にある書き出し先は上書きしない。
//! 書き込みに失敗したら作ったファイルを消す。
//! 標準エラーには固定の文言と引数の番号だけを出し、引数の値は出さない。

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use measure_metal_gemm_arm::source::{
    GEMM_SOURCE_PATH, GEMM_SOURCE_SHA256, MAX_SOURCE_BYTES, SOURCE_REV, check_rev,
    check_source_path, parse_sha256_hex, verify_source_against,
};

const USAGE: &str = "usage: fetch-kernel-source <clone> <output> [<rev> <path> <sha256>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match args.len() {
        2 => {
            if let Some(code) = check_dest(&args[1]) {
                return code;
            }
            let spec = format!("{SOURCE_REV}:{GEMM_SOURCE_PATH}");
            fetch_and_write(&args[0], &args[1], &spec, &GEMM_SOURCE_SHA256)
        }
        5 => {
            if let Err(e) = check_rev(&args[2]) {
                eprintln!("error: arg 3: {e}");
                return ExitCode::from(2);
            }
            if let Err(e) = check_source_path(&args[3]) {
                eprintln!("error: arg 4: {e}");
                return ExitCode::from(2);
            }
            let digest = match parse_sha256_hex(&args[4]) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: arg 5: {e}");
                    return ExitCode::from(2);
                }
            };
            if let Some(code) = check_dest(&args[1]) {
                return code;
            }
            let spec = format!("{}:{}", args[2], args[3]);
            fetch_and_write(&args[0], &args[1], &spec, &digest)
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// 書き出し先が既にあれば誤用の終了コードを返す。
fn check_dest(dest: &str) -> Option<ExitCode> {
    if fs::symlink_metadata(Path::new(dest)).is_ok() {
        eprintln!("error: arg 2: output already exists");
        return Some(ExitCode::from(2));
    }
    None
}

/// `git show <spec>` の出力を上限つきで読み、`expected` で照らして通ったものだけを新規作成で書く。
/// 2 引数の形と 5 引数の形で共通の本体。
fn fetch_and_write(clone: &str, dest: &str, spec: &str, expected: &[u8; 32]) -> ExitCode {
    let dest = Path::new(dest);
    let mut child = match Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(clone)
        .arg("show")
        .arg(spec)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            eprintln!("error: arg 1: git show failed");
            return ExitCode::from(1);
        }
    };
    let mut buf = Vec::new();
    let read = match child.stdout.take() {
        Some(out) => out.take(MAX_SOURCE_BYTES as u64 + 1).read_to_end(&mut buf),
        None => Err(std::io::Error::other("no stdout")),
    };
    if read.is_err() || buf.len() > MAX_SOURCE_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        eprintln!("error: arg 1: git show failed");
        return ExitCode::from(1);
    }
    match child.wait() {
        Ok(s) if s.success() => {}
        _ => {
            eprintln!("error: arg 1: git show failed");
            return ExitCode::from(1);
        }
    }

    let text = match verify_source_against(&buf, expected) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let mut file = match OpenOptions::new().write(true).create_new(true).open(dest) {
        Ok(f) => f,
        Err(_) => {
            eprintln!("error: arg 2: cannot create output");
            return ExitCode::from(2);
        }
    };
    if file
        .write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .is_err()
    {
        drop(file);
        let _ = fs::remove_file(dest);
        eprintln!("error: arg 2: cannot write output");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
