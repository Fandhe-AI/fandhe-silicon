//! 一覧の 1 行からパイプラインを作るコマンド（TASK-485 のポインタ）。
//!
//! 使い方は `accept-kernel <manifest> <dir> <path> <entry>`。1 プロセスで 1 入口を扱い、
//! device を作るのは照合をすべて通した後の最後の手順だけ。投入はしない。
//!
//! 終了コード:
//! - 0: パイプラインを作れた（標準出力に `accepted` の 1 行）。
//! - 1: ソースの読み取り・sha256・入口の並びの照合に失敗した、または device の手順で失敗した。
//!   パイプラインの作成の失敗は、標準出力に固定の種類の 1 行を出す。
//! - 2: 引数・一覧が不正、または `<path> <entry>` の組が一覧に無い（device に届く前に返る）。
//! - 3: macOS 以外で、Metal を使えない。
//!
//! 標準エラーには固定の英語の文言と引数の番号だけを出し、引数の値・入口名・パスは出さない。

use std::path::Path;
use std::process::ExitCode;

use measure_kernel_accept::accept::{EXIT_USAGE, select_row};
use measure_kernel_accept::manifest;

const USAGE: &str = "usage: accept-kernel <manifest> <dir> <path> <entry>";

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(EXIT_USAGE)
}

fn main() -> ExitCode {
    let raw: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if raw.len() != 4 {
        eprintln!("{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    }
    let mut args: Vec<&str> = Vec::with_capacity(4);
    for (i, a) in raw.iter().enumerate() {
        match a.to_str() {
            Some(s) => args.push(s),
            None => return usage_error(&format!("arg {}: not valid UTF-8", i + 1)),
        }
    }
    let m = match manifest::load(Path::new(args[0])) {
        Ok(m) => m,
        Err(e) => return usage_error(&format!("arg 1: {e}")),
    };
    let Some(row) = select_row(&m, args[2], args[3]) else {
        return usage_error("arg 3-4: pair is not in the manifest");
    };
    run(&m, row, Path::new(args[1]))
}

#[cfg(not(target_os = "macos"))]
fn run(_m: &manifest::Manifest, _row: &manifest::ManifestEntry, _dir: &Path) -> ExitCode {
    eprintln!("error: Metal is available only on macOS");
    ExitCode::from(measure_kernel_accept::accept::EXIT_NOT_MACOS)
}

#[cfg(target_os = "macos")]
fn run(m: &manifest::Manifest, row: &manifest::ManifestEntry, dir: &Path) -> ExitCode {
    use measure_kernel_accept::accept::{
        DeviceStepError, EXIT_ACCEPTED, EXIT_FAILED, create_on_device, entries_match,
        pipeline_input, source_file,
    };
    use measure_kernel_accept::constants::specialized_constants;
    use measure_metal_gemm_arm::source::load_source_against;

    let source = match load_source_against(&source_file(dir, row), row.sha256()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: arg 2: {e}");
            return ExitCode::from(EXIT_FAILED);
        }
    };
    if !entries_match(m, row.path(), &source) {
        eprintln!("error: entry list differs from the source");
        return ExitCode::from(EXIT_FAILED);
    }
    // ここまでは device を作らない。以降の 1 回だけが Metal に触れる。
    let constants = specialized_constants();
    match create_on_device(pipeline_input(row, &source, &constants)) {
        Ok(()) => {
            println!("accepted");
            ExitCode::from(EXIT_ACCEPTED)
        }
        Err(DeviceStepError::Request(kind)) => {
            eprintln!("error: device request failed: {kind}");
            ExitCode::from(EXIT_FAILED)
        }
        Err(DeviceStepError::NoCompute) => {
            eprintln!("error: device has no compute ops");
            ExitCode::from(EXIT_FAILED)
        }
        Err(DeviceStepError::Create(kind)) => {
            println!("{kind}");
            ExitCode::from(EXIT_FAILED)
        }
    }
}
