//! `clock-resolution` の入口。引数を `measure_clock_resolution::args_from_os` と
//! `measure_clock_resolution::parse_args` に渡し、CPU の時計の分解能を測って結果の 1 行を標準出力に書く。
//! 失敗は標準エラーに英語 1 行を出して非ゼロで終える（値を黙って採らない）。

use std::process::ExitCode;

fn main() -> ExitCode {
    let config = match measure_clock_resolution::args_from_os(std::env::args_os().skip(1))
        .and_then(|args| measure_clock_resolution::parse_args(&args))
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let mut stdout = std::io::stdout().lock();
    match measure_clock_resolution::run(&config, &mut stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
