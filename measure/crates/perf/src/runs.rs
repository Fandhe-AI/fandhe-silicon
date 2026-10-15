//! run ごとの記録（読み込み済みの JSON）から run 間の集計を出し、決定的な 1 行の JSON にする。
//!
//! コマンド `across-runs` から呼ばれる。記録は信頼しない入力として扱い、読む欄を白表で検証し、
//! 標本から中央値を再計算して記録の統計と突き合わせる。条件の欄が違う run の混在、`run_id` の重複、
//! 測っていないラベル（`not-run`・`indeterminate`）の run は集計に入れず失敗にする。
//! 集計は 1/4 単位の整数で行い（`f64` を経由しない）、変動帯は「最大 − 最小」と「最大 ÷ 最小」の両方を出す。
//! どちらで判定するかは本モジュールでは決めない。純粋な関数だけで、時計・ファイル・環境に触れない。
//! 判定に使える run かどうかの除外は `scripts/perf-record-check.sh --session` が先に行う前提で、
//! 本モジュールは渡された run を集計するだけである。

use crate::json;
use crate::jsonread::Value;
use crate::record::BODY_KIND_PERF_SAMPLES;
use crate::record::{Id, Machine, MachineCombination, NO_SESSION, RefId, SCHEMA_VERSION};
use crate::stats::MAX_EXACT_NS;
use std::fmt;

/// 集計の失敗。入力の値は埋め込まず、欄の名前（本 crate が持つ固定の綴り）だけを持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AcrossError {
    /// run が 1 件も無い。
    Empty,
    /// 欄が無い・型が違う・値が白表や上限に合わない。
    Field(&'static str),
    /// 標本から再計算した値が記録の統計と合わない。
    Inconsistent(&'static str),
    /// run の間で同じであるべき欄が違う。
    Heterogeneous(&'static str),
    /// `run_id` が重複している。
    DuplicateRunId,
    /// 測っていないラベルの run が混じっている。
    NotMeasured,
    /// 集計の計算が `u64` に収まらない。
    Overflow,
}

impl fmt::Display for AcrossError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("no runs"),
            Self::Field(n) => write!(f, "invalid field: {n}"),
            Self::Inconsistent(n) => write!(f, "inconsistent stats: {n}"),
            Self::Heterogeneous(n) => write!(f, "heterogeneous runs: {n}"),
            Self::DuplicateRunId => f.write_str("duplicate run_id"),
            Self::NotMeasured => f.write_str("run was not measured"),
            Self::Overflow => f.write_str("arithmetic overflow"),
        }
    }
}

impl std::error::Error for AcrossError {}

/// 1 件の記録から集計に要る欄だけを検証して取り出したもの。
#[derive(Debug, Clone, PartialEq)]
pub struct RunInput {
    run_id: String,
    session_ref: String,
    machine_combination: &'static str,
    machine: &'static str,
    task_ids: Vec<String>,
    behavior_ids: Vec<String>,
    sample_counts: Value,
    wgpu_baseline: Option<Value>,
    /// 計測の区間の CPU 時間の中央値（1/4 単位）。
    cpu_median: u64,
    /// 計測の区間の GPU 時間の中央値（1/4 単位）。取得不能の標本が含まれていれば `None`。
    gpu_median: Option<u64>,
}

impl RunInput {
    /// run の ID。
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
}

fn field<'a>(v: &'a Value, key: &'static str) -> Result<&'a Value, AcrossError> {
    v.get(key).ok_or(AcrossError::Field(key))
}

fn text<'a>(v: &'a Value, key: &'static str) -> Result<&'a str, AcrossError> {
    field(v, key)?.as_str().ok_or(AcrossError::Field(key))
}

fn ref_ids(v: &Value, key: &'static str) -> Result<Vec<String>, AcrossError> {
    let arr = field(v, key)?.as_array().ok_or(AcrossError::Field(key))?;
    arr.iter()
        .map(|e| {
            let s = e.as_str().ok_or(AcrossError::Field(key))?;
            RefId::parse(s).map_err(|_| AcrossError::Field(key))?;
            Ok(s.to_owned())
        })
        .collect()
}

/// 昇順の標本の中央値を 1/4 単位で出す（偶数個は中央の 2 つの平均）。
fn median_quarters(sorted: &[u64]) -> Result<u64, AcrossError> {
    let n = sorted.len();
    let hi = *sorted.get(n / 2).ok_or(AcrossError::Empty)?;
    if n % 2 == 1 {
        return hi.checked_mul(4).ok_or(AcrossError::Overflow);
    }
    let lo = *sorted.get(n / 2 - 1).ok_or(AcrossError::Empty)?;
    lo.checked_add(hi)
        .and_then(|s| s.checked_mul(2))
        .ok_or(AcrossError::Overflow)
}

/// 標本の配列（`null` を許すとき `allow_null`）を数に直す。`null` があれば `None` の要素を返す。
fn samples(v: &Value, key: &'static str) -> Result<Vec<Option<u64>>, AcrossError> {
    let arr = v.as_array().ok_or(AcrossError::Field(key))?;
    arr.iter()
        .map(|e| match e {
            Value::Null => Ok(None),
            e => {
                let n = e.as_u64().ok_or(AcrossError::Field(key))?;
                if n > MAX_EXACT_NS {
                    return Err(AcrossError::Field(key));
                }
                Ok(Some(n))
            }
        })
        .collect()
}

/// 条件値の比較用に、object のキーを再帰的に並べ替えた写しを返す（キーの出現順だけの違いを同一条件として扱う）。
fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(kv) => {
            let mut out: Vec<(String, Value)> =
                kv.iter().map(|(k, x)| (k.clone(), canonical(x))).collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// 読み込んだ 1 件の記録を検証して、集計に要る欄を取り出す。
pub fn run_input(rec: &Value) -> Result<RunInput, AcrossError> {
    if text(rec, "schema_version")? != SCHEMA_VERSION {
        return Err(AcrossError::Field("schema_version"));
    }
    if text(rec, "body_kind")? != BODY_KIND_PERF_SAMPLES {
        return Err(AcrossError::Field("body_kind"));
    }
    let run_id = text(rec, "run_id")?;
    Id::parse(run_id).map_err(|_| AcrossError::Field("run_id"))?;
    let session_ref = text(rec, "session_ref")?;
    if session_ref != NO_SESSION {
        Id::parse(session_ref).map_err(|_| AcrossError::Field("session_ref"))?;
    }
    let mc = text(rec, "machine_combination")?;
    let machine_combination = MachineCombination::ALL
        .iter()
        .map(|m| m.name())
        .find(|n| *n == mc)
        .ok_or(AcrossError::Field("machine_combination"))?;
    let mach = text(rec, "machine")?;
    let machine = Machine::ALL
        .iter()
        .map(|m| m.name())
        .find(|n| *n == mach)
        .ok_or(AcrossError::Field("machine"))?;
    match text(rec, "label")? {
        "ok" | "ng" | "pending" => {}
        "not-run" | "indeterminate" => return Err(AcrossError::NotMeasured),
        _ => return Err(AcrossError::Field("label")),
    }
    let task_ids = ref_ids(rec, "task_ids")?;
    let behavior_ids = ref_ids(rec, "behavior_ids")?;
    let wgpu_baseline = rec.get("wgpu_baseline").map(canonical);

    let body = field(rec, "body")?;
    let sample_counts = canonical(field(body, "sample_counts")?);
    let cpu = samples(field(field(body, "cpu_ns")?, "measured")?, "cpu_ns")?;
    let cpu: Vec<u64> = cpu
        .into_iter()
        .map(|o| o.ok_or(AcrossError::Field("cpu_ns")))
        .collect::<Result<_, _>>()?;
    if sample_counts.get("measured").and_then(Value::as_u64) != Some(cpu.len() as u64)
        || cpu.is_empty()
    {
        return Err(AcrossError::Field("sample_counts"));
    }
    let gpu = samples(field(field(body, "gpu_ns")?, "measured")?, "gpu_ns")?;
    if gpu.len() != cpu.len() {
        return Err(AcrossError::Field("gpu_ns"));
    }

    let stats = field(body, "stats")?;
    let mut sorted = cpu;
    sorted.sort_unstable();
    let cpu_median = median_quarters(&sorted)?;
    let recorded = field(field(stats, "cpu_ns")?, "median")?
        .as_quarter_units()
        .ok_or(AcrossError::Field("stats"))?;
    if recorded != cpu_median {
        return Err(AcrossError::Inconsistent("cpu_ns"));
    }

    let gpu_stats = field(stats, "gpu_ns")?;
    let gpu_median = match gpu.into_iter().collect::<Option<Vec<u64>>>() {
        None => {
            if *gpu_stats != Value::Null {
                return Err(AcrossError::Inconsistent("gpu_ns"));
            }
            None
        }
        Some(mut g) => {
            g.sort_unstable();
            let m = median_quarters(&g)?;
            let recorded = field(gpu_stats, "median")
                .map_err(|_| AcrossError::Inconsistent("gpu_ns"))?
                .as_quarter_units()
                .ok_or(AcrossError::Field("stats"))?;
            if recorded != m {
                return Err(AcrossError::Inconsistent("gpu_ns"));
            }
            Some(m)
        }
    };

    Ok(RunInput {
        run_id: run_id.to_owned(),
        session_ref: session_ref.to_owned(),
        machine_combination,
        machine,
        task_ids,
        behavior_ids,
        sample_counts,
        wgpu_baseline,
        cpu_median,
        gpu_median,
    })
}

/// run 間の中央値と変動帯（時間の値は 1/4 単位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// 各 run の中央値の中央値。
    pub median: u64,
    /// 各 run の中央値の最小。
    pub min: u64,
    /// 各 run の中央値の最大。
    pub max: u64,
    /// 変動帯その 1: 最大 − 最小。
    pub spread_abs: u64,
    /// 変動帯その 2: 最大 ÷ 最小を千分の一単位（切り捨て）にしたもの。最小が 0 のときは `None`。
    pub spread_ratio_milli: Option<u64>,
}

fn band(medians: &[u64]) -> Result<Band, AcrossError> {
    let mut sorted = medians.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let hi = *sorted.get(n / 2).ok_or(AcrossError::Empty)?;
    let median = if n % 2 == 1 {
        hi
    } else {
        let lo = *sorted.get(n / 2 - 1).ok_or(AcrossError::Empty)?;
        let sum = lo.checked_add(hi).ok_or(AcrossError::Overflow)?;
        if sum % 2 != 0 {
            return Err(AcrossError::Inconsistent("median"));
        }
        sum / 2
    };
    let min = *sorted.first().ok_or(AcrossError::Empty)?;
    let max = *sorted.last().ok_or(AcrossError::Empty)?;
    let scaled = max.checked_mul(1000).ok_or(AcrossError::Overflow)?;
    let spread_ratio_milli = scaled.checked_div(min);
    Ok(Band {
        median,
        min,
        max,
        spread_abs: max - min,
        spread_ratio_milli,
    })
}

/// run 間の集計の出力。
#[derive(Debug, Clone, PartialEq)]
pub struct AcrossOutput {
    run_ids: Vec<String>,
    session_ref: String,
    machine_combination: &'static str,
    machine: &'static str,
    task_ids: Vec<String>,
    behavior_ids: Vec<String>,
    /// CPU 時間の集計。
    pub cpu: Band,
    /// GPU 時間の集計。1 件でも取得不能の run があれば `None`（その run を落として集計しない）。
    pub gpu: Option<Band>,
}

impl AcrossOutput {
    /// 集計に入れた run の ID（昇順）。
    pub fn run_ids(&self) -> &[String] {
        &self.run_ids
    }
}

/// 検証済みの run を集計する。0 件・`run_id` の重複・条件の欄の違いは失敗。
pub fn aggregate(inputs: &[RunInput]) -> Result<AcrossOutput, AcrossError> {
    let first = inputs.first().ok_or(AcrossError::Empty)?;
    let mut ids: Vec<String> = inputs.iter().map(|r| r.run_id.clone()).collect();
    ids.sort_unstable();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(AcrossError::DuplicateRunId);
    }
    for r in inputs {
        if r.machine_combination != first.machine_combination {
            return Err(AcrossError::Heterogeneous("machine_combination"));
        }
        if r.machine != first.machine {
            return Err(AcrossError::Heterogeneous("machine"));
        }
        if r.session_ref != first.session_ref {
            return Err(AcrossError::Heterogeneous("session_ref"));
        }
        if r.task_ids != first.task_ids {
            return Err(AcrossError::Heterogeneous("task_ids"));
        }
        if r.behavior_ids != first.behavior_ids {
            return Err(AcrossError::Heterogeneous("behavior_ids"));
        }
        if r.sample_counts != first.sample_counts {
            return Err(AcrossError::Heterogeneous("sample_counts"));
        }
        if r.wgpu_baseline != first.wgpu_baseline {
            return Err(AcrossError::Heterogeneous("wgpu_baseline"));
        }
    }
    let cpu: Vec<u64> = inputs.iter().map(|r| r.cpu_median).collect();
    let gpu: Option<Vec<u64>> = inputs.iter().map(|r| r.gpu_median).collect();
    Ok(AcrossOutput {
        run_ids: ids,
        session_ref: first.session_ref.clone(),
        machine_combination: first.machine_combination,
        machine: first.machine,
        task_ids: first.task_ids.clone(),
        behavior_ids: first.behavior_ids.clone(),
        cpu: band(&cpu)?,
        gpu: gpu.map(|g| band(&g)).transpose()?,
    })
}

fn push_band(out: &mut String, b: &Band) {
    out.push_str("{\"median\":");
    json::push_quarter(out, b.median);
    out.push_str(",\"min\":");
    json::push_quarter(out, b.min);
    out.push_str(",\"max\":");
    json::push_quarter(out, b.max);
    out.push_str(",\"spread_abs\":");
    json::push_quarter(out, b.spread_abs);
    out.push_str(",\"spread_ratio\":");
    match b.spread_ratio_milli {
        Some(m) => json::push_milli(out, m),
        None => out.push_str("null"),
    }
    out.push('}');
}

/// 集計を決定的な 1 行の JSON（ASCII だけ・欄順固定・末尾の改行なし）にする。
///
/// 出すのは検証済みの ID・白表の綴り・数だけで、入力のパスや自由な文字列は含まない。
pub fn render(a: &AcrossOutput) -> String {
    let mut out = String::from("{\"output_kind\":\"across-runs\",\"run_count\":");
    out.push_str(&a.run_ids.len().to_string());
    out.push_str(",\"run_ids\":");
    json::push_str_array(&mut out, a.run_ids.iter().map(String::as_str));
    out.push_str(",\"session_ref\":");
    json::push_string(&mut out, &a.session_ref);
    out.push_str(",\"machine_combination\":");
    json::push_string(&mut out, a.machine_combination);
    out.push_str(",\"machine\":");
    json::push_string(&mut out, a.machine);
    out.push_str(",\"task_ids\":");
    json::push_str_array(&mut out, a.task_ids.iter().map(String::as_str));
    out.push_str(",\"behavior_ids\":");
    json::push_str_array(&mut out, a.behavior_ids.iter().map(String::as_str));
    out.push_str(",\"cpu_ns\":");
    push_band(&mut out, &a.cpu);
    out.push_str(",\"gpu_ns\":");
    match &a.gpu {
        Some(b) => push_band(&mut out, b),
        None => out.push_str("null"),
    }
    out.push('}');
    out
}
