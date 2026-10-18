//! `execution` のパイプライン生成の入力の型を外の crate から組み立てて読み出す試験（TASK-470 のポインタ）。
//!
//! feature なしで動く。`#[non_exhaustive]` の型の `match` には `_` の腕が要る。

use fandhe_silicon_core::execution::{
    FunctionConstant, FunctionConstantValue, MslFloatFunctions, MslMathMode, PipelineInput,
};

/// variant ごとに入口名を取り出す（外の crate からの見え方の確認）。
fn entry_of<'a>(input: &PipelineInput<'a>) -> &'a str {
    match input {
        PipelineInput::Msl { entry_point, .. }
        | PipelineInput::SpirV { entry_point, .. }
        | PipelineInput::CudaC { entry_point, .. }
        | PipelineInput::Ptx { entry_point, .. }
        | PipelineInput::Dxil { entry_point, .. }
        | PipelineInput::Wgsl { entry_point, .. }
        | PipelineInput::MslSpecialized { entry_point, .. }
        | PipelineInput::MslWithOptions { entry_point, .. }
        | PipelineInput::MslSpecializedWithOptions { entry_point, .. } => entry_point,
        _ => "",
    }
}

#[test]
fn execution_source_variants_keep_fields() {
    let words = [0x0723_0203_u32, 1, 2];
    let bytes = *b"DXIL";
    let inputs = [
        PipelineInput::Msl {
            source: "m",
            entry_point: "e0",
        },
        PipelineInput::SpirV {
            words: &words,
            entry_point: "e1",
        },
        PipelineInput::CudaC {
            source: "c",
            entry_point: "e2",
        },
        PipelineInput::Ptx {
            source: "p",
            entry_point: "e3",
        },
        PipelineInput::Dxil {
            bytes: &bytes,
            entry_point: "e4",
        },
        PipelineInput::Wgsl {
            source: "w",
            entry_point: "e5",
        },
    ];
    for (i, input) in inputs.iter().enumerate() {
        assert_eq!(entry_of(input), format!("e{i}"));
    }
    match &inputs[1] {
        PipelineInput::SpirV { words: w, .. } => assert_eq!(*w, &words[..]),
        _ => panic!("unexpected variant"),
    }
    match &inputs[4] {
        PipelineInput::Dxil { bytes: b, .. } => assert_eq!(*b, &bytes[..]),
        _ => panic!("unexpected variant"),
    }
    match &inputs[0] {
        PipelineInput::Msl { source, .. } => assert_eq!(*source, "m"),
        _ => panic!("unexpected variant"),
    }
}

#[test]
fn execution_mode_enums_are_copy_and_eq() {
    let a = MslMathMode::Safe;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(MslMathMode::Safe, MslMathMode::Fast);
    let c = MslFloatFunctions::Precise;
    let d = c;
    assert_eq!(c, d);
    assert_ne!(MslFloatFunctions::Fast, MslFloatFunctions::Precise);
}

#[test]
fn execution_function_constants_keep_count_and_values() {
    let constants = [
        FunctionConstant {
            index: 0,
            value: FunctionConstantValue::U32(7),
        },
        FunctionConstant {
            index: 3,
            value: FunctionConstantValue::Bool(true),
        },
    ];
    let copied = constants[0];
    assert_eq!(copied.index, 0);

    let specialized = PipelineInput::MslSpecialized {
        source: "m",
        entry_point: "k",
        function_constants: &constants,
    };
    match specialized {
        PipelineInput::MslSpecialized {
            function_constants, ..
        } => {
            assert_eq!(function_constants.len(), 2);
            assert_eq!(function_constants[1].index, 3);
            assert!(matches!(
                function_constants[0].value,
                FunctionConstantValue::U32(7)
            ));
            assert!(matches!(
                function_constants[1].value,
                FunctionConstantValue::Bool(true)
            ));
        }
        _ => panic!("unexpected variant"),
    }
}

#[test]
fn execution_msl_option_variants_keep_fields() {
    let with_options = PipelineInput::MslWithOptions {
        source: "m",
        entry_point: "k",
        math_mode: MslMathMode::Fast,
    };
    match with_options {
        PipelineInput::MslWithOptions { math_mode, .. } => {
            assert_eq!(math_mode, MslMathMode::Fast);
        }
        _ => panic!("unexpected variant"),
    }

    let constants = [FunctionConstant {
        index: 1,
        value: FunctionConstantValue::Bool(false),
    }];
    let full = PipelineInput::MslSpecializedWithOptions {
        source: "m",
        entry_point: "k",
        function_constants: &constants,
        math_mode: MslMathMode::Safe,
        float_functions: MslFloatFunctions::Precise,
    };
    match full {
        PipelineInput::MslSpecializedWithOptions {
            function_constants,
            math_mode,
            float_functions,
            ..
        } => {
            assert_eq!(function_constants.len(), 1);
            assert_eq!(math_mode, MslMathMode::Safe);
            assert_eq!(float_functions, MslFloatFunctions::Precise);
        }
        _ => panic!("unexpected variant"),
    }
}
