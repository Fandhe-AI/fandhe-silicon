//! 時刻の分解能を測る測定の部品（TASK-2950 のポインタ）。
//!
//! 測定専用の別 workspace `measure/` に置く crate で、製品の crate からは使われない。
//! bin `clock-resolution`（`src/main.rs`）が [`parse_args`] と [`run`] を呼ぶ。
//! CPU の時計は `Instant::now()` を [`CPU_READINGS`] 回続けて読み、隣り合う値の差のうち 0 でない最小の値と、
//! 差の分布（差の値 → 回数）を出す。結果は 1 行の JSON にして標準出力へ書き、ファイルは作らない。
//! 差の計算・最小の値・分布・秒の区間から整数 ns への丸めは、device に触れない純粋な関数として置く。
//! 値を 0 で埋めず、0 でない差が無いとき・時刻が戻ったとき・有限でないときは [`ResolutionError`] を返す。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-2950（ポインタのみ）。

use measure_perf::json;
use measure_perf::record::{CommitSha, Id, RecordError, SessionRef, UtcTime};
use measure_perf::sample::duration_to_ns;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// 出力の `task_id` に載せるタスクの ID。
pub const TASK_ID: &str = "TASK-2950";

/// CPU の時計を続けて読む回数。
pub const CPU_READINGS: usize = 1_000_000;

/// 出力の `clock` に載せる、CPU の時計の名前。
pub const CLOCK_CPU: &str = "cpu-instant";

/// 分解能の計算と測定の失敗。文言は固定の英語で、入力の値を含まない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolutionError {
    /// 入力が空。
    Empty,
    /// 0 でない差が 1 つも無い。
    NoNonzeroDifference,
    /// 異なる値が 2 つ未満。
    NoDistinctPair,
    /// 時刻が戻った、有限でない、0 以下、または終わりが始まりより前。
    InvalidTime,
    /// 値が `u64` の ns に収まらない。
    Overflow,
    /// 引数の誤り。
    Usage(&'static str),
    /// 壁時計を読めない。
    Clock,
    /// 出力に書けない。
    Output,
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty input"),
            Self::NoNonzeroDifference => f.write_str("no nonzero difference"),
            Self::NoDistinctPair => f.write_str("fewer than two distinct values"),
            Self::InvalidTime => f.write_str("invalid time value"),
            Self::Overflow => f.write_str("value does not fit in u64 nanoseconds"),
            Self::Usage(m) => write!(f, "usage: {m}"),
            Self::Clock => f.write_str("cannot read the wall clock"),
            Self::Output => f.write_str("cannot write the output"),
        }
    }
}

impl std::error::Error for ResolutionError {}

/// 続けて読んだ時刻（先頭からの ns）の、隣り合う値の差の列を返す。
///
/// 長さ `n` の入力から長さ `n − 1` の列を返す。入力が 2 つ未満なら [`ResolutionError::Empty`]、
/// 値が前より小さい（時刻が戻った）なら [`ResolutionError::InvalidTime`]。
pub fn adjacent_differences(readings_ns: &[u64]) -> Result<Vec<u64>, ResolutionError> {
    if readings_ns.len() < 2 {
        return Err(ResolutionError::Empty);
    }
    readings_ns
        .windows(2)
        .map(|w| w[1].checked_sub(w[0]).ok_or(ResolutionError::InvalidTime))
        .collect()
}

/// 0 でない値のうち最小の値を返す。空なら [`ResolutionError::Empty`]、全部 0 なら
/// [`ResolutionError::NoNonzeroDifference`]。
pub fn min_nonzero(values: &[u64]) -> Result<u64, ResolutionError> {
    if values.is_empty() {
        return Err(ResolutionError::Empty);
    }
    values
        .iter()
        .copied()
        .filter(|v| *v != 0)
        .min()
        .ok_or(ResolutionError::NoNonzeroDifference)
}

/// 値 → 回数の表（値の昇順）を返す。
pub fn histogram(values: &[u64]) -> BTreeMap<u64, u64> {
    let mut out = BTreeMap::new();
    for v in values {
        *out.entry(*v).or_insert(0) += 1;
    }
    out
}

/// 異なる値を昇順に並べたときの、隣り合う値の差の列を返す（異なる値が 2 つ未満なら空）。
pub fn distinct_differences(values: &[u64]) -> Vec<u64> {
    let mut distinct: Vec<u64> = values.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    distinct.windows(2).map(|w| w[1] - w[0]).collect()
}

/// 異なる値の間の 0 でない最小の差を返す。空なら [`ResolutionError::Empty`]、
/// 異なる値が 2 つ未満なら [`ResolutionError::NoDistinctPair`]。
pub fn min_distinct_difference(values: &[u64]) -> Result<u64, ResolutionError> {
    if values.is_empty() {
        return Err(ResolutionError::Empty);
    }
    distinct_differences(values)
        .into_iter()
        .min()
        .ok_or(ResolutionError::NoDistinctPair)
}

/// 秒の区間 `[start, end]` を `round((end − start) × 1e9)`（0.5 は 0 から遠い側）で整数 ns にする。
///
/// どちらかが有限でない・0 以下、または `end < start` なら [`ResolutionError::InvalidTime`]。
/// 結果が `u64` に収まらなければ [`ResolutionError::Overflow`]。
pub fn seconds_span_to_ns(start: f64, end: f64) -> Result<u64, ResolutionError> {
    if !start.is_finite() || !end.is_finite() || start <= 0.0 || end <= 0.0 || end < start {
        return Err(ResolutionError::InvalidTime);
    }
    let ns = ((end - start) * 1e9).round();
    if !ns.is_finite() || ns < 0.0 || ns >= 18_446_744_073_709_551_616.0 {
        return Err(ResolutionError::Overflow);
    }
    // 上の範囲の検査で、値は 0 以上 2^64 未満の整数。
    Ok(ns as u64)
}

/// `Instant::now()` を `count` 回続けて読み、先頭からの ns の列を返す。
///
/// `count` が 2 未満なら [`ResolutionError::Empty`]。
pub fn read_cpu_clock(count: usize) -> Result<Vec<u64>, ResolutionError> {
    if count < 2 {
        return Err(ResolutionError::Empty);
    }
    let mut readings: Vec<Instant> = Vec::with_capacity(count);
    for _ in 0..count {
        readings.push(Instant::now());
    }
    let first = readings[0];
    readings
        .iter()
        .map(|t| {
            let d = t
                .checked_duration_since(first)
                .ok_or(ResolutionError::InvalidTime)?;
            duration_to_ns(d).map_err(|_| ResolutionError::Overflow)
        })
        .collect()
}

/// 分解能と差の分布。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// 読んだ回数。
    pub readings: u64,
    /// 0 でない最小の差（ns）。
    pub resolution_ns: u64,
    /// 差の値（ns）→ 回数。
    pub differences: BTreeMap<u64, u64>,
}

/// 続けて読んだ時刻（先頭からの ns）から、CPU の時計の分解能と差の分布を作る。
pub fn cpu_resolution(readings_ns: &[u64]) -> Result<Resolution, ResolutionError> {
    let diffs = adjacent_differences(readings_ns)?;
    Ok(Resolution {
        readings: u64::try_from(readings_ns.len()).map_err(|_| ResolutionError::Overflow)?,
        resolution_ns: min_nonzero(&diffs)?,
        differences: histogram(&diffs),
    })
}

/// bin の設定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// 出力の `run_id`。
    pub run_id: Id,
    /// 回への参照。無ければ回に属さない。
    pub session_ref: SessionRef,
    /// 流した本リポの commit（無ければ `null`）。
    pub repo_commit: Option<CommitSha>,
}

/// 引数（プログラム名を除く）を解析する。未知の引数・重複・値の欠け・不正な値は `Err`。必須は `--run-id`。
pub fn parse_args(args: &[String]) -> Result<Config, ResolutionError> {
    const KEYS: [&str; 3] = ["--run-id", "--session-ref", "--repo-commit"];
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut it = args.iter();
    while let Some(key) = it.next() {
        let key = KEYS
            .iter()
            .find(|k| **k == key.as_str())
            .ok_or(ResolutionError::Usage("unknown argument"))?;
        let value = it.next().ok_or(ResolutionError::Usage("missing value"))?;
        if values.insert(key, value).is_some() {
            return Err(ResolutionError::Usage("duplicate argument"));
        }
    }
    let bad = |_: RecordError| ResolutionError::Usage("invalid value");
    let run_id = values
        .get("--run-id")
        .ok_or(ResolutionError::Usage("--run-id is required"))?;
    Ok(Config {
        run_id: Id::parse(run_id).map_err(bad)?,
        session_ref: match values.get("--session-ref") {
            Some(v) => SessionRef::Session(Id::parse(v).map_err(bad)?),
            None => SessionRef::NoSession,
        },
        repo_commit: values
            .get("--repo-commit")
            .map(|v| CommitSha::parse(v))
            .transpose()
            .map_err(bad)?,
    })
}

/// 結果の 1 行の JSON（ASCII・欄の順は固定・末尾の改行なし）を作る。
pub fn summary_json(
    config: &Config,
    started: &UtcTime,
    ended: &UtcTime,
    clock: &str,
    r: &Resolution,
) -> String {
    let mut out = String::from("{\"task_id\":");
    json::push_string(&mut out, TASK_ID);
    let field = |out: &mut String, key: &str, value: &str| {
        out.push_str(&format!(",\"{key}\":"));
        json::push_string(out, value);
    };
    field(&mut out, "run_id", config.run_id.as_str());
    field(&mut out, "session_ref", config.session_ref.as_str());
    field(&mut out, "started_utc", started.as_str());
    field(&mut out, "ended_utc", ended.as_str());
    out.push_str(",\"repo_commit\":");
    match &config.repo_commit {
        Some(c) => json::push_string(&mut out, c.as_str()),
        None => out.push_str("null"),
    }
    field(&mut out, "clock", clock);
    out.push_str(&format!(
        ",\"readings\":{},\"resolution_ns\":{},\"differences\":[",
        r.readings, r.resolution_ns
    ));
    for (i, (value, count)) in r.differences.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("[{value},{count}]"));
    }
    out.push_str("]}");
    out
}

/// 現在の UTC 時刻。
fn now_utc() -> Result<UtcTime, ResolutionError> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ResolutionError::Clock)?
        .as_secs();
    UtcTime::from_unix_secs(secs).map_err(|_| ResolutionError::Clock)
}

/// CPU の時計を [`CPU_READINGS`] 回読み、結果の 1 行を `out` に書く。
pub fn run(config: &Config, out: &mut impl Write) -> Result<(), ResolutionError> {
    let started = now_utc()?;
    let readings = read_cpu_clock(CPU_READINGS)?;
    let ended = now_utc()?;
    let r = cpu_resolution(&readings)?;
    let line = summary_json(config, &started, &ended, CLOCK_CPU, &r);
    writeln!(out, "{line}").map_err(|_| ResolutionError::Output)
}
