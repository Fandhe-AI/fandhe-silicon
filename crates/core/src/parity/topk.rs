//! Top-k の CPU 参照と、T-2 の比べ方（spec TASK-24 の 5/8）。
//!
//! 検証用のテスト・測定から呼ばれ、背後実装が返した Top-k を CPU の参照とビット同一で比べる。
//! feature `test-support` の裏にだけ公開される（置き場は未承認の仮置き。`docs/parity-tool.md` の Q1・Q3）。
//! T-1 の許容誤差（[`super::tolerance`]）は通さない。
//! spec との対応: `docs/spec/02-poc-plan.md` の T-2、PARITY-6（ポインタのみ）。

use std::collections::TryReserveError;
use std::fmt;

use super::tolerance::CompareError;

/// Top-k の 1 件（スロット番号とスコア）。
///
/// `PartialEq` を実装しない。`f32` の `==` は `-0.0` と `+0.0` を同じとみなすので、
/// 比べは [`compare_topk`] のビット比較に限る。
#[derive(Debug, Clone, Copy)]
pub struct TopKEntry {
    slot: u32,
    score: f32,
}

impl TopKEntry {
    /// スロット番号とスコアから 1 件を作る。
    pub fn new(slot: u32, score: f32) -> Self {
        Self { slot, score }
    }

    /// スロット番号（参照では入力の添字）。
    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// スコア。
    pub fn score(&self) -> f32 {
        self.score
    }

    /// スコアのビット列。
    pub fn score_bits(&self) -> u32 {
        self.score.to_bits()
    }
}

/// Top-k の参照を作れなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopKReferenceError {
    /// 入力の添字が `u32` に収まらなかった。
    SlotOverflow,
    /// 結果の領域を確保できなかった。
    AllocationFailed,
}

impl fmt::Display for TopKReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlotOverflow => write!(f, "slot index does not fit in u32"),
            Self::AllocationFailed => write!(f, "allocation failed"),
        }
    }
}

impl std::error::Error for TopKReferenceError {}

impl From<TryReserveError> for TopKReferenceError {
    fn from(_: TryReserveError) -> Self {
        Self::AllocationFailed
    }
}

/// スコア列から上位 `k` 件を選ぶ CPU 参照。
///
/// 有限でないスコア（NaN・±Inf）は候補から外す。残りをスコアの降順（`f32::total_cmp` の全順序）、
/// 同点はスロット番号（入力の添字）の昇順に並べ、先頭 `min(k, 有限の件数)` 件を返す。
/// `k == 0` や空の列は空の結果を `Ok` で返す。
pub fn reference_topk_f32(scores: &[f32], k: usize) -> Result<Vec<TopKEntry>, TopKReferenceError> {
    let finite = scores.iter().filter(|s| s.is_finite()).count();
    let mut entries: Vec<TopKEntry> = Vec::new();
    entries.try_reserve_exact(finite)?;
    for (i, &score) in scores.iter().enumerate() {
        if score.is_finite() {
            let slot = u32::try_from(i).map_err(|_| TopKReferenceError::SlotOverflow)?;
            entries.push(TopKEntry { slot, score });
        }
    }
    // スロット番号が一意なので全順序になり、安定ソートと結果が同じ。
    entries.sort_unstable_by(|a, b| b.score.total_cmp(&a.score).then(a.slot.cmp(&b.slot)));
    entries.truncate(k);
    Ok(entries)
}

/// 二つの Top-k の比べの集計。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopKComparison {
    compared: usize,
    mismatched: usize,
    first_mismatch: Option<usize>,
}

impl TopKComparison {
    /// 比べた件数。
    pub fn compared(&self) -> usize {
        self.compared
    }

    /// 一致しなかった件数。
    pub fn mismatched(&self) -> usize {
        self.mismatched
    }

    /// 最初に一致しなかった位置。
    pub fn first_mismatch(&self) -> Option<usize> {
        self.first_mismatch
    }

    /// 1 件も外れなければ真。
    pub fn is_match(&self) -> bool {
        self.mismatched == 0
    }
}

/// 参照と実測の Top-k を位置ごとにビット同一で比べる（T-2）。
///
/// 位置ごとにスロット番号が等しく、かつスコアのビット列が等しいときだけ一致。
/// 長さ違いは [`CompareError::LengthMismatch`]、長さ 0 は [`CompareError::EmptyInput`]。
/// 検査が済むまで添字アクセスをしない。確保はしない。
pub fn compare_topk(
    reference: &[TopKEntry],
    actual: &[TopKEntry],
) -> Result<TopKComparison, CompareError> {
    if reference.len() != actual.len() {
        return Err(CompareError::LengthMismatch {
            reference: reference.len(),
            actual: actual.len(),
        });
    }
    if reference.is_empty() {
        return Err(CompareError::EmptyInput);
    }
    let mut result = TopKComparison {
        compared: reference.len(),
        mismatched: 0,
        first_mismatch: None,
    };
    for (i, (r, a)) in reference.iter().zip(actual).enumerate() {
        if r.slot != a.slot || r.score_bits() != a.score_bits() {
            result.mismatched += 1;
            result.first_mismatch.get_or_insert(i);
        }
    }
    Ok(result)
}
