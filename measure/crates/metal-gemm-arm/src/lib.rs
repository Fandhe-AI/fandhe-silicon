//! Metal 直接の GEMM の腕の crate（TASK-1008 のポインタ）。
//!
//! 測定 → 製品の一方向だけに依存する。ここには、外部のカーネルのソースを固定の rev・パス・sha256 で
//! 照らす [`source`] module と、門の 1 セルの起動の表と詰め物・切り出しを持つ [`launch`] module を置く。
//! bin `fetch-kernel-source` が取得し、腕が [`source::load_source`] で読む。
//!
//! cooperative matrix の GEMM の MSL と起動の形・offline compiler のコマンドの引数は [`coopmat`] module に置く（TASK-1009 のポインタ）。
//!
//! macOS 向けにだけ、橋の module `metal` が Metal を直接呼んで 1 セルを 1 回投入する `run_g6` と `run_g7` を出す。
//! `run_g7` は MSL を offline compiler で metallib にして読み、cooperative matrix の 1 セルを 1 回投入する（TASK-1009 のポインタ）。
//! `unsafe` を書くのは `src/metal.rs` だけで、個数は `tests/unsafe_count.rs` の表が固定する。
//! 門の 1 セルを device で流す試験は `tests/gate.rs`（`run_g6`）と `tests/gate_g7.rs`（`run_g7`）にあり、どちらも `#[ignore]` 付き。

use std::fmt;

pub mod coopmat;
pub mod launch;
pub mod source;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod metal;
#[cfg(target_os = "macos")]
mod metallib;

#[cfg(target_os = "macos")]
pub use metal::{run_g6, run_g7};

/// 投入した metallib の pipeline について読み出した事実。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetallibFacts {
    /// pipeline が受け付ける threadgroup あたりのスレッド数の上限。
    pub max_total_threads_per_threadgroup: usize,
    /// 読み込んだ metallib のバイト列の sha256。
    pub metallib_sha256: [u8; 32],
}

/// 投入した pipeline について読み出した事実。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineFacts {
    /// pipeline が受け付ける threadgroup あたりのスレッド数の上限。
    pub max_total_threads_per_threadgroup: usize,
    /// 設定した threadgroup のメモリ長（index 0）。
    pub threadgroup_memory_length: usize,
}

/// 腕の失敗。文言は固定の英語で、パス・ハッシュ・ソース・入力の値を含まない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArmError {
    /// このセルは腕の対象外。
    UnsupportedCell,
    /// 入力の dtype がセルと食い違う。
    DtypeMismatch,
    /// 入力・出力の長さまたは次元が食い違う。
    LengthMismatch,
    /// 既定の device が無い。
    NoDevice,
    /// コンパイルのオプションの設定の関数が無い。
    MissingCompileOption,
    /// ライブラリのコンパイルに失敗した。
    CompileFailed,
    /// 入口の関数を作れなかった。
    FunctionFailed,
    /// pipeline を作れなかった。
    PipelineFailed,
    /// スレッド数が pipeline の上限を超える。
    ThreadsExceeded,
    /// バッファまたは文字列の確保に失敗した。
    AllocationFailed,
    /// 投入が完了しなかった。
    SubmitFailed,
    /// 寸法の計算があふれた。
    Overflow,
    /// カーネルのソースが固定のハッシュと一致しない。
    SourceMismatch,
    /// offline compiler が失敗した。
    ToolFailed,
    /// 作業のディレクトリまたはファイルの操作に失敗した。
    WorkDirFailed,
    /// metallib の読み込みに失敗した。
    LibraryLoadFailed,
}

impl fmt::Display for ArmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ArmError::UnsupportedCell => "cell is not supported by this arm",
            ArmError::DtypeMismatch => "input dtype does not match the cell",
            ArmError::LengthMismatch => "length or dimension mismatch",
            ArmError::NoDevice => "no default Metal device",
            ArmError::MissingCompileOption => "compile option setter is unavailable",
            ArmError::CompileFailed => "library compilation failed",
            ArmError::FunctionFailed => "kernel function creation failed",
            ArmError::PipelineFailed => "pipeline creation failed",
            ArmError::ThreadsExceeded => "thread count exceeds the pipeline limit",
            ArmError::AllocationFailed => "allocation failed",
            ArmError::SubmitFailed => "command buffer did not complete",
            ArmError::SourceMismatch => "kernel source differs from the pinned digest",
            ArmError::Overflow => "size computation overflowed",
            ArmError::ToolFailed => "offline shader compiler failed",
            ArmError::WorkDirFailed => "work directory operation failed",
            ArmError::LibraryLoadFailed => "metallib loading failed",
        };
        f.write_str(text)
    }
}

impl std::error::Error for ArmError {}
