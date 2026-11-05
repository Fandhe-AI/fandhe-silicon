//! `wasm-parity-host` の入口。引数を `wasm_parity_host::run` に渡し、失敗を終了コード 2 で返す。

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match wasm_parity_host::run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "{e}");
            ExitCode::from(2)
        }
    }
}
