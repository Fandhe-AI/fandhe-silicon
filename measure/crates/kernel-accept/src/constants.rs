//! 一覧で `specialized` の行に渡す function constant の並び（TASK-485 のポインタ）。
//!
//! bin `accept-kernel` が `PipelineInput::MslSpecialized` を組むときに呼ぶ。
//! index と型は起動の表（`launch_for`）の f32・`Sq512` のセルの定数と同じで、値だけを契約の型へ写す。

use fandhe_silicon_core::execution::FunctionConstant;
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::launch::launch_for;

use crate::flow::launch_function_constants;

/// 一覧で `specialized` の行に渡す function constant の並び。起動の表のセルが引けないときは空を返す。
pub fn specialized_constants() -> Vec<FunctionConstant> {
    let Some(cell) = gate_cells().into_iter().find(|c| {
        c.arm() == GemmGateArm::G6
            && c.row() == GemmGateRow::Sq512
            && c.dtype() == GemmInputDtype::F32
    }) else {
        return Vec::new();
    };
    let Some(launch) = launch_for(cell) else {
        return Vec::new();
    };
    launch_function_constants(&launch)
}
