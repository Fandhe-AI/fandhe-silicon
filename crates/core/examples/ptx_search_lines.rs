//! PTX のファイルを検索して、結果を 1 行ずつ標準出力へ書き出す（TASK-100）。
//!
//! 使い方: `cargo run -q -p fandhe-silicon-core --features test-support --example ptx_search_lines -- <file>...`。
//! 引数のファイルを順に読み、組み込みのパターン（`BUILTIN_PATTERNS`）で検索する。
//! ファイルの番号は 1 始まりで、パスは出力に出さない。
//!
//! 標準出力の行（`<n>` はファイルの番号）:
//! - `file=<n> sha256=<64 桁の小文字の 16 進>`（読めたファイルごと）
//! - `file=<n> ` に続けて `report_lines` の各行
//!
//! 標準エラーの行: `file=<n> error=<理由>`、引数の誤りの文言。
//!
//! 終了コード: 0 は全ファイルの検索が成功、1 は検索が `Err` のファイルがある、
//! 2 は引数の誤り・読めないファイル・UTF-8 でないファイル・標準出力の書き出しの失敗。

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::ExitCode;

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::ptx_search::{BUILTIN_PATTERNS, report_lines, search};

/// 標準エラーへ 1 行書く。書き出しの失敗では panic にしない。
fn err_line(msg: &str) {
    let _ = writeln!(std::io::stderr().lock(), "{msg}");
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.is_empty() {
        err_line("usage: ptx_search_lines <file>...");
        return ExitCode::from(2);
    }
    let mut bad_option = false;
    for (i, a) in args.iter().enumerate() {
        if a.as_encoded_bytes().first() == Some(&b'-') {
            err_line(&format!("argument {}: unknown option", i + 1));
            bad_option = true;
        }
    }
    if bad_option {
        return ExitCode::from(2);
    }

    let mut out = String::new();
    let mut search_failed = false;
    let mut io_failed = false;
    for (i, path) in args.iter().enumerate() {
        let n = i + 1;
        let Ok(bytes) = std::fs::read(path) else {
            err_line(&format!("file={n} error=cannot read file"));
            io_failed = true;
            continue;
        };
        let mut hex = String::new();
        for b in sha256_bytes(&bytes) {
            let _ = write!(hex, "{b:02x}");
        }
        let _ = writeln!(out, "file={n} sha256={hex}");
        let Ok(text) = String::from_utf8(bytes) else {
            err_line(&format!("file={n} error=not valid UTF-8"));
            io_failed = true;
            continue;
        };
        match search(&text, &BUILTIN_PATTERNS) {
            Ok(report) => {
                for line in report_lines(&report) {
                    let _ = writeln!(out, "file={n} {line}");
                }
            }
            Err(e) => {
                err_line(&format!("file={n} error={e}"));
                search_failed = true;
            }
        }
    }
    // 出力先の切断などの失敗は panic にせず、終了コードで返す。
    if std::io::stdout().lock().write_all(out.as_bytes()).is_err() {
        return ExitCode::from(2);
    }
    if io_failed {
        ExitCode::from(2)
    } else if search_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
