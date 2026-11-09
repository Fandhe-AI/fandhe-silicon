//! 門の 1 セルを Metal の腕（`run_g7`）で流す device の試験（TASK-1009 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、device の要る専用の回でだけ `--ignored` で流す。
//! 枠の `run_selected_from_env` は 1 つのテストのバイナリにつき 1 回だけ呼べるので、このファイルの試験は 1 本だけにする。
//! 標準エラーには何も書かず、標準出力には結果の行と 2 つの値だけを出す。
#![cfg(target_os = "macos")]

use fandhe_silicon_core::parity::gemm_gate::run_selected_from_env;
use fandhe_silicon_core::parity::gemm_population::GemmGateArm;
use measure_metal_gemm_arm::{ArmError, run_g7};

/// pipeline が受け付けるスレッド数の下限の期待。
const MIN_THREADS: usize = 32;

/// 32 バイトの値を 64 桁の小文字の 16 進にする。
fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn g7_gate_cell() {
    let mut facts = None;
    // PARITY-37 のポインタ: 選ばれた 1 セルの出力を参照とビットで比べる。
    let outcome = run_selected_from_env(|cell, inputs| {
        if cell.arm() != GemmGateArm::G7 {
            return Err(ArmError::UnsupportedCell);
        }
        let (out, f) = run_g7(cell, inputs)?;
        facts = Some(f);
        Ok(out)
    });
    let result = match outcome {
        Ok(r) => r,
        Err(e) => panic!("gate cell failed: {e}"),
    };
    let Some(facts) = facts else {
        panic!("arm did not report metallib facts");
    };

    assert_eq!(result.cell().arm(), GemmGateArm::G7);
    assert_eq!(result.bit_mismatches(), 0);
    assert_eq!(result.max_abs_diff(), 0.0);
    assert!(result.comparison().is_match());
    assert!(result.validation_layer());
    assert!(facts.max_total_threads_per_threadgroup >= MIN_THREADS);

    println!("{result}");
    println!("metallib_sha256={}", hex(&facts.metallib_sha256));
    println!(
        "max_total_threads_per_threadgroup={}",
        facts.max_total_threads_per_threadgroup
    );
}
