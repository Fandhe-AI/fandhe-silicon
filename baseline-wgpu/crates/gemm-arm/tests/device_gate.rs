//! g3 のセルを device で流す試験（PARITY-36 のポインタ。TASK-1010）。
//!
//! device の要る専用の回でだけ `--ignored` で流す。どの試験も `#[ignore]` で、既定の `cargo test` では流れない。
//! 大きな行は試験にしない（37×41×53 の行と 512×512×512 の行の 2 行 × f32・f16acc の 4 本）。

use baseline_wgpu_gemm_arm::device::run_g3;
use fandhe_silicon_core::parity::gemm_gate::{ValidationLayerSetting, run_cell};
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell, gate_cells};

fn g3_cell(row: GemmGateRow, dtype: GemmInputDtype) -> GemmGateCell {
    gate_cells()
        .into_iter()
        .find(|c| c.arm() == GemmGateArm::G3 && c.row() == row && c.dtype() == dtype)
        .expect("g3 cell")
}

fn run_and_check(row: GemmGateRow, dtype: GemmInputDtype) {
    let cell = g3_cell(row, dtype);
    let result = run_cell(cell, ValidationLayerSetting::from_env(), |inputs| {
        run_g3(cell, inputs)
    })
    .expect("the cell runs");
    assert!(result.comparison().is_match(), "{result}");
    assert_eq!(result.bit_mismatches(), 0, "{result}");
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn odd_f32() {
    run_and_check(GemmGateRow::Odd37x41x53, GemmInputDtype::F32);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn odd_f16acc() {
    run_and_check(GemmGateRow::Odd37x41x53, GemmInputDtype::F16Acc);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn sq512_f32() {
    run_and_check(GemmGateRow::Sq512, GemmInputDtype::F32);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn sq512_f16acc() {
    run_and_check(GemmGateRow::Sq512, GemmInputDtype::F16Acc);
}
