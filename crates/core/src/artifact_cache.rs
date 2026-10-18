//! 成果物キャッシュの照合の部品の置き場（TASK-483・ART-52 のポインタ）。
//!
//! 現状はエントリの型 [`CacheEntry`] だけを持つ。入れ物・保存先・読み込み時の照合は持たない。
//! `fandhe-silicon-exec` / `fandhe-silicon-contract` が成果物を保存・復元するときに、
//! キーとバイト列とそのバイト列の SHA-256 を組にして受け渡すための共通の型である。
//!
//! SHA-256 は完全性の識別のためのもので、認証や改ざん耐性（秘密の比較・MAC）の用途は対象外。

use crate::execution::CacheKey;
use crate::hash::sha256_bytes;

/// 成果物キャッシュのエントリ（キー・成果物のバイト列・そのバイト列の SHA-256 の組）。
///
/// 保持する SHA-256 がバイト列と一致しないエントリも表せる（[`CacheEntry::from_parts`]）。
/// 一致の確認はこの型ではしない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    key: CacheKey,
    bytes: Vec<u8>,
    sha256: [u8; 32],
}

impl CacheEntry {
    /// バイト列の SHA-256 を計算してエントリを作る。計算量はバイト列の長さに比例する。
    #[must_use]
    pub fn new(key: CacheKey, bytes: Vec<u8>) -> Self {
        let sha256 = sha256_bytes(&bytes);
        Self { key, bytes, sha256 }
    }

    /// 保存済みの部品からそのまま組み立てる。SHA-256 の再計算も照合もしない。
    ///
    /// 渡した SHA-256 がバイト列と一致するかは確かめないので、照合済みのエントリとして扱ってはならない。
    #[must_use]
    pub fn from_parts(key: CacheKey, bytes: Vec<u8>, sha256: [u8; 32]) -> Self {
        Self { key, bytes, sha256 }
    }

    /// キー。
    #[must_use]
    pub fn key(&self) -> &CacheKey {
        &self.key
    }

    /// 成果物のバイト列。
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// 保持している SHA-256。バイト列から計算した値とは限らない（[`CacheEntry::from_parts`]）。
    #[must_use]
    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    /// キー・バイト列・SHA-256 に分解する。
    #[must_use]
    pub fn into_parts(self) -> (CacheKey, Vec<u8>, [u8; 32]) {
        (self.key, self.bytes, self.sha256)
    }
}
