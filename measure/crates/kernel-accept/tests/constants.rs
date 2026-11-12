//! `specialized_constants` の試験（TASK-485 のポインタ）。起動の表のセルの定数と並び・型・値が同じことを確かめる。

use fandhe_silicon_core::execution::FunctionConstantValue;
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_kernel_accept::constants::specialized_constants;
use measure_metal_gemm_arm::launch::{ConstantValue, launch_for};

#[test]
fn eighteen_constants_with_ascending_indices() {
    let c = specialized_constants();
    assert_eq!(c.len(), 18);
    for (i, k) in c.iter().enumerate() {
        assert_eq!(k.index as usize, i);
    }
}

#[test]
fn matches_the_launch_table_cell() {
    let cell = gate_cells()
        .into_iter()
        .find(|c| {
            c.arm() == GemmGateArm::G6
                && c.row() == GemmGateRow::Sq512
                && c.dtype() == GemmInputDtype::F32
        })
        .expect("cell");
    let launch = launch_for(cell).expect("launch");
    let c = specialized_constants();
    assert_eq!(c.len(), launch.constants.len());
    for (got, &(index, want)) in c.iter().zip(launch.constants.iter()) {
        assert_eq!(got.index, index);
        match (got.value, want) {
            (FunctionConstantValue::U32(a), ConstantValue::UInt(b)) => assert_eq!(a, b),
            (FunctionConstantValue::Bool(a), ConstantValue::Bool(b)) => assert_eq!(a, b),
            _ => panic!("type differs at index {index}"),
        }
    }
}
