//! `clock-resolution` の入口。引数を `measure_clock_resolution::parse_args` に渡し、
//! CPU の時計の分解能を測って結果の 1 行を標準出力に書く。
//! 失敗は標準エラーに英語 1 行を出して非ゼロで終える（値を黙って採らない）。

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args: Vec<String> = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => args.push(s),
            Err(_) => {
                eprintln!("error: usage: invalid value");
                return ExitCode::from(2);
            }
        }
    }
    let config = match measure_clock_resolution::parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let mut stdout = std::io::stdout().lock();
    let result = measure_clock_resolution::run(&config, &mut stdout).and_then(|()| {
        stdout
            .flush()
            .map_err(|_| measure_clock_resolution::ResolutionError::Output)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
