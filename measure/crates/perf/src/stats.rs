//! run ごとの統計。`sample` で集めた計測の区間の標本（ns）から、中央値・四分位・p95・最小・最大を出す。
//!
//! 純粋な関数だけで時計に触れない。結果は `across` の入力になる。

use crate::PerfError;

/// 統計を正確に扱える標本の上限（ns）。これを超える値は [`PerfError::ValueTooLarge`] で返す。
///
/// 偶数個の中央値に 0.5 の端数が出ても `f64` で正確に表せる範囲に収めるための値。
pub const MAX_EXACT_NS: u64 = 1 << 52;

/// 1 つの run の統計。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunStats {
    /// 中央値。個数が偶数なら中央の 2 つの平均（端数 0.5 があり得る）。
    pub median: f64,
    /// 下位四分位。添字 `round(0.25 × (n − 1))` の標本（補間なし）。
    pub q1: u64,
    /// 上位四分位。添字 `round(0.75 × (n − 1))` の標本（補間なし）。
    pub q3: u64,
    /// p95。添字 `round(0.95 × (n − 1))` の標本（補間なし）。
    pub p95: u64,
    /// 最小。
    pub min: u64,
    /// 最大。
    pub max: u64,
}

/// 割合 `p` に対し、昇順の標本から添字 `round(p × (n − 1))` の標本を補間なしで取る。
/// 丸めは `f64::round`（端数 0.5 は 0 から遠い側）。
fn pick(sorted: &[u64], p: f64) -> Result<u64, PerfError> {
    let last = sorted.len().checked_sub(1).ok_or(PerfError::Empty)?;
    let idx = (p * last as f64).round() as usize;
    sorted.get(idx.min(last)).copied().ok_or(PerfError::Empty)
}

/// 昇順の標本の中央値。偶数個は中央の 2 つの平均。
fn median_of_sorted(sorted: &[u64]) -> Result<f64, PerfError> {
    let n = sorted.len();
    let hi = *sorted.get(n / 2).ok_or(PerfError::Empty)?;
    if n % 2 == 1 {
        return Ok(hi as f64);
    }
    let lo = *sorted.get(n / 2 - 1).ok_or(PerfError::Empty)?;
    // 和は u128 で取るので溢れない。上限内なら半分と 0.5 の端数は f64 で正確に表せる。
    let sum = u128::from(lo) + u128::from(hi);
    Ok((sum / 2) as f64 + if sum % 2 == 1 { 0.5 } else { 0.0 })
}

/// 計測の区間の標本（ns）から run ごとの統計を出す。入力は書き換えない。
///
/// 空は [`PerfError::Empty`]、上限を超える値は [`PerfError::ValueTooLarge`]。
pub fn run_stats(samples: &[u64]) -> Result<RunStats, PerfError> {
    if samples.is_empty() {
        return Err(PerfError::Empty);
    }
    if samples.iter().any(|&v| v > MAX_EXACT_NS) {
        return Err(PerfError::ValueTooLarge);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    Ok(RunStats {
        median: median_of_sorted(&sorted)?,
        q1: pick(&sorted, 0.25)?,
        q3: pick(&sorted, 0.75)?,
        p95: pick(&sorted, 0.95)?,
        min: *sorted.first().ok_or(PerfError::Empty)?,
        max: *sorted.last().ok_or(PerfError::Empty)?,
    })
}
