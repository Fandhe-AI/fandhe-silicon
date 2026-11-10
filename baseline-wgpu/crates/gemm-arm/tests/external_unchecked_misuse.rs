//! `check_g2_request`（`run_g2` が最初に呼ぶ device の前の検査）の試験（TASK-1010 のポインタ）。
//!
//! 呼ぶのは safe な `check_g2_request` だけで、シェーダーも device も作らない。環境変数は書き換えず、
//! 検証レイヤーの環境変数を外した（または立てた）子プロセスで同じ試験を流す。

use std::process::{Command, Stdio};

use baseline_wgpu_gemm_arm::device::{ArmError, check_g2_request};
use fandhe_silicon_core::parity::gemm_gate::{DEBUG_LAYER_ENV, GateInputs, SHADER_VALIDATION_ENV};
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype, generate_f32};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell, gate_cells};

const CHILD_MARK: &str = "CHECK_G2_REQUEST_CHILD";

fn cell(arm: GemmGateArm, row: GemmGateRow, dtype: GemmInputDtype) -> GemmGateCell {
    gate_cells()
        .into_iter()
        .find(|c| c.arm() == arm && c.row() == row && c.dtype() == dtype)
        .expect("cell")
}

fn f32_inputs(row: GemmGateRow) -> GateInputs {
    GateInputs::F32(generate_f32(row).expect("inputs"))
}

#[test]
fn cells_of_other_arms_are_refused() {
    let row = GemmGateRow::Odd37x41x53;
    for arm in [GemmGateArm::G1, GemmGateArm::G3] {
        let c = cell(arm, row, GemmInputDtype::F32);
        assert!(matches!(
            check_g2_request(c, &f32_inputs(row)),
            Err(ArmError::WrongArm)
        ));
    }
}

#[test]
fn binary16_inputs_are_refused() {
    let row = GemmGateRow::Odd37x41x53;
    let c = cell(GemmGateArm::G2, row, GemmInputDtype::F32);
    let GateInputs::F32(i) = f32_inputs(row) else {
        unreachable!()
    };
    let inputs = GateInputs::Binary16(i.to_binary16().expect("binary16"));
    assert!(matches!(
        check_g2_request(c, &inputs),
        Err(ArmError::InputMismatch)
    ));
}

#[test]
fn input_of_another_row_is_refused() {
    let c = cell(
        GemmGateArm::G2,
        GemmGateRow::Odd37x41x53,
        GemmInputDtype::F32,
    );
    assert!(matches!(
        check_g2_request(c, &f32_inputs(GemmGateRow::Sq512)),
        Err(ArmError::LengthMismatch)
    ));
}

fn spawn_child(test: &str, validation: bool) -> bool {
    let mut cmd = Command::new(std::env::current_exe().expect("exe"));
    cmd.args(["--exact", test])
        .env(CHILD_MARK, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if validation {
        cmd.env(DEBUG_LAYER_ENV, "1")
            .env(SHADER_VALIDATION_ENV, "1");
    } else {
        cmd.env_remove(DEBUG_LAYER_ENV)
            .env_remove(SHADER_VALIDATION_ENV);
    }
    cmd.status().expect("spawn").success()
}

fn valid_request() -> (GemmGateCell, GateInputs) {
    let row = GemmGateRow::Odd37x41x53;
    (
        cell(GemmGateArm::G2, row, GemmInputDtype::F32),
        f32_inputs(row),
    )
}

#[test]
fn validation_layer_is_required() {
    if std::env::var_os(CHILD_MARK).is_some() {
        let (c, inputs) = valid_request();
        assert!(matches!(
            check_g2_request(c, &inputs),
            Err(ArmError::ValidationLayerRequired)
        ));
        return;
    }
    assert!(spawn_child("validation_layer_is_required", false));
}

#[test]
fn valid_request_with_the_validation_layer_passes() {
    if std::env::var_os(CHILD_MARK).is_some() {
        let (c, inputs) = valid_request();
        assert!(check_g2_request(c, &inputs).is_ok());
        return;
    }
    assert!(spawn_child(
        "valid_request_with_the_validation_layer_passes",
        true
    ));
}
