//! GEMM の門の run の枠の確認（spec TASK-1005 の 5/6）。腕は CPU 参照を返す偽の腕で、device には触れない。

use std::cell::Cell;
use std::convert::Infallible;
use std::ffi::OsString;

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::binary16::round_slice_to_binary16;
use fandhe_silicon_core::parity::gemm_gate::{
    DEBUG_LAYER_ENV, GATE_CELL_ENV, GateError, GateInputs, GateOutput, GateSelection,
    SHADER_VALIDATION_ENV, ValidationLayerSetting, run_cell, run_selected_from_lookup,
};
use fandhe_silicon_core::parity::gemm_inputs::{
    GemmGateRow, GemmInputDtype, generate_f32, reference_for, reference_for_descending_k,
};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell, gate_cells};

fn cell(id: &str) -> GemmGateCell {
    GemmGateCell::from_id(id).expect("known cell id")
}

fn off() -> ValidationLayerSetting {
    ValidationLayerSetting::from_lookup(|_| None)
}

fn reference_values(c: GemmGateCell) -> Vec<f32> {
    let inputs = generate_f32(c.row()).unwrap();
    reference_for(c.row(), c.dtype(), &inputs)
        .unwrap()
        .values()
        .to_vec()
}

fn lookup_of(pairs: Vec<(&'static str, OsString)>) -> impl Fn(&str) -> Option<OsString> {
    move |name| {
        pairs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.clone())
    }
}

#[test]
fn reference_arm_matches_f32() {
    let c = cell("g1-odd37x41x53-f32");
    let want = reference_values(c);
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    assert!(r.comparison().is_match());
    assert_eq!(r.comparison().compared(), 37 * 41);
    assert_eq!(r.bit_mismatches(), 0);
    assert_eq!(r.max_abs_diff(), 0.0);
    assert!(!r.validation_layer());
}

#[test]
fn reference_arm_matches_on_sq512() {
    let c = cell("g1-sq512-f32");
    let want = reference_values(c);
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    assert!(r.comparison().is_match());
    assert_eq!(r.bit_mismatches(), 0);
}

#[test]
fn binary16_arm_matches_rounded_reference() {
    let c = cell("g3-odd37x41x53-f16acc");
    let rounded = round_slice_to_binary16(&reference_values(c)).unwrap();
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::Binary16(rounded.clone()))
    })
    .unwrap();
    assert!(r.comparison().is_match());
    assert_eq!(r.bit_mismatches(), 0);
    assert_eq!(r.max_abs_diff(), 0.0);
}

#[test]
fn input_kind_follows_dtype() {
    let c16 = cell("g3-odd37x41x53-f16acc");
    run_cell(c16, off(), |i| {
        assert!(matches!(i, GateInputs::Binary16(_)));
        Ok::<_, Infallible>(GateOutput::F32(reference_values(c16)))
    })
    .unwrap();
    let c32 = cell("g3-odd37x41x53-f32");
    run_cell(c32, off(), |i| {
        assert!(matches!(i, GateInputs::F32(_)));
        Ok::<_, Infallible>(GateOutput::F32(reference_values(c32)))
    })
    .unwrap();
}

#[test]
fn f32_output_on_f16acc_cell_is_accepted() {
    let c = cell("g3-odd37x41x53-f16acc");
    let want = reference_values(c);
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    assert!(r.comparison().is_match());
    assert_eq!(r.bit_mismatches(), 0);
}

#[test]
fn perturbed_arm_is_detected() {
    let c = cell("g1-odd37x41x53-f32");
    let mut v = reference_values(c);
    v[5] += 100.0;
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(v.clone()))
    })
    .unwrap();
    assert_eq!(r.comparison().exceeded(), 1);
    assert_eq!(r.comparison().first_mismatch(), Some(5));
    assert_eq!(r.bit_mismatches(), 1);
    assert!(r.max_abs_diff() >= 99.0);
}

#[test]
fn wrong_length_is_error() {
    let c = cell("g1-odd37x41x53-f32");
    let e = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(vec![0.0; 3]))
    })
    .unwrap_err();
    assert!(matches!(
        e,
        GateError::OutputLength {
            expected: 1517,
            actual: 3
        }
    ));
}

#[test]
fn binary16_on_f32_cell_is_error() {
    let c = cell("g1-odd37x41x53-f32");
    let e = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::Binary16(vec![0; 37 * 41]))
    })
    .unwrap_err();
    assert!(matches!(e, GateError::OutputKindMismatch));
}

#[test]
fn arm_error_is_propagated() {
    let c = cell("g1-odd37x41x53-f32");
    let e = run_cell(c, off(), |_| Err::<GateOutput, _>("arm broke")).unwrap_err();
    assert!(matches!(e, GateError::Arm("arm broke")));
    assert_eq!(e.to_string(), "arm failed: arm broke");
}

#[test]
fn submit_is_called_once() {
    let c = cell("g1-odd37x41x53-f32");
    let calls = Cell::new(0);
    let want = reference_values(c);
    run_cell(c, off(), |_| {
        calls.set(calls.get() + 1);
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
}

#[test]
fn sha256_is_little_endian_bytes() {
    let c = cell("g1-odd37x41x53-f32");
    let want = reference_values(c);
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    let mut bytes = Vec::new();
    for v in &want {
        bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    assert_eq!(r.output_sha256(), sha256_bytes(&bytes));

    let c16 = cell("g3-odd37x41x53-f16acc");
    let rounded = round_slice_to_binary16(&reference_values(c16)).unwrap();
    let r16 = run_cell(c16, off(), |_| {
        Ok::<_, Infallible>(GateOutput::Binary16(rounded.clone()))
    })
    .unwrap();
    let mut bytes16 = Vec::new();
    for v in &rounded {
        bytes16.extend_from_slice(&v.to_le_bytes());
    }
    assert_eq!(r16.output_sha256(), sha256_bytes(&bytes16));
}

#[test]
fn display_is_one_line() {
    let c = cell("g1-odd37x41x53-f32");
    let want = reference_values(c);
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    let s = r.to_string();
    assert!(!s.contains('\n'));
    assert!(s.starts_with("cell=g1-odd37x41x53-f32 compared=1517 exceeded=0"));
    assert!(s.ends_with("validation=off"));
}

#[test]
fn validation_setting_requires_both_exact_one() {
    let on = |a: &'static str, b: &'static str| {
        ValidationLayerSetting::from_lookup(lookup_of(vec![
            (DEBUG_LAYER_ENV, a.into()),
            (SHADER_VALIDATION_ENV, b.into()),
        ]))
        .is_enabled()
    };
    assert!(on("1", "1"));
    assert!(!on("1", "0"));
    assert!(!on("0", "1"));
    assert!(!on("true", "1"));
    assert!(!on("1", "11"));
    assert!(!on("1", ""));
    let only_one =
        ValidationLayerSetting::from_lookup(lookup_of(vec![(DEBUG_LAYER_ENV, "1".into())]));
    assert!(!only_one.is_enabled());
    assert!(!off().is_enabled());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let bad = OsString::from_vec(vec![0xff]);
        let s = ValidationLayerSetting::from_lookup(lookup_of(vec![
            (DEBUG_LAYER_ENV, bad),
            (SHADER_VALIDATION_ENV, "1".into()),
        ]));
        assert!(!s.is_enabled());
    }
}

#[test]
fn validation_state_is_recorded_in_result() {
    let c = cell("g1-odd37x41x53-f32");
    let want = reference_values(c);
    let on = ValidationLayerSetting::from_lookup(lookup_of(vec![
        (DEBUG_LAYER_ENV, "1".into()),
        (SHADER_VALIDATION_ENV, "1".into()),
    ]));
    let r = run_cell(c, on, |_| {
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    assert!(r.validation_layer());
    assert!(r.to_string().ends_with("validation=on"));
}

#[test]
fn selection_parses_exact_ids() {
    for c in gate_cells() {
        let sel = GateSelection::from_lookup(lookup_of(vec![(GATE_CELL_ENV, c.id().into())]));
        assert_eq!(sel, GateSelection::Cell(c));
    }
    assert_eq!(GateSelection::from_lookup(|_| None), GateSelection::Unset);
    let invalid = |v: &str| {
        GateSelection::from_lookup(lookup_of(vec![(GATE_CELL_ENV, v.into())]))
            == GateSelection::Invalid
    };
    assert!(invalid(""));
    assert!(invalid("unknown"));
    assert!(invalid("G1-sq512-f32"));
    assert!(invalid(" g1-sq512-f32"));
    assert!(invalid("g1-sq512-f32 "));
    assert!(invalid(&"a".repeat(65)));
    // 母数に入らない腕も、セルとしては流せる。
    assert!(!invalid("mps-sq512-f32"));
    // 腕が流さない dtype の組は存在しない。
    assert!(invalid("g1-sq512-f16acc"));
}

#[test]
fn selection_errors_do_not_echo_value() {
    let secret = "SECRET-VALUE-123";
    let e = run_selected_from_lookup(lookup_of(vec![(GATE_CELL_ENV, secret.into())]), |_, _| {
        Ok::<GateOutput, Infallible>(GateOutput::F32(vec![]))
    })
    .unwrap_err();
    assert!(matches!(e, GateError::InvalidSelection));
    assert!(!e.to_string().contains(secret));
    assert!(!format!("{e:?}").contains(secret));
}

#[test]
fn unselected_does_not_call_arm() {
    let calls = Cell::new(0);
    let arm = |_: GemmGateCell, _: &GateInputs| {
        calls.set(calls.get() + 1);
        Ok::<GateOutput, Infallible>(GateOutput::F32(vec![]))
    };
    let e = run_selected_from_lookup(|_| None, arm).unwrap_err();
    assert!(matches!(e, GateError::NotSelected));
    let e =
        run_selected_from_lookup(lookup_of(vec![(GATE_CELL_ENV, "x".into())]), arm).unwrap_err();
    assert!(matches!(e, GateError::InvalidSelection));
    assert_eq!(calls.get(), 0);
}

// 選択つきの入口（プロセスで 1 回の守り）を呼ぶのは、このテストのバイナリでこの試験だけ。
#[test]
fn second_run_in_process_is_rejected() {
    let id = "g1-odd37x41x53-f32";
    let c = cell(id);
    let want = reference_values(c);
    let lookup = lookup_of(vec![
        (GATE_CELL_ENV, id.into()),
        (DEBUG_LAYER_ENV, "1".into()),
        (SHADER_VALIDATION_ENV, "1".into()),
    ]);
    let calls = Cell::new(0);
    let r = run_selected_from_lookup(&lookup, |got, _| {
        calls.set(calls.get() + 1);
        assert_eq!(got, c);
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap();
    assert!(r.comparison().is_match());
    assert!(r.validation_layer());
    let e = run_selected_from_lookup(&lookup, |_, _| {
        calls.set(calls.get() + 1);
        Ok::<_, Infallible>(GateOutput::F32(want.clone()))
    })
    .unwrap_err();
    assert!(matches!(e, GateError::AlreadyRanInProcess));
    assert_eq!(calls.get(), 1);
}

#[test]
fn descending_arm_is_compared_like_any_arm() {
    let row = GemmGateRow::Odd37x41x53;
    let c = cell("g1-odd37x41x53-f32");
    assert_eq!(c.row(), row);
    let inputs = generate_f32(row).unwrap();
    let desc = reference_for_descending_k(row, GemmInputDtype::F32, &inputs)
        .unwrap()
        .values()
        .to_vec();
    let r = run_cell(c, off(), |_| {
        Ok::<_, Infallible>(GateOutput::F32(desc.clone()))
    })
    .unwrap();
    assert_eq!(r.comparison().exceeded(), 0);
}

#[test]
fn from_id_round_trips_all_cells() {
    for c in gate_cells() {
        assert_eq!(GemmGateCell::from_id(&c.id()), Some(c));
    }
    assert_eq!(
        GemmGateCell::from_id("mps-sq512-f32").unwrap().arm(),
        GemmGateArm::Mps
    );
}
