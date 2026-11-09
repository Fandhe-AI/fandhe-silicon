//! 門の 1 セルを Metal の腕（`run_g6`）で流す device の試験（TASK-1008 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、専用の実機の回でだけ `--ignored` で流す。
//! 枠の `run_selected_from_env` は 1 つのテストのバイナリにつき 1 回だけ呼べるので、このファイルの試験は 1 本だけにする。
//! 標準エラーには何も書かず、標準出力には結果の行と pipeline の 2 つの値だけを出す。
#![cfg(target_os = "macos")]

use std::path::Path;

use fandhe_silicon_core::parity::gemm_gate::run_selected_from_env;
use fandhe_silicon_core::parity::gemm_population::GemmGateArm;
use measure_metal_gemm_arm::launch::launch_for;
use measure_metal_gemm_arm::source::load_source;
use measure_metal_gemm_arm::{ArmError, run_g6};

/// 照合済みのカーネルのソースのパスを渡す環境変数の名前。
const SOURCE_ENV: &str = "FANDHE_SILICON_G6_SOURCE";

/// pipeline が受け付けるスレッド数の上限の期待。
const EXPECTED_MAX_THREADS: usize = 1024;

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn g6_gate_cell() {
    let path = std::env::var_os(SOURCE_ENV).filter(|v| !v.is_empty());
    let Some(path) = path else {
        panic!("kernel source path env is not set");
    };
    let source = match load_source(Path::new(&path)) {
        Ok(s) => s,
        Err(e) => panic!("kernel source rejected: {e}"),
    };

    let mut facts = None;
    // PARITY-33 のポインタ: 選ばれた 1 セルの出力を参照とビットで比べる。
    let outcome = run_selected_from_env(|cell, inputs| {
        if cell.arm() != GemmGateArm::G6 {
            return Err(ArmError::UnsupportedCell);
        }
        let (out, f) = run_g6(cell, inputs, &source)?;
        facts = Some(f);
        Ok(out)
    });
    let result = match outcome {
        Ok(r) => r,
        Err(e) => panic!("gate cell failed: {e}"),
    };
    let Some(facts) = facts else {
        panic!("arm did not report pipeline facts");
    };

    assert_eq!(result.cell().arm(), GemmGateArm::G6);
    assert_eq!(result.bit_mismatches(), 0);
    assert_eq!(result.max_abs_diff(), 0.0);
    assert!(result.comparison().is_match());
    assert!(result.validation_layer());
    assert_eq!(
        facts.max_total_threads_per_threadgroup,
        EXPECTED_MAX_THREADS
    );
    let launch = launch_for(result.cell()).expect("launch row exists for the selected cell");
    assert_eq!(facts.threadgroup_memory_length, launch.threadgroup_memory);

    println!("{result}");
    println!(
        "max_total_threads_per_threadgroup={}",
        facts.max_total_threads_per_threadgroup
    );
    println!(
        "threadgroup_memory_length={}",
        facts.threadgroup_memory_length
    );
}
