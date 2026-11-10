//! `check_g3prime_request`（`run_g3prime` が最初に呼ぶ device の前の検査）と詰め物の試験（TASK-1010 のポインタ）。
//!
//! 呼ぶのは safe な `check_g3prime_request` と `pad_g3prime` だけで、テキストも device も扱わない。
//! 環境変数は書き換えず、検証レイヤーの環境変数を外した（または立てた）子プロセスで同じ試験を流す。

use std::process::{Command, Stdio};

use baseline_wgpu_gemm_arm::device::ArmError;
use baseline_wgpu_gemm_arm::prime::{check_g3prime_request, pad_g3prime};
use fandhe_silicon_core::parity::gemm_gate::{DEBUG_LAYER_ENV, GateInputs, SHADER_VALIDATION_ENV};
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype, generate_f32};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell, gate_cells};

const CHILD_MARK: &str = "CHECK_G3PRIME_REQUEST_CHILD";

fn cell(arm: GemmGateArm, row: GemmGateRow, dtype: GemmInputDtype) -> GemmGateCell {
    gate_cells()
        .into_iter()
        .find(|c| c.arm() == arm && c.row() == row && c.dtype() == dtype)
        .expect("cell")
}

fn f32_inputs(row: GemmGateRow) -> GateInputs {
    GateInputs::F32(generate_f32(row).expect("inputs"))
}

fn binary16_inputs(row: GemmGateRow) -> GateInputs {
    let GateInputs::F32(i) = f32_inputs(row) else {
        unreachable!()
    };
    GateInputs::Binary16(i.to_binary16().expect("binary16"))
}

#[test]
fn cells_of_other_arms_are_refused() {
    let row = GemmGateRow::Odd37x41x53;
    for arm in [GemmGateArm::G1, GemmGateArm::G2, GemmGateArm::G3] {
        let c = cell(arm, row, GemmInputDtype::F32);
        assert!(matches!(
            check_g3prime_request(c, &f32_inputs(row)),
            Err(ArmError::WrongArm)
        ));
    }
}

#[test]
fn dtype_and_input_kind_must_agree() {
    let row = GemmGateRow::Odd37x41x53;
    let f32_cell = cell(GemmGateArm::G3Prime, row, GemmInputDtype::F32);
    let f16_cell = cell(GemmGateArm::G3Prime, row, GemmInputDtype::F16Acc);
    assert!(matches!(
        check_g3prime_request(f32_cell, &binary16_inputs(row)),
        Err(ArmError::InputMismatch)
    ));
    assert!(matches!(
        check_g3prime_request(f16_cell, &f32_inputs(row)),
        Err(ArmError::InputMismatch)
    ));
}

#[test]
fn input_of_another_row_is_refused_by_the_padding() {
    let c = cell(
        GemmGateArm::G3Prime,
        GemmGateRow::Odd37x41x53,
        GemmInputDtype::F32,
    );
    assert!(matches!(
        pad_g3prime(c, &f32_inputs(GemmGateRow::Sq512)),
        Err(ArmError::Layout(_))
    ));
}

#[test]
fn padding_reports_the_byte_lengths_of_a_b_and_c() {
    let row = GemmGateRow::Odd37x41x53;
    for (dtype, inputs, elem) in [
        (GemmInputDtype::F32, f32_inputs(row), 4u32),
        (GemmInputDtype::F16Acc, binary16_inputs(row), 2u32),
    ] {
        let c = cell(GemmGateArm::G3Prime, row, dtype);
        let job = pad_g3prime(c, &inputs).expect("job");
        let word = |i: usize| u32::from_le_bytes(job.sizes()[i..i + 4].try_into().expect("4"));
        let l = job.layout();
        assert_eq!(word(0), l.mp() * l.kp() * elem);
        assert_eq!(word(4), l.kp() * l.np() * elem);
        assert_eq!(word(8), l.mp() * l.np() * 4);
        assert_eq!(job.is_f16(), dtype == GemmInputDtype::F16Acc);
    }
}

#[test]
fn new_error_texts_are_fixed_english() {
    let all = [
        ArmError::UnsupportedTarget,
        ArmError::LanguageVersion,
        ArmError::NoMetalDevice,
        ArmError::Compile,
        ArmError::Function,
        ArmError::Pipeline,
        ArmError::ThreadsExceeded,
        ArmError::Allocation,
        ArmError::Submit,
    ];
    for e in all {
        let text = e.to_string();
        assert!(!text.is_empty() && text.is_ascii(), "{text}");
        assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
    }
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

#[test]
fn validation_layer_is_required() {
    if std::env::var_os(CHILD_MARK).is_some() {
        let row = GemmGateRow::Odd37x41x53;
        for (dtype, inputs) in [
            (GemmInputDtype::F32, f32_inputs(row)),
            (GemmInputDtype::F16Acc, binary16_inputs(row)),
        ] {
            let c = cell(GemmGateArm::G3Prime, row, dtype);
            assert!(matches!(
                check_g3prime_request(c, &inputs),
                Err(ArmError::ValidationLayerRequired)
            ));
        }
        return;
    }
    assert!(spawn_child("validation_layer_is_required", false));
}

#[test]
fn valid_request_with_the_validation_layer_passes() {
    if std::env::var_os(CHILD_MARK).is_some() {
        let row = GemmGateRow::Odd37x41x53;
        for (dtype, inputs) in [
            (GemmInputDtype::F32, f32_inputs(row)),
            (GemmInputDtype::F16Acc, binary16_inputs(row)),
        ] {
            let c = cell(GemmGateArm::G3Prime, row, dtype);
            assert!(check_g3prime_request(c, &inputs).is_ok());
        }
        return;
    }
    assert!(spawn_child(
        "valid_request_with_the_validation_layer_passes",
        true
    ));
}
