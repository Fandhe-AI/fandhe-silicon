//! bin `accept-kernel` の手順の部品（TASK-485 のポインタ）。
//!
//! 一覧の 1 行を選ぶ・ソースの置き場を決める・入口の並びを照らす・パイプラインの入力を組む部分は
//! 純粋で、どの OS でも試験する。device を作るのは macOS の cfg の内側の [`create_on_device`] だけで、
//! bin が照合をすべて通した後の最後の手順で呼ぶ。エラーは固定の文字列の種類に写し、
//! Metal の説明や入口名を含む `ContractError` の文言は外へ出さない。

use std::path::{Path, PathBuf};

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{FunctionConstant, PipelineInput};

use crate::manifest::{Manifest, ManifestEntry, Mode};
use crate::scan::scan_entries;

/// パイプラインを作れた。
pub const EXIT_ACCEPTED: u8 = 0;
/// 照合か device の手順で失敗した。
pub const EXIT_FAILED: u8 = 1;
/// 引数・一覧・選んだ組が不正で、device に届く前に返った。
pub const EXIT_USAGE: u8 = 2;
/// macOS 以外で、Metal を使えない。
pub const EXIT_NOT_MACOS: u8 = 3;

/// 一覧から `<path> <entry>` の組の行を選ぶ。入口名で引いた行のパスが `path` と違えば `None`。
pub fn select_row<'a>(m: &'a Manifest, path: &str, entry: &str) -> Option<&'a ManifestEntry> {
    m.find(entry).filter(|row| row.path() == path)
}

/// 行のソースを読むパス。`dir` の直下の、パスの最後の成分のファイル。
pub fn source_file(dir: &Path, row: &ManifestEntry) -> PathBuf {
    dir.join(row.file_name())
}

/// ソースから抜き出した入口名の並びが、一覧のそのパスの入口の並び（数も順も）と一致するか。
pub fn entries_match(m: &Manifest, path: &str, source: &str) -> bool {
    let Some(file) = m.files().into_iter().find(|f| f.path() == path) else {
        return false;
    };
    let listed: Vec<&str> = file.entries().iter().map(|e| e.entry()).collect();
    scan_entries(source) == listed
}

/// 行の種類に合うパイプラインの入力を組む。`constants` は `specialized` の行だけが使う。
pub fn pipeline_input<'a>(
    row: &'a ManifestEntry,
    source: &'a str,
    constants: &'a [FunctionConstant],
) -> PipelineInput<'a> {
    match row.mode() {
        Mode::Msl => PipelineInput::Msl {
            source,
            entry_point: row.entry(),
        },
        Mode::Specialized => PipelineInput::MslSpecialized {
            source,
            entry_point: row.entry(),
            function_constants: constants,
        },
    }
}

/// 契約のエラーを、値を含まない固定の種類の文字列に写す。
pub fn error_kind(e: &ContractError) -> &'static str {
    match e {
        ContractError::InvalidCombination(_) => "invalid-combination",
        ContractError::DeviceUnavailable(_) => "device-unavailable",
        ContractError::DeviceLost { .. } => "device-lost",
        ContractError::DeviceMismatch => "device-mismatch",
        ContractError::InvalidArgument(_) => "invalid-argument",
        ContractError::AllocationFailed { .. } => "allocation-failed",
        ContractError::ArtifactRejected { kind, .. } => match *kind {
            "msl" => "artifact-rejected-msl",
            "compute-pipeline" => "artifact-rejected-pipeline",
            _ => "artifact-rejected",
        },
        ContractError::ValidationFailed(_) => "validation-failed",
        ContractError::Timeout => "timeout",
        ContractError::Unsupported { .. } => "unsupported",
        ContractError::CapabilityInsufficient { .. } => "capability-insufficient",
        ContractError::DeviceIdentityMismatch { .. } => "device-identity-mismatch",
        ContractError::UnsupportedExtensionDefault { .. } => "unsupported-extension-default",
        ContractError::BackendFailure { .. } => "backend-failure",
        ContractError::CapacityExceeded { .. } => "capacity-exceeded",
        _ => "other",
    }
}

/// device の手順の失敗。中身は固定の種類の文字列だけ。
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStepError {
    /// device の取得に失敗した。
    Request(&'static str),
    /// device が compute の口を持たない。
    NoCompute,
    /// パイプラインの作成に失敗した。
    Create(&'static str),
}

/// Metal の device を 1 つ作り、空の配置でパイプラインを作る。投入はしない。
///
/// bin の手順 6 から呼ばれる。ソースの照合と入口の並びの照合を通した後にだけ呼ぶ。
#[cfg(target_os = "macos")]
pub fn create_on_device(input: PipelineInput<'_>) -> Result<(), DeviceStepError> {
    use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
    use fandhe_silicon_core::capability::BackendKind;
    use fandhe_silicon_core::device::{DeviceOwnership, DeviceProvider, DeviceRequest, Profile};
    use fandhe_silicon_core::execution::BindingLayout;

    let device = MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .map_err(|e| DeviceStepError::Request(error_kind(&e)))?;
    let compute = device.compute().ok_or(DeviceStepError::NoCompute)?;
    compute
        .create_pipeline(input, &BindingLayout::new(vec![]))
        .map(|_| ())
        .map_err(|e| DeviceStepError::Create(error_kind(&e)))
}
