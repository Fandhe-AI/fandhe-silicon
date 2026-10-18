//! 測り直しと判定不能の扱いを、行ごとの純粋な関数として持つ（TASK-1027 の後半）。
//!
//! 呼び手は測定のハーネスと、後の段の再現の試験である。入力は、`gate` で絞り `runs` で集めた
//! 腕ごとの run の値の列（1/4 ns の整数）を、測った回ごとに束ねたものと、行の中の比較の一覧である。
//! 1 回目だけを渡して [`judge_row`] を呼ぶと、規則（`variation`）に当たりが無ければ判定が付き、
//! 当たりがあれば [`RowJudgement::NeedsRemeasure`] が返る。呼び手は該当する行だけを 1 回測り直し、
//! 2 回目を足して再び呼ぶ。測り直しは行ごとに 1 回までで、3 回目は渡せない（回数は設定の表が持つ）。
//! 測り直した後も当たりが残れば、設定の表が持つ範囲に従って比較を判定不能にする。
//!
//! 判定に使う値は run 間の中央値だけである（2 倍で持ち、丸めない）。2 回目を渡したときは
//! 2 回目の値だけで判定し、1 回目の値は混ぜない。1 run の値に何を使うか（中央値か p95 か）は
//! `variation::VariationSetting::statistic` に従って呼び手が列を作る。測り直す行を選ぶのも呼び手で、
//! 本モジュールは当たりの無い行への測り直しを `Err` にする。
//!
//! 時計・ファイル・環境に触れず、`record::RunRecord` の JSON も変えない。本モジュールの判定の
//! ラベルは行・比較の単位のもので、run の記録の `record::Label` とは別に持つ。比の上限は呼び手が渡す。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-1027、`docs/spec/04-behavior/performance.md`
//! （ポインタのみ）。

use std::collections::BTreeMap;
use std::fmt;

use crate::variation::{
    self, ArmSpread, Criterion, IndeterminateScope, Other, VariationError, VariationRule,
};

/// 腕の名前と、その腕の run の値の列（1/4 ns 単位）。
pub type ArmRuns<'a> = (&'a str, &'a [u64]);

/// 比較の相手。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counterpart<'a> {
    /// 同じ行の別の腕。
    Arm(&'a str),
    /// 定数（1/4 ns 単位）。
    Constant(u64),
}

/// 比の上限（`target ÷ 相手 ≤ num ÷ den` なら OK）。値は呼び手が渡す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bound {
    /// 分子。
    pub num: u64,
    /// 分母。0 は `Err`。
    pub den: u64,
}

/// 行の中の 1 つの比較。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison<'a> {
    /// 比較の識別子（行の中で一意）。
    pub id: &'a str,
    /// 比べられる側の腕の名前。
    pub target: &'a str,
    /// 比較の相手。
    pub counterpart: Counterpart<'a>,
    /// 比の上限。
    pub bound: Bound,
}

/// 判定に使った回。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Attempt {
    /// 1 回目。
    First,
    /// 測り直した 2 回目。
    Remeasured,
}

impl Attempt {
    /// 出力用の綴り。
    pub const fn name(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Remeasured => "remeasured",
        }
    }
}

/// 比較の判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum JudgedLabel {
    /// 上限以内。
    Ok,
    /// 上限を超えた。
    Ng,
    /// 測り直した後も変動の規則に当たり、判定できない。
    Indeterminate,
}

impl JudgedLabel {
    /// 出力用の綴り（`record::Label` の語と同じ綴り）。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Ng => "ng",
            Self::Indeterminate => "indeterminate",
        }
    }

    /// 判定不能の理由の綴り（判定不能のときだけ `Some`）。
    pub const fn reason_name(self) -> Option<&'static str> {
        match self {
            Self::Indeterminate => Some("variation-exceeded"),
            _ => None,
        }
    }
}

/// 腕の run 間の中央値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmValue {
    /// 腕の名前。
    pub name: String,
    /// run 間の中央値の 2 倍（1/4 ns 単位）。
    pub median_x2: u128,
}

/// 1 つの比較の判定と、判定に使った値（どちらも 2 倍）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonJudgement {
    /// 比較の識別子。
    pub id: String,
    /// 判定。
    pub label: JudgedLabel,
    /// 比べられる側の run 間の中央値の 2 倍。
    pub target_x2: u128,
    /// 相手の値の 2 倍（定数なら定数の 2 倍）。
    pub counterpart_x2: u128,
}

/// 1 行の扱いの結果。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RowJudgement {
    /// 測り直しが要る。当たった腕の名前（PoC-11 は比較の識別子）を昇順で持つ。
    NeedsRemeasure {
        /// 当たった腕（または比較）の名前の昇順。
        triggered: Vec<String>,
    },
    /// 判定が付いた（判定不能を含む）。
    Settled {
        /// 判定に使った回。
        attempt: Attempt,
        /// 腕ごとの run 間の中央値（名前の昇順）。
        arms: Vec<ArmValue>,
        /// 比較ごとの判定（識別子の昇順）。
        comparisons: Vec<ComparisonJudgement>,
    },
}

/// 失敗。入力の値は埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RemeasureError {
    /// 規則の適用の失敗（run の数の不一致・名前の重複など）。
    Variation(VariationError),
    /// 回の数が 0 件、または測り直しの回数の上限を超えている。
    AttemptCount,
    /// 規則に当たっていない行に測り直しが渡された。
    UnneededRemeasure,
    /// 回どうしで腕の名前の集合が違う。
    ArmSetMismatch,
    /// 比較が、行に無い腕を指している。
    UnknownArm,
    /// 比較の相手が自分自身。
    SelfComparison,
    /// 比較の識別子が重複している。
    DuplicateComparison,
    /// 比較が 0 件。
    EmptyComparisons,
    /// 比の上限の分母が 0。
    ZeroBoundDenominator,
    /// 計算が溢れた。
    Overflow,
}

impl fmt::Display for RemeasureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Variation(e) => e.fmt(f),
            Self::AttemptCount => f.write_str("attempt count is out of range"),
            Self::UnneededRemeasure => f.write_str("remeasure given for a row without a trigger"),
            Self::ArmSetMismatch => f.write_str("arm sets differ between attempts"),
            Self::UnknownArm => f.write_str("comparison refers to an unknown arm"),
            Self::SelfComparison => f.write_str("comparison target equals its counterpart"),
            Self::DuplicateComparison => f.write_str("duplicate comparison id"),
            Self::EmptyComparisons => f.write_str("no comparisons"),
            Self::ZeroBoundDenominator => f.write_str("bound denominator is zero"),
            Self::Overflow => f.write_str("arithmetic overflow"),
        }
    }
}

impl std::error::Error for RemeasureError {}

impl From<VariationError> for RemeasureError {
    fn from(e: VariationError) -> Self {
        Self::Variation(e)
    }
}

/// 1 回分の評価（腕の要約と、規則に当たった名前）。
struct Evaluation {
    spreads: BTreeMap<String, ArmSpread>,
    /// 当たった腕の名前（PoC-11 は比較の識別子）。昇順。
    triggered: Vec<String>,
}

fn evaluate(
    rule: VariationRule,
    arms: &[ArmRuns<'_>],
    comparisons: &[Comparison<'_>],
) -> Result<Evaluation, RemeasureError> {
    let setting = rule.setting();
    if arms.is_empty() {
        return Err(VariationError::Empty.into());
    }
    let mut spreads = BTreeMap::new();
    let mut raw: BTreeMap<&str, &[u64]> = BTreeMap::new();
    for (name, values) in arms {
        let spread = ArmSpread::from_runs(values, setting.runs)?;
        if spreads.insert((*name).to_string(), spread).is_some() {
            return Err(VariationError::DuplicateArm.into());
        }
        raw.insert(name, values);
    }
    let mut triggered = Vec::new();
    if matches!(setting.criterion, Criterion::DifferenceWithinSpread) {
        for c in comparisons {
            let (Some(target), other) = (raw.get(c.target), c.counterpart) else {
                continue; // 未知の腕は judge_row が先に弾く。
            };
            let other = match other {
                Counterpart::Constant(v) => Other::Constant(v),
                Counterpart::Arm(n) => match raw.get(n) {
                    Some(values) => Other::Arm(values),
                    None => continue,
                },
            };
            if variation::check_difference(target, other)? {
                triggered.push(c.id.to_string());
            }
        }
    } else {
        for (name, spread) in &spreads {
            if variation::check_arm(rule, spread)? {
                triggered.push(name.clone());
            }
        }
    }
    triggered.sort();
    Ok(Evaluation { spreads, triggered })
}

/// 比較ごとの判定。`indeterminate` は設定の範囲で判定不能になる比較。
fn judge_comparisons(
    scope: IndeterminateScope,
    eval: &Evaluation,
    comparisons: &[Comparison<'_>],
) -> Result<Vec<ComparisonJudgement>, RemeasureError> {
    let hit = |name: &str| eval.triggered.iter().any(|t| t == name);
    let mut out = Vec::with_capacity(comparisons.len());
    for c in comparisons {
        let target_x2 = eval.spreads[c.target].median_x2();
        let (counterpart_x2, counterpart_arm) = match c.counterpart {
            Counterpart::Arm(n) => (eval.spreads[n].median_x2(), Some(n)),
            Counterpart::Constant(v) => (2 * u128::from(v), None),
        };
        let indeterminate = match scope {
            IndeterminateScope::Row => !eval.triggered.is_empty(),
            IndeterminateScope::ComparisonsWithExceededArm => {
                hit(c.target) || counterpart_arm.is_some_and(hit)
            }
            IndeterminateScope::Comparison => hit(c.id),
        };
        let label = if indeterminate {
            JudgedLabel::Indeterminate
        } else {
            let lhs = target_x2
                .checked_mul(u128::from(c.bound.den))
                .ok_or(RemeasureError::Overflow)?;
            let rhs = counterpart_x2
                .checked_mul(u128::from(c.bound.num))
                .ok_or(RemeasureError::Overflow)?;
            if lhs <= rhs {
                JudgedLabel::Ok
            } else {
                JudgedLabel::Ng
            }
        };
        out.push(ComparisonJudgement {
            id: c.id.to_string(),
            label,
            target_x2,
            counterpart_x2,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// 1 行について、規則に当たったかを見て測り直しの要否を返し、判定が付くなら判定する。
///
/// `attempts` は測った回の順（1 回目、あれば測り直しの 2 回目）。2 回目を渡したときは 2 回目の値だけで
/// 判定する。入力の不整合（回の数・不要な測り直し・腕の集合の不一致・未知の腕・自分との比較・
/// 比較の重複や 0 件・上限の分母 0・run の数の不一致）と計算の溢れは `Err`。渡す順に結果は依らない。
pub fn judge_row(
    rule: VariationRule,
    attempts: &[&[ArmRuns<'_>]],
    comparisons: &[Comparison<'_>],
) -> Result<RowJudgement, RemeasureError> {
    let setting = rule.setting();
    if attempts.is_empty() || attempts.len() > 1 + setting.remeasures {
        return Err(RemeasureError::AttemptCount);
    }
    if comparisons.is_empty() {
        return Err(RemeasureError::EmptyComparisons);
    }
    let mut ids: Vec<&str> = comparisons.iter().map(|c| c.id).collect();
    ids.sort_unstable();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(RemeasureError::DuplicateComparison);
    }
    if comparisons.iter().any(|c| c.bound.den == 0) {
        return Err(RemeasureError::ZeroBoundDenominator);
    }
    let evals = attempts
        .iter()
        .map(|arms| evaluate(rule, arms, comparisons))
        .collect::<Result<Vec<_>, _>>()?;
    let first_names: Vec<&String> = evals[0].spreads.keys().collect();
    if evals
        .iter()
        .any(|e| !e.spreads.keys().eq(first_names.iter().copied()))
    {
        return Err(RemeasureError::ArmSetMismatch);
    }
    for c in comparisons {
        let known = |n: &str| evals[0].spreads.contains_key(n);
        if !known(c.target) {
            return Err(RemeasureError::UnknownArm);
        }
        if let Counterpart::Arm(n) = c.counterpart {
            if !known(n) {
                return Err(RemeasureError::UnknownArm);
            }
            if n == c.target {
                return Err(RemeasureError::SelfComparison);
            }
        }
    }
    // 引数の検査を終えてから、規則の適用で組んだ当たりを使う（未知の腕は上で弾き済み）。
    if evals.len() == 1 {
        let eval = &evals[0];
        if !eval.triggered.is_empty() {
            return Ok(RowJudgement::NeedsRemeasure {
                triggered: eval.triggered.clone(),
            });
        }
        return settle(setting.scope, Attempt::First, eval, comparisons);
    }
    if evals[0].triggered.is_empty() {
        return Err(RemeasureError::UnneededRemeasure);
    }
    settle(setting.scope, Attempt::Remeasured, &evals[1], comparisons)
}

fn settle(
    scope: IndeterminateScope,
    attempt: Attempt,
    eval: &Evaluation,
    comparisons: &[Comparison<'_>],
) -> Result<RowJudgement, RemeasureError> {
    Ok(RowJudgement::Settled {
        attempt,
        arms: eval
            .spreads
            .iter()
            .map(|(name, s)| ArmValue {
                name: name.clone(),
                median_x2: s.median_x2(),
            })
            .collect(),
        comparisons: judge_comparisons(scope, eval, comparisons)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NARROW: [u64; 6] = [1000; 6];
    /// 幅が中央値に対して大きい合成値（中央値 1000）。
    const WIDE: [u64; 6] = [100, 1000, 1000, 1000, 1000, 1900];
    /// 最大 ÷ 最小が大きい合成値。
    const SKEWED: [u64; 6] = [1000, 1000, 1000, 1000, 1000, 2000];

    fn cmp<'a>(
        id: &'a str,
        target: &'a str,
        cp: Counterpart<'a>,
        num: u64,
        den: u64,
    ) -> Comparison<'a> {
        Comparison {
            id,
            target,
            counterpart: cp,
            bound: Bound { num, den },
        }
    }

    fn settled(r: RowJudgement) -> (Attempt, Vec<ArmValue>, Vec<ComparisonJudgement>) {
        match r {
            RowJudgement::Settled {
                attempt,
                arms,
                comparisons,
            } => (attempt, arms, comparisons),
            other => panic!("expected Settled, got {other:?}"),
        }
    }

    fn labels(c: &[ComparisonJudgement]) -> Vec<JudgedLabel> {
        c.iter().map(|x| x.label).collect()
    }

    #[test]
    fn first_attempt_without_trigger_is_judged_with_inclusive_boundary() {
        let rule = VariationRule::Poc10Row2;
        let b = [2000u64; 6];
        let run = |a: &[u64; 6]| {
            let arms: Vec<ArmRuns<'_>> = vec![("a", a), ("b", &b)];
            let cs = [cmp("c", "a", Counterpart::Arm("b"), 1, 2)];
            settled(judge_row(rule, &[&arms], &cs).unwrap())
        };
        let (att, arms, cs) = run(&[1000; 6]);
        assert_eq!(att, Attempt::First);
        assert_eq!(arms[0].name, "a");
        assert_eq!(arms[0].median_x2, 2000);
        assert_eq!(labels(&cs), vec![JudgedLabel::Ok]); // ちょうど等しい
        let (_, _, cs) = run(&[1001; 6]);
        assert_eq!(labels(&cs), vec![JudgedLabel::Ng]);
    }

    #[test]
    fn constant_counterpart_is_doubled() {
        let arms: Vec<ArmRuns<'_>> = vec![("a", &NARROW)];
        let ok = [cmp("c", "a", Counterpart::Constant(2000), 1, 2)];
        let ng = [cmp("c", "a", Counterpart::Constant(1999), 1, 2)];
        let r = |cs: &[Comparison<'_>]| {
            settled(judge_row(VariationRule::Poc10Row2, &[&arms], cs).unwrap()).2
        };
        assert_eq!(labels(&r(&ok)), vec![JudgedLabel::Ok]);
        assert_eq!(labels(&r(&ng)), vec![JudgedLabel::Ng]);
        assert_eq!(r(&ok)[0].counterpart_x2, 4000);
    }

    #[test]
    fn trigger_on_first_attempt_needs_remeasure_with_sorted_names() {
        let arms: Vec<ArmRuns<'_>> = vec![("b", &WIDE), ("a", &WIDE), ("c", &NARROW)];
        let cs = [cmp("c1", "a", Counterpart::Arm("c"), 1, 1)];
        let r = judge_row(VariationRule::Poc10Row2, &[&arms], &cs).unwrap();
        assert_eq!(
            r,
            RowJudgement::NeedsRemeasure {
                triggered: vec!["a".into(), "b".into()]
            }
        );
    }

    #[test]
    fn remeasured_values_alone_decide_the_label() {
        let rule = VariationRule::Poc10Row2;
        let b = [2000u64; 6];
        let first: Vec<ArmRuns<'_>> = vec![("a", &WIDE), ("b", &b)];
        // 1 回目の a の中央値 1000 なら OK、2 回目の 1001 なら NG になる上限。
        let second_a = [1001u64; 6];
        let second: Vec<ArmRuns<'_>> = vec![("a", &second_a), ("b", &b)];
        let cs = [cmp("c", "a", Counterpart::Arm("b"), 1, 2)];
        let (att, _, out) = settled(judge_row(rule, &[&first, &second], &cs).unwrap());
        assert_eq!(att, Attempt::Remeasured);
        assert_eq!(labels(&out), vec![JudgedLabel::Ng]);
    }

    #[test]
    fn row_scope_makes_every_comparison_indeterminate() {
        let rule = VariationRule::Poc10Row2;
        let arms: Vec<ArmRuns<'_>> = vec![("a", &WIDE), ("b", &NARROW), ("c", &NARROW)];
        let cs = [
            cmp("c1", "b", Counterpart::Arm("c"), 1, 1),
            cmp("c2", "c", Counterpart::Constant(1000), 1, 1),
        ];
        let (att, _, out) = settled(judge_row(rule, &[&arms, &arms], &cs).unwrap());
        assert_eq!(att, Attempt::Remeasured);
        assert_eq!(labels(&out), vec![JudgedLabel::Indeterminate; 2]);
        assert_eq!(out[0].label.reason_name(), Some("variation-exceeded"));
        assert_eq!(JudgedLabel::Ok.reason_name(), None);
    }

    #[test]
    fn exceeded_arm_scope_limits_to_comparisons_touching_the_arm() {
        let rule = VariationRule::Poc9;
        let arms: Vec<ArmRuns<'_>> = vec![("a", &SKEWED), ("b", &NARROW), ("c", &NARROW)];
        let cs = [
            cmp("c1", "b", Counterpart::Arm("a"), 1, 1), // 相手が当たった腕
            cmp("c2", "a", Counterpart::Constant(1000), 1, 1), // target が当たった腕
            cmp("c3", "b", Counterpart::Arm("c"), 1, 1),
        ];
        assert!(matches!(
            judge_row(rule, &[&arms], &cs).unwrap(),
            RowJudgement::NeedsRemeasure { .. }
        ));
        let (_, _, out) = settled(judge_row(rule, &[&arms, &arms], &cs).unwrap());
        assert_eq!(
            labels(&out),
            vec![
                JudgedLabel::Indeterminate,
                JudgedLabel::Indeterminate,
                JudgedLabel::Ok
            ]
        );
    }

    #[test]
    fn comparison_scope_limits_to_differences_within_spread() {
        let rule = VariationRule::Poc11;
        let t = [990u64, 1000, 1000, 1000, 1000, 1010]; // 幅 20
        let arms: Vec<ArmRuns<'_>> = vec![("t", &t)];
        let cs = [
            cmp("c2", "t", Counterpart::Constant(1500), 1, 1), // 差が幅の外
            cmp("c1", "t", Counterpart::Constant(1020), 1, 1), // 差が幅ちょうど
        ];
        assert_eq!(
            judge_row(rule, &[&arms], &cs).unwrap(),
            RowJudgement::NeedsRemeasure {
                triggered: vec!["c1".into()]
            }
        );
        let (att, _, out) = settled(judge_row(rule, &[&arms, &arms], &cs).unwrap());
        assert_eq!(att, Attempt::Remeasured);
        assert_eq!(out[0].id, "c1");
        assert_eq!(
            labels(&out),
            vec![JudgedLabel::Indeterminate, JudgedLabel::Ok]
        );
    }

    #[test]
    fn remeasure_clearing_the_trigger_is_settled_as_remeasured() {
        let rule = VariationRule::Poc10Row2;
        let first: Vec<ArmRuns<'_>> = vec![("a", &WIDE)];
        let second: Vec<ArmRuns<'_>> = vec![("a", &NARROW)];
        let cs = [cmp("c", "a", Counterpart::Constant(1000), 1, 1)];
        let (att, _, out) = settled(judge_row(rule, &[&first, &second], &cs).unwrap());
        assert_eq!(att, Attempt::Remeasured);
        assert_eq!(labels(&out), vec![JudgedLabel::Ok]);
        assert_eq!(Attempt::Remeasured.name(), "remeasured");
        assert_eq!(Attempt::First.name(), "first");
    }

    #[test]
    fn invalid_inputs_are_errors() {
        let rule = VariationRule::Poc10Row2;
        let a: Vec<ArmRuns<'_>> = vec![("a", &NARROW), ("b", &NARROW)];
        let wide: Vec<ArmRuns<'_>> = vec![("a", &WIDE), ("b", &NARROW)];
        let ok = [cmp("c", "a", Counterpart::Arm("b"), 1, 1)];
        let e = |attempts: &[&[ArmRuns<'_>]], cs: &[Comparison<'_>]| {
            judge_row(rule, attempts, cs).unwrap_err()
        };
        assert_eq!(e(&[], &ok), RemeasureError::AttemptCount);
        assert_eq!(e(&[&wide, &wide, &wide], &ok), RemeasureError::AttemptCount);
        assert_eq!(e(&[&a, &a], &ok), RemeasureError::UnneededRemeasure);
        let other: Vec<ArmRuns<'_>> = vec![("a", &NARROW), ("z", &NARROW)];
        assert_eq!(e(&[&wide, &other], &ok), RemeasureError::ArmSetMismatch);
        assert_eq!(
            e(&[&a], &[cmp("c", "a", Counterpart::Arm("q"), 1, 1)]),
            RemeasureError::UnknownArm
        );
        assert_eq!(
            e(&[&a], &[cmp("c", "q", Counterpart::Constant(1), 1, 1)]),
            RemeasureError::UnknownArm
        );
        assert_eq!(
            e(&[&a], &[cmp("c", "a", Counterpart::Arm("a"), 1, 1)]),
            RemeasureError::SelfComparison
        );
        assert_eq!(
            e(&[&a], &[ok[0], ok[0]]),
            RemeasureError::DuplicateComparison
        );
        assert_eq!(e(&[&a], &[]), RemeasureError::EmptyComparisons);
        assert_eq!(
            e(&[&a], &[cmp("c", "a", Counterpart::Arm("b"), 1, 0)]),
            RemeasureError::ZeroBoundDenominator
        );
        let short = [1000u64; 5];
        let bad: Vec<ArmRuns<'_>> = vec![("a", &short), ("b", &NARROW)];
        assert_eq!(
            e(&[&bad], &ok),
            RemeasureError::Variation(VariationError::RunCount)
        );
        let dup: Vec<ArmRuns<'_>> = vec![("a", &NARROW), ("a", &NARROW)];
        assert_eq!(
            e(&[&dup], &ok),
            RemeasureError::Variation(VariationError::DuplicateArm)
        );
        assert_eq!(
            e(&[&[]], &ok),
            RemeasureError::Variation(VariationError::Empty)
        );
    }

    #[test]
    fn result_does_not_depend_on_input_order() {
        let rule = VariationRule::Poc9;
        let x: Vec<ArmRuns<'_>> = vec![("a", &SKEWED), ("b", &NARROW), ("c", &NARROW)];
        let y: Vec<ArmRuns<'_>> = vec![("c", &NARROW), ("a", &SKEWED), ("b", &NARROW)];
        let c1 = cmp("c1", "b", Counterpart::Arm("c"), 1, 1);
        let c2 = cmp("c2", "a", Counterpart::Constant(1000), 1, 1);
        assert_eq!(
            judge_row(rule, &[&x, &x], &[c1, c2]).unwrap(),
            judge_row(rule, &[&y, &y], &[c2, c1]).unwrap()
        );
    }

    #[test]
    fn overflow_is_an_error() {
        let m = u64::MAX;
        let big = [m; 6];
        let arms: Vec<ArmRuns<'_>> = vec![("a", &big)];
        let cs = [cmp("c", "a", Counterpart::Constant(m), m, m)];
        assert_eq!(
            judge_row(VariationRule::Poc10Row2, &[&arms], &cs),
            Err(RemeasureError::Overflow)
        );
        // 溢れない大きさでは判定できる。
        let cs = [cmp("c", "a", Counterpart::Constant(m), 1, 1)];
        let (_, _, out) = settled(judge_row(VariationRule::Poc10Row2, &[&arms], &cs).unwrap());
        assert_eq!(labels(&out), vec![JudgedLabel::Ok]);
    }

    #[test]
    fn names_are_lowercase_ascii() {
        for n in [
            JudgedLabel::Ok.name(),
            JudgedLabel::Ng.name(),
            JudgedLabel::Indeterminate.name(),
        ] {
            assert!(n.bytes().all(|b| b.is_ascii_lowercase()));
        }
    }
}
