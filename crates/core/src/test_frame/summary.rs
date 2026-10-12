//! 組み合わせごとの結果のラベルの集計と、その表の組み立て。
//!
//! 試験の側（または試験を束ねる呼び出し側）が集めた [`Outcome`] の列から、7 通りの組み合わせの行ごとに
//! 5 種のラベルの件数を数える。`Outcome` の getter（`declared_combinations`・`ran_on`・`label`）だけを入力にする純粋な計算で、
//! 環境・ファイル・標準出力には触れない。ファイルや標準出力への書き出しと、複数の試験（別プロセスを含む）の
//! 結果を 1 つに集める経路は持たない。記録の組み立ては `record` が、同じ `Outcome` の上で行う。
//! 5 種の件数を常に持つので、`ok` 以外が表から消えない。走らせなかった結果は件数として見える。
//! spec TASK-23。

use super::combination::MachineCombination;
use super::label::LabelKind;
use super::outcome::Outcome;
use std::fmt;

/// 集計の失敗。入力の値は含めず、種類だけを表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryError {
    /// 件数が `usize` に収まらなくなった。
    CountOverflow,
}

impl SummaryError {
    /// 失敗の種類を英語の ASCII で返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            SummaryError::CountOverflow => "summary count overflowed",
        }
    }
}

impl fmt::Display for SummaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for SummaryError {}

/// 種類から件数の配列の添字を作る（網羅的な `match` なので範囲外にならない）。
const fn kind_index(kind: LabelKind) -> usize {
    match kind {
        LabelKind::Ok => 0,
        LabelKind::Ng => 1,
        LabelKind::Pending => 2,
        LabelKind::NotRun => 3,
        LabelKind::Indeterminate => 4,
    }
}

/// 組み合わせから行の配列の添字を作る（`MachineCombination::ALL` の順）。
const fn combination_index(c: MachineCombination) -> usize {
    match c {
        MachineCombination::MetalMacMini => 0,
        MachineCombination::VulkanNodeB => 1,
        MachineCombination::VulkanNodeA => 2,
        MachineCombination::VulkanAmd => 3,
        MachineCombination::VulkanIntel => 4,
        MachineCombination::CudaNodeB => 5,
        MachineCombination::CudaNodeA => 6,
    }
}

/// 5 種のラベルの件数。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LabelCounts {
    counts: [usize; 5],
}

impl LabelCounts {
    /// 種類ごとの件数。
    pub fn get(&self, kind: LabelKind) -> usize {
        self.counts[kind_index(kind)]
    }

    /// 5 種の合計。`usize` を超えるときは最大値で止める。
    pub fn total(&self) -> usize {
        self.counts.iter().fold(0usize, |a, c| a.saturating_add(*c))
    }

    /// 種類の件数を 1 足す。あふれたら `Err`（値は変えない）。
    fn bump(&mut self, kind: LabelKind) -> Result<(), SummaryError> {
        let slot = &mut self.counts[kind_index(kind)];
        *slot = slot.checked_add(1).ok_or(SummaryError::CountOverflow)?;
        Ok(())
    }
}

/// 組み合わせごとの集計。7 通りの行（`MachineCombination::ALL`）と全体の件数を持つ。
///
/// 数え方（`Outcome` 1 件につき）:
/// - 走らせた組み合わせの行に、そのラベルの種類を 1 件
/// - 宣言した組み合わせのうち走らせていない行に、`not-run` を 1 件
/// - 宣言していない行は数えない
/// - 全体には、ラベルの種類を 1 件
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    rows: [LabelCounts; 7],
    totals: LabelCounts,
}

impl Summary {
    /// 全て 0 件の集計を作る。
    pub fn new() -> Summary {
        Summary::default()
    }

    /// `Outcome` を 1 件足す。失敗したら集計は変わらない。
    pub fn record(&mut self, outcome: &Outcome) -> Result<(), SummaryError> {
        let mut next = self.clone();
        let ran = outcome.ran_on();
        if let Some(c) = ran {
            next.rows[combination_index(c)].bump(outcome.label().kind())?;
        }
        for c in outcome.declared_combinations() {
            if ran != Some(*c) {
                next.rows[combination_index(*c)].bump(LabelKind::NotRun)?;
            }
        }
        next.totals.bump(outcome.label().kind())?;
        *self = next;
        Ok(())
    }

    /// `Outcome` の列から集計を作る。
    pub fn from_outcomes(outcomes: &[Outcome]) -> Result<Summary, SummaryError> {
        let mut s = Summary::new();
        for o in outcomes {
            s.record(o)?;
        }
        Ok(s)
    }

    /// 組み合わせの行の件数。
    pub fn combination(&self, c: MachineCombination) -> &LabelCounts {
        &self.rows[combination_index(c)]
    }

    /// 全体の件数（`Outcome` 1 件につき 1 件）。
    pub fn totals(&self) -> &LabelCounts {
        &self.totals
    }

    /// `ng` が 1 件でもあるか（`Label::fails_test` の集計版）。
    pub fn has_ng(&self) -> bool {
        self.totals.get(LabelKind::Ng) > 0
    }
}

/// Markdown の表。見出し、区切り、`MachineCombination::ALL` の順の 7 行。
/// セルは固定のトークンと 10 進の数だけで、試験の ID・ビヘイビアの ID・理由・環境の値は出さない。
impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "| combination |")?;
        for k in LabelKind::ALL {
            write!(f, " {} |", k.name())?;
        }
        write!(f, "\n| --- |")?;
        for _ in LabelKind::ALL {
            write!(f, " ---: |")?;
        }
        for c in MachineCombination::ALL {
            write!(f, "\n| {} |", c.name())?;
            let row = self.combination(c);
            for k in LabelKind::ALL {
                write!(f, " {} |", row.get(k))?;
            }
        }
        Ok(())
    }
}
