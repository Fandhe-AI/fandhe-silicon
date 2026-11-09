//! 比較基準の MPS の腕（TASK-1011）の純粋な部分の試験。device は使わない。

use baseline_wgpu_mps::{
    ElementType, OutputKind, PlanError, check_row_bytes, pack_f16, pack_f32, plan, unpack_f16,
    unpack_f32,
};
use fandhe_silicon_core::parity::gemm_inputs::GemmInputDtype;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell, gate_cells};

fn mps_cells() -> Vec<GemmGateCell> {
    let cells: Vec<_> = gate_cells()
        .into_iter()
        .filter(|c| c.arm() == GemmGateArm::Mps)
        .collect();
    assert!(!cells.is_empty());
    cells
}

#[test]
fn mps_cells_plan_with_same_as_input() {
    for cell in mps_cells() {
        let p = plan(cell, OutputKind::SameAsInput).unwrap();
        let row = cell.row();
        assert_eq!((p.m(), p.n(), p.k()), (row.m(), row.n(), row.k()));
        let expected = match cell.dtype() {
            GemmInputDtype::F32 => ElementType::F32,
            GemmInputDtype::F16Acc => ElementType::F16,
        };
        assert_eq!((p.input(), p.output()), (expected, expected));
    }
}

#[test]
fn f16acc_cells_plan_with_f32_output() {
    let mut seen = 0;
    for cell in mps_cells() {
        if cell.dtype() != GemmInputDtype::F16Acc {
            continue;
        }
        seen += 1;
        let p = plan(cell, OutputKind::F32).unwrap();
        let row = cell.row();
        assert_eq!(
            (p.input(), p.output()),
            (ElementType::F16, ElementType::F32)
        );
        assert_eq!((p.m(), p.n(), p.k()), (row.m(), row.n(), row.k()));
    }
    assert!(seen > 0);
}

#[test]
fn f32_cell_rejects_f32_output() {
    let mut seen = 0;
    for cell in mps_cells() {
        if cell.dtype() == GemmInputDtype::F32 {
            seen += 1;
            assert_eq!(
                plan(cell, OutputKind::F32),
                Err(PlanError::OutputNotForCell)
            );
        }
    }
    assert!(seen > 0);
}

#[test]
fn non_mps_cells_are_rejected() {
    let mut seen = 0;
    for cell in gate_cells() {
        if cell.arm() != GemmGateArm::Mps {
            seen += 1;
            assert_eq!(
                plan(cell, OutputKind::SameAsInput),
                Err(PlanError::NotMpsCell)
            );
            assert_eq!(plan(cell, OutputKind::F32), Err(PlanError::NotMpsCell));
        }
    }
    assert!(seen > 0);
}

#[test]
fn element_sizes() {
    assert_eq!(ElementType::F32.size(), 4);
    assert_eq!(ElementType::F16.size(), 2);
}

#[test]
fn check_row_bytes_boundaries() {
    for (element, size) in [(ElementType::F32, 4usize), (ElementType::F16, 2usize)] {
        let min = 5 * size;
        assert_eq!(check_row_bytes(5, element, min, 3), Ok(3 * min));
        assert_eq!(
            check_row_bytes(5, element, min + size, 3),
            Ok(3 * (min + size))
        );
        assert_eq!(
            check_row_bytes(5, element, min - 1, 3),
            Err(PlanError::RowBytesTooSmall)
        );
        assert_eq!(
            check_row_bytes(5, element, min + 1, 3),
            Err(PlanError::RowBytesNotAligned)
        );
        assert_eq!(check_row_bytes(0, element, 0, 7), Ok(0));
        assert_eq!(
            check_row_bytes(usize::MAX, element, usize::MAX, 1),
            Err(PlanError::SizeOverflow)
        );
        assert_eq!(
            check_row_bytes(1, element, size, usize::MAX),
            Err(PlanError::SizeOverflow)
        );
    }
}

fn f32_values(rows: usize, columns: usize) -> Vec<f32> {
    (0..rows * columns).map(|i| i as f32 * 0.5 - 3.0).collect()
}

fn f16_values(rows: usize, columns: usize) -> Vec<u16> {
    (0..rows * columns)
        .map(|i| (i as u16).wrapping_mul(257))
        .collect()
}

#[test]
fn pack_unpack_round_trip_f32() {
    for row_bytes in [12, 16, 32] {
        let dense = f32_values(5, 3);
        let bytes = pack_f32(&dense, 5, 3, row_bytes).unwrap();
        assert_eq!(bytes.len(), 5 * row_bytes);
        assert_eq!(unpack_f32(&bytes, 5, 3, row_bytes).unwrap(), dense);
    }
}

#[test]
fn pack_unpack_round_trip_f16() {
    for row_bytes in [6, 8, 16] {
        let dense = f16_values(4, 3);
        let bytes = pack_f16(&dense, 4, 3, row_bytes).unwrap();
        assert_eq!(bytes.len(), 4 * row_bytes);
        assert_eq!(unpack_f16(&bytes, 4, 3, row_bytes).unwrap(), dense);
    }
}

#[test]
fn pack_pads_with_zero_and_keeps_little_endian() {
    let bytes = pack_f32(&[1.0, 2.0], 2, 1, 8).unwrap();
    assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
    assert_eq!(&bytes[4..8], &[0; 4]);
    assert_eq!(&bytes[8..12], &2.0f32.to_le_bytes());
    assert_eq!(&bytes[12..16], &[0; 4]);

    let bytes = pack_f16(&[0x0102, 0x0304], 2, 1, 4).unwrap();
    assert_eq!(bytes, vec![0x02, 0x01, 0, 0, 0x04, 0x03, 0, 0]);
}

#[test]
fn unpack_ignores_padding() {
    let mut bytes = pack_f16(&[7, 9], 2, 1, 4).unwrap();
    bytes[2] = 0xff;
    bytes[7] = 0xaa;
    assert_eq!(unpack_f16(&bytes, 2, 1, 4).unwrap(), vec![7, 9]);
}

#[test]
fn length_mismatch_is_err() {
    assert_eq!(
        pack_f32(&[1.0; 5], 2, 3, 12),
        Err(PlanError::LengthMismatch)
    );
    assert_eq!(pack_f16(&[1; 7], 2, 3, 6), Err(PlanError::LengthMismatch));
    assert_eq!(
        unpack_f32(&[0; 23], 2, 3, 12),
        Err(PlanError::LengthMismatch)
    );
    assert_eq!(
        unpack_f16(&[0; 13], 2, 3, 6),
        Err(PlanError::LengthMismatch)
    );
}

#[test]
fn invalid_row_bytes_are_err_in_pack_and_unpack() {
    assert_eq!(
        pack_f32(&[1.0; 6], 2, 3, 8),
        Err(PlanError::RowBytesTooSmall)
    );
    assert_eq!(
        unpack_f16(&[0; 14], 2, 3, 7),
        Err(PlanError::RowBytesNotAligned)
    );
}

#[test]
fn zero_columns_round_trip() {
    let bytes = pack_f32(&[], 3, 0, 8).unwrap();
    assert_eq!(bytes, vec![0; 24]);
    assert_eq!(unpack_f32(&bytes, 3, 0, 8).unwrap(), Vec::<f32>::new());
}
