//! cooperative matrix の GEMM の MSL と起動の形の試験（TASK-1009 のポインタ）。device と外部コマンドは使わない。

use std::ffi::OsString;
use std::path::Path;

use fandhe_silicon_core::parity::gemm_inputs::GemmGateRow;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::ArmError;
use measure_metal_gemm_arm::coopmat::{
    AIR_FILE_NAME, COOPMAT_ENTRY, COOPMAT_MSL, COOPMAT_THREADS, METALLIB_FILE_NAME, MSL_FILE_NAME,
    compile_commands, coopmat_launch_for,
};
use measure_metal_gemm_arm::launch::pad8_dim;

#[test]
fn only_g7_cells_have_a_coopmat_launch() {
    let mut some = 0;
    for cell in gate_cells() {
        let launch = coopmat_launch_for(cell);
        if cell.arm() == GemmGateArm::G7 {
            assert!(launch.is_some());
            some += 1;
        } else {
            assert!(launch.is_none());
        }
    }
    assert_eq!(some, 5);
}

#[test]
fn effective_dims_are_pad8_of_the_row() {
    for cell in gate_cells() {
        let Some(l) = coopmat_launch_for(cell) else {
            continue;
        };
        for eff in [l.eff_m, l.eff_n, l.eff_k] {
            assert_eq!(eff % 8, 0);
        }
        let orig = match cell.row() {
            GemmGateRow::Sq512 => (512, 512, 512),
            GemmGateRow::Sq2048 => (2048, 2048, 2048),
            GemmGateRow::Sq4096 => (4096, 4096, 4096),
            GemmGateRow::Rect4096x1024x4096 => (4096, 1024, 4096),
            GemmGateRow::Odd37x41x53 => (37, 41, 53),
        };
        assert_eq!(pad8_dim(orig.0).unwrap(), l.eff_m);
        assert_eq!(pad8_dim(orig.1).unwrap(), l.eff_n);
        assert_eq!(pad8_dim(orig.2).unwrap(), l.eff_k);
        assert!(l.eff_m >= orig.0 && l.eff_n >= orig.1 && l.eff_k >= orig.2);
        if cell.row() == GemmGateRow::Odd37x41x53 {
            assert_eq!((l.eff_m, l.eff_n, l.eff_k), (40, 48, 56));
        } else {
            assert_eq!((l.eff_m, l.eff_n, l.eff_k), orig);
        }
    }
}

#[test]
fn groups_and_threads_follow_the_formula() {
    for cell in gate_cells() {
        let Some(l) = coopmat_launch_for(cell) else {
            continue;
        };
        assert_eq!(l.groups_width * 8, l.eff_n);
        assert_eq!(l.groups_height * 8, l.eff_m);
        assert_eq!(l.threads, 32);
        assert_eq!(l.threads, COOPMAT_THREADS);
    }
}

fn os(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

#[test]
fn compile_commands_match_the_fixed_shape() {
    let dir = Path::new("/nonexistent/coopmat-dir");
    let [first, second] = compile_commands(dir);
    let msl = dir.join(MSL_FILE_NAME);
    let air = dir.join(AIR_FILE_NAME);
    let lib = dir.join(METALLIB_FILE_NAME);
    let mut want_first = os(&[
        "-sdk",
        "macosx",
        "metal",
        "-c",
        "-fmetal-math-mode=safe",
        "-o",
    ]);
    want_first.push(air.clone().into_os_string());
    want_first.push(msl.into_os_string());
    let mut want_second = os(&["-sdk", "macosx", "metallib", "-o"]);
    want_second.push(lib.into_os_string());
    want_second.push(air.into_os_string());
    assert_eq!(first, want_first);
    assert_eq!(second, want_second);

    for arg in first.iter().chain(second.iter()) {
        let text = arg.to_string_lossy();
        assert_ne!(text, "xcrun");
        assert!(!text.starts_with("-std"));
        assert!(!text.starts_with("-O"));
        assert!(!text.starts_with("-fmetal-math-fp32-functions"));
    }
    for path_arg in [&first[6], &first[7], &second[4], &second[5]] {
        assert!(Path::new(path_arg).starts_with(dir));
    }
    assert_eq!(MSL_FILE_NAME, "coopmat_gemm_f32.metal");
    assert_eq!(AIR_FILE_NAME, "coopmat_gemm_f32.air");
    assert_eq!(METALLIB_FILE_NAME, "coopmat_gemm_f32.metallib");
}

#[test]
fn msl_has_the_entry_and_bindings() {
    assert_eq!(COOPMAT_ENTRY, "coopmat_gemm_f32");
    assert!(COOPMAT_MSL.contains(&format!("kernel void {COOPMAT_ENTRY}(")));
    assert!(COOPMAT_MSL.contains("[[buffer(3)]]"));
    assert!(COOPMAT_MSL.contains("#include <metal_simdgroup_matrix>"));
}

#[test]
fn new_errors_display_fixed_english() {
    for (err, text) in [
        (ArmError::ToolFailed, "offline shader compiler failed"),
        (ArmError::WorkDirFailed, "work directory operation failed"),
        (ArmError::LibraryLoadFailed, "metallib loading failed"),
    ] {
        let shown = err.to_string();
        assert_eq!(shown, text);
        assert!(shown.is_ascii());
        assert!(!shown.chars().any(|c| c.is_ascii_digit()));
    }
}
