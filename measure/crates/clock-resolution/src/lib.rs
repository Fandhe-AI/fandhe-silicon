//! 時刻の分解能を測る測定の部品（TASK-2950 のポインタ）。
//!
//! 測定専用の別 workspace `measure/` に置く crate で、製品の crate からは使われない。
//! bin `clock-resolution`（`src/main.rs`）が [`parse_args`] と [`run`] を呼ぶ。
//! CPU の時計は `Instant::now()` を [`CPU_READINGS`] 回続けて読み、隣り合う値の差のうち 0 でない最小の値と、
//! 差の分布（差の値 → 回数）を出す。結果は 1 行の JSON にして標準出力へ書き、ファイルは作らない。
//! 差の計算・最小の値・分布・秒の区間から整数 ns への丸めは、device に触れない純粋な関数として置く。
//! 値を 0 で埋めず、0 でない差が無いとき・時刻が戻ったとき・有限でないときは [`ResolutionError`] を返す。
//! 読んだ回数・値の数に比例する領域は `try_reserve_exact` で確保した列だけにし、確保できないときは panic せずに
//! [`ResolutionError::Allocation`] を返す。分布は確保した列を並べて連なりを数え、JSON は出力へ直に書く。
//! 時刻の順は隣り合う値どうしで比べ、壁時計は秒未満まで比べる。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-2950（ポインタのみ）。

use measure_perf::record::{CommitSha, Id, RecordError, SessionRef, UtcTime};
use measure_perf::sample::duration_to_ns;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    /// 列の領域を確保できない。
    Allocation,
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
            Self::Allocation => f.write_str("cannot allocate memory"),
        }
    }
}

impl std::error::Error for ResolutionError {}

impl ResolutionError {
    /// bin の終了コード。引数の誤り（[`ResolutionError::Usage`]）は 2、ほかの失敗は 1。
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 2,
            _ => 1,
        }
    }
}

/// 続けて読んだ時刻（先頭からの ns）の、隣り合う値の差の列を返す。
///
/// 長さ `n` の入力から長さ `n − 1` の列を返す。入力が 2 つ未満なら [`ResolutionError::Empty`]、
/// 値が前より小さい（時刻が戻った）なら [`ResolutionError::InvalidTime`]、
/// 列の領域を確保できなければ [`ResolutionError::Allocation`]。
pub fn adjacent_differences(readings_ns: &[u64]) -> Result<Vec<u64>, ResolutionError> {
    adjacent_differences_in(readings_ns, reserve)
}

/// [`adjacent_differences`] の本体。差の列の領域を `reserve` で確保し、失敗はそのまま返す。
fn adjacent_differences_in(
    readings_ns: &[u64],
    reserve: impl FnOnce(usize) -> Result<Vec<u64>, ResolutionError>,
) -> Result<Vec<u64>, ResolutionError> {
    if readings_ns.len() < 2 {
        return Err(ResolutionError::Empty);
    }
    let mut out = reserve(readings_ns.len() - 1)?;
    for w in readings_ns.windows(2) {
        out.push(gap(w[0], w[1])?);
    }
    Ok(out)
}

/// 前の値 `prev` から次の値 `next` までの差。`next < prev`（時刻が戻った）なら [`ResolutionError::InvalidTime`]。
fn gap(prev: u64, next: u64) -> Result<u64, ResolutionError> {
    next.checked_sub(prev).ok_or(ResolutionError::InvalidTime)
}

/// 長さ `len` の空の列を、確保の失敗を [`ResolutionError::Allocation`] にして作る。
fn reserve<T>(len: usize) -> Result<Vec<T>, ResolutionError> {
    let mut out = Vec::new();
    out.try_reserve_exact(len)
        .map_err(|_| ResolutionError::Allocation)?;
    Ok(out)
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

/// `values` をその場で昇順に並べ、値 → 回数（値の昇順）を [`runs`] で返す。新しい領域を確保しない。
pub fn histogram(values: &mut [u64]) -> Runs<'_> {
    values.sort_unstable();
    runs(values)
}

/// 昇順に並べた列の、同じ値の連なりを `(値, 回数)` で先頭から順に返す。新しい領域を確保しない。
pub fn runs(sorted: &[u64]) -> Runs<'_> {
    Runs { rest: sorted }
}

/// [`runs`] が返す反復子。
#[derive(Debug, Clone)]
pub struct Runs<'a> {
    rest: &'a [u64],
}

impl Iterator for Runs<'_> {
    type Item = (u64, usize);

    fn next(&mut self) -> Option<(u64, usize)> {
        let value = *self.rest.first()?;
        let count = self.rest.iter().take_while(|v| **v == value).count();
        self.rest = &self.rest[count..];
        Some((value, count))
    }
}

/// 異なる値を昇順に並べたときの、隣り合う値の差の列を返す（異なる値が 2 つ未満なら空）。
///
/// 値の写しと差の列の領域を確保できなければ [`ResolutionError::Allocation`]。
pub fn distinct_differences(values: &[u64]) -> Result<Vec<u64>, ResolutionError> {
    distinct_differences_in(values, reserve)
}

/// [`distinct_differences`] の本体。値の写しと差の列の 2 つの領域を `reserve` で確保し、失敗はそのまま返す。
fn distinct_differences_in(
    values: &[u64],
    mut reserve: impl FnMut(usize) -> Result<Vec<u64>, ResolutionError>,
) -> Result<Vec<u64>, ResolutionError> {
    let mut distinct = reserve(values.len())?;
    distinct.extend_from_slice(values);
    distinct.sort_unstable();
    distinct.dedup();
    let mut out = reserve(distinct.len().saturating_sub(1))?;
    for w in distinct.windows(2) {
        out.push(w[1] - w[0]);
    }
    Ok(out)
}

/// 異なる値の間の 0 でない最小の差を返す。空なら [`ResolutionError::Empty`]、
/// 異なる値が 2 つ未満なら [`ResolutionError::NoDistinctPair`]、
/// 列の領域を確保できなければ [`ResolutionError::Allocation`]。
pub fn min_distinct_difference(values: &[u64]) -> Result<u64, ResolutionError> {
    if values.is_empty() {
        return Err(ResolutionError::Empty);
    }
    distinct_differences(values)?
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

/// `Instant::now()` を `count` 回続けて読み、先頭からの ns の列を返す（[`elapsed_ns`] で換える）。
///
/// `count` が 2 未満なら [`ResolutionError::Empty`]、列の領域を確保できなければ [`ResolutionError::Allocation`]。
pub fn read_cpu_clock(count: usize) -> Result<Vec<u64>, ResolutionError> {
    if count < 2 {
        return Err(ResolutionError::Empty);
    }
    let mut readings: Vec<Instant> = reserve(count)?;
    for _ in 0..count {
        readings.push(Instant::now());
    }
    elapsed_ns(&readings)
}

/// 続けて読んだ時刻を、先頭からの ns の列にする。
///
/// 入力が 2 つ未満なら [`ResolutionError::Empty`]。どれかの時刻が直前の時刻より前なら
/// [`ResolutionError::InvalidTime`]、ns が `u64` に収まらなければ [`ResolutionError::Overflow`]、
/// 列の領域を確保できなければ [`ResolutionError::Allocation`]。
pub fn elapsed_ns(readings: &[Instant]) -> Result<Vec<u64>, ResolutionError> {
    if readings.len() < 2 {
        return Err(ResolutionError::Empty);
    }
    let first = readings[0];
    let mut out = reserve(readings.len())?;
    let mut prev = first;
    for t in readings {
        if t.checked_duration_since(prev).is_none() {
            return Err(ResolutionError::InvalidTime);
        }
        prev = *t;
        let d = t
            .checked_duration_since(first)
            .ok_or(ResolutionError::InvalidTime)?;
        out.push(duration_to_ns(d).map_err(|_| ResolutionError::Overflow)?);
    }
    Ok(out)
}

/// 分解能と差の分布。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// 読んだ回数。
    pub readings: u64,
    /// 0 でない最小の差（ns）。
    pub resolution_ns: u64,
    /// 差の値（ns）を昇順に並べた列。分布（差の値 → 回数）は [`runs`] で数える。
    pub differences: Vec<u64>,
}

/// 続けて読んだ時刻（先頭からの ns）から、CPU の時計の分解能と差の分布を作る。
///
/// 差の列を [`adjacent_differences`] で作り、その場で昇順に並べる。入力が 2 つ未満なら [`ResolutionError::Empty`]、
/// 値が前より小さいなら [`ResolutionError::InvalidTime`]、0 でない差が無ければ
/// [`ResolutionError::NoNonzeroDifference`]、列の領域を確保できなければ [`ResolutionError::Allocation`]。
pub fn cpu_resolution(readings_ns: &[u64]) -> Result<Resolution, ResolutionError> {
    let mut differences = adjacent_differences(readings_ns)?;
    differences.sort_unstable();
    let readings = u64::try_from(readings_ns.len()).map_err(|_| ResolutionError::Overflow)?;
    let resolution_ns = differences
        .iter()
        .copied()
        .find(|d| *d != 0)
        .ok_or(ResolutionError::NoNonzeroDifference)?;
    Ok(Resolution {
        readings,
        resolution_ns,
        differences,
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

/// OS から受けた引数（プログラム名を除く）を文字列にする。UTF-8 でない引数は
/// [`ResolutionError::Usage`]（`invalid value`）、列の領域を確保できなければ [`ResolutionError::Allocation`]。
pub fn args_from_os<I>(args: I) -> Result<Vec<String>, ResolutionError>
where
    I: IntoIterator<Item = OsString>,
    I::IntoIter: ExactSizeIterator,
{
    let args = args.into_iter();
    let mut out = reserve(args.len())?;
    for a in args {
        out.push(
            a.into_string()
                .map_err(|_| ResolutionError::Usage("invalid value"))?,
        );
    }
    Ok(out)
}

/// 引数の列を、`keys` の並びの値の配列にする。未知の引数・値の欠け・重複は `Err`（[`ResolutionError::Usage`] の文言）。
fn parse_keys<'a, const N: usize>(
    args: &'a [String],
    keys: &[&str; N],
) -> Result<[Option<&'a str>; N], &'static str> {
    let mut values = [None; N];
    let mut it = args.iter();
    while let Some(key) = it.next() {
        let slot = keys
            .iter()
            .position(|k| *k == key.as_str())
            .and_then(|i| values.get_mut(i))
            .ok_or("unknown argument")?;
        let value = it.next().ok_or("missing value")?;
        if slot.replace(value.as_str()).is_some() {
            return Err("duplicate argument");
        }
    }
    Ok(values)
}

/// `--run-id`・`--session-ref`・`--repo-commit` の値から設定を作る。`--run-id` が無い・形に合わない値は `Err`。
fn parse_base(
    run_id: Option<&str>,
    session_ref: Option<&str>,
    repo_commit: Option<&str>,
) -> Result<Config, ResolutionError> {
    let bad = |_: RecordError| ResolutionError::Usage("invalid value");
    let run_id = run_id.ok_or(ResolutionError::Usage("--run-id is required"))?;
    Ok(Config {
        run_id: Id::parse(run_id).map_err(bad)?,
        session_ref: match session_ref {
            Some(v) => SessionRef::Session(Id::parse(v).map_err(bad)?),
            None => SessionRef::NoSession,
        },
        repo_commit: repo_commit.map(CommitSha::parse).transpose().map_err(bad)?,
    })
}

/// 引数（プログラム名を除く）を解析する。未知の引数・重複・値の欠け・不正な値は `Err`。必須は `--run-id`。
pub fn parse_args(args: &[String]) -> Result<Config, ResolutionError> {
    const KEYS: [&str; 3] = ["--run-id", "--session-ref", "--repo-commit"];
    let [run_id, session_ref, repo_commit] =
        parse_keys(args, &KEYS).map_err(ResolutionError::Usage)?;
    parse_base(run_id, session_ref, repo_commit)
}

/// 結果の 1 行の JSON（ASCII・欄の順は固定・末尾の改行なし）を `out` に直に書く。文字列を組み立てない。
///
/// 書けなければ、その誤りをそのまま返す。
pub fn write_summary_json(
    out: &mut impl Write,
    config: &Config,
    started: &UtcTime,
    ended: &UtcTime,
    clock: &str,
    r: &Resolution,
) -> io::Result<()> {
    out.write_all(b"{\"task_id\":")?;
    write_json_string(out, TASK_ID)?;
    for (key, value) in [
        ("run_id", config.run_id.as_str()),
        ("session_ref", config.session_ref.as_str()),
        ("started_utc", started.as_str()),
        ("ended_utc", ended.as_str()),
    ] {
        write!(out, ",\"{key}\":")?;
        write_json_string(out, value)?;
    }
    out.write_all(b",\"repo_commit\":")?;
    match &config.repo_commit {
        Some(c) => write_json_string(out, c.as_str())?,
        None => out.write_all(b"null")?,
    }
    out.write_all(b",\"clock\":")?;
    write_json_string(out, clock)?;
    write!(
        out,
        ",\"readings\":{},\"resolution_ns\":{},\"differences\":[",
        r.readings, r.resolution_ns
    )?;
    for (i, (value, count)) in runs(&r.differences).enumerate() {
        if i > 0 {
            out.write_all(b",")?;
        }
        write!(out, "[{value},{count}]")?;
    }
    out.write_all(b"]}")
}

/// JSON の文字列リテラル（引用符つき）を `out` に直に書く。`measure_perf::json::push_string` と同じバイトを出す。
///
/// `"`・`\`・制御文字・非 ASCII をエスケープし、BMP の外の文字はサロゲート対で出す。
fn write_json_string(out: &mut impl Write, s: &str) -> io::Result<()> {
    out.write_all(b"\"")?;
    for c in s.chars() {
        match c {
            '"' => out.write_all(b"\\\"")?,
            '\\' => out.write_all(b"\\\\")?,
            '\n' => out.write_all(b"\\n")?,
            '\r' => out.write_all(b"\\r")?,
            '\t' => out.write_all(b"\\t")?,
            c if c.is_ascii() && !c.is_ascii_control() => {
                let mut buf = [0u8; 4];
                out.write_all(c.encode_utf8(&mut buf).as_bytes())?;
            }
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    write!(out, "\\u{unit:04x}")?;
                }
            }
        }
    }
    out.write_all(b"\"")
}

/// 壁時計の今の時刻（UNIX 時刻からの経過。秒未満を含む）。
fn wall_now() -> Result<Duration, ResolutionError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ResolutionError::Clock)
}

/// UNIX 時刻からの経過を、秒未満を切り捨てた UTC 時刻にする。
fn utc_secs(since_epoch: Duration) -> Result<UtcTime, ResolutionError> {
    UtcTime::from_unix_secs(since_epoch.as_secs()).map_err(|_| ResolutionError::Clock)
}

/// 壁時計の終わりが始まりより前（壁時計が戻った）なら [`ResolutionError::InvalidTime`]。秒未満まで比べる。
pub fn check_time_order(started: Duration, ended: Duration) -> Result<(), ResolutionError> {
    if ended < started {
        return Err(ResolutionError::InvalidTime);
    }
    Ok(())
}

/// 壁時計の始まりと終わり（UNIX 時刻からの経過）と、CPU の時計を続けて読んだ列から、結果の 1 行を
/// `out` に書いて flush する。
///
/// 壁時計が戻ったとき・列から値を作れないときは何も書かずに `Err`。書けない・flush できないときは
/// [`ResolutionError::Output`]。
pub fn write_summary(
    config: &Config,
    started: Duration,
    ended: Duration,
    readings_ns: &[u64],
    out: &mut impl Write,
) -> Result<(), ResolutionError> {
    check_time_order(started, ended)?;
    let r = cpu_resolution(readings_ns)?;
    let (started, ended) = (utc_secs(started)?, utc_secs(ended)?);
    write_summary_json(out, config, &started, &ended, CLOCK_CPU, &r)
        .and_then(|()| out.write_all(b"\n"))
        .map_err(|_| ResolutionError::Output)?;
    out.flush().map_err(|_| ResolutionError::Output)
}

/// CPU の時計を [`CPU_READINGS`] 回読み、[`write_summary`] で結果の 1 行を `out` に書く。
pub fn run(config: &Config, out: &mut impl Write) -> Result<(), ResolutionError> {
    let started = wall_now()?;
    let readings = read_cpu_clock(CPU_READINGS)?;
    let ended = wall_now()?;
    write_summary(config, started, ended, &readings, out)
}

/// 失敗の文言を `err` に `error: <文言>` の 1 行で書いて flush し、`code` を返す。
///
/// 書けない・flush できないときも `code` を返す（診断の出力の失敗で終了コードを変えない）。
pub fn report_error(err: &mut impl Write, error: &impl fmt::Display, code: u8) -> u8 {
    match writeln!(err, "error: {error}").and_then(|()| err.flush()) {
        Ok(()) | Err(_) => code,
    }
}

/// bin `clock-resolution` の本体。引数（プログラム名を除く）を解析して [`run`] で `out` に書き、終了コードを返す。
///
/// 成功は 0。失敗は [`report_error`] で `err` に 1 行を書き、[`ResolutionError::exit_code`]
/// （引数の誤りは 2、ほかは 1）を返す。
pub fn run_main<I>(args: I, out: &mut impl Write, err: &mut impl Write) -> u8
where
    I: IntoIterator<Item = OsString>,
    I::IntoIter: ExactSizeIterator,
{
    match args_from_os(args)
        .and_then(|args| parse_args(&args))
        .and_then(|config| run(&config, out))
    {
        Ok(()) => 0,
        Err(e) => report_error(err, &e, e.exit_code()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ResolutionError, adjacent_differences_in, distinct_differences_in, reserve,
        write_json_string,
    };

    #[test]
    fn adjacent_differences_return_a_failure_to_allocate_the_differences() {
        let mut lengths = Vec::new();
        let got = adjacent_differences_in(&[0, 41, 83], |len| {
            lengths.push(len);
            Err(ResolutionError::Allocation)
        });
        assert_eq!(got, Err(ResolutionError::Allocation));
        assert_eq!(lengths, [2]);
    }

    #[test]
    fn json_strings_are_written_with_the_same_bytes_as_push_string() {
        for s in [
            "cpu-instant",
            "",
            "q\"b\\s\n\r\t\u{1}\u{7f}",
            "\u{e9}\u{1f600}",
        ] {
            let mut expected = String::new();
            measure_perf::json::push_string(&mut expected, s);
            let mut got = Vec::new();
            write_json_string(&mut got, s).unwrap();
            assert_eq!(got, expected.as_bytes());
        }
    }

    #[test]
    fn distinct_differences_return_a_failure_to_allocate_the_copy() {
        let mut lengths = Vec::new();
        let got = distinct_differences_in(&[125, 42, 125, 0, 83], |len| {
            lengths.push(len);
            Err(ResolutionError::Allocation)
        });
        assert_eq!(got, Err(ResolutionError::Allocation));
        assert_eq!(lengths, [5]);
    }

    #[test]
    fn distinct_differences_return_a_failure_to_allocate_the_differences() {
        let mut lengths = Vec::new();
        let got = distinct_differences_in(&[125, 42, 125, 0, 83], |len| {
            lengths.push(len);
            if lengths.len() == 1 {
                reserve(len)
            } else {
                Err(ResolutionError::Allocation)
            }
        });
        assert_eq!(got, Err(ResolutionError::Allocation));
        assert_eq!(lengths, [5, 3]);
    }
}
