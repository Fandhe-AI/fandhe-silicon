//! 成果物キャッシュの照合の部品の置き場（TASK-483・ART-52 のポインタ）。
//!
//! 現状はエントリの型 [`CacheEntry`]・読み込み時の照合（[`RejectReason`]・[`LoadOutcome`]・[`verify_entry`]）・
//! エントリを複数持つ最小のメモリ上の入れ物 [`CacheStore`] を持つ。台帳と保存先は持たない（後の段で扱う）。
//! `fandhe-silicon-exec` / `fandhe-silicon-contract` が成果物を保存・復元するときに、
//! キーとバイト列とそのバイト列の SHA-256 を組にして受け渡すための共通の型である。
//!
//! 読み込みのたびに、キーを比べ、次にバイト列から SHA-256 を計算し直して保持する値と比べる。
//! どちらかが合わなければ必ず拒否し、照合を飛ばして返す経路は持たない。
//!
//! SHA-256 は完全性の識別のためのもので、認証や改ざん耐性（秘密の比較・MAC）の用途は対象外。
//! 秘密を比べないので、ダイジェストの比較は定数時間でなくてよい。

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

/// 読み込みを拒否した理由。
///
/// 外の crate の `match` を壊さずに後から variant を足せるよう `#[non_exhaustive]` とする。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RejectReason {
    /// 求めたキーに合うエントリが無い。
    KeyMismatch,
    /// キーは合うが、バイト列から計算し直した SHA-256 が保持する値と違う。
    HashMismatch,
}

/// 読み込みの結果。
///
/// 外の crate の `match` を壊さずに後から variant を足せるよう `#[non_exhaustive]` とする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadOutcome<'a> {
    /// 照合を通ったエントリ（借用。バイト列は複製しない）。
    Hit(&'a CacheEntry),
    /// 照合で拒否した。
    Rejected(RejectReason),
}

/// エントリを照合する。キー、次に SHA-256 の順に確かめる。
///
/// キーが違うときはハッシュを計算せず [`RejectReason::KeyMismatch`] を返す
/// （バイト列が壊れていてもキーの不一致が先に返る）。
/// キーが合えばバイト列から SHA-256 を計算し直して保持する値と比べ、違えば [`RejectReason::HashMismatch`]。
/// 計算量はバイト列の長さに比例する。
///
/// # Errors
///
/// 上記の拒否の理由を `Err` で返す。
#[must_use = "照合の結果を確かめずにエントリを使ってはならない"]
pub fn verify_entry(entry: &CacheEntry, requested: &CacheKey) -> Result<(), RejectReason> {
    if entry.key() != requested {
        return Err(RejectReason::KeyMismatch);
    }
    if sha256_bytes(entry.bytes()) != *entry.sha256() {
        return Err(RejectReason::HashMismatch);
    }
    Ok(())
}

/// エントリを複数持つ最小のメモリ上の入れ物。
///
/// `Vec` を線形に探す（探索の計算量はエントリ数に比例）。上限や追い出しは持たない。
/// 台帳や拒否の数え上げは持たない。
#[derive(Debug, Clone, Default)]
pub struct CacheStore {
    entries: Vec<CacheEntry>,
}

impl CacheStore {
    /// 空の入れ物を作る。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// エントリを入れる。照合はしない（読み込みのときに照合する）。
    ///
    /// 同じキーのエントリがあれば置き換えて前のものを返し、無ければ末尾に足して `None` を返す。
    pub fn insert(&mut self, entry: CacheEntry) -> Option<CacheEntry> {
        match self.entries.iter_mut().find(|e| e.key() == entry.key()) {
            Some(slot) => Some(std::mem::replace(slot, entry)),
            None => {
                self.entries.push(entry);
                None
            }
        }
    }

    /// キーに合うエントリを探し、[`verify_entry`] で照合して返す。
    ///
    /// 入れ物の中身は変えない（拒否したエントリも残す。作り直したものは呼び出し側が [`CacheStore::insert`] で置き換える）。
    #[must_use]
    pub fn load(&self, requested: &CacheKey) -> LoadOutcome<'_> {
        match self.entries.iter().find(|e| e.key() == requested) {
            None => LoadOutcome::Rejected(RejectReason::KeyMismatch),
            Some(entry) => match verify_entry(entry, requested) {
                Ok(()) => LoadOutcome::Hit(entry),
                Err(reason) => LoadOutcome::Rejected(reason),
            },
        }
    }
}

/// 列挙型の網羅の試験（TASK-483 のポインタ）。
///
/// 同じ crate の中では `#[non_exhaustive]` が効かないので `_` の腕を持たない `match` が書ける。
/// variant を足すとこのコンパイルが落ち、試験の更新が要る。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_reason_has_two_variants() {
        for r in [RejectReason::KeyMismatch, RejectReason::HashMismatch] {
            match r {
                RejectReason::KeyMismatch | RejectReason::HashMismatch => {}
            }
        }
    }

    #[test]
    fn load_outcome_variants_are_covered() {
        let e = CacheEntry::new(
            CacheKey {
                source_hash: 0,
                compiler_id: "c",
                compiler_version: 0,
                target: "t",
                options_hash: 0,
                bounds_checked: false,
                specialization_hash: 0,
                driver_version: None,
            },
            vec![],
        );
        for o in [
            LoadOutcome::Hit(&e),
            LoadOutcome::Rejected(RejectReason::HashMismatch),
        ] {
            match o {
                LoadOutcome::Hit(_) | LoadOutcome::Rejected(_) => {}
            }
        }
    }
}
