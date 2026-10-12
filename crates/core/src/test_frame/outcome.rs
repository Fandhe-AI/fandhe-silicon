//! 試験 1 回の結果の決め方（`judge`）と、決まった結果（`Outcome`）。
//!
//! 試験の関数の末尾から呼ばれる。実行の可否（`decision::decide`）、本体の結果、閾値の承認の有無
//! （`threshold::ThresholdId::is_approved`）から、ラベルを 1 つ決める純粋な関数で、入力以外の状態を読まない。
//! `parity` の比べの結果は `From` で `Verdict` に写す（`parity` 側は変えない）。書き出しは持たない
//! （組み合わせごとの集計は `summary` が、記録の組み立ては `record` が、`Outcome` の getter の上で行う）。
//! `Termination` は実装せず、試験の関数の戻り値には使えない
//! （走らせなかった結果が、戻り値の上で成功に見えるのを避けるため）。
//! 失敗にするかは呼び出し側が `Label::fails_test` で明示的に決める。
//! spec TASK-23。

use super::combination::MachineCombination;
use super::decision::{Decision, decide};
use super::declaration::Declaration;
use super::identifier::{BehaviorId, IdentifierError, TestId};
use super::label::{IndeterminateReason, Label};
use super::setting::RunSetting;
use super::threshold::{ThresholdId, ThresholdIdError};
use crate::parity::{pixel, special_values, tolerance, topk};
use std::fmt;

/// 1 つの試験が持てるビヘイビア項目の ID の件数の上限。
pub const MAX_BEHAVIORS: usize = 16;
/// 1 つの試験が持てる閾値の ID の件数の上限。
pub const MAX_THRESHOLDS: usize = 16;

/// 比べの結果の判定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// 一致した。
    Match,
    /// 一致しなかった。
    Mismatch,
}

impl Verdict {
    /// 真偽から作る。`true` が一致。
    pub const fn from_match(matched: bool) -> Verdict {
        if matched {
            Verdict::Match
        } else {
            Verdict::Mismatch
        }
    }
}

impl From<&tolerance::Comparison> for Verdict {
    fn from(c: &tolerance::Comparison) -> Self {
        Verdict::from_match(c.is_match())
    }
}

impl From<&topk::TopKComparison> for Verdict {
    fn from(c: &topk::TopKComparison) -> Self {
        Verdict::from_match(c.is_match())
    }
}

impl From<&pixel::ExactPixelComparison> for Verdict {
    fn from(c: &pixel::ExactPixelComparison) -> Self {
        Verdict::from_match(c.is_match())
    }
}

impl From<&pixel::PixelComparison> for Verdict {
    fn from(c: &pixel::PixelComparison) -> Self {
        Verdict::from_match(c.is_match())
    }
}

impl From<&special_values::SpecialComparison> for Verdict {
    fn from(c: &special_values::SpecialComparison) -> Self {
        Verdict::from_match(c.is_match())
    }
}

/// 試験の本体が返す結果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunResult {
    /// 比べた結果。
    Verdict(Verdict),
    /// 判定できなかった。
    Indeterminate(IndeterminateReason),
}

/// [`Subject::from_parts`] の失敗。入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectError {
    /// 試験の ID が検査を通らなかった。
    TestId(IdentifierError),
    /// ビヘイビア項目の ID が検査を通らなかった。
    BehaviorId(IdentifierError),
    /// 閾値の ID が検査を通らなかった。
    ThresholdId(ThresholdIdError),
    /// ビヘイビア項目の件数が [`MAX_BEHAVIORS`] を超えた。
    TooManyBehaviors,
    /// 閾値の件数が [`MAX_THRESHOLDS`] を超えた。
    TooManyThresholds,
}

impl SubjectError {
    /// 失敗の種類を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            SubjectError::TestId(e) | SubjectError::BehaviorId(e) => e.as_str(),
            SubjectError::ThresholdId(e) => e.as_str(),
            SubjectError::TooManyBehaviors => "subject has too many behavior ids",
            SubjectError::TooManyThresholds => "subject has too many threshold ids",
        }
    }
}

impl fmt::Display for SubjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for SubjectError {}

/// 試験が自分について宣言する識別子の集まり。重複は取り除かれ、順は宣言の順。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
    id: TestId,
    behaviors: Vec<BehaviorId>,
    thresholds: Vec<ThresholdId>,
}

impl Subject {
    /// 名前を受けて検査し、作る。
    pub fn from_parts(
        id: &str,
        behaviors: &[&str],
        thresholds: &[&str],
    ) -> Result<Subject, SubjectError> {
        if behaviors.len() > MAX_BEHAVIORS {
            return Err(SubjectError::TooManyBehaviors);
        }
        if thresholds.len() > MAX_THRESHOLDS {
            return Err(SubjectError::TooManyThresholds);
        }
        let id = TestId::new(id).map_err(SubjectError::TestId)?;
        let mut bs: Vec<BehaviorId> = Vec::with_capacity(behaviors.len());
        for b in behaviors {
            let b = BehaviorId::new(b).map_err(SubjectError::BehaviorId)?;
            if !bs.contains(&b) {
                bs.push(b);
            }
        }
        let mut ts: Vec<ThresholdId> = Vec::with_capacity(thresholds.len());
        for t in thresholds {
            let t = ThresholdId::new(t).map_err(SubjectError::ThresholdId)?;
            if !ts.contains(&t) {
                ts.push(t);
            }
        }
        Ok(Subject {
            id,
            behaviors: bs,
            thresholds: ts,
        })
    }

    /// 試験の ID。
    pub fn id(&self) -> &TestId {
        &self.id
    }

    /// 対象のビヘイビア項目の ID（重複なし）。
    pub fn behaviors(&self) -> &[BehaviorId] {
        &self.behaviors
    }

    /// 宣言した閾値の ID（重複なし。宣言の順）。
    pub fn thresholds(&self) -> &[ThresholdId] {
        &self.thresholds
    }
}

/// [`judge`] が決めた 1 回分の結果。`judge` 以外からは作れない。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    id: TestId,
    behaviors: Vec<BehaviorId>,
    declared: Vec<MachineCombination>,
    ran_on: Option<MachineCombination>,
    label: Label,
}

impl Outcome {
    /// 試験の ID。
    pub fn id(&self) -> &TestId {
        &self.id
    }

    /// 対象のビヘイビア項目の ID。
    pub fn behaviors(&self) -> &[BehaviorId] {
        &self.behaviors
    }

    /// 宣言した組み合わせ。
    pub fn declared_combinations(&self) -> &[MachineCombination] {
        &self.declared
    }

    /// 走らせた組み合わせ。本体を呼んだときだけ `Some`。
    pub fn ran_on(&self) -> Option<MachineCombination> {
        self.ran_on
    }

    /// 結果のラベル。
    pub fn label(&self) -> &Label {
        &self.label
    }
}

/// 結果を決める。上から先に当たったものが勝つ。
///
/// 1. 実行の可否が `NotRun` なら未実施（本体は呼ばない）
/// 2. 本体が判定不能を返したら判定不能
/// 3. 宣言した閾値に未承認があれば判定保留（宣言の順で最初の未承認の ID）
/// 4. 一致なら OK、不一致なら NG
///
/// 入力以外の状態を読まず、自分では panic しない。`body` の panic はそのまま伝わる。
pub fn judge<F>(
    subject: &Subject,
    declaration: &Declaration,
    setting: &RunSetting,
    body: F,
) -> Outcome
where
    F: FnOnce(MachineCombination) -> RunResult,
{
    judge_with_approval(
        subject,
        declaration,
        setting,
        ThresholdId::is_approved,
        body,
    )
}

/// [`judge`] の本体。結果を決める順は `judge` の説明のとおりで、閾値の承認の有無だけを `is_approved` で受ける。
///
/// 公開しない。承認の有無は表（[`ThresholdId::is_approved`]）の 1 か所が持つので、`judge` だけがこれを呼ぶ。
fn judge_with_approval<A, F>(
    subject: &Subject,
    declaration: &Declaration,
    setting: &RunSetting,
    is_approved: A,
    body: F,
) -> Outcome
where
    A: Fn(&ThresholdId) -> bool,
    F: FnOnce(MachineCombination) -> RunResult,
{
    let mut ran_on = None;
    let label = match decide(declaration, setting) {
        Decision::NotRun(r) => Label::NotRun(r),
        Decision::Run(c) => {
            ran_on = Some(c);
            match body(c) {
                RunResult::Indeterminate(r) => Label::Indeterminate(r),
                RunResult::Verdict(v) => {
                    match subject.thresholds.iter().find(|t| !is_approved(t)) {
                        Some(t) => Label::Pending(t.clone()),
                        None => match v {
                            Verdict::Match => Label::Ok,
                            Verdict::Mismatch => Label::Ng,
                        },
                    }
                }
            }
        }
    };
    Outcome {
        id: subject.id.clone(),
        behaviors: subject.behaviors.clone(),
        declared: declaration.combinations().to_vec(),
        ran_on,
        label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_frame::setting::COMBINATION_ENV;
    use std::ffi::OsString;

    fn running_setting() -> RunSetting {
        RunSetting::from_lookup(|n| {
            (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
        })
    }

    fn declaration() -> Declaration {
        Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap()
    }

    /// 承認の判定を差し替えられるのはこの crate の中だけ。差し替えた判定が、宣言の順の最初の未承認を選ぶ。
    #[test]
    fn injected_approval_picks_the_first_unapproved_threshold_in_declared_order() {
        let s = Subject::from_parts("inject", &[], &["t-1", "t-13", "t-12"]).unwrap();
        let approve = |t: &ThresholdId| t.as_str() == "t-1";
        for body in [Verdict::Match, Verdict::Mismatch] {
            let o = judge_with_approval(&s, &declaration(), &running_setting(), approve, |_| {
                RunResult::Verdict(body)
            });
            assert_eq!(
                o.label(),
                &Label::Pending(ThresholdId::new("t-13").unwrap())
            );
        }
    }

    /// 全て承認済みと判定されれば、比べの結果がそのまま OK・NG になる。
    #[test]
    fn injected_full_approval_maps_verdicts_to_ok_and_ng() {
        let s = Subject::from_parts("inject", &[], &["t-12", "t-13"]).unwrap();
        let all = |_: &ThresholdId| true;
        let ok = judge_with_approval(&s, &declaration(), &running_setting(), all, |_| {
            RunResult::Verdict(Verdict::Match)
        });
        let ng = judge_with_approval(&s, &declaration(), &running_setting(), all, |_| {
            RunResult::Verdict(Verdict::Mismatch)
        });
        assert_eq!(ok.label(), &Label::Ok);
        assert_eq!(ng.label(), &Label::Ng);
    }
}
