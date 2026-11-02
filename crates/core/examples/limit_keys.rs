//! 上限の問合せの鍵の一覧を、検査へ渡すために 1 列のテキストで標準出力へ書き出す（TASK-1362）。
//!
//! 呼び出し元は `scripts/feature-key-check.sh` の `--example`。
//! 使い方: `cargo run -q -p fandhe-silicon-core --example limit_keys`。
//! 出力は見出しなしの 1 列（鍵の文字列そのもの）で、1 行が一覧の鍵 1 件に当たる。
//! 出力は `LIMIT_QUERY_KEYS` の並び。引数は読まず、ファイルは作らない。

use std::io::Write;
use std::process::ExitCode;

use fandhe_silicon_core::limit_key::LIMIT_QUERY_KEYS;

fn main() -> ExitCode {
    let mut out = String::new();
    for k in LIMIT_QUERY_KEYS {
        out.push_str(k.0);
        out.push('\n');
    }
    // 出力先の切断などの失敗は panic にせず、終了コードで返す。
    match std::io::stdout().lock().write_all(out.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
