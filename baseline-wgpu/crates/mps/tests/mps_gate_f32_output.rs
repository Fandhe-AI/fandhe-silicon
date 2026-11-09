//! MPS の行列積で門の run の 1 セルを f32 の出力で流す試験（TASK-1011 のポインタ）。macOS 向けだけ。
//!
//! device の要る専用の回でだけ `--ignored` で流す。既定の `cargo test` では流れない。
//! 結果の行を出力するだけで、一致の判定の assert は置かない。

#![cfg(target_os = "macos")]

use baseline_wgpu_mps::{MpsError, OutputKind, PlanError, plan, run_matmul};
use fandhe_silicon_core::parity::gemm_gate::run_selected_from_env;

/// 計画の失敗と実行の失敗をまとめる。
#[derive(Debug)]
enum ArmError {
    Plan(PlanError),
    Run(MpsError),
}

impl std::fmt::Display for ArmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plan(e) => write!(f, "plan failed: {e}"),
            Self::Run(e) => write!(f, "run failed: {e}"),
        }
    }
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn mps_gate_cell_f32_output_runs() {
    let outcome = run_selected_from_env(|cell, inputs| {
        let plan = plan(cell, OutputKind::F32).map_err(ArmError::Plan)?;
        run_matmul(&plan, inputs).map_err(ArmError::Run)
    });
    match outcome {
        Ok(result) => {
            println!("{result}");
            println!("output=f32");
        }
        Err(e) => panic!("{e}"),
    }
}
