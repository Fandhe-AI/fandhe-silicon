//! `execution` の公開の型の形を外の crate から固定する試験（TASK-470 のポインタ）。
//!
//! `#![deny(unreachable_patterns)]` により、`#[non_exhaustive]` の enum で全 variant を並べたうえに
//! `_` の腕を置いた `match` は、属性が外れると到達不能になってコンパイルが落ちる。
//! struct は `..` なしの分解で、フィールドの集合を固定する。feature なしで動く。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::execution::{
    CacheKey, DType, FunctionConstant, FunctionConstantValue, MslFloatFunctions, MslMathMode,
    OperandSpec, PipelineInput,
};

#[test]
fn execution_enums_stay_non_exhaustive() {
    let math = |m: MslMathMode| match m {
        MslMathMode::Safe => "safe",
        MslMathMode::Fast => "fast",
        _ => "other",
    };
    assert_eq!(math(MslMathMode::Safe), "safe");
    assert_eq!(math(MslMathMode::Fast), "fast");

    let funcs = |f: MslFloatFunctions| match f {
        MslFloatFunctions::Fast => "fast",
        MslFloatFunctions::Precise => "precise",
        _ => "other",
    };
    assert_eq!(funcs(MslFloatFunctions::Fast), "fast");
    assert_eq!(funcs(MslFloatFunctions::Precise), "precise");

    let value = |v: FunctionConstantValue| match v {
        FunctionConstantValue::U32(_) => "u32",
        FunctionConstantValue::Bool(_) => "bool",
        _ => "other",
    };
    assert_eq!(value(FunctionConstantValue::U32(0)), "u32");
    assert_eq!(value(FunctionConstantValue::Bool(false)), "bool");

    let dtype = |d: DType| match d {
        DType::F32 => "f32",
        DType::F16 => "f16",
        DType::U32 => "u32",
        DType::I32 => "i32",
        DType::Bool => "bool",
        _ => "other",
    };
    let names: Vec<_> = [DType::F32, DType::F16, DType::U32, DType::I32, DType::Bool]
        .into_iter()
        .map(dtype)
        .collect();
    assert_eq!(names, ["f32", "f16", "u32", "i32", "bool"]);
}

#[test]
fn execution_pipeline_input_stays_non_exhaustive() {
    let e = "main";
    let fc: &[FunctionConstant] = &[];
    let inputs = [
        PipelineInput::Msl {
            source: "",
            entry_point: e,
        },
        PipelineInput::SpirV {
            words: &[],
            entry_point: e,
        },
        PipelineInput::CudaC {
            source: "",
            entry_point: e,
        },
        PipelineInput::Ptx {
            source: "",
            entry_point: e,
        },
        PipelineInput::Dxil {
            bytes: &[],
            entry_point: e,
        },
        PipelineInput::Wgsl {
            source: "",
            entry_point: e,
        },
        PipelineInput::MslSpecialized {
            source: "",
            entry_point: e,
            function_constants: fc,
        },
        PipelineInput::MslWithOptions {
            source: "",
            entry_point: e,
            math_mode: MslMathMode::Safe,
        },
        PipelineInput::MslSpecializedWithOptions {
            source: "",
            entry_point: e,
            function_constants: fc,
            math_mode: MslMathMode::Fast,
            float_functions: MslFloatFunctions::Fast,
        },
    ];
    let label = |i: &PipelineInput<'_>| match i {
        PipelineInput::Msl { .. } => "msl",
        PipelineInput::SpirV { .. } => "spirv",
        PipelineInput::CudaC { .. } => "cuda",
        PipelineInput::Ptx { .. } => "ptx",
        PipelineInput::Dxil { .. } => "dxil",
        PipelineInput::Wgsl { .. } => "wgsl",
        PipelineInput::MslSpecialized { .. } => "msl-spec",
        PipelineInput::MslWithOptions { .. } => "msl-opt",
        PipelineInput::MslSpecializedWithOptions { .. } => "msl-spec-opt",
        _ => "other",
    };
    let labels: Vec<_> = inputs.iter().map(label).collect();
    assert_eq!(
        labels,
        [
            "msl",
            "spirv",
            "cuda",
            "ptx",
            "dxil",
            "wgsl",
            "msl-spec",
            "msl-opt",
            "msl-spec-opt"
        ]
    );
}

#[test]
fn execution_structs_keep_field_sets() {
    // `..` なしの分解なので、フィールドが増減すればコンパイルが落ちる。
    let CacheKey {
        source_hash,
        compiler_id,
        compiler_version,
        target,
        options_hash,
        bounds_checked,
        specialization_hash,
        driver_version,
    } = CacheKey {
        source_hash: 1,
        compiler_id: "c",
        compiler_version: 2,
        target: "t",
        options_hash: 3,
        bounds_checked: true,
        specialization_hash: 4,
        driver_version: Some(5),
    };
    assert_eq!(
        (
            source_hash,
            compiler_version,
            options_hash,
            specialization_hash
        ),
        (1, 2, 3, 4)
    );
    assert_eq!(
        (compiler_id, target, bounds_checked, driver_version),
        ("c", "t", true, Some(5))
    );

    let shape = [2_u64, 3];
    let stride = [3_i64, 1];
    let OperandSpec {
        slot,
        dtype,
        shape: s,
        stride: st,
        alignment,
    } = OperandSpec {
        slot: 0,
        dtype: DType::F32,
        shape: &shape,
        stride: &stride,
        alignment: 16,
    };
    assert_eq!(
        (slot, dtype, s, st, alignment),
        (0, DType::F32, &shape[..], &stride[..], 16)
    );

    let FunctionConstant { index, value } = FunctionConstant {
        index: 7,
        value: FunctionConstantValue::Bool(true),
    };
    assert_eq!(index, 7);
    assert!(matches!(value, FunctionConstantValue::Bool(true)));
}
