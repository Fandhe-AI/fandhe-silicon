//! 実行時に環境変数で受けるカーネルのソースと、そのソースで 1 セルを投入する起動の形（TASK-1009 のポインタ）。
//!
//! 腕 `run_g5`（macOS 向けの橋の module `metal`）が [`ExternalKernel::from_env`] でソースのファイルのパス・
//! 期待の sha256・入口名を受け、sha256 を照らしてから device を作る。どの OS でもコンパイルする。
//! パス・バイト列・文字列を引数で受けて [`ExternalKernel`] を作る公開の口は持たない。

use std::ffi::OsString;
use std::fmt;
use std::path::Path;

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_gate::{GateInputs, ValidationLayerSetting};
use fandhe_silicon_core::parity::gemm_inputs::GemmInputDtype;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

use crate::ArmError;
use crate::source::{SourceError, load_source_against, parse_sha256_hex};

/// ソースのファイルのパスを受ける環境変数の名前。
pub const KERNEL_SOURCE_ENV: &str = "FANDHE_SILICON_KERNEL_SOURCE";

/// 期待の sha256（64 桁の小文字の 16 進）を受ける環境変数の名前。
pub const KERNEL_SHA256_ENV: &str = "FANDHE_SILICON_KERNEL_SHA256";

/// 入口名を受ける環境変数の名前。
pub const KERNEL_ENTRY_ENV: &str = "FANDHE_SILICON_KERNEL_ENTRY";

/// threadgroup の 1 辺のスレッド数。
pub const EXTERNAL_TILE: usize = 16;

/// 入口名の最大バイト数。
const MAX_ENTRY_BYTES: usize = 64;

/// 設定の失敗。文言は固定の英語で、値・パスを含まない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExternalError {
    /// 3 つの環境変数のどれかが未設定。
    Missing,
    /// sha256 が 64 桁の小文字の 16 進でない。
    InvalidDigest,
    /// 入口名が `[A-Za-z_][A-Za-z0-9_]{0,63}` でない。
    InvalidEntry,
    /// ソースを読めない、または照合に通らない。
    Source(SourceError),
}

impl fmt::Display for ExternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("a kernel variable is not set"),
            Self::InvalidDigest => f.write_str("kernel sha256 must be 64 lowercase hex digits"),
            Self::InvalidEntry => f.write_str("kernel entry name is invalid"),
            Self::Source(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ExternalError {}

/// 照合を通ったソースと入口名と計算した sha256。フィールドは private で、作る口は `from_lookup` と `from_env` だけ。
pub struct ExternalKernel {
    source: String,
    entry: String,
    sha256: [u8; 32],
}

impl fmt::Debug for ExternalKernel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalKernel").finish_non_exhaustive()
    }
}

fn is_valid_entry(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > MAX_ENTRY_BYTES {
        return false;
    }
    (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

impl ExternalKernel {
    /// `lookup` で 3 つの値を引き、sha256 の形・入口名の形・ソースの照合の順に検査する。
    pub fn from_lookup<F: Fn(&str) -> Option<OsString>>(lookup: F) -> Result<Self, ExternalError> {
        let (Some(path), Some(digest), Some(entry)) = (
            lookup(KERNEL_SOURCE_ENV),
            lookup(KERNEL_SHA256_ENV),
            lookup(KERNEL_ENTRY_ENV),
        ) else {
            return Err(ExternalError::Missing);
        };
        let digest = digest
            .to_str()
            .and_then(|s| parse_sha256_hex(s).ok())
            .ok_or(ExternalError::InvalidDigest)?;
        let entry = entry
            .to_str()
            .filter(|s| is_valid_entry(s))
            .ok_or(ExternalError::InvalidEntry)?
            .to_owned();
        let source =
            load_source_against(Path::new(&path), &digest).map_err(ExternalError::Source)?;
        let sha256 = sha256_bytes(source.as_bytes());
        Ok(Self {
            source,
            entry,
            sha256,
        })
    }

    /// プロセスの環境変数から作る。
    pub fn from_env() -> Result<Self, ExternalError> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// 照合済みのソースについて計算した sha256。
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    /// 照合済みのソース。
    pub fn source(&self) -> &str {
        &self.source
    }

    /// 検査済みの入口名。
    pub fn entry(&self) -> &str {
        &self.entry
    }
}

/// 1 セルの起動の形。詰め物はせず、M・N・K は元のまま。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalLaunch {
    /// threadgroup の幅のスレッド数。
    pub threads_width: usize,
    /// threadgroup の高さのスレッド数。
    pub threads_height: usize,
    /// threadgroup 数の幅（`ceil(n / 16)`）。
    pub groups_width: usize,
    /// threadgroup 数の高さ（`ceil(m / 16)`）。
    pub groups_height: usize,
}

/// 腕が f32 の `GemmGateArm::G5` のセルのときだけ起動の形を返す。
pub fn external_launch_for(cell: GemmGateCell) -> Option<ExternalLaunch> {
    if cell.arm() != GemmGateArm::G5 || cell.dtype() != GemmInputDtype::F32 {
        return None;
    }
    Some(ExternalLaunch {
        threads_width: EXTERNAL_TILE,
        threads_height: EXTERNAL_TILE,
        groups_width: cell.row().n().div_ceil(EXTERNAL_TILE),
        groups_height: cell.row().m().div_ceil(EXTERNAL_TILE),
    })
}

/// `run_g5` が device を作る前に行う検査（腕・起動の形・dtype・入力の長さ・検証レイヤーの順）。
///
/// `run_g5`（macOS 向けの橋の module `metal`）が本体の最初に呼ぶ。device もカーネルも使わない safe な関数で、
/// device に届く前に返る `Err` の試験が `unsafe` なしで直に呼べるように、橋の外に置く。
pub fn check_g5_request(cell: GemmGateCell, inputs: &GateInputs) -> Result<(), ArmError> {
    if cell.arm() != GemmGateArm::G5 {
        return Err(ArmError::UnsupportedCell);
    }
    external_launch_for(cell).ok_or(ArmError::UnsupportedCell)?;
    let GateInputs::F32(i) = inputs else {
        return Err(ArmError::DtypeMismatch);
    };
    let (m, n, k) = (cell.row().m(), cell.row().n(), cell.row().k());
    let a_elems = m.checked_mul(k).ok_or(ArmError::Overflow)?;
    let b_elems = k.checked_mul(n).ok_or(ArmError::Overflow)?;
    m.checked_mul(n).ok_or(ArmError::Overflow)?;
    if i.a().len() != a_elems || i.b().len() != b_elems {
        return Err(ArmError::LengthMismatch);
    }
    if !ValidationLayerSetting::from_env().is_enabled() {
        return Err(ArmError::ValidationLayerRequired);
    }
    Ok(())
}
