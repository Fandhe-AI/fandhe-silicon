//! 経路の報告と結果の値の型。
//!
//! 下-2 の入口が返す値に使う。どの経路が選ばれたか、使えない理由は何か、
//! f32 の非正規化数の扱いを報告として持つ。

use crate::route::{OpId, Route, UnavailableReason};
use fandhe_silicon_core::capability::CapabilityLevel;

/// f32 の非正規化数の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum F32DenormalBehavior {
    /// 非正規化数を保つ。
    Preserve,
    /// 非正規化数を 0 に落とす。
    FlushToZero,
}

/// f32 の非正規化数の扱いの根拠。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum F32DenormalBasis {
    /// 実行して観察した。
    Observed,
}

/// f32 の非正規化数の項。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct F32DenormalReport {
    /// 扱い。
    pub behavior: F32DenormalBehavior,
    /// 扱いの根拠。
    pub basis: F32DenormalBasis,
    /// 文書で示された扱い。無ければ `None`。
    pub documented: Option<F32DenormalBehavior>,
}

/// 経路の報告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct RouteReport {
    /// 対象の操作。
    pub op: OpId,
    /// 選ばれた経路。`None` は経路が無いことを表す。
    pub route: Option<Route>,
    /// device の機能をそのまま使えない理由。
    pub native_reason: Option<UnavailableReason>,
    /// 代わりの実行が使えない理由。
    pub fallback_reason: Option<UnavailableReason>,
    /// 観察した能力の段階。
    pub observed_level: Option<CapabilityLevel>,
    /// f32 の非正規化数の項。FB-56。
    pub f32_denormal: Option<F32DenormalReport>,
}

/// 入口が返す結果の値。各場合は報告を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Outcome {
    /// 実行した場合。
    Ran {
        /// 経路の報告。
        report: RouteReport,
    },
    /// 実行できなかった場合。
    Unavailable {
        /// 経路の報告。
        report: RouteReport,
    },
}

impl Outcome {
    /// どちらの場合でも経路の報告を返す。
    pub fn report(&self) -> &RouteReport {
        match self {
            Outcome::Ran { report } | Outcome::Unavailable { report } => report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RouteReport {
        RouteReport {
            op: OpId::F16Fma,
            route: Some(Route::Cpu),
            native_reason: Some(UnavailableReason::CapabilityUnknown),
            fallback_reason: None,
            observed_level: Some(CapabilityLevel::Unsupported),
            f32_denormal: Some(F32DenormalReport {
                behavior: F32DenormalBehavior::Preserve,
                basis: F32DenormalBasis::Observed,
                documented: None,
            }),
        }
    }

    #[test]
    fn outcome_report_is_same_in_both_cases() {
        let report = sample();
        assert_eq!(*Outcome::Ran { report }.report(), report);
        assert_eq!(*Outcome::Unavailable { report }.report(), report);
    }

    #[test]
    fn route_reports_compare_by_value() {
        let a = sample();
        assert_eq!(a, sample());
        let mut b = sample();
        b.route = None;
        assert_ne!(a, b);
        let mut b = sample();
        b.native_reason = None;
        assert_ne!(a, b);
        let mut b = sample();
        b.observed_level = None;
        assert_ne!(a, b);
        let mut b = sample();
        b.f32_denormal = None;
        assert_ne!(a, b);
    }
}
