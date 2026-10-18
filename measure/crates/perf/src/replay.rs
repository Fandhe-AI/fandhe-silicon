//! 記録済みの run 間の値を入力にして、行ごとの判定のラベルを再現する純粋な関数（TASK-1027 の 5/6）。
//!
//! 呼び手はコマンド `variation-replay`（記録のパスを引数で受ける手元の照合）と公開テスト（合成の入力）である。
//! 入力の JSON を `jsonread` で厳格に読み、行ごとに [`crate::remeasure::judge_row`] を呼び、
//! 結果を入力が持つ期待のラベルと突き合わせて 1 行の JSON に描く。判定の規則そのものは
//! `variation`・`remeasure` が持ち、本モジュールは規則を足さない。
//!
//! 入力は「腕ごとに run の数だけ並んだ run ごとの統計値（1/4 ns 単位の整数）」である。
//! PoC の生の標本からの変換は本リポの外の手元作業で、本モジュールはしない。時刻の分解能は
//! 入力の整数（ピコ秒単位）をそのまま出力へ通すだけで、判定には使わず、設定の表にも書かない。
//! 未知のキー・未知の規則・小数・期待の欠けや余りは入力の違反で、暗黙に一致へ倒さない。
//! 時計・ファイル・環境に触れず、`record::RunRecord` の JSON も変えない。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-1027（ポインタのみ）。

use crate::json::push_string;
use crate::jsonread::Value;
use crate::remeasure::{
    ArmRuns, Bound, Comparison, Counterpart, JudgedLabel, RemeasureError, RowJudgement, judge_row,
};
use crate::variation::VariationRule;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};

/// 行の件数の上限。
pub const MAX_ROWS: usize = 1024;
/// 1 回あたりの腕の件数の上限。
pub const MAX_ARMS: usize = 64;
/// 1 行あたりの比較の件数の上限。
pub const MAX_COMPARISONS: usize = 256;
/// 1 腕あたりの run の値の件数の上限。
pub const MAX_RUNS: usize = 64;
/// 1 行あたりの回の件数の上限（判定側の上限より緩く、超過は判定側が `Err` にする）。
pub const MAX_ATTEMPTS: usize = 8;
/// 名前（行・腕・比較の識別子）の長さの上限。
pub const MAX_NAME: usize = 64;

/// 入力の違反の種類。入力の値は埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayErrorKind {
    /// 形が違う（型・欄の不足・未知のキー）。
    Shape,
    /// 未知の規則。
    UnknownRule,
    /// 名前が空・長すぎる・使えない文字を含む、または重複している。
    BadName,
    /// 件数の上限を超えた。
    TooMany,
    /// 値が整数でない、または範囲外。
    BadNumber,
    /// 期待のラベルが未知、または比較の集合と合わない。
    BadExpected,
    /// 判定側が入力を拒否した。
    Judge(RemeasureError),
}

/// 入力の違反。`row` は 0 始まりの行の位置（行より外の違反は `None`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayError {
    /// 違反の種類。
    pub kind: ReplayErrorKind,
    /// 行の位置。
    pub row: Option<usize>,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ReplayErrorKind::Shape => f.write_str("malformed input shape")?,
            ReplayErrorKind::UnknownRule => f.write_str("unknown rule")?,
            ReplayErrorKind::BadName => f.write_str("invalid or duplicate name")?,
            ReplayErrorKind::TooMany => f.write_str("too many items")?,
            ReplayErrorKind::BadNumber => f.write_str("invalid number")?,
            ReplayErrorKind::BadExpected => f.write_str("invalid expected labels")?,
            ReplayErrorKind::Judge(e) => write!(f, "judge rejected input: {e}")?,
        }
        match self.row {
            Some(r) => write!(f, " (row {})", r + 1),
            None => Ok(()),
        }
    }
}

impl std::error::Error for ReplayError {}

fn err(kind: ReplayErrorKind, row: Option<usize>) -> ReplayError {
    ReplayError { kind, row }
}

/// 行の期待。
#[derive(Debug)]
enum Expected {
    NeedsRemeasure,
    Labels(BTreeMap<String, JudgedLabel>),
}

/// 比較の入力（所有）。相手は腕か定数のどちらか一方。
#[derive(Debug)]
struct OwnedComparison {
    id: String,
    target: String,
    arm: Option<String>,
    constant: u64,
    bound: Bound,
}

#[derive(Debug)]
struct Row {
    id: String,
    attempts: Vec<Vec<(String, Vec<u64>)>>,
    comparisons: Vec<OwnedComparison>,
    expected: Expected,
}

/// 読み込んだ入力。
#[derive(Debug)]
pub struct ReplayInput {
    rule: VariationRule,
    cpu_ps: u64,
    gpu_ps: Option<u64>,
    rows: Vec<Row>,
}

fn check_keys(v: &Value, allowed: &[&str], row: Option<usize>) -> Result<(), ReplayError> {
    match v {
        Value::Object(m) if m.iter().all(|(k, _)| allowed.contains(&k.as_str())) => Ok(()),
        _ => Err(err(ReplayErrorKind::Shape, row)),
    }
}

fn need<'a>(v: &'a Value, key: &str, row: Option<usize>) -> Result<&'a Value, ReplayError> {
    v.get(key).ok_or_else(|| err(ReplayErrorKind::Shape, row))
}

fn name(v: &Value, row: Option<usize>) -> Result<String, ReplayError> {
    let s = v.as_str().ok_or_else(|| err(ReplayErrorKind::Shape, row))?;
    let ok = !s.is_empty()
        && s.len() <= MAX_NAME
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    if ok {
        Ok(s.to_owned())
    } else {
        Err(err(ReplayErrorKind::BadName, row))
    }
}

fn uint(v: &Value, row: Option<usize>) -> Result<u64, ReplayError> {
    v.as_u64()
        .ok_or_else(|| err(ReplayErrorKind::BadNumber, row))
}

fn array(v: &Value, max: usize, row: Option<usize>) -> Result<&[Value], ReplayError> {
    let a = v
        .as_array()
        .ok_or_else(|| err(ReplayErrorKind::Shape, row))?;
    if a.len() > max {
        return Err(err(ReplayErrorKind::TooMany, row));
    }
    Ok(a)
}

fn parse_attempt(v: &Value, r: Option<usize>) -> Result<Vec<(String, Vec<u64>)>, ReplayError> {
    let mut out: Vec<(String, Vec<u64>)> = Vec::new();
    for arm in array(v, MAX_ARMS, r)? {
        check_keys(arm, &["arm", "runs"], r)?;
        let n = name(need(arm, "arm", r)?, r)?;
        if out.iter().any(|(k, _)| *k == n) {
            return Err(err(ReplayErrorKind::BadName, r));
        }
        let runs = array(need(arm, "runs", r)?, MAX_RUNS, r)?
            .iter()
            .map(|x| uint(x, r))
            .collect::<Result<Vec<_>, _>>()?;
        out.push((n, runs));
    }
    Ok(out)
}

fn parse_comparison(v: &Value, r: Option<usize>) -> Result<OwnedComparison, ReplayError> {
    const KEYS: [&str; 6] = [
        "id",
        "target",
        "counterpart_arm",
        "counterpart_const",
        "bound_num",
        "bound_den",
    ];
    check_keys(v, &KEYS, r)?;
    let arm = v.get("counterpart_arm").map(|x| name(x, r)).transpose()?;
    let constant = v.get("counterpart_const").map(|x| uint(x, r)).transpose()?;
    // 相手は腕か定数のどちらか一方だけ。
    if arm.is_some() == constant.is_some() {
        return Err(err(ReplayErrorKind::Shape, r));
    }
    Ok(OwnedComparison {
        id: name(need(v, "id", r)?, r)?,
        target: name(need(v, "target", r)?, r)?,
        arm,
        constant: constant.unwrap_or(0),
        bound: Bound {
            num: uint(need(v, "bound_num", r)?, r)?,
            den: uint(need(v, "bound_den", r)?, r)?,
        },
    })
}

fn label_from(s: &str) -> Option<JudgedLabel> {
    [JudgedLabel::Ok, JudgedLabel::Ng, JudgedLabel::Indeterminate]
        .into_iter()
        .find(|l| l.name() == s)
}

fn parse_expected(v: &Value, r: Option<usize>) -> Result<Expected, ReplayError> {
    check_keys(v, &["needs_remeasure", "labels"], r)?;
    match (v.get("needs_remeasure"), v.get("labels")) {
        (Some(Value::Bool(true)), None) => Ok(Expected::NeedsRemeasure),
        (None, Some(Value::Object(entries))) => {
            let mut out = BTreeMap::new();
            for (id, l) in entries {
                let label = l
                    .as_str()
                    .and_then(label_from)
                    .ok_or_else(|| err(ReplayErrorKind::BadExpected, r))?;
                out.insert(id.clone(), label);
            }
            Ok(Expected::Labels(out))
        }
        _ => Err(err(ReplayErrorKind::BadExpected, r)),
    }
}

fn parse_row(v: &Value, idx: usize) -> Result<Row, ReplayError> {
    let r = Some(idx);
    check_keys(v, &["row", "attempts", "comparisons", "expected"], r)?;
    Ok(Row {
        id: name(need(v, "row", r)?, r)?,
        attempts: array(need(v, "attempts", r)?, MAX_ATTEMPTS, r)?
            .iter()
            .map(|a| parse_attempt(a, r))
            .collect::<Result<_, _>>()?,
        comparisons: array(need(v, "comparisons", r)?, MAX_COMPARISONS, r)?
            .iter()
            .map(|c| parse_comparison(c, r))
            .collect::<Result<_, _>>()?,
        expected: parse_expected(need(v, "expected", r)?, r)?,
    })
}

/// 入力の JSON の値を検証して読み替える。
pub fn parse_input(v: &Value) -> Result<ReplayInput, ReplayError> {
    check_keys(v, &["rule", "resolution", "rows"], None)?;
    let rule_name = need(v, "rule", None)?
        .as_str()
        .ok_or_else(|| err(ReplayErrorKind::Shape, None))?;
    let rule = VariationRule::ALL
        .into_iter()
        .find(|r| r.name() == rule_name)
        .ok_or_else(|| err(ReplayErrorKind::UnknownRule, None))?;
    let res = need(v, "resolution", None)?;
    check_keys(res, &["cpu_ps", "gpu_ps"], None)?;
    let cpu_ps = uint(need(res, "cpu_ps", None)?, None)?;
    let gpu_ps = res.get("gpu_ps").map(|x| uint(x, None)).transpose()?;
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, rv) in array(need(v, "rows", None)?, MAX_ROWS, None)?
        .iter()
        .enumerate()
    {
        let row = parse_row(rv, i)?;
        if !seen.insert(row.id.clone()) {
            return Err(err(ReplayErrorKind::BadName, Some(i)));
        }
        rows.push(row);
    }
    Ok(ReplayInput {
        rule,
        cpu_ps,
        gpu_ps,
        rows,
    })
}

/// 比較ごとの再現結果。
struct CmpOut {
    id: String,
    label: JudgedLabel,
    expected: Option<JudgedLabel>,
}

enum RowResult {
    Needs(Vec<String>),
    Settled(&'static str, Vec<CmpOut>),
}

/// 行ごとの再現結果。
struct RowOut {
    id: String,
    result: RowResult,
    matched: bool,
}

/// 再現の集計。
#[derive(Debug)]
pub struct ReplayOutcome {
    line: String,
    total: usize,
    matched: usize,
}

impl ReplayOutcome {
    /// 1 行の JSON（改行なし）。
    pub fn line(&self) -> &str {
        &self.line
    }

    /// すべての行が期待と一致したか。
    pub fn all_matched(&self) -> bool {
        self.total == self.matched
    }
}

fn replay_row(rule: VariationRule, idx: usize, row: &Row) -> Result<RowOut, ReplayError> {
    let r = Some(idx);
    let attempts_owned: Vec<Vec<ArmRuns<'_>>> = row
        .attempts
        .iter()
        .map(|a| a.iter().map(|(n, v)| (n.as_str(), v.as_slice())).collect())
        .collect();
    let attempts: Vec<&[ArmRuns<'_>]> = attempts_owned.iter().map(Vec::as_slice).collect();
    let comparisons: Vec<Comparison<'_>> = row
        .comparisons
        .iter()
        .map(|c| Comparison {
            id: &c.id,
            target: &c.target,
            counterpart: match &c.arm {
                Some(a) => Counterpart::Arm(a),
                None => Counterpart::Constant(c.constant),
            },
            bound: c.bound,
        })
        .collect();
    let judged =
        judge_row(rule, &attempts, &comparisons).map_err(|e| err(ReplayErrorKind::Judge(e), r))?;
    // 期待の欠け・余りは判定結果によらず入力の違反。暗黙に一致へ倒さない。
    if let Expected::Labels(map) = &row.expected
        && (map.len() != comparisons.len() || !comparisons.iter().all(|c| map.contains_key(c.id)))
    {
        return Err(err(ReplayErrorKind::BadExpected, r));
    }
    match (judged, &row.expected) {
        (RowJudgement::NeedsRemeasure { triggered }, exp) => Ok(RowOut {
            id: row.id.clone(),
            matched: matches!(exp, Expected::NeedsRemeasure),
            result: RowResult::Needs(triggered),
        }),
        (
            RowJudgement::Settled {
                attempt,
                comparisons: cs,
                ..
            },
            exp,
        ) => {
            let outs: Vec<CmpOut> = match exp {
                Expected::Labels(map) => cs
                    .iter()
                    .map(|c| CmpOut {
                        id: c.id.clone(),
                        label: c.label,
                        expected: map.get(&c.id).copied(),
                    })
                    .collect(),
                Expected::NeedsRemeasure => cs
                    .iter()
                    .map(|c| CmpOut {
                        id: c.id.clone(),
                        label: c.label,
                        expected: None,
                    })
                    .collect(),
            };
            let matched = outs.iter().all(|c| c.expected == Some(c.label));
            Ok(RowOut {
                id: row.id.clone(),
                matched,
                result: RowResult::Settled(attempt.name(), outs),
            })
        }
        // 将来の変種は暗黙に一致へ倒さず、入力の違反として拒否する。
        #[allow(unreachable_patterns)]
        _ => Err(err(ReplayErrorKind::BadExpected, r)),
    }
}

fn push_opt_name(s: &mut String, v: Option<&str>) {
    match v {
        Some(x) => push_string(s, x),
        None => s.push_str("null"),
    }
}

/// 入力を再現して結果の 1 行の JSON を作る。行・比較は識別子の昇順に並べる。
pub fn replay(input: &ReplayInput) -> Result<ReplayOutcome, ReplayError> {
    let mut outs = Vec::with_capacity(input.rows.len());
    for (i, row) in input.rows.iter().enumerate() {
        outs.push(replay_row(input.rule, i, row)?);
    }
    outs.sort_by(|a, b| a.id.cmp(&b.id));
    for o in &mut outs {
        if let RowResult::Settled(_, cs) = &mut o.result {
            cs.sort_by(|a, b| a.id.cmp(&b.id));
        }
    }
    let matched = outs.iter().filter(|o| o.matched).count();
    let mut s = String::new();
    s.push_str("{\"rule\":");
    push_string(&mut s, input.rule.name());
    let _ = write!(
        s,
        ",\"resolution\":{{\"cpu_ps\":{},\"gpu_ps\":",
        input.cpu_ps
    );
    match input.gpu_ps {
        Some(g) => {
            let _ = write!(s, "{g}");
        }
        None => s.push_str("null"),
    }
    s.push_str("},\"rows\":[");
    for (i, o) in outs.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"row\":");
        push_string(&mut s, &o.id);
        match &o.result {
            RowResult::Needs(t) => {
                s.push_str(",\"result\":\"needs-remeasure\",\"triggered\":[");
                for (j, n) in t.iter().enumerate() {
                    if j > 0 {
                        s.push(',');
                    }
                    push_string(&mut s, n);
                }
                s.push(']');
            }
            RowResult::Settled(attempt, cs) => {
                s.push_str(",\"result\":\"settled\",\"attempt\":");
                push_string(&mut s, attempt);
                s.push_str(",\"comparisons\":[");
                for (j, c) in cs.iter().enumerate() {
                    if j > 0 {
                        s.push(',');
                    }
                    s.push_str("{\"id\":");
                    push_string(&mut s, &c.id);
                    s.push_str(",\"label\":");
                    push_string(&mut s, c.label.name());
                    s.push_str(",\"reason\":");
                    push_opt_name(&mut s, c.label.reason_name());
                    s.push_str(",\"expected\":");
                    push_opt_name(&mut s, c.expected.map(JudgedLabel::name));
                    let _ = write!(s, ",\"match\":{}}}", c.expected == Some(c.label));
                }
                s.push(']');
            }
        }
        let _ = write!(s, ",\"match\":{}}}", o.matched);
    }
    let _ = write!(
        s,
        "],\"rows_total\":{},\"rows_matched\":{matched}}}",
        outs.len()
    );
    Ok(ReplayOutcome {
        line: s,
        total: outs.len(),
        matched,
    })
}
