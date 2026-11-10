//! 検証レイヤーが有効でない環境で `run_g5` が `Err` を返すことの試験（TASK-1009 のポインタ）。macOS 向けだけ。
//!
//! 環境変数を書き換えずに済むよう、検証レイヤーとカーネルの環境変数を外した子プロセスで同じ試験を流す。
//! 検証レイヤーの確認は device を作る前なので、device には届かない。
#![cfg(target_os = "macos")]

use std::process::{Command, Stdio};

use fandhe_silicon_core::parity::gemm_gate::{DEBUG_LAYER_ENV, GateInputs, SHADER_VALIDATION_ENV};
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype, generate_f32};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::external::{KERNEL_ENTRY_ENV, KERNEL_SHA256_ENV, KERNEL_SOURCE_ENV};
use measure_metal_gemm_arm::{ArmError, run_g5};

const CHILD_MARK: &str = "RUN_G5_VALIDATION_CHILD";

#[test]
fn run_g5_requires_the_validation_layer() {
    if std::env::var_os(CHILD_MARK).is_some() {
        let row = GemmGateRow::Odd37x41x53;
        let cell = gate_cells()
            .into_iter()
            .find(|c| {
                c.arm() == GemmGateArm::G5 && c.row() == row && c.dtype() == GemmInputDtype::F32
            })
            .expect("g5 f32 cell");
        let inputs = GateInputs::F32(generate_f32(row).expect("inputs"));
        assert_eq!(
            run_g5(cell, &inputs).err(),
            Some(ArmError::ValidationLayerRequired)
        );
        return;
    }
    let status = Command::new(std::env::current_exe().expect("exe"))
        .args(["--exact", "run_g5_requires_the_validation_layer"])
        .env_remove(DEBUG_LAYER_ENV)
        .env_remove(SHADER_VALIDATION_ENV)
        .env_remove(KERNEL_SOURCE_ENV)
        .env_remove(KERNEL_SHA256_ENV)
        .env_remove(KERNEL_ENTRY_ENV)
        .env(CHILD_MARK, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert!(status.success());
}
