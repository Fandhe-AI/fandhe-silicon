//! `cpu-ops-arm` の入口。引数を `measure_cpu_ops_arm::parse_args` に渡し、
//! 10 個の腕を順に測って腕ごとの記録を書く。1 プロセスが 1 run。
//! 失敗は標準エラーに英語 1 行を出して非ゼロで終える（値を黙って採らない）。

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match measure_cpu_ops_arm::parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let mut stdout = std::io::stdout().lock();
    let result = measure_cpu_ops_arm::run(&config, &mut stdout);
    let _ = stdout.flush();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
