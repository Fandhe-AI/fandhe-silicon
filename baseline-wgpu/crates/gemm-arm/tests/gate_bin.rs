//! bin `baseline-wgpu-gemm-gate` の誤用の試験（spec TASK-1010 のポインタ）。
//!
//! device に触れないケースだけを流す（セルの選択が無い・不正・g3 以外の腕・引数あり）。どのケースも
//! 検証レイヤーとセルの環境変数を消してから必要なものだけ設定し、時計・ネットワーク・ファイルは使わない。

use fandhe_silicon_core::parity::gemm_gate::{
    DEBUG_LAYER_ENV, GATE_CELL_ENV, SHADER_VALIDATION_ENV,
};
use fandhe_silicon_core::parity::gemm_inputs::GemmGateRow;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use std::process::{Command, Output};

fn run(cell_value: Option<&str>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_baseline-wgpu-gemm-gate"));
    cmd.args(args)
        .env_remove(GATE_CELL_ENV)
        .env_remove(DEBUG_LAYER_ENV)
        .env_remove(SHADER_VALIDATION_ENV);
    if let Some(v) = cell_value {
        cmd.env(GATE_CELL_ENV, v);
    }
    cmd.output().expect("spawn the bin")
}

fn assert_refused(out: &Output) {
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "stdout must stay empty");
}

#[test]
fn no_selection_is_refused() {
    assert_refused(&run(None, &[]));
}

#[test]
fn invalid_values_are_refused_without_echo() {
    for value in ["G3-SQ512-F32", "", "no-such-cell"] {
        let out = run(Some(value), &[]);
        assert_refused(&out);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            value.is_empty() || !err.contains(value),
            "the value must not be echoed"
        );
    }
}

#[test]
fn cell_of_another_arm_is_refused() {
    let cell = gate_cells()
        .into_iter()
        .find(|c| c.arm() != GemmGateArm::G3 && c.row() == GemmGateRow::ALL[0])
        .expect("a cell of another arm");
    assert_refused(&run(Some(&cell.id()), &[]));
}

#[test]
fn arguments_are_refused() {
    assert_refused(&run(None, &["x"]));
}
