//! 一覧で `specialized` の行に渡す function constant の並び（TASK-485 のポインタ）。
//!
//! bin `accept-kernel` が `PipelineInput::MslSpecialized` を組むときに、`specialized_constants` を呼ぶ。
//! `launch_function_constants` は `specialized_constants` と試験から呼ぶ。
//! index と型は起動の表（`launch_for`）の f32・`Sq512` のセルの定数と同じで、値だけを契約の型へ写す。

use fandhe_silicon_core::execution::{FunctionConstant, FunctionConstantValue};
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::launch::{ConstantValue, Launch, launch_for};

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

/// `Launch` の function constant を契約の型に写す。
pub fn launch_function_constants(launch: &Launch) -> Vec<FunctionConstant> {
    launch
        .constants
        .iter()
        .map(|&(index, v)| FunctionConstant {
            index,
            value: match v {
                ConstantValue::UInt(u) => FunctionConstantValue::U32(u),
                ConstantValue::Bool(b) => FunctionConstantValue::Bool(b),
            },
        })
        .collect()
}
