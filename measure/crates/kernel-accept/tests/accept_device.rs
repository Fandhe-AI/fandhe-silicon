//! 一覧の全入口をパイプラインにできるかを、実機の Metal で確かめる試験（TASK-485 のポインタ）。
//!
//! Metal の device に触るので `#[ignore]` を付け、専用の実機の回でだけ流す。
//! 一覧とディレクトリは環境変数で受け、一覧の 1 行ごとに子プロセス `accept-kernel` を起動する（1 プロセス 1 device）。
//! 判定は試験の枠で行い、組み合わせの環境変数が無い未実施も失敗にする。
#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;

use fandhe_silicon_core::test_frame::{
    Declaration, Label, MachineCombination, RunResult, RunSetting, Subject, Verdict,
    judge_and_export,
};
use measure_kernel_accept::manifest::{self, Mode};

const MANIFEST_ENV: &str = "FANDHE_SILICON_KERNEL_MANIFEST";
const DIR_ENV: &str = "FANDHE_SILICON_KERNEL_DIR";

fn env_path(name: &str) -> PathBuf {
    match std::env::var_os(name) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => panic!("a required environment variable is missing"),
    }
}

fn run_mode(test_id: &str, behavior: &str, mode: Mode) {
    let manifest_path = env_path(MANIFEST_ENV);
    let dir = env_path(DIR_ENV);
    let m = manifest::load(&manifest_path).unwrap_or_else(|e| panic!("manifest: {e}"));

    let subject = Subject::from_parts(test_id, &[behavior], &[]).expect("subject");
    let declaration =
        Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).expect("declaration");
    let setting = RunSetting::from_env();

    let outcome = judge_and_export(&subject, &declaration, &setting, |_| {
        let (mut rows, mut zero, mut nonzero, mut signal) = (0usize, 0usize, 0usize, 0usize);
        for row in m.entries().iter().filter(|r| r.mode() == mode) {
            rows += 1;
            let status = Command::new(env!("CARGO_BIN_EXE_accept-kernel"))
                .arg(&manifest_path)
                .arg(&dir)
                .arg(row.path())
                .arg(row.entry())
                .output()
                .expect("spawn accept-kernel")
                .status;
            match status.code() {
                Some(0) => {
                    zero += 1;
                    println!("line {}: exit 0", row.line());
                }
                Some(c) => {
                    nonzero += 1;
                    println!("line {}: exit {c}", row.line());
                }
                None => {
                    signal += 1;
                    println!("line {}: signal", row.line());
                }
            }
        }
        println!("rows={rows} zero={zero} nonzero={nonzero} signal={signal}");
        if rows > 0 && zero == rows && signal == 0 {
            RunResult::Verdict(Verdict::Match)
        } else {
            RunResult::Verdict(Verdict::Mismatch)
        }
    })
    .expect("export");
    assert_eq!(outcome.label(), &Label::Ok);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn msl_entries_return_pipelines() {
    // 項目 ID のポインタ: ART-1
    run_mode("kernel_accept_msl", "ART-1", Mode::Msl);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn specialized_entries_return_pipelines() {
    // 項目 ID のポインタ: ART-2
    run_mode("kernel_accept_specialized", "ART-2", Mode::Specialized);
}
