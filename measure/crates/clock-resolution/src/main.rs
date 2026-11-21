//! `clock-resolution` の入口。標準出力と標準エラーを `measure_clock_resolution::run_main` に渡し、
//! 返った終了コードで終える。引数の解析・測定・結果の 1 行・失敗の 1 行と終了コード（引数の誤りは 2、
//! ほかの失敗は 1）は `run_main` が受け持つ（値を黙って採らない）。

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    ExitCode::from(measure_clock_resolution::run_main(
        std::env::args_os().skip(1),
        &mut stdout,
        &mut stderr,
    ))
}
