//! コマンド `fetch-kernel-source`: ローカルの clone から固定の rev のカーネルのソースを取り出し、
//! sha256 で照らしたうえで repo の外のファイルに書く。
//!
//! 使い方: `fetch-kernel-source <clone> <output>`。取得したファイルを repo の中に置かない。
//!
//! 終了コード: 0 書いた / 1 取得または照合の失敗 / 2 誤用（引数の数・既にある書き出し先・書き出し先を作れない）。
//! 書き出しは照合を通った後に新規作成だけで行い、既にある書き出し先は上書きしない。
//! 書き込みに失敗したら作ったファイルを消す。
//! 標準エラーには固定の文言と引数の番号だけを出す。

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use measure_metal_gemm_arm::source::{
    GEMM_SOURCE_PATH, MAX_SOURCE_BYTES, SOURCE_REV, verify_source,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("usage: fetch-kernel-source <clone> <output>");
        return ExitCode::SUCCESS;
    }
    if args.len() != 2 {
        eprintln!("usage: fetch-kernel-source <clone> <output>");
        return ExitCode::from(2);
    }
    let clone = &args[0];
    let dest = Path::new(&args[1]);

    if fs::symlink_metadata(dest).is_ok() {
        eprintln!("error: arg 2: output already exists");
        return ExitCode::from(2);
    }

    let mut child = match Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(clone)
        .arg("show")
        .arg(format!("{SOURCE_REV}:{GEMM_SOURCE_PATH}"))
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

    let text = match verify_source(&buf) {
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
