//! 経路の選び方と報告の組み立ての内部の関数。
//!
//! `entry` の 4 つの入口が、能力の問合せの値と効く opt-in の印から経路の届き方を決め、
//! 報告を組み立てるために呼ぶ。経路の選び方は判定の関数を引数で受け、判定を求める操作では
//! 届いた経路に順に判定を求める。crate の外には公開しない。

use crate::opt_in::OptIn;
use crate::report::{F32DenormalBasis, F32DenormalBehavior, F32DenormalReport, RouteReport};
use crate::route::{OpId, Route, UnavailableReason};
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::error::ContractError;

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
    /// `Native` を候補に入れない印。
    #[cfg(feature = "fault-injection")]
    pub(crate) forced: bool,
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
    SelectionInput {
        op,
        opt_in,
        query,
        #[cfg(feature = "fault-injection")]
        forced: false,
    }
}

/// f32 の非正規化数の扱いの判定の結果。経路ごとに判定の関数が返す。
///
/// `denormal` の判定の関数が試しの実行の出力から作る（feature `fault-injection` の差し込みも値を作る）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DenormalJudgment {
    /// 非正規化数を保つ。
    Preserve,
    /// 非正規化数を 0 に流す。
    FlushToZero,
    /// 判定できない。
    Unknown,
}

/// 能力の段階と効く印から経路の候補を並べ、判定を求める操作では候補の順に判定の関数を呼んで経路を決める。
///
/// `SubgroupSumF32`・`AtomicAdd` 以外の操作では判定の関数を呼ばない。
/// 判定の関数が `Err` を返したときは、以降の候補を試さずそのまま返す。
pub(crate) fn select_route(
    input: &SelectionInput,
    mut judge: impl FnMut(Route) -> Result<DenormalJudgment, ContractError>,
) -> Result<SelectionOutput, ContractError> {
    let (native_reachable, mut native_reason) = match input.query.level {
        // 指定した操作では段階を見ずに Native を候補に入れない。
        #[cfg(feature = "fault-injection")]
        _ if input.forced => (false, Some(UnavailableReason::ForcedUnsupported)),
        CapabilityLevel::S2
        | CapabilityLevel::S3
        | CapabilityLevel::S4
        | CapabilityLevel::S5
        | CapabilityLevel::S6 => (true, None),
        CapabilityLevel::Unsupported => (
            false,
            Some(UnavailableReason::CapabilityUnsupported(
                input.query.unsupported_reason,
            )),
        ),
        _ => (false, Some(UnavailableReason::CapabilityUnknown)),
    };
    let candidates = [
        native_reachable.then_some(Route::Native),
        input
            .opt_in
            .allow_gpu_emulated
            .then_some(Route::GpuEmulated),
        input.opt_in.allow_cpu.then_some(Route::Cpu),
    ];
    let needs_judgment = matches!(input.op, OpId::SubgroupSumF32 | OpId::AtomicAdd);
    let mut chosen: Option<(Route, Option<F32DenormalBehavior>)> = None;
    for route in candidates.into_iter().flatten() {
        if !needs_judgment {
            chosen = Some((route, None));
            break;
        }
        match judge(route)? {
            DenormalJudgment::Preserve => {
                chosen = Some((route, Some(F32DenormalBehavior::Preserve)));
                break;
            }
            DenormalJudgment::FlushToZero => {
                chosen = Some((route, Some(F32DenormalBehavior::FlushToZero)));
                break;
            }
            DenormalJudgment::Unknown => {
                if route == Route::Native {
                    native_reason = Some(UnavailableReason::F32DenormalBehaviorUnknown);
                }
            }
        }
    }
    Ok(match chosen {
        Some((route, f32_denormal)) => SelectionOutput {
            route: Some(route),
            native_reason: if route == Route::Native {
                None
            } else {
                native_reason
            },
            fallback_reason: None,
            f32_denormal,
        },
        None => SelectionOutput {
            route: None,
            native_reason,
            fallback_reason: Some(
                if input.opt_in.allow_gpu_emulated || input.opt_in.allow_cpu {
                    UnavailableReason::F32DenormalBehaviorUnknown
                } else {
                    UnavailableReason::OptInNotGiven
                },
            ),
            f32_denormal: None,
        },
    })
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

    fn preserve(_: Route) -> Result<DenormalJudgment, ContractError> {
        Ok(DenormalJudgment::Preserve)
    }

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
                let out = select_route(&selection_input(OpId::F16Fma, o, q), preserve).unwrap();
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
            let out = select_route(
                &selection_input(
                    OpId::AtomicAdd,
                    opt(false, false),
                    query(CapabilityLevel::Unsupported, reason),
                ),
                preserve,
            )
            .unwrap();
            assert_eq!(
                out.native_reason,
                Some(UnavailableReason::CapabilityUnsupported(reason))
            );
        }
        for level in [CapabilityLevel::S0, CapabilityLevel::S1] {
            let out = select_route(
                &selection_input(OpId::AtomicAdd, opt(true, true), query(level, None)),
                preserve,
            )
            .unwrap();
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
                let out = select_route(
                    &selection_input(OpId::SubgroupSumU32, o, query(level, None)),
                    preserve,
                )
                .unwrap();
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

    use std::cell::RefCell;

    /// 呼ばれた経路を積みながら、経路ごとに決めた答えを返す判定の関数で選ぶ。
    fn run(
        op: OpId,
        o: OptIn,
        q: CapabilityQuery,
        answers: &[(Route, Result<DenormalJudgment, ContractError>)],
    ) -> (Result<SelectionOutput, ContractError>, Vec<Route>) {
        let called = RefCell::new(Vec::new());
        let out = select_route(&selection_input(op, o, q), |r| {
            called.borrow_mut().push(r);
            answers
                .iter()
                .find(|(k, _)| *k == r)
                .map(|(_, v)| v.clone())
                .unwrap_or(Ok(DenormalJudgment::Preserve))
        });
        (out, called.into_inner())
    }

    #[test]
    fn judged_native_is_chosen_with_behavior() {
        for (j, b) in [
            (DenormalJudgment::Preserve, F32DenormalBehavior::Preserve),
            (
                DenormalJudgment::FlushToZero,
                F32DenormalBehavior::FlushToZero,
            ),
        ] {
            let (out, called) = run(
                OpId::AtomicAdd,
                opt(false, false),
                query(CapabilityLevel::S2, None),
                &[(Route::Native, Ok(j))],
            );
            let out = out.unwrap();
            assert_eq!(out.route, Some(Route::Native));
            assert_eq!(out.native_reason, None);
            assert_eq!(out.fallback_reason, None);
            assert_eq!(out.f32_denormal, Some(b));
            assert_eq!(called, [Route::Native]);
        }
    }

    #[test]
    fn unknown_native_moves_to_allowed_fallback() {
        let (out, called) = run(
            OpId::SubgroupSumF32,
            opt(true, false),
            query(CapabilityLevel::S2, None),
            &[
                (Route::Native, Ok(DenormalJudgment::Unknown)),
                (Route::GpuEmulated, Ok(DenormalJudgment::FlushToZero)),
            ],
        );
        let out = out.unwrap();
        assert_eq!(out.route, Some(Route::GpuEmulated));
        assert_eq!(
            out.native_reason,
            Some(UnavailableReason::F32DenormalBehaviorUnknown)
        );
        assert_eq!(out.fallback_reason, None);
        assert_eq!(out.f32_denormal, Some(F32DenormalBehavior::FlushToZero));
        assert_eq!(called, [Route::Native, Route::GpuEmulated]);
    }

    #[test]
    fn unknown_native_without_opt_in_is_unavailable() {
        let (out, called) = run(
            OpId::AtomicAdd,
            opt(false, false),
            query(CapabilityLevel::S2, None),
            &[(Route::Native, Ok(DenormalJudgment::Unknown))],
        );
        let out = out.unwrap();
        assert_eq!(out.route, None);
        assert_eq!(
            out.native_reason,
            Some(UnavailableReason::F32DenormalBehaviorUnknown)
        );
        assert_eq!(out.fallback_reason, Some(UnavailableReason::OptInNotGiven));
        assert_eq!(called, [Route::Native]);
    }

    #[test]
    fn all_unknown_with_opt_in_is_unavailable() {
        // FB-8
        let (out, called) = run(
            OpId::SubgroupSumF32,
            opt(true, true),
            query(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NotSupported),
            ),
            &[
                (Route::GpuEmulated, Ok(DenormalJudgment::Unknown)),
                (Route::Cpu, Ok(DenormalJudgment::Unknown)),
            ],
        );
        let out = out.unwrap();
        assert_eq!(out.route, None);
        assert_eq!(
            out.native_reason,
            Some(UnavailableReason::CapabilityUnsupported(Some(
                UnsupportedReason::NotSupported
            )))
        );
        assert_eq!(
            out.fallback_reason,
            Some(UnavailableReason::F32DenormalBehaviorUnknown)
        );
        assert_eq!(called, [Route::GpuEmulated, Route::Cpu]);
    }

    #[test]
    fn judge_error_is_returned_unchanged() {
        let (out, called) = run(
            OpId::AtomicAdd,
            opt(true, true),
            query(CapabilityLevel::S2, None),
            &[(Route::Native, Err(ContractError::Timeout))],
        );
        assert_eq!(out, Err(ContractError::Timeout));
        assert_eq!(called, [Route::Native]);
    }

    #[test]
    fn judge_is_not_called_without_need() {
        let levels = [
            CapabilityLevel::S0,
            CapabilityLevel::S1,
            CapabilityLevel::S2,
            CapabilityLevel::S3,
            CapabilityLevel::S4,
            CapabilityLevel::S5,
            CapabilityLevel::S6,
            CapabilityLevel::Unsupported,
        ];
        for op in [OpId::F16Fma, OpId::SubgroupSumU32] {
            for level in levels {
                for o in ALL_OPT {
                    let (out, called) = run(op, o, query(level, None), &[]);
                    assert!(out.is_ok());
                    assert!(called.is_empty());
                }
            }
        }
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            for level in [CapabilityLevel::S0, CapabilityLevel::Unsupported] {
                let (_, called) = run(op, opt(false, false), query(level, None), &[]);
                assert!(called.is_empty());
            }
        }
    }

    /// 能力の値・印・判定の答えの全組み合わせを、候補を並べて先頭から当てる規則で数えて確かめる。
    mod combination_tests {
        use super::*;

        type Answer = Result<DenormalJudgment, ContractError>;
        type Outcome = (Result<SelectionOutput, ContractError>, Vec<Route>);

        const ROUTES: [Route; 3] = [Route::Native, Route::GpuEmulated, Route::Cpu];

        struct Case {
            op: OpId,
            opt_in: OptIn,
            query: CapabilityQuery,
            /// `Native`・`GpuEmulated`・`Cpu` の順の答え。判定しない操作は `None`。
            answers: Option<[Answer; 3]>,
        }

        fn queries() -> Vec<CapabilityQuery> {
            let mut v: Vec<CapabilityQuery> = [
                CapabilityLevel::S0,
                CapabilityLevel::S1,
                CapabilityLevel::S2,
                CapabilityLevel::S3,
                CapabilityLevel::S4,
                CapabilityLevel::S5,
                CapabilityLevel::S6,
            ]
            .into_iter()
            .map(|l| query(l, None))
            .collect();
            for r in [
                None,
                Some(UnsupportedReason::NoApi),
                Some(UnsupportedReason::NotSupported),
                Some(UnsupportedReason::UnknownFeature),
            ] {
                v.push(query(CapabilityLevel::Unsupported, r));
            }
            v
        }

        fn answer(i: usize) -> Answer {
            match i {
                0 => Ok(DenormalJudgment::Preserve),
                1 => Ok(DenormalJudgment::FlushToZero),
                2 => Ok(DenormalJudgment::Unknown),
                _ => Err(ContractError::Timeout),
            }
        }

        fn cases() -> Vec<Case> {
            let mut v = Vec::new();
            for q in queries() {
                for o in ALL_OPT {
                    for op in [OpId::F16Fma, OpId::SubgroupSumU32] {
                        v.push(Case {
                            op,
                            opt_in: o,
                            query: q,
                            answers: None,
                        });
                    }
                    for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
                        for n in 0..64 {
                            v.push(Case {
                                op,
                                opt_in: o,
                                query: q,
                                answers: Some([answer(n % 4), answer(n / 4 % 4), answer(n / 16)]),
                            });
                        }
                    }
                }
            }
            v
        }

        fn behavior(j: DenormalJudgment) -> Option<F32DenormalBehavior> {
            match j {
                DenormalJudgment::Preserve => Some(F32DenormalBehavior::Preserve),
                DenormalJudgment::FlushToZero => Some(F32DenormalBehavior::FlushToZero),
                DenormalJudgment::Unknown => None,
            }
        }

        /// 候補を並べ、先頭から判定の答えを当てた期待の結果と呼ばれた経路の並び。
        fn expected(c: &Case) -> Outcome {
            let reachable = [
                CapabilityLevel::S2,
                CapabilityLevel::S3,
                CapabilityLevel::S4,
                CapabilityLevel::S5,
                CapabilityLevel::S6,
            ]
            .contains(&c.query.level);
            let mut native_reason = if reachable {
                None
            } else if c.query.level == CapabilityLevel::Unsupported {
                Some(UnavailableReason::CapabilityUnsupported(
                    c.query.unsupported_reason,
                ))
            } else {
                Some(UnavailableReason::CapabilityUnknown)
            };
            let mut candidates = Vec::new();
            if reachable {
                candidates.push(Route::Native);
            }
            if c.opt_in.allow_gpu_emulated {
                candidates.push(Route::GpuEmulated);
            }
            if c.opt_in.allow_cpu {
                candidates.push(Route::Cpu);
            }
            let fallback_allowed = c.opt_in.allow_gpu_emulated || c.opt_in.allow_cpu;
            let mut called = Vec::new();
            let mut chosen: Option<(Route, Option<F32DenormalBehavior>)> = None;
            match &c.answers {
                None => chosen = candidates.first().map(|r| (*r, None)),
                Some(answers) => {
                    for r in &candidates {
                        called.push(*r);
                        let idx = ROUTES.iter().position(|x| x == r).unwrap();
                        match &answers[idx] {
                            Err(e) => return (Err(e.clone()), called),
                            Ok(DenormalJudgment::Unknown) => {
                                if *r == Route::Native {
                                    native_reason =
                                        Some(UnavailableReason::F32DenormalBehaviorUnknown);
                                }
                            }
                            Ok(j) => {
                                chosen = Some((*r, behavior(*j)));
                                break;
                            }
                        }
                    }
                }
            }
            let out = match chosen {
                Some((route, f32_denormal)) => SelectionOutput {
                    route: Some(route),
                    native_reason: if route == Route::Native {
                        None
                    } else {
                        native_reason
                    },
                    fallback_reason: None,
                    f32_denormal,
                },
                None => SelectionOutput {
                    route: None,
                    native_reason,
                    fallback_reason: Some(if fallback_allowed {
                        UnavailableReason::F32DenormalBehaviorUnknown
                    } else {
                        UnavailableReason::OptInNotGiven
                    }),
                    f32_denormal: None,
                },
            };
            (Ok(out), called)
        }

        fn actual(c: &Case) -> Outcome {
            let table: Vec<(Route, Answer)> = match &c.answers {
                Some(a) => ROUTES.iter().copied().zip(a.iter().cloned()).collect(),
                None => Vec::new(),
            };
            run(c.op, c.opt_in, c.query, &table)
        }

        #[test]
        fn every_combination_matches_candidate_rule() {
            let all = cases();
            assert_eq!(all.len(), 11 * 4 * (2 + 2 * 64));
            let mut checked = 0;
            for c in &all {
                let (want, want_called) = expected(c);
                let (got, got_called) = actual(c);
                assert_eq!(got, want, "{:?} {:?} {:?}", c.op, c.opt_in, c.query);
                assert_eq!(got_called, want_called);
                checked += 1;
            }
            assert_eq!(checked, 5720);
        }

        #[test]
        fn unsupported_rows_keep_query_reason() {
            // FB-6
            let mut seen = 0;
            for c in cases()
                .iter()
                .filter(|c| c.query.level == CapabilityLevel::Unsupported)
            {
                let (got, called) = actual(c);
                assert!(!called.contains(&Route::Native));
                if let Ok(out) = got {
                    assert_eq!(
                        out.native_reason,
                        Some(UnavailableReason::CapabilityUnsupported(
                            c.query.unsupported_reason
                        ))
                    );
                }
                seen += 1;
            }
            assert_eq!(seen, 4 * 4 * (2 + 2 * 64));
        }

        #[test]
        fn unknown_levels_never_choose_native() {
            // FB-7
            let mut seen = 0;
            for c in cases()
                .iter()
                .filter(|c| matches!(c.query.level, CapabilityLevel::S0 | CapabilityLevel::S1))
            {
                let (got, called) = actual(c);
                assert!(!called.contains(&Route::Native));
                if let Ok(out) = got {
                    assert_ne!(out.route, Some(Route::Native));
                    assert_eq!(
                        out.native_reason,
                        Some(UnavailableReason::CapabilityUnknown)
                    );
                }
                seen += 1;
            }
            assert_eq!(seen, 2 * 4 * (2 + 2 * 64));
        }

        #[test]
        fn report_f32_item_follows_judged_route() {
            for c in &cases() {
                let (want, _) = expected(c);
                let Ok(want) = want else { continue };
                let (got, _) = actual(c);
                let out = got.unwrap();
                let input = selection_input(c.op, c.opt_in, c.query);
                let report = build_report(&input, &out);
                let has = matches!(c.op, OpId::SubgroupSumF32 | OpId::AtomicAdd);
                let exp = match (has, want.route, want.f32_denormal) {
                    (true, Some(_), Some(behavior)) => Some(F32DenormalReport {
                        behavior,
                        basis: F32DenormalBasis::Observed,
                        documented: None,
                    }),
                    _ => None,
                };
                assert_eq!(report.f32_denormal, exp);
            }
        }
    }
}

#[cfg(all(test, feature = "fault-injection"))]
mod forced_tests {
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

    const ALL_OPS: [OpId; 4] = [
        OpId::F16Fma,
        OpId::SubgroupSumU32,
        OpId::SubgroupSumF32,
        OpId::AtomicAdd,
    ];

    type Answers = [Result<DenormalJudgment, ContractError>; 3];

    fn queries() -> Vec<CapabilityQuery> {
        let mut v = Vec::new();
        for level in [
            CapabilityLevel::S0,
            CapabilityLevel::S1,
            CapabilityLevel::S2,
            CapabilityLevel::S3,
            CapabilityLevel::S4,
            CapabilityLevel::S5,
            CapabilityLevel::S6,
        ] {
            v.push(CapabilityQuery {
                level,
                unsupported_reason: None,
                evidence: None,
            });
        }
        for reason in [
            None,
            Some(UnsupportedReason::NoApi),
            Some(UnsupportedReason::NotSupported),
            Some(UnsupportedReason::UnknownFeature),
        ] {
            v.push(CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: reason,
                evidence: None,
            });
        }
        v
    }

    /// 判定の答えを経路ごとに持つ組（Native, GpuEmulated, Cpu の順）の全 64 通り。
    fn answers() -> Vec<Answers> {
        let one = |i: usize| -> Result<DenormalJudgment, ContractError> {
            match i {
                0 => Ok(DenormalJudgment::Preserve),
                1 => Ok(DenormalJudgment::FlushToZero),
                2 => Ok(DenormalJudgment::Unknown),
                _ => Err(ContractError::Timeout),
            }
        };
        let mut v = Vec::new();
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    v.push([one(a), one(b), one(c)]);
                }
            }
        }
        v
    }

    fn slot(r: Route) -> usize {
        match r {
            Route::Native => 0,
            Route::GpuEmulated => 1,
            Route::Cpu => 2,
        }
    }

    fn run(
        input: &SelectionInput,
        ans: &Answers,
    ) -> (Result<SelectionOutput, ContractError>, Vec<Route>) {
        let mut called = Vec::new();
        let out = select_route(input, |r| {
            called.push(r);
            ans[slot(r)].clone()
        });
        (out, called)
    }

    #[test]
    fn forced_never_chooses_native() {
        let mut seen = 0;
        for op in ALL_OPS {
            let judged = matches!(op, OpId::SubgroupSumF32 | OpId::AtomicAdd);
            let answer_sets = if judged {
                answers()
            } else {
                vec![[
                    Ok(DenormalJudgment::Preserve),
                    Ok(DenormalJudgment::Preserve),
                    Ok(DenormalJudgment::Preserve),
                ]]
            };
            for q in queries() {
                for o in ALL_OPT {
                    for ans in &answer_sets {
                        let forced_in = SelectionInput {
                            forced: true,
                            ..selection_input(op, o, q)
                        };
                        let unsupported = CapabilityQuery {
                            level: CapabilityLevel::Unsupported,
                            ..q
                        };
                        let base_in = selection_input(op, o, unsupported);
                        let (got, got_calls) = run(&forced_in, ans);
                        let (want, want_calls) = run(&base_in, ans);
                        assert!(!got_calls.contains(&Route::Native));
                        assert_eq!(got_calls, want_calls);
                        match (got, want) {
                            (Ok(g), Ok(w)) => {
                                assert_ne!(g.route, Some(Route::Native));
                                assert_eq!(
                                    g.native_reason,
                                    Some(UnavailableReason::ForcedUnsupported)
                                );
                                let g2 = SelectionOutput {
                                    native_reason: None,
                                    ..g
                                };
                                let w2 = SelectionOutput {
                                    native_reason: None,
                                    ..w
                                };
                                assert_eq!(g2, w2);
                            }
                            (Err(g), Err(w)) => assert_eq!(g, w),
                            _ => panic!("forced and base disagree on Ok/Err"),
                        }
                        seen += 1;
                    }
                }
            }
        }
        // 能力の値 11 通り × 印 4 通り × (判定なし 2 操作 + 判定あり 2 操作 × 答え 64 組)
        assert_eq!(seen, 11 * 4 * (2 + 2 * 64));
    }
}
