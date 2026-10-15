//! 投入 1 回あたりの標本（CPU 時間・GPU 時間）の取り方と、区間ごとの集め方。
//!
//! CPU 時間は単調時計で測った整数の ns。GPU 時間は呼び手が背後実装の時刻から測った値を受け取る
//! だけで、取れないときは `None` のまま持つ（0 で埋めない）。本モジュールは device に触れない。
//! 集めた標本は `stats::run_stats` に渡される。

use crate::PerfError;
use std::fmt;
use std::time::{Duration, Instant};

/// 投入 1 回の標本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmissionSample {
    /// CPU 時間（ns）。
    pub cpu_ns: u64,
    /// GPU 時間（ns）。取得不能なら `None`。
    pub gpu_ns: Option<u64>,
}

/// 区間（cold・warmup・計測）ごとの標本の数の指定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleCounts {
    /// cold の区間の投入回数。
    pub cold: usize,
    /// warmup の区間の投入回数。
    pub warmup: usize,
    /// 計測の区間の投入回数（1 以上）。
    pub measured: usize,
}

/// 区間ごとに集めた標本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseSamples {
    /// cold の区間。
    pub cold: Vec<SubmissionSample>,
    /// warmup の区間。
    pub warmup: Vec<SubmissionSample>,
    /// 計測の区間。
    pub measured: Vec<SubmissionSample>,
}

impl PhaseSamples {
    /// 計測の区間の CPU 時間（ns）の列。
    pub fn measured_cpu_ns(&self) -> Vec<u64> {
        self.measured.iter().map(|s| s.cpu_ns).collect()
    }

    /// 計測の区間の GPU 時間（ns）の列。取得不能な標本が 1 つでもあれば `None`（欠けた分を詰めない）。
    pub fn measured_gpu_ns(&self) -> Option<Vec<u64>> {
        self.measured.iter().map(|s| s.gpu_ns).collect()
    }
}

/// 集める途中の失敗。呼び手の失敗は握りつぶさずそのまま返す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectError<E> {
    /// 本 crate 側の失敗（数の指定・時間の変換）。
    Perf(PerfError),
    /// 呼び手の閉包が返した失敗。
    Submit(E),
}

impl<E: fmt::Display> fmt::Display for CollectError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Perf(e) => write!(f, "perf error: {e}"),
            Self::Submit(e) => write!(f, "submission failed: {e}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for CollectError<E> {}

/// `Duration` を整数の ns（`u64`）にする。収まらなければ飽和させず [`PerfError::DurationOverflow`]。
pub fn duration_to_ns(d: Duration) -> Result<u64, PerfError> {
    u64::try_from(d.as_nanos()).map_err(|_| PerfError::DurationOverflow)
}

/// 投入 1 回を実行して標本にする。`submit` の実行時間を CPU 時間とし、
/// `submit` が返した GPU 時間（取得不能は `None`）をそのまま持つ。
pub fn sample_once<E>(
    submit: &mut impl FnMut() -> Result<Option<u64>, E>,
) -> Result<SubmissionSample, CollectError<E>> {
    let start = Instant::now();
    let gpu_ns = submit().map_err(CollectError::Submit)?;
    let cpu_ns = duration_to_ns(start.elapsed()).map_err(CollectError::Perf)?;
    Ok(SubmissionSample { cpu_ns, gpu_ns })
}

/// 指定した数だけ cold・warmup・計測の順に `submit` を呼び、区間ごとの標本を集める。
/// 計測の回数が 0 なら [`PerfError::InvalidCount`]。
pub fn collect<E>(
    counts: SampleCounts,
    mut submit: impl FnMut() -> Result<Option<u64>, E>,
) -> Result<PhaseSamples, CollectError<E>> {
    if counts.measured == 0 {
        return Err(CollectError::Perf(PerfError::InvalidCount));
    }
    let mut run_phase = |n: usize| -> Result<Vec<SubmissionSample>, CollectError<E>> {
        let mut out = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            out.push(sample_once(&mut submit)?);
        }
        Ok(out)
    };
    Ok(PhaseSamples {
        cold: run_phase(counts.cold)?,
        warmup: run_phase(counts.warmup)?,
        measured: run_phase(counts.measured)?,
    })
}
