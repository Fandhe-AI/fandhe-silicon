//! 照合に合わないソースを腕の入口が device の前で断ることの試験（TASK-1008 のポインタ）。値は合成のものだけ。
//!
//! device の要る専用の回でだけ `--ignored` で流す。
#![cfg(target_os = "macos")]

use std::fs;
use std::path::PathBuf;

use fandhe_silicon_core::parity::gemm_gate::GateInputs;
use fandhe_silicon_core::parity::gemm_inputs::{GemmInputDtype, generate_f32};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::source::{SourceError, load_source};
use measure_metal_gemm_arm::{ArmError, run_g6};

/// 合成の小さなカーネル（固定の sha256 には合わない）。
const SYNTHETIC_MSL: &str = "kernel void k() {}\n";

fn tmp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("device-{name}"))
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn load_source_rejects_mismatched_digest() {
    let path = tmp("synthetic.metal");
    let _ = fs::remove_file(&path);
    fs::write(&path, SYNTHETIC_MSL).expect("write synthetic source");
    let result = load_source(&path);
    let _ = fs::remove_file(&path);
    assert_eq!(result, Err(SourceError::DigestMismatch));
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn run_g6_rejects_unpinned_source_before_device() {
    let cell = gate_cells()
        .into_iter()
        .filter(|c| c.arm() == GemmGateArm::G6 && c.dtype() == GemmInputDtype::F32)
        .min_by_key(|c| c.row().m() * c.row().n() * c.row().k())
        .expect("a G6 f32 cell exists");
    let inputs = GateInputs::F32(generate_f32(cell.row()).expect("generate inputs"));
    let result = run_g6(cell, &inputs, SYNTHETIC_MSL);
    assert_eq!(result.err(), Some(ArmError::SourceMismatch));
}
