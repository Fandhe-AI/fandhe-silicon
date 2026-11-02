//! 経路の選び方と報告の組み立ての内部の関数。
//!
//! `entry` の 4 つの入口が、能力の問合せの値と効く opt-in の印から経路の届き方を決め、
//! 報告を組み立てるために呼ぶ。crate の外には公開しない。

use crate::opt_in::OptIn;
use crate::report::{F32DenormalBasis, F32DenormalBehavior, F32DenormalReport, RouteReport};
use crate::route::{OpId, Route, UnavailableReason};
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};

/// 呼び出しごとの上書きがあればそれを丸ごと、なければ既定の印を効く印にする。
pub(crate) fn effective_opt_in(default: OptIn, call: Option<OptIn>) -> OptIn {
    match call {
        Some(o) => o,
        None => default,
    }
}

/// 選び方の入力。操作・効く印・能力の問合せの値。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionInput {
    pub(crate) op: OpId,
    pub(crate) opt_in: OptIn,
    pub(crate) query: CapabilityQuery,
}

/// 選び方の出力。経路・2 つの理由・f32 の非正規化数の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionOutput {
    pub(crate) route: Option<Route>,
    pub(crate) native_reason: Option<UnavailableReason>,
    pub(crate) fallback_reason: Option<UnavailableReason>,
    pub(crate) f32_denormal: Option<F32DenormalBehavior>,
}

/// 入口が選び方に渡す入力を組み立てる。
pub(crate) fn selection_input(op: OpId, opt_in: OptIn, query: CapabilityQuery) -> SelectionInput {
    SelectionInput { op, opt_in, query }
}

/// 能力の段階と効く印から、経路の届き方を決める。
pub(crate) fn choose(input: &SelectionInput) -> SelectionOutput {
    let native_reason = match input.query.level {
        CapabilityLevel::S2
        | CapabilityLevel::S3
        | CapabilityLevel::S4
        | CapabilityLevel::S5
        | CapabilityLevel::S6 => {
            return SelectionOutput {
                route: Some(Route::Native),
                native_reason: None,
                fallback_reason: None,
                f32_denormal: None,
            };
        }
        CapabilityLevel::Unsupported => {
            UnavailableReason::CapabilityUnsupported(input.query.unsupported_reason)
        }
        _ => UnavailableReason::CapabilityUnknown,
    };
    let (route, fallback_reason) = if input.opt_in.allow_gpu_emulated {
        (Some(Route::GpuEmulated), None)
    } else if input.opt_in.allow_cpu {
        (Some(Route::Cpu), None)
    } else {
        (None, Some(UnavailableReason::OptInNotGiven))
    };
    SelectionOutput {
        route,
        native_reason: Some(native_reason),
        fallback_reason,
        f32_denormal: None,
    }
}

/// 入力と選び方の出力から報告を組み立てる。
pub(crate) fn build_report(input: &SelectionInput, output: &SelectionOutput) -> RouteReport {
    let has_f32_item = matches!(input.op, OpId::SubgroupSumF32 | OpId::AtomicAdd);
    let f32_denormal = match output.f32_denormal {
        Some(behavior) if has_f32_item && output.route.is_some() => Some(F32DenormalReport {
            behavior,
            basis: F32DenormalBasis::Observed,
            documented: None,
        }),
        _ => None,
    };
    RouteReport {
        op: input.op,
        route: output.route,
        native_reason: output.native_reason,
        fallback_reason: output.fallback_reason,
        observed_level: Some(input.query.level),
        f32_denormal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_silicon_core::error::UnsupportedReason;

    const fn opt(g: bool, c: bool) -> OptIn {
        OptIn {
            allow_gpu_emulated: g,
            allow_cpu: c,
        }
    }

    const ALL_OPT: [OptIn; 4] = [
        opt(false, false),
        opt(true, false),
        opt(false, true),
        opt(true, true),
    ];

    fn query(level: CapabilityLevel, reason: Option<UnsupportedReason>) -> CapabilityQuery {
        CapabilityQuery {
            level,
            unsupported_reason: reason,
            evidence: None,
        }
    }

    #[test]
    fn call_opt_in_replaces_default_whole() {
        let default = opt(true, false);
        let over = opt(false, true);
        assert_eq!(effective_opt_in(default, Some(over)), over);
        assert_eq!(effective_opt_in(default, None), default);
    }

    #[test]
    fn fallback_order_for_unreachable_native() {
        let cases = [
            query(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NotSupported),
            ),
            query(CapabilityLevel::Unsupported, None),
            query(CapabilityLevel::S0, None),
            query(CapabilityLevel::S1, None),
        ];
        for q in cases {
            for o in ALL_OPT {
                let out = choose(&selection_input(OpId::F16Fma, o, q));
                let expected = match (o.allow_gpu_emulated, o.allow_cpu) {
                    (true, _) => Some(Route::GpuEmulated),
                    (false, true) => Some(Route::Cpu),
                    (false, false) => None,
                };
                assert_eq!(out.route, expected);
                assert_eq!(
                    out.fallback_reason,
                    if expected.is_none() {
                        Some(UnavailableReason::OptInNotGiven)
                    } else {
                        None
                    }
                );
                assert_eq!(out.f32_denormal, None);
            }
        }
    }

    #[test]
    fn native_reason_follows_level() {
        for reason in [Some(UnsupportedReason::NotSupported), None] {
            let out = choose(&selection_input(
                OpId::AtomicAdd,
                opt(false, false),
                query(CapabilityLevel::Unsupported, reason),
            ));
            assert_eq!(
                out.native_reason,
                Some(UnavailableReason::CapabilityUnsupported(reason))
            );
        }
        for level in [CapabilityLevel::S0, CapabilityLevel::S1] {
            let out = choose(&selection_input(
                OpId::AtomicAdd,
                opt(true, true),
                query(level, None),
            ));
            assert_eq!(
                out.native_reason,
                Some(UnavailableReason::CapabilityUnknown)
            );
        }
    }

    #[test]
    fn native_levels_choose_native_for_all_opt_ins() {
        for level in [CapabilityLevel::S2, CapabilityLevel::S6] {
            for o in ALL_OPT {
                let out = choose(&selection_input(
                    OpId::SubgroupSumU32,
                    o,
                    query(level, None),
                ));
                assert_eq!(out.route, Some(Route::Native));
                assert_eq!(out.native_reason, None);
                assert_eq!(out.fallback_reason, None);
            }
        }
    }

    #[test]
    fn report_f32_item_only_for_f32_ops_with_route() {
        let q = query(CapabilityLevel::Unsupported, None);
        let with_route = SelectionOutput {
            route: Some(Route::Cpu),
            native_reason: None,
            fallback_reason: None,
            f32_denormal: Some(F32DenormalBehavior::Preserve),
        };
        let expected = Some(F32DenormalReport {
            behavior: F32DenormalBehavior::Preserve,
            basis: F32DenormalBasis::Observed,
            documented: None,
        });
        for op in [
            OpId::F16Fma,
            OpId::SubgroupSumU32,
            OpId::SubgroupSumF32,
            OpId::AtomicAdd,
        ] {
            let input = selection_input(op, opt(false, true), q);
            let has = matches!(op, OpId::SubgroupSumF32 | OpId::AtomicAdd);
            let r = build_report(&input, &with_route);
            assert_eq!(r.f32_denormal, if has { expected } else { None });
            assert_eq!(r.op, op);
            assert_eq!(r.observed_level, Some(CapabilityLevel::Unsupported));
            let none_route = SelectionOutput {
                route: None,
                ..with_route
            };
            assert_eq!(build_report(&input, &none_route).f32_denormal, None);
        }
    }
}
