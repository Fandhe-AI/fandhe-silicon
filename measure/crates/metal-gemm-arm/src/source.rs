//! 外部のカーネルのソース 1 本を、固定の rev・パス・sha256 で照らす純粋な関数と定数。
//!
//! bin `fetch-kernel-source` が取得したバイト列を [`verify_source`] に通し、
//! 後続の腕が [`load_source`] で書き出し済みのファイルを読み直す。取得したソースは repo に置かない。

use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use fandhe_silicon_core::hash::sha256_bytes;

/// 取得するカーネルのソースの rev（40 桁の 16 進）。
pub const SOURCE_REV: &str = "b3c935c7d5816766c272388bc90b4e02fd498d7a";

/// 取得するカーネルのソースの、clone の中のパス。
pub const GEMM_SOURCE_PATH: &str = "crates/backend-metal/src/shaders/gemm.metal";

/// 取得するカーネルのソースの sha256。
pub const GEMM_SOURCE_SHA256: [u8; 32] = [
    0x7c, 0x01, 0x27, 0xd7, 0x7e, 0x84, 0x8b, 0x82, 0x3d, 0xf2, 0x7c, 0x09, 0xbc, 0x6b, 0x2b, 0x6e,
    0xc6, 0xd8, 0x37, 0xb1, 0xd3, 0xee, 0x4d, 0x57, 0xfb, 0x19, 0x14, 0x95, 0x23, 0xd9, 0xd1, 0xa4,
];

/// 受け付けるソースの最大バイト数。超える入力はハッシュの前に拒否する。
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// ソースの照合・読み取りの失敗。文言は固定で、パスやハッシュの値を含まない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceError {
    /// ファイルを通常のファイルとして読めない。
    Unreadable,
    /// 長さが [`MAX_SOURCE_BYTES`] を超える。
    TooLarge,
    /// sha256 が期待の値と違う。
    DigestMismatch,
    /// UTF-8 として読めない。
    NotUtf8,
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unreadable => "cannot read the source file",
            Self::TooLarge => "source exceeds the size limit",
            Self::DigestMismatch => "source sha256 differs from the pinned value",
            Self::NotUtf8 => "source is not valid UTF-8",
        })
    }
}

impl std::error::Error for SourceError {}

/// `bytes` を長さ、sha256、UTF-8 の順に照らし、通ったときだけ文字列で返す。
pub fn verify_source_against<'a>(
    bytes: &'a [u8],
    expected: &[u8; 32],
) -> Result<&'a str, SourceError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(SourceError::TooLarge);
    }
    if sha256_bytes(bytes) != *expected {
        return Err(SourceError::DigestMismatch);
    }
    std::str::from_utf8(bytes).map_err(|_| SourceError::NotUtf8)
}

/// `bytes` を [`GEMM_SOURCE_SHA256`] で照らす。
pub fn verify_source(bytes: &[u8]) -> Result<&str, SourceError> {
    verify_source_against(bytes, &GEMM_SOURCE_SHA256)
}

/// `path` の通常のファイルを上限つきで読み、[`verify_source`] を通ったものだけを返す。
pub fn load_source(path: &Path) -> Result<String, SourceError> {
    let meta = fs::metadata(path).map_err(|_| SourceError::Unreadable)?;
    if !meta.is_file() {
        return Err(SourceError::Unreadable);
    }
    let file = File::open(path).map_err(|_| SourceError::Unreadable)?;
    let mut buf = Vec::new();
    file.take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| SourceError::Unreadable)?;
    verify_source(&buf).map(str::to_owned)
}
