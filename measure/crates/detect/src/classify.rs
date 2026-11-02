//! 変異体 1 件の入力から分類を出す純粋関数（`measure-detect` の中核）。
//!
//! 呼び手は変異体ごとに [`MutantInput`] を作り、[`classify`] で [`MutantClass`] を得る。
//! 不整合な入力（空・重複・欠け・余分）は黙って分類せず [`DetectError`] で返す。

use crate::DetectError;
use std::collections::BTreeSet;

/// 検出の手段の番号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Means {
    /// 手段の番号 D1。
    D1,
    /// 手段の番号 D2。
    D2,
    /// 手段の番号 D3。
    D3,
    /// 手段の番号 D4。
    D4,
    /// 手段の番号 D5。
    D5,
    /// 手段の番号 D6。
    D6,
    /// 手段の番号 D7。
    D7,
    /// 手段の番号 D8。
    D8,
    /// 手段の番号 D9。
    D9,
}

impl Means {
    /// 全 9 値（番号順）。
    pub const ALL: [Self; 9] = [
        Self::D1,
        Self::D2,
        Self::D3,
        Self::D4,
        Self::D5,
        Self::D6,
        Self::D7,
        Self::D8,
        Self::D9,
    ];
}

/// (手段, 試験群) の組。試験群は意味を持たない自由な文字列。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pair {
    /// 手段の番号。
    pub means: Means,
    /// 試験群の名前。
    pub group: String,
}

/// 1 つの組の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairResult {
    /// 陽性。
    Positive,
    /// 陰性。
    Negative,
    /// 完走しない。
    NotCompleted,
}

/// 変異体 1 件の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutantInput {
    /// 型検査で拒否された。
    TypeRejected,
    /// 等価な変異体。
    Equivalent,
    /// 流していない。
    NotRun {
        /// 流していない理由の名前。
        reason: String,
    },
    /// 流した。
    Ran {
        /// 数える組。
        counted: Vec<Pair>,
        /// 組ごとの結果。
        results: Vec<(Pair, PairResult)>,
    },
}

/// 変異体 1 件の分類。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutantClass {
    /// 検出された。
    Detected,
    /// 見逃された。
    Missed,
    /// 判定不能。
    Indeterminate,
    /// 型検査で拒否された。
    TypeRejected,
    /// 等価。
    Equivalent,
    /// 流していない。
    NotRun {
        /// 流していない理由の名前。
        reason: String,
    },
}

impl MutantClass {
    /// 分母に入る分類（`Detected`・`Missed`・`Indeterminate`）なら真。
    pub fn in_denominator(&self) -> bool {
        matches!(self, Self::Detected | Self::Missed | Self::Indeterminate)
    }
}

/// 入力 1 件を分類する。検査は空 → 数える組の重複 → 結果の余分・重複 → 欠けの順で行う。
///
/// 数える組の外の結果を黙って捨てない。spec との対応: `MAINT-26`（ポインタのみ）。
pub fn classify(input: &MutantInput) -> Result<MutantClass, DetectError> {
    let (counted, results) = match input {
        MutantInput::TypeRejected => return Ok(MutantClass::TypeRejected),
        MutantInput::Equivalent => return Ok(MutantClass::Equivalent),
        MutantInput::NotRun { reason } => {
            return Ok(MutantClass::NotRun {
                reason: reason.clone(),
            });
        }
        MutantInput::Ran { counted, results } => (counted, results),
    };
    if counted.is_empty() {
        return Err(DetectError::EmptyCounted);
    }
    let mut counted_set: BTreeSet<&Pair> = BTreeSet::new();
    for p in counted {
        if !counted_set.insert(p) {
            return Err(DetectError::DuplicateCounted);
        }
    }
    let mut seen: BTreeSet<&Pair> = BTreeSet::new();
    for (p, _) in results {
        if !counted_set.contains(p) {
            return Err(DetectError::UnexpectedResult);
        }
        if !seen.insert(p) {
            return Err(DetectError::DuplicateResult);
        }
    }
    if seen.len() != counted_set.len() {
        return Err(DetectError::MissingResult);
    }
    let any = |want: PairResult| results.iter().any(|(_, r)| *r == want);
    Ok(if any(PairResult::Positive) {
        MutantClass::Detected
    } else if any(PairResult::NotCompleted) {
        MutantClass::Indeterminate
    } else {
        MutantClass::Missed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(means: Means, group: &str) -> Pair {
        Pair {
            means,
            group: group.to_string(),
        }
    }

    fn ran(counted: Vec<Pair>, results: Vec<(Pair, PairResult)>) -> MutantInput {
        MutantInput::Ran { counted, results }
    }

    fn two() -> Vec<Pair> {
        vec![pair(Means::D1, "g1"), pair(Means::D2, "g2")]
    }

    #[test]
    fn non_run_states_pass_through() {
        assert_eq!(
            classify(&MutantInput::TypeRejected),
            Ok(MutantClass::TypeRejected)
        );
        assert_eq!(
            classify(&MutantInput::Equivalent),
            Ok(MutantClass::Equivalent)
        );
        assert_eq!(
            classify(&MutantInput::NotRun { reason: "r".into() }),
            Ok(MutantClass::NotRun { reason: "r".into() })
        );
    }

    #[test]
    fn positive_wins_over_not_completed() {
        let c = two();
        let r = vec![
            (c[0].clone(), PairResult::Positive),
            (c[1].clone(), PairResult::NotCompleted),
        ];
        assert_eq!(classify(&ran(c, r)), Ok(MutantClass::Detected));
    }

    #[test]
    fn negative_and_not_completed_is_indeterminate() {
        let c = two();
        let r = vec![
            (c[0].clone(), PairResult::Negative),
            (c[1].clone(), PairResult::NotCompleted),
        ];
        assert_eq!(classify(&ran(c, r)), Ok(MutantClass::Indeterminate));
    }

    #[test]
    fn all_negative_is_missed() {
        let c = two();
        let r = vec![
            (c[0].clone(), PairResult::Negative),
            (c[1].clone(), PairResult::Negative),
        ];
        assert_eq!(classify(&ran(c, r)), Ok(MutantClass::Missed));
    }

    #[test]
    fn empty_counted_is_error() {
        assert_eq!(
            classify(&ran(vec![], vec![])),
            Err(DetectError::EmptyCounted)
        );
    }

    #[test]
    fn duplicate_counted_is_error() {
        let p = pair(Means::D1, "g1");
        let r = vec![(p.clone(), PairResult::Negative)];
        assert_eq!(
            classify(&ran(vec![p.clone(), p], r)),
            Err(DetectError::DuplicateCounted)
        );
    }

    #[test]
    fn missing_result_is_error() {
        let c = two();
        let r = vec![(c[0].clone(), PairResult::Negative)];
        assert_eq!(classify(&ran(c, r)), Err(DetectError::MissingResult));
    }

    #[test]
    fn unexpected_result_is_error() {
        let c = two();
        let r = vec![
            (c[0].clone(), PairResult::Negative),
            (c[1].clone(), PairResult::Negative),
            (pair(Means::D3, "g3"), PairResult::Positive),
        ];
        assert_eq!(classify(&ran(c, r)), Err(DetectError::UnexpectedResult));
    }

    #[test]
    fn duplicate_result_is_error() {
        let c = two();
        let r = vec![
            (c[0].clone(), PairResult::Negative),
            (c[0].clone(), PairResult::Negative),
        ];
        assert_eq!(classify(&ran(c, r)), Err(DetectError::DuplicateResult));
    }

    #[test]
    fn denominator_membership() {
        assert!(MutantClass::Detected.in_denominator());
        assert!(MutantClass::Missed.in_denominator());
        assert!(MutantClass::Indeterminate.in_denominator());
        assert!(!MutantClass::TypeRejected.in_denominator());
        assert!(!MutantClass::Equivalent.in_denominator());
        assert!(!MutantClass::NotRun { reason: "r".into() }.in_denominator());
    }

    #[test]
    fn means_all_has_nine_distinct_values() {
        let set: BTreeSet<Means> = Means::ALL.iter().copied().collect();
        assert_eq!(Means::ALL.len(), 9);
        assert_eq!(set.len(), 9);
    }
}
