//! run 間の集計。各 run の中央値の列から、run 間の中央値と変動帯の 2 つの値を出す。
//!
//! どちらの変動帯を判定に使うかは本モジュールでは決めない。純粋な関数だけで時計に触れない。

use crate::PerfError;
use crate::stats::RunStats;

/// run 間の集計結果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcrossRuns {
    /// 各 run の中央値の中央値。偶数個なら中央の 2 つの平均。
    pub median: f64,
    /// 変動帯その 1: 最大 − 最小。
    pub spread_abs: f64,
    /// 変動帯その 2: 最大 ÷ 最小。最小が 0 のときは計算できないので `None`（無限大や NaN は返さない）。
    pub spread_ratio: Option<f64>,
}

/// run ごとの中央値の列から集計する。空は [`PerfError::Empty`]、
/// 有限でない値・負の値は [`PerfError::InvalidValue`]。
pub fn across_runs(run_medians: &[f64]) -> Result<AcrossRuns, PerfError> {
    if run_medians.is_empty() {
        return Err(PerfError::Empty);
    }
    if run_medians.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(PerfError::InvalidValue);
    }
    let mut sorted = run_medians.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let hi = *sorted.get(n / 2).ok_or(PerfError::Empty)?;
    let median = if n % 2 == 1 {
        hi
    } else {
        let lo = *sorted.get(n / 2 - 1).ok_or(PerfError::Empty)?;
        lo / 2.0 + hi / 2.0
    };
    let min = *sorted.first().ok_or(PerfError::Empty)?;
    let max = *sorted.last().ok_or(PerfError::Empty)?;
    let spread_ratio = if min == 0.0 { None } else { Some(max / min) };
    Ok(AcrossRuns {
        median,
        spread_abs: max - min,
        spread_ratio: spread_ratio.filter(|r| r.is_finite()),
    })
}

/// 各 run の統計から集計する（各 run の中央値を取り出して [`across_runs`] に渡す）。
pub fn across_run_stats(runs: &[RunStats]) -> Result<AcrossRuns, PerfError> {
    let medians: Vec<f64> = runs.iter().map(|r| r.median).collect();
    across_runs(&medians)
}
