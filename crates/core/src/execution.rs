//! 3 段で受け渡す実行まわりの共通の型。パイプライン生成の入力の型を置く（TASK-470 のポインタ）。
//!
//! 上（`fandhe-silicon-upper`）や下-2（`fandhe-silicon-exec`）が組み立てた入力を、
//! 下-1（`fandhe-silicon-contract`）の背後実装が受け取るときの形を定める。
//! ここには型の定義だけを置き、検証・コンパイル・チップ固有の型は持たない。
//! 入力は借用で持つので、確保も所有権の移動も起きない。

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
