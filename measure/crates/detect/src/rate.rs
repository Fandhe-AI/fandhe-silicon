//! 件数から検出率の下限・上限とラベルを出す純粋関数の置き場。
//!
//! 浮動小数を使わず、整数の交差の積で閾値と比べる。計算があふれたときは panic せず
//! `DetectError::Overflow` を返す。`classify` の分類の列を受け、`judge_cell` がセルのラベルを出す。

use std::collections::BTreeMap;
use std::fmt;

use crate::DetectError;
use crate::classify::MutantClass;

/// 約分しない分数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction {
    /// 分子。
    pub numerator: u64,
    /// 分母。
    pub denominator: u64,
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

/// 検出率の閾値。定数で、呼び手は読めるが変えられない。判定の関数は必ずこれを読む。
pub const DETECTION_THRESHOLD: Fraction = Fraction {
    numerator: 9,
    denominator: 10,
};

/// セルの分母の最小件数。定数で、呼び手は読めるが変えられない。
pub const MIN_CELL_DENOMINATOR: u64 = 10;

/// 分母が最小件数に満たないときの判定不能の理由。
pub const REASON_FEWER_THAN_MIN: &str = "fewer than 10 mutants in the denominator";

/// 機材が接続されていないときの判定不能の理由。
pub const REASON_NOT_CONNECTED: &str = "device not connected";

/// 下限が閾値に届かず上限が閾値以上のときの判定不能の理由。
pub const REASON_BETWEEN_BOUNDS: &str = "bounds straddle the threshold";

/// 検出率の下限と上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// 下限（検出 / 分母）。
    pub lower: Fraction,
    /// 上限（（検出 + 判定不能）/ 分母）。
    pub upper: Fraction,
}

/// 検出率のラベル。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLabel {
    /// 下限が閾値以上。
    Ok,
    /// 上限が閾値未満。
    Ng,
    /// 判定できない。
    Indeterminate {
        /// 判定不能の理由（固定の英語の文字列）。
        reason: &'static str,
    },
}

/// 検出・見逃し・判定不能の件数から下限・上限を出す。分母は 3 つの和で、0 なら `ZeroDenominator`。
pub fn bounds(detected: u64, missed: u64, indeterminate: u64) -> Result<Bounds, DetectError> {
    let n = detected
        .checked_add(missed)
        .and_then(|v| v.checked_add(indeterminate))
        .ok_or(DetectError::Overflow)?;
    if n == 0 {
        return Err(DetectError::ZeroDenominator);
    }
    let upper = detected
        .checked_add(indeterminate)
        .ok_or(DetectError::Overflow)?;
    Ok(Bounds {
        lower: Fraction {
            numerator: detected,
            denominator: n,
        },
        upper: Fraction {
            numerator: upper,
            denominator: n,
        },
    })
}

/// 下限・上限から閾値との比較でラベルを出す。下限が閾値以上なら `Ok`、上限が閾値未満なら `Ng`、ほかは判定不能。
///
/// 分母が 0 の `Bounds` は `ZeroDenominator`、積があふれたら `Overflow` を返す。
pub fn label(bounds: &Bounds) -> Result<RateLabel, DetectError> {
    if bounds.lower.denominator == 0 || bounds.upper.denominator == 0 {
        return Err(DetectError::ZeroDenominator);
    }
    let t = DETECTION_THRESHOLD;
    if at_least(bounds.lower, t)? {
        return Ok(RateLabel::Ok);
    }
    if !at_least(bounds.upper, t)? {
        return Ok(RateLabel::Ng);
    }
    Ok(RateLabel::Indeterminate {
        reason: REASON_BETWEEN_BOUNDS,
    })
}

/// `x >= t` を交差の積で比べる。
fn at_least(x: Fraction, t: Fraction) -> Result<bool, DetectError> {
    let l = x
        .numerator
        .checked_mul(t.denominator)
        .ok_or(DetectError::Overflow)?;
    let r = t
        .numerator
        .checked_mul(x.denominator)
        .ok_or(DetectError::Overflow)?;
    Ok(l >= r)
}

/// セル 1 つの集計結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellResult {
    /// 検出の件数。
    pub detected: u64,
    /// 見逃しの件数。
    pub missed: u64,
    /// 判定不能の件数。
    pub indeterminate: u64,
    /// 型で拒否の件数（分母に入れない）。
    pub type_rejected: u64,
    /// 等価の件数（分母に入れない）。
    pub equivalent: u64,
    /// 流していない件数を理由の昇順に並べたもの（分母に入れない）。
    pub not_run: Vec<(String, u64)>,
    /// 下限・上限。分母が 0 より大きいときだけ `Some`。
    pub bounds: Option<Bounds>,
    /// セルのラベル。
    pub label: RateLabel,
}

/// 分類の列からセルのラベルを出す。
///
/// 判定の順は、未接続 → 分母が最小件数未満 → 下限・上限。
/// 型で拒否・等価・流していないは分母に入れず、件数だけ返す。
///
/// spec との対応: `MAINT-27`（ポインタのみ）。
pub fn judge_cell(classes: &[MutantClass], connected: bool) -> Result<CellResult, DetectError> {
    let (mut detected, mut missed, mut indeterminate) = (0u64, 0u64, 0u64);
    let (mut type_rejected, mut equivalent) = (0u64, 0u64);
    let mut not_run: BTreeMap<String, u64> = BTreeMap::new();
    for c in classes {
        let slot = match c {
            MutantClass::Detected => &mut detected,
            MutantClass::Missed => &mut missed,
            MutantClass::Indeterminate => &mut indeterminate,
            MutantClass::TypeRejected => &mut type_rejected,
            MutantClass::Equivalent => &mut equivalent,
            MutantClass::NotRun { reason } => not_run.entry(reason.clone()).or_insert(0),
        };
        *slot = slot.checked_add(1).ok_or(DetectError::Overflow)?;
    }
    let denominator = detected
        .checked_add(missed)
        .and_then(|v| v.checked_add(indeterminate))
        .ok_or(DetectError::Overflow)?;
    let bounds = if denominator > 0 {
        Some(self::bounds(detected, missed, indeterminate)?)
    } else {
        None
    };
    let label = if !connected {
        RateLabel::Indeterminate {
            reason: REASON_NOT_CONNECTED,
        }
    } else if denominator < MIN_CELL_DENOMINATOR {
        RateLabel::Indeterminate {
            reason: REASON_FEWER_THAN_MIN,
        }
    } else {
        match &bounds {
            Some(b) => self::label(b)?,
            None => return Err(DetectError::ZeroDenominator),
        }
    };
    Ok(CellResult {
        detected,
        missed,
        indeterminate,
        type_rejected,
        equivalent,
        not_run: not_run.into_iter().collect(),
        bounds,
        label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: u64, d: u64) -> Fraction {
        Fraction {
            numerator: n,
            denominator: d,
        }
    }

    fn many(c: MutantClass, n: usize) -> Vec<MutantClass> {
        vec![c; n]
    }

    fn nr(r: &str) -> MutantClass {
        MutantClass::NotRun {
            reason: r.to_string(),
        }
    }

    #[test]
    fn constants_are_fixed() {
        assert_eq!(DETECTION_THRESHOLD, f(9, 10));
        assert_eq!(MIN_CELL_DENOMINATOR, 10);
    }

    #[test]
    fn bounds_values() {
        let b = bounds(10, 0, 0).unwrap();
        assert_eq!((b.lower, b.upper), (f(10, 10), f(10, 10)));
        let b = bounds(0, 10, 0).unwrap();
        assert_eq!((b.lower, b.upper), (f(0, 10), f(0, 10)));
        let b = bounds(3, 4, 3).unwrap();
        assert_eq!((b.lower, b.upper), (f(3, 10), f(6, 10)));
    }

    #[test]
    fn bounds_errors() {
        assert_eq!(bounds(0, 0, 0), Err(DetectError::ZeroDenominator));
        assert_eq!(bounds(u64::MAX, 1, 0), Err(DetectError::Overflow));
    }

    #[test]
    fn label_lower_exactly_threshold_is_ok() {
        assert_eq!(label(&bounds(9, 1, 0).unwrap()), Ok(RateLabel::Ok));
    }

    #[test]
    fn label_upper_exactly_threshold_is_not_ng() {
        let want = RateLabel::Indeterminate {
            reason: REASON_BETWEEN_BOUNDS,
        };
        assert_eq!(label(&bounds(8, 1, 1).unwrap()), Ok(want));
        assert_eq!(label(&bounds(8, 0, 2).unwrap()), Ok(want));
    }

    #[test]
    fn label_upper_below_threshold_is_ng() {
        assert_eq!(label(&bounds(89, 11, 0).unwrap()), Ok(RateLabel::Ng));
    }

    #[test]
    fn label_errors() {
        assert_eq!(
            label(&bounds(u64::MAX, 0, 0).unwrap()),
            Err(DetectError::Overflow)
        );
        let zero = Bounds {
            lower: f(0, 0),
            upper: f(0, 10),
        };
        assert_eq!(label(&zero), Err(DetectError::ZeroDenominator));
    }

    #[test]
    fn cell_not_connected_comes_first() {
        let r = judge_cell(&many(MutantClass::Detected, 10), false).unwrap();
        assert_eq!(
            r.label,
            RateLabel::Indeterminate {
                reason: REASON_NOT_CONNECTED
            }
        );
        assert!(r.bounds.is_some());
    }

    #[test]
    fn cell_fewer_than_min_before_bounds() {
        let r = judge_cell(&many(MutantClass::Detected, 9), true).unwrap();
        assert_eq!(
            r.label,
            RateLabel::Indeterminate {
                reason: REASON_FEWER_THAN_MIN
            }
        );
        assert_eq!(r.bounds, Some(bounds(9, 0, 0).unwrap()));
    }

    #[test]
    fn cell_ok_at_threshold() {
        let mut v = many(MutantClass::Detected, 9);
        v.push(MutantClass::Missed);
        let r = judge_cell(&v, true).unwrap();
        assert_eq!(r.label, RateLabel::Ok);
        let b = r.bounds.unwrap();
        assert_eq!((b.lower, b.upper), (f(9, 10), f(9, 10)));
    }

    #[test]
    fn cell_excluded_classes_only_counted() {
        let mut v = many(MutantClass::Detected, 10);
        v.push(MutantClass::TypeRejected);
        v.push(MutantClass::Equivalent);
        v.push(nr("x"));
        let r = judge_cell(&v, true).unwrap();
        assert_eq!(r.label, RateLabel::Ok);
        assert_eq!(r.bounds.unwrap().lower, f(10, 10));
        assert_eq!((r.type_rejected, r.equivalent), (1, 1));
        assert_eq!(r.not_run, vec![("x".to_string(), 1)]);
    }

    #[test]
    fn cell_not_run_sorted_by_reason() {
        let v = vec![nr("b"), nr("a"), nr("b")];
        let r = judge_cell(&v, true).unwrap();
        assert_eq!(r.not_run, vec![("a".to_string(), 1), ("b".to_string(), 2)]);
    }

    #[test]
    fn cell_empty_denominator_has_no_bounds() {
        let r = judge_cell(&[], true).unwrap();
        assert_eq!(r.bounds, None);
        assert_eq!(
            r.label,
            RateLabel::Indeterminate {
                reason: REASON_FEWER_THAN_MIN
            }
        );
        let r = judge_cell(&[], false).unwrap();
        assert_eq!(r.bounds, None);
        assert_eq!(
            r.label,
            RateLabel::Indeterminate {
                reason: REASON_NOT_CONNECTED
            }
        );
    }
}
