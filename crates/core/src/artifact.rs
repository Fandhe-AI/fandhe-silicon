//! 成果物の検査の口（TASK-492）。feature `artifact-inspection` のビルドにだけある。
//!
//! `Device::artifact_hooks` が返す `ArtifactHooks` は、ソースや事前にコンパイルした成果物から
//! パイプラインを作る 3 つの操作を束ねる。持つ背後実装だけが `Device::artifact_hooks` を上書きする。

use crate::error::ContractError;
use crate::execution::{MslMathMode, OperandSpec, PipelineHandle};

/// 成果物からパイプラインを作る試験・診断用の操作の束。
///
/// `Device::artifact_hooks` 経由で呼ばれる。既定の本体は無く、実装が 3 つとも与える。
pub trait ArtifactHooks {
    /// ソースを数学モードつきでコンパイルし、`path` に binary archive を書き出して、書き出したバイト列を返す。
    fn compile_and_write_binary_archive(
        &self,
        source: &str,
        entry_point: &str,
        math_mode: MslMathMode,
        path: &std::path::Path,
    ) -> Result<Vec<u8>, ContractError>;

    /// `path` の binary archive を使ってパイプラインを作る。`fail_on_archive_miss` が `true` なら archive に無いときに失敗する。
    fn create_pipeline_from_archive_path(
        &self,
        source: &str,
        entry_point: &str,
        math_mode: MslMathMode,
        path: &std::path::Path,
        fail_on_archive_miss: bool,
    ) -> Result<PipelineHandle, ContractError>;

    /// 事前にコンパイルした metallib のファイルから `entry_point` の関数を読み、パイプラインを作る。ソースからはコンパイルしない。
    fn create_pipeline_checked_from_metallib(
        &self,
        metallib_path: &std::path::Path,
        entry_point: &str,
        declared: &[OperandSpec<'_>],
    ) -> Result<PipelineHandle, ContractError>;
}
