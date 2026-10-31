//! 3 段で受け渡す実行まわりの共通の型。パイプライン生成の入力の型・引数の宣言の型・キャッシュキーの型を置く（TASK-470 のポインタ）。
//!
//! 上（`fandhe-silicon-upper`）や下-2（`fandhe-silicon-exec`）が組み立てた入力を、
//! 下-1（`fandhe-silicon-contract`）の背後実装が受け取るときの形を定める。
//! ここには型の定義と、型に固有の定数を返す関数、引数の宣言の C4〜C8 を判定する純粋関数（`dispatch_check`）を置き、コンパイル・チップ固有の型は持たない。
//! 入力は借用で持つので、確保も所有権の移動も起きない。

mod compute;
mod dispatch_check;
mod submit;

pub use compute::{
    Binding, BindingLayout, ComputeOps, ConstantBinding, PipelineHandle, ThreadgroupMemory,
};
pub use dispatch_check::{
    OperandBinding, OperandConstant, required_bytes, validate_dispatch_operands,
};
#[doc(hidden)]
pub use submit::{__BackendEncoderCloser, __BackendSharedEncoderCloser, __backend_next_encoder_id};
pub use submit::{CommandEncoder, CommandQueue, GpuTiming, SubmitToken};

/// 相互運用で取り込んだセマフォを `submit` に結び付ける不透明なハンドル。
///
/// `ext-interop` の有無によらず置く。投入の型（`submit` モジュール）が `wait` の引数にこの型を使う。
/// 定義は `execution` の直下に置く（公開 API の検査が定義のパスを `execution` の中に限るため）。
/// 背後実装は `__backend_` で始まる口で作り、読み出す。
#[derive(Debug)]
pub struct ImportedSemaphoreHandle {
    id: u64,
}

impl ImportedSemaphoreHandle {
    /// 背後実装向けの組み立ての口。
    #[doc(hidden)]
    pub fn __backend_new(id: u64) -> Self {
        Self { id }
    }

    /// 背後実装向けの読み出しの口。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }
}

/// MSL の算術関数の扱い（数学モード）。
///
/// `#[non_exhaustive]` なので、後から variant を足しても外の crate の `match` は壊れない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MslMathMode {
    /// 安全側の算術（NaN・無限大の扱いを保つ）。
    Safe,
    /// 速さを優先する算術。
    Fast,
}

/// MSL の浮動小数点関数の精度の選択。
///
/// `#[non_exhaustive]` なので、後から variant を足せる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MslFloatFunctions {
    /// 速い版の関数を使う。
    Fast,
    /// 精度を優先した版の関数を使う。
    Precise,
}

/// function constant に与える値。
///
/// `#[non_exhaustive]` なので、後から型を足せる。
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum FunctionConstantValue {
    /// 32 ビット符号なし整数。
    U32(u32),
    /// 真偽値。
    Bool(bool),
}

/// function constant 1 件（番号と値の組）。
#[derive(Debug, Clone, Copy)]
pub struct FunctionConstant {
    /// シェーダ側で宣言された function constant の番号。
    pub index: u32,
    /// その番号に与える値。
    pub value: FunctionConstantValue,
}

/// パイプライン生成に渡す成果物と入口名の組。
///
/// 各 variant は受け取る成果物の種類を表す。成果物と入口名は借用で、
/// 生成が終わるまで呼び出し側が生かしておく。`#[non_exhaustive]` なので、
/// 後から variant を足しても外の crate の `match` は壊れない。
#[derive(Debug)]
#[non_exhaustive]
pub enum PipelineInput<'a> {
    /// MSL のソース。
    Msl {
        /// MSL のソース文字列。
        source: &'a str,
        /// 入口の関数名。
        entry_point: &'a str,
    },
    /// SPIR-V のワード列。
    SpirV {
        /// SPIR-V のワード列。
        words: &'a [u32],
        /// 入口の関数名。
        entry_point: &'a str,
    },
    /// CUDA C のソース。
    CudaC {
        /// CUDA C のソース文字列。
        source: &'a str,
        /// 入口の関数名。
        entry_point: &'a str,
    },
    /// PTX のテキスト。
    Ptx {
        /// PTX のテキスト。
        source: &'a str,
        /// 入口の関数名。
        entry_point: &'a str,
    },
    /// DXIL のバイト列。
    Dxil {
        /// DXIL のバイト列。
        bytes: &'a [u8],
        /// 入口の関数名。
        entry_point: &'a str,
    },
    /// WGSL のソース。
    Wgsl {
        /// WGSL のソース文字列。
        source: &'a str,
        /// 入口の関数名。
        entry_point: &'a str,
    },
    /// function constant で特殊化する MSL のソース。
    MslSpecialized {
        /// MSL のソース文字列。
        source: &'a str,
        /// 入口の関数名。
        entry_point: &'a str,
        /// 特殊化に使う function constant の並び。
        function_constants: &'a [FunctionConstant],
    },
    /// コンパイルの選択を伴う MSL のソース。
    MslWithOptions {
        /// MSL のソース文字列。
        source: &'a str,
        /// 入口の関数名。
        entry_point: &'a str,
        /// 算術の数学モード。
        math_mode: MslMathMode,
    },
    /// 特殊化とコンパイルの選択を両方伴う MSL のソース。
    MslSpecializedWithOptions {
        /// MSL のソース文字列。
        source: &'a str,
        /// 入口の関数名。
        entry_point: &'a str,
        /// 特殊化に使う function constant の並び。
        function_constants: &'a [FunctionConstant],
        /// 算術の数学モード。
        math_mode: MslMathMode,
        /// 浮動小数点関数の精度の選択。
        float_functions: MslFloatFunctions,
    },
}

/// 引数 1 要素の型。
///
/// `#[non_exhaustive]` なので、後から variant を足しても外の crate の `match` は壊れない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DType {
    /// 32 bit 浮動小数点。
    F32,
    /// 16 bit 浮動小数点（binary16）。
    F16,
    /// 符号なし 32 bit 整数。
    U32,
    /// 符号付き 32 bit 整数。
    I32,
    /// 真偽値。
    Bool,
}

impl DType {
    /// 1 要素のバイト数を返す（`F32`・`U32`・`I32` は 4、`F16` は 2、`Bool` は 1）。
    #[must_use]
    pub const fn byte_size(self) -> u64 {
        match self {
            Self::F32 | Self::U32 | Self::I32 => 4,
            Self::F16 => 2,
            Self::Bool => 1,
        }
    }
}

/// 引数 1 つの宣言。
///
/// 形と stride は借用で持つので、確保も所有権の移動も起きない。
/// この型自体は値を保持するだけで、検査と必要バイト数の計算は `validate_dispatch_operands`・`required_bytes` が行う。
#[derive(Debug, Clone, Copy)]
pub struct OperandSpec<'a> {
    /// 束縛の番号。
    pub slot: u32,
    /// 要素の型。
    pub dtype: DType,
    /// 各次元の要素数。
    pub shape: &'a [u64],
    /// 各次元の stride。
    pub stride: &'a [i64],
    /// offset の境界のバイト数。
    pub alignment: u32,
}

/// パイプラインのキャッシュを引くためのキー。
///
/// 値を保持するだけで、ハッシュの計算やキーの組み立てはしない。
/// 全フィールドが等価比較とハッシュに使われるので、1 つでも違えば別のキーになる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// ソースのハッシュ値。
    pub source_hash: u64,
    /// コンパイラの識別名。
    pub compiler_id: &'static str,
    /// コンパイラの版の値。
    pub compiler_version: u64,
    /// ビルドの対象の名前。
    pub target: &'static str,
    /// コンパイルの選択のハッシュ値。
    pub options_hash: u64,
    /// 境界の検査を有効にしたか。
    pub bounds_checked: bool,
    /// 特殊化の値のハッシュ値。
    pub specialization_hash: u64,
    /// ドライバの版の値。取れないときは `None`。
    pub driver_version: Option<u64>,
}
/// 列挙型の網羅の試験（TASK-470 のポインタ）。
///
/// 同じ crate の中では `#[non_exhaustive]` が効かないので、`_` の腕を持たない `match` が書ける。
/// variant を足すとこのコンパイルが落ち、試験の更新が要る。外の crate からは `_` が要る
/// （`crates/core/tests/execution_type_coverage.rs`）。
#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline_index(input: &PipelineInput<'_>) -> usize {
        match input {
            PipelineInput::Msl { .. } => 0,
            PipelineInput::SpirV { .. } => 1,
            PipelineInput::CudaC { .. } => 2,
            PipelineInput::Ptx { .. } => 3,
            PipelineInput::Dxil { .. } => 4,
            PipelineInput::Wgsl { .. } => 5,
            PipelineInput::MslSpecialized { .. } => 6,
            PipelineInput::MslWithOptions { .. } => 7,
            PipelineInput::MslSpecializedWithOptions { .. } => 8,
        }
    }

    #[test]
    fn execution_pipeline_input_has_nine_variants() {
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
                float_functions: MslFloatFunctions::Precise,
            },
        ];
        let mut seen = [false; 9];
        for input in &inputs {
            seen[pipeline_index(input)] = true;
        }
        assert!(seen.iter().all(|s| *s));
        assert_eq!(inputs.len(), 9);
    }

    #[test]
    fn execution_small_enums_have_expected_variants() {
        let modes = [MslMathMode::Safe, MslMathMode::Fast];
        for m in modes {
            match m {
                MslMathMode::Safe | MslMathMode::Fast => {}
            }
        }
        assert_ne!(modes[0], modes[1]);

        let funcs = [MslFloatFunctions::Fast, MslFloatFunctions::Precise];
        for f in funcs {
            match f {
                MslFloatFunctions::Fast | MslFloatFunctions::Precise => {}
            }
        }
        assert_ne!(funcs[0], funcs[1]);

        let values = [
            FunctionConstantValue::U32(1),
            FunctionConstantValue::Bool(true),
        ];
        for v in values {
            match v {
                FunctionConstantValue::U32(_) | FunctionConstantValue::Bool(_) => {}
            }
        }
        assert!(matches!(values[0], FunctionConstantValue::U32(1)));
        assert!(matches!(values[1], FunctionConstantValue::Bool(true)));
    }

    #[test]
    fn execution_dtype_has_five_variants() {
        let all = [DType::F32, DType::F16, DType::U32, DType::I32, DType::Bool];
        for d in all {
            match d {
                DType::F32 | DType::F16 | DType::U32 | DType::I32 | DType::Bool => {}
            }
        }
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(all.len(), 5);
    }
}
