//! 起動の表と詰め物・切り出しの試験（TASK-1008 のポインタ）。値は表の定数と式との突き合わせと合成の行列だけ。

use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::ArmError;
use measure_metal_gemm_arm::launch::{
    BufferLayout, CONSTANT_COUNT, ConstantValue, Entry, OutputKind, crop_matrix, launch_for,
    pad_matrix, pad8_dim,
};

fn g6_cells() -> Vec<(
    fandhe_silicon_core::parity::gemm_population::GemmGateCell,
    measure_metal_gemm_arm::launch::Launch,
)> {
    gate_cells()
        .into_iter()
        .filter_map(|c| launch_for(c).map(|l| (c, l)))
        .collect()
}

#[test]
fn only_g6_cells_have_a_launch() {
    let mut some = 0;
    for cell in gate_cells() {
        let launch = launch_for(cell);
        if cell.arm() == GemmGateArm::G6 {
            assert!(launch.is_some(), "{}", cell.id());
            some += 1;
        } else {
            assert!(launch.is_none(), "{}", cell.id());
        }
    }
    assert_eq!(some, 10);
}

#[test]
fn rows_respect_limits_and_formulas() {
    let cells = g6_cells();
    assert_eq!(cells.len(), 10);
    let mut max_memory = 0;
    for (cell, l) in &cells {
        let id = cell.id();
        assert!(l.threads <= 1024, "{id}");
        assert!(l.threadgroup_memory <= 32768, "{id}");
        assert_eq!(l.threadgroup_memory % 16, 0, "{id}");
        max_memory = max_memory.max(l.threadgroup_memory);
        let t = l.tile;
        assert_eq!(l.threads, (t.wm * t.wn * 32) as usize, "{id}");
        // 実効次元は 8 の倍数で、元の次元以上で、tile で割り切れる。
        let (m, n, k) = (cell.row().m(), cell.row().n(), cell.row().k());
        for (eff, orig) in [(l.eff_m, m), (l.eff_n, n), (l.eff_k, k)] {
            assert_eq!(eff % 8, 0, "{id}");
            assert!(eff >= orig, "{id}");
            assert_eq!(pad8_dim(orig).unwrap(), eff, "{id}");
        }
        assert_eq!(l.eff_m % t.bm as usize, 0, "{id}");
        assert_eq!(l.eff_n % t.bn as usize, 0, "{id}");
        assert_eq!(l.eff_k % t.bk as usize, 0, "{id}");
        assert_eq!(l.groups_width, l.eff_n / t.bn as usize, "{id}");
        assert_eq!(l.groups_height, l.eff_m / t.bm as usize, "{id}");
        // threadgroup のメモリ長の式。
        let pad = if t.staged { 4 } else { 0 };
        let tile_elems = (t.bm * (t.bk + pad) + t.bk * (t.bn + pad)) as usize;
        let (bm, bn) = (t.bm as usize, t.bn as usize);
        let want = match l.entry {
            Entry::F32 => (if t.staged { tile_elems * 4 } else { 0 }).max(16),
            Entry::F16Acc => bm * bn * 4 + if t.staged { tile_elems * 2 } else { 0 },
        };
        assert_eq!(l.threadgroup_memory, want, "{id}");
    }
    assert_eq!(max_memory, 15104);
}

#[test]
fn constants_cover_indices_zero_to_seventeen_once() {
    for (cell, l) in g6_cells() {
        assert_eq!(l.constants.len(), CONSTANT_COUNT);
        let mut seen = [false; CONSTANT_COUNT];
        for (index, _) in l.constants {
            assert!(!seen[index as usize], "{}", cell.id());
            seen[index as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "{}", cell.id());
        let t = l.tile;
        assert_eq!(l.constants[0].1, ConstantValue::UInt(t.bm));
        assert_eq!(l.constants[5].1, ConstantValue::Bool(t.staged));
        assert_eq!(
            l.constants[6].1,
            ConstantValue::UInt(if t.staged { 4 } else { 0 })
        );
    }
}

#[test]
fn entry_buffers_and_output_follow_dtype() {
    for (cell, l) in g6_cells() {
        match cell.dtype() {
            GemmInputDtype::F32 => {
                assert_eq!(l.entry, Entry::F32);
                assert_eq!(l.buffers, BufferLayout::F32Seven);
                assert_eq!(l.output, OutputKind::F32);
            }
            GemmInputDtype::F16Acc => {
                assert_eq!(l.entry, Entry::F16Acc);
                assert_eq!(l.buffers, BufferLayout::F16Four);
                assert_eq!(l.output, OutputKind::Binary16);
            }
        }
    }
    assert_ne!(Entry::F32.name(), Entry::F16Acc.name());
}

#[test]
fn only_the_odd_row_is_padded() {
    for (cell, l) in g6_cells() {
        let padded =
            (l.eff_m, l.eff_n, l.eff_k) != (cell.row().m(), cell.row().n(), cell.row().k());
        assert_eq!(
            padded,
            cell.row() == GemmGateRow::Odd37x41x53,
            "{}",
            cell.id()
        );
    }
}

#[test]
fn pad_and_crop_round_trip_f32_and_u16() {
    let a: Vec<f32> = (0..37 * 53).map(|i| i as f32 + 1.0).collect();
    let padded = pad_matrix(&a, 37, 53, 40, 56, 0.0_f32).unwrap();
    assert_eq!(padded.len(), 40 * 56);
    // 詰めた部分は正の 0。
    for r in 0..40 {
        for c in 0..56 {
            if r >= 37 || c >= 53 {
                assert_eq!(padded[r * 56 + c].to_bits(), 0, "({r},{c})");
            }
        }
    }
    assert_eq!(crop_matrix(&padded, 40, 56, 37, 53).unwrap(), a);

    let b: Vec<u16> = (0..53 * 41).map(|i| (i % 65535) as u16 + 1).collect();
    let padded = pad_matrix(&b, 53, 41, 56, 48, 0_u16).unwrap();
    assert_eq!(
        padded.iter().filter(|v| **v == 0).count(),
        56 * 48 - 53 * 41
    );
    assert_eq!(crop_matrix(&padded, 56, 48, 53, 41).unwrap(), b);
}

#[test]
fn multiples_of_eight_pass_through() {
    let a: Vec<f32> = (0..16 * 8).map(|i| i as f32).collect();
    assert_eq!(pad_matrix(&a, 16, 8, 16, 8, 0.0_f32).unwrap(), a);
    assert_eq!(pad8_dim(8).unwrap(), 8);
    assert_eq!(pad8_dim(9).unwrap(), 16);
    assert_eq!(pad8_dim(1).unwrap(), 8);
}

#[test]
fn mismatched_lengths_and_overflow_are_errors() {
    let a = vec![0.0_f32; 6];
    assert_eq!(
        pad_matrix(&a, 2, 4, 8, 8, 0.0_f32),
        Err(ArmError::LengthMismatch)
    );
    assert_eq!(
        pad_matrix(&a, 2, 3, 1, 3, 0.0_f32),
        Err(ArmError::LengthMismatch)
    );
    assert_eq!(
        pad_matrix(&a, 2, 3, usize::MAX, 2, 0.0_f32),
        Err(ArmError::Overflow)
    );
    assert_eq!(crop_matrix(&a, 2, 3, 3, 3), Err(ArmError::LengthMismatch));
    assert_eq!(crop_matrix(&a, 3, 3, 1, 1), Err(ArmError::LengthMismatch));
    assert_eq!(
        crop_matrix(&a, usize::MAX, 2, 1, 1),
        Err(ArmError::Overflow)
    );
    assert_eq!(pad8_dim(usize::MAX), Err(ArmError::Overflow));
}

#[test]
fn display_is_fixed_english_without_values() {
    let all = [
        ArmError::UnsupportedCell,
        ArmError::DtypeMismatch,
        ArmError::LengthMismatch,
        ArmError::NoDevice,
        ArmError::MissingCompileOption,
        ArmError::CompileFailed,
        ArmError::FunctionFailed,
        ArmError::PipelineFailed,
        ArmError::ThreadsExceeded,
        ArmError::AllocationFailed,
        ArmError::SubmitFailed,
        ArmError::Overflow,
    ];
    for e in all {
        let text = e.to_string();
        assert!(!text.is_empty());
        assert!(text.is_ascii());
        assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
    }
}
