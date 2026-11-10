//! `check_g5_request` の device に届く前に返る入力の試験（TASK-1009 のポインタ）。macOS 向けだけ。
//!
//! 腕・dtype・入力の長さの食い違いは、検証レイヤーの確認より前に `Err` で返る。device は作らない。
#![cfg(target_os = "macos")]

use fandhe_silicon_core::parity::gemm_gate::GateInputs;
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype, generate_f32};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell, gate_cells};
use measure_metal_gemm_arm::ArmError;
use measure_metal_gemm_arm::external::check_g5_request;

fn g5_cell(row: GemmGateRow) -> GemmGateCell {
    gate_cells()
        .into_iter()
        .find(|c| c.arm() == GemmGateArm::G5 && c.row() == row && c.dtype() == GemmInputDtype::F32)
        .expect("g5 f32 cell")
}

#[test]
fn g6_cell_is_unsupported() {
    let cell = gate_cells()
        .into_iter()
        .find(|c| c.arm() == GemmGateArm::G6 && c.dtype() == GemmInputDtype::F32)
        .expect("g6 f32 cell");
    let inputs = GateInputs::F32(generate_f32(cell.row()).expect("inputs"));
    let result = check_g5_request(cell, &inputs);
    assert_eq!(result.err(), Some(ArmError::UnsupportedCell));
}

#[test]
fn binary16_inputs_are_a_dtype_mismatch() {
    let row = GemmGateRow::Odd37x41x53;
    let half = generate_f32(row)
        .expect("inputs")
        .to_binary16()
        .expect("binary16");
    let result = check_g5_request(g5_cell(row), &GateInputs::Binary16(half));
    assert_eq!(result.err(), Some(ArmError::DtypeMismatch));
}

#[test]
fn inputs_of_another_row_are_a_length_mismatch() {
    let other = GateInputs::F32(generate_f32(GemmGateRow::Odd37x41x53).expect("inputs"));
    let result = check_g5_request(g5_cell(GemmGateRow::Sq512), &other);
    assert_eq!(result.err(), Some(ArmError::LengthMismatch));
}
