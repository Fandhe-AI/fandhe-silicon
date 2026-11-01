//! テクスチャの形式の問合せの鍵の表を、検査へ渡すために TSV で標準出力へ書き出す（TASK-1313）。
//!
//! 呼び出し元は `scripts/texture-format-key-check.sh`。
//! 使い方: `cargo run -q -p fandhe-silicon-core --example texture_format_keys`。
//! 出力は見出しなしの 3 列（`<row>` / 鍵 / 形式の `Debug`）で、1 行が表の鍵 1 件に当たる。
//! 並びは `TEXTURE_FORMAT_ROWS` の並び。引数は読まず、ファイルは作らない。

use std::io::Write;
use std::process::ExitCode;

use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_ROWS;

fn main() -> ExitCode {
    let mut out = String::new();
    for row in TEXTURE_FORMAT_ROWS {
        for k in row.keys {
            out.push_str(&format!("{}\t{}\t{:?}\n", row.row, k.key.0, k.format));
        }
    }
    // 出力先の切断などの失敗は panic にせず、終了コードで返す。
    match std::io::stdout().lock().write_all(out.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
