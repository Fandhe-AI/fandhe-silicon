//! 2 つの値の比から「同等」「差がある」の区分を出す（T-4）。
//!
//! 呼び手（run 間の集計の出力側）が、`across` や `stats` の中央値を分子と分母として渡し、
//! 試験の TASK の ID とあわせて区分を得る。純粋な関数で、時計にも記録にも触れない
//! （`record::RunRecord` の JSON は変えない）。
//!
//! 閾値は整数比（13 / 10）で持ち、判定に浮動小数点の割り算を使わない。
//! 適用範囲は [`RATIO_SCOPE_TASKS`] の 1 か所だけに置く。一覧に無い ID は範囲外として
//! 判定保留に倒す（fail-closed）。範囲を外から差し替える公開の口は無い。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-27（ポインタのみ）。

use crate::PerfError;
use crate::record::RefId;
use crate::stats::MAX_EXACT_NS;

/// 区分の閾値 1.3 の分子。
pub const DIFF_RATIO_NUM: u64 = 13;
/// 区分の閾値 1.3 の分母。
pub const DIFF_RATIO_DEN: u64 = 10;

/// 閾値を整数比（分子, 分母）で返す。
pub const fn diff_ratio() -> (u64, u64) {
    (DIFF_RATIO_NUM, DIFF_RATIO_DEN)
}

/// 閾値を表示用の `f64`（1.3）で返す。判定には使わない。
pub fn diff_ratio_f64() -> f64 {
    DIFF_RATIO_NUM as f64 / DIFF_RATIO_DEN as f64
}

/// 区分を適用する範囲の TASK の ID（唯一の置き場）。一覧に無い ID は範囲外になる。
pub const RATIO_SCOPE_TASKS: [&str; 0] = [];

/// 区分の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RatioClass {
    /// 比が閾値未満。
    Equivalent,
    /// 比が閾値以上（ちょうど閾値も含む）。
    Differs,
    /// 判定保留。理由は [`RatioClass::reason_name`]。
    Pending,
}

impl RatioClass {
    /// 出力用の綴り（英小文字 ASCII）。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Equivalent => "equivalent",
            Self::Differs => "differs",
            Self::Pending => "pending",
        }
    }

    /// 判定保留の理由の綴り。保留以外は `None`。
    pub const fn reason_name(self) -> Option<&'static str> {
        match self {
            Self::Pending => Some("out-of-scope"),
            _ => None,
        }
    }
}

/// 値を 4 倍の整数にする。有限・0 以上・上限以下・1/4 刻みのときだけ `Ok`。
fn quarter_units(x: f64) -> Result<u64, PerfError> {
    if !x.is_finite() || x < 0.0 {
        return Err(PerfError::InvalidValue);
    }
    if x > MAX_EXACT_NS as f64 {
        return Err(PerfError::ValueTooLarge);
    }
    // 4 倍は 2 の冪の拡大なので f64 で正確。
    let q = x * 4.0;
    if q.fract() != 0.0 {
        return Err(PerfError::InvalidValue);
    }
    Ok(q as u64)
}

/// 範囲の判定を差し替えられる本体。入力の検査を範囲の判定より先に行う。
fn classify_with_scope(
    numerator: f64,
    denominator: f64,
    task: &RefId,
    in_scope: impl Fn(&RefId) -> bool,
) -> Result<RatioClass, PerfError> {
    let a = quarter_units(numerator)?;
    let b = quarter_units(denominator)?;
    if b == 0 {
        return Err(PerfError::ZeroDenominator);
    }
    if !in_scope(task) {
        return Ok(RatioClass::Pending);
    }
    let lhs = u128::from(a)
        .checked_mul(u128::from(DIFF_RATIO_DEN))
        .ok_or(PerfError::ValueTooLarge)?;
    let rhs = u128::from(b)
        .checked_mul(u128::from(DIFF_RATIO_NUM))
        .ok_or(PerfError::ValueTooLarge)?;
    Ok(if lhs >= rhs {
        RatioClass::Differs
    } else {
        RatioClass::Equivalent
    })
}

/// 分子 ÷ 分母 を閾値と比べて区分する。片側だけの比較で、比の向きは呼び手が選ぶ。
///
/// 範囲外の TASK の ID は [`RatioClass::Pending`]。不正な入力（有限でない・負・分母 0・
/// 上限超え・1/4 刻みに乗らない値）は範囲外でも `Err`。
pub fn classify(numerator: f64, denominator: f64, task: &RefId) -> Result<RatioClass, PerfError> {
    classify_with_scope(numerator, denominator, task, |t| {
        RATIO_SCOPE_TASKS.contains(&t.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(n: f64, d: f64) -> Result<RatioClass, PerfError> {
        let t = RefId::parse("TASK-27").unwrap();
        classify_with_scope(n, d, &t, |_| true)
    }

    #[test]
    fn boundary_exactly_threshold_differs() {
        assert_eq!(run(13.0, 10.0), Ok(RatioClass::Differs));
        assert_eq!(run(26.0, 20.0), Ok(RatioClass::Differs));
        assert_eq!(run(130.0, 100.0), Ok(RatioClass::Differs));
    }

    #[test]
    fn just_below_and_above() {
        assert_eq!(run(12.75, 10.0), Ok(RatioClass::Equivalent));
        assert_eq!(run(129.0, 100.0), Ok(RatioClass::Equivalent));
        assert_eq!(run(131.0, 100.0), Ok(RatioClass::Differs));
    }

    #[test]
    fn one_sided() {
        assert_eq!(run(1.0, 2.0), Ok(RatioClass::Equivalent));
        assert_eq!(run(0.0, 5.0), Ok(RatioClass::Equivalent));
    }

    #[test]
    fn invalid_inputs_err() {
        assert_eq!(run(f64::NAN, 1.0), Err(PerfError::InvalidValue));
        assert_eq!(run(1.0, f64::INFINITY), Err(PerfError::InvalidValue));
        assert_eq!(run(-1.0, 1.0), Err(PerfError::InvalidValue));
        assert_eq!(run(1.0, 0.0), Err(PerfError::ZeroDenominator));
        assert_eq!(run(1.1, 1.0), Err(PerfError::InvalidValue));
        assert_eq!(run(1.0e300, 1.0), Err(PerfError::ValueTooLarge));
    }

    #[test]
    fn out_of_scope_is_pending_but_inputs_checked_first() {
        let t = RefId::parse("TASK-27").unwrap();
        assert_eq!(
            classify_with_scope(100.0, 1.0, &t, |_| false),
            Ok(RatioClass::Pending)
        );
        assert_eq!(
            classify_with_scope(1.0, 0.0, &t, |_| false),
            Err(PerfError::ZeroDenominator)
        );
    }

    #[test]
    fn max_values_do_not_overflow() {
        let m = MAX_EXACT_NS as f64;
        assert_eq!(run(m, 1.0), Ok(RatioClass::Differs));
        assert_eq!(run(m, m), Ok(RatioClass::Equivalent));
    }
}
