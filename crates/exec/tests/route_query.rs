//! `Exec::query_route` と 4 つの入口の返しを、模擬の能力の値で照らす試験。
//!
//! 公開のパスだけを使い、`common::Scripted::answering` が返す能力の値ごとに、
//! `query_route` の報告と入口の報告が一致すること、経路の選び方、`query_ex` の呼び出し回数を確かめる。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::feature_key::{
    FEATURE_SHADER_F16, FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_SUBGROUP,
};
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OpId, OptIn, Outcome, Route, SubgroupSumF32Operands,
    SubgroupSumU32Operands, UnavailableReason,
};

const fn opt(g: bool, c: bool) -> OptIn {
    OptIn {
        allow_gpu_emulated: g,
        allow_cpu: c,
    }
}

const NONE: OptIn = opt(false, false);
const G_ONLY: OptIn = opt(true, false);
const C_ONLY: OptIn = opt(false, true);
const BOTH: OptIn = opt(true, true);

const OPS: [OpId; 4] = [
    OpId::F16Fma,
    OpId::SubgroupSumU32,
    OpId::SubgroupSumF32,
    OpId::AtomicAdd,
];

/// 上書きの 5 通り（`None` は `Exec` の既定の印を使う）。
const OVERS: [Option<OptIn>; 5] = [None, Some(NONE), Some(G_ONLY), Some(C_ONLY), Some(BOTH)];

fn q(level: CapabilityLevel, reason: Option<UnsupportedReason>) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: reason,
        evidence: None,
    }
}

/// 入口を 1 回呼ぶ。返しと、出力の列が呼ぶ前のままかを返す。
fn call_entry(
    exec: &Exec<'_>,
    over: Option<OptIn>,
    op: OpId,
) -> (Result<Outcome, ContractError>, bool) {
    let in16 = [1u16; 2];
    let in_u = [1u32; 2];
    let in_f = [1.0f32; 2];
    let (mut o16, mut ou, mut of, mut oa) = ([7u16; 2], [7u32; 1], [7.0f32; 1], [7.0f32; 2]);
    let r = match op {
        OpId::F16Fma => exec.f16_fma(
            over,
            2,
            F16FmaOperands::Host {
                a: &in16,
                b: &in16,
                c: &in16,
                out: &mut o16,
            },
        ),
        OpId::SubgroupSumU32 => exec.subgroup_sum_u32(
            over,
            2,
            SubgroupSumU32Operands::Host {
                x: &in_u,
                out: &mut ou,
            },
        ),
        OpId::SubgroupSumF32 => exec.subgroup_sum_f32(
            over,
            2,
            SubgroupSumF32Operands::Host {
                x: &in_f,
                out: &mut of,
            },
        ),
        OpId::AtomicAdd => exec.atomic_add(
            over,
            2,
            1,
            AtomicAddOperands::Host {
                x: &in_f,
                out: &mut oa,
            },
        ),
        _ => unreachable!(),
    };
    let untouched = o16 == [7; 2] && ou == [7; 1] && of == [7.0; 1] && oa == [7.0; 2];
    (r, untouched)
}

fn is_unsupported(r: &Result<impl Sized, ContractError>) -> bool {
    matches!(r, Err(ContractError::Unsupported { .. }))
}

/// 検査する能力の値 9 通り。
fn all_answers() -> Vec<CapabilityQuery> {
    use CapabilityLevel::*;
    let mut v: Vec<_> = [S0, S1, S2, S3, S4, S5, S6]
        .into_iter()
        .map(|l| q(l, None))
        .collect();
    v.push(q(Unsupported, None));
    v.push(q(Unsupported, Some(UnsupportedReason::NotSupported)));
    v
}

#[test]
fn query_route_matches_entry_report() {
    // FB-24
    for answer in all_answers() {
        let dev = Scripted::answering(answer);
        let exec = Exec::deny_by_default(&dev);
        for op in OPS {
            for over in OVERS {
                let (entry, untouched) = call_entry(&exec, over, op);
                let route = exec.query_route(op, over);
                let ctx = format!("{answer:?} {op:?} {over:?}");
                if !matches!(entry, Ok(Outcome::Ran { .. })) {
                    assert!(untouched, "{ctx}");
                }
                match entry {
                    Ok(Outcome::Ran { report }) => {
                        assert!(matches!(op, OpId::F16Fma | OpId::SubgroupSumU32), "{ctx}");
                        let Ok(r) = route else {
                            panic!("expected Ok: {ctx}");
                        };
                        assert_eq!(r, report, "{ctx}");
                        assert_eq!(r.route, Some(Route::Cpu), "{ctx}");
                        assert!(!untouched, "{ctx}");
                    }
                    Ok(Outcome::Unavailable { report }) => {
                        let Ok(r) = route else {
                            panic!("expected Ok: {ctx}");
                        };
                        assert_eq!(r, report, "{ctx}");
                    }
                    Err(ContractError::Unsupported { .. }) => match op {
                        OpId::F16Fma | OpId::SubgroupSumU32 => {
                            let Ok(r) = route else {
                                panic!("expected Ok: {ctx}");
                            };
                            assert!(r.route.is_some(), "{ctx}");
                        }
                        _ => assert!(is_unsupported(&route), "{ctx}"),
                    },
                    other => panic!("unexpected entry result {other:?}: {ctx}"),
                }
            }
        }
    }
}

#[test]
fn native_levels_report_native_route() {
    use CapabilityLevel::*;
    for level in [S2, S3, S4, S5, S6] {
        let dev = Scripted::answering(q(level, None));
        let exec = Exec::deny_by_default(&dev);
        for over in OVERS {
            for op in [OpId::F16Fma, OpId::SubgroupSumU32] {
                let Ok(r) = exec.query_route(op, over) else {
                    panic!("expected Ok: {level:?} {op:?} {over:?}");
                };
                assert_eq!(r.op, op);
                assert_eq!(r.route, Some(Route::Native));
                assert_eq!(r.native_reason, None);
                assert_eq!(r.fallback_reason, None);
                assert_eq!(r.observed_level, Some(level));
                assert_eq!(r.f32_denormal, None);
            }
            for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
                let Err(ContractError::Unsupported { op: name, backend }) =
                    exec.query_route(op, over)
                else {
                    panic!("expected Unsupported: {level:?} {op:?} {over:?}");
                };
                assert_eq!(name, "query_route");
                assert_eq!(backend, dev.backend());
            }
        }
    }
}

#[test]
fn unreachable_native_follows_opt_in_order() {
    use CapabilityLevel::*;
    let answers = [
        q(Unsupported, None),
        q(Unsupported, Some(UnsupportedReason::NoApi)),
        q(Unsupported, Some(UnsupportedReason::NotSupported)),
        q(Unsupported, Some(UnsupportedReason::UnknownFeature)),
        q(S0, None),
        q(S1, None),
    ];
    for answer in answers {
        let dev = Scripted::answering(answer);
        let exec = Exec::deny_by_default(&dev);
        let native = if answer.level == Unsupported {
            UnavailableReason::CapabilityUnsupported(answer.unsupported_reason)
        } else {
            UnavailableReason::CapabilityUnknown
        };
        for (over, want) in [
            (Some(G_ONLY), Some(Route::GpuEmulated)),
            (Some(C_ONLY), Some(Route::Cpu)),
            (Some(BOTH), Some(Route::GpuEmulated)),
            (None, None),
            (Some(NONE), None),
        ] {
            for op in OPS {
                let ctx = format!("{answer:?} {op:?} {over:?}");
                let r = match exec.query_route(op, over) {
                    Ok(r) => r,
                    Err(e) => {
                        // 経路が届く SubgroupSumF32・AtomicAdd は Unsupported。
                        assert!(
                            want.is_some() && matches!(op, OpId::SubgroupSumF32 | OpId::AtomicAdd),
                            "unexpected {e:?}: {ctx}"
                        );
                        assert!(matches!(e, ContractError::Unsupported { .. }), "{ctx}");
                        continue;
                    }
                };
                assert_eq!(r.op, op, "{ctx}");
                assert_eq!(r.route, want, "{ctx}");
                assert_eq!(r.native_reason, Some(native), "{ctx}");
                assert_eq!(r.observed_level, Some(answer.level), "{ctx}");
                assert_eq!(r.f32_denormal, None, "{ctx}");
                let fallback = want.is_none().then_some(UnavailableReason::OptInNotGiven);
                assert_eq!(r.fallback_reason, fallback, "{ctx}");
            }
        }
    }
}

#[test]
fn query_route_calls_query_ex_once() {
    let makers: [fn() -> Scripted; 2] = [Scripted::bare, || {
        Scripted::answering(q(CapabilityLevel::S2, None))
    }];
    for make in makers {
        for op in OPS {
            for over in [None, Some(C_ONLY), Some(BOTH)] {
                let dev = make();
                let exec = Exec::deny_by_default(&dev);
                let _ = exec.query_route(op, over);
                assert_eq!(dev.query_ex_calls(), 1, "{op:?} {over:?}");
                assert_eq!(dev.memory_calls(), 0, "{op:?} {over:?}");
                assert_eq!(dev.queue_calls(), 0, "{op:?} {over:?}");
            }
        }
    }
}

/// 操作が問い合わせる機能の鍵。
fn key_of(op: OpId) -> fandhe_silicon_core::capability::FeatureId {
    match op {
        OpId::F16Fma => FEATURE_SHADER_F16,
        OpId::SubgroupSumU32 | OpId::SubgroupSumF32 => FEATURE_SUBGROUP,
        OpId::AtomicAdd => FEATURE_SHADER_FLOAT32_ATOMIC,
        _ => unreachable!(),
    }
}

/// 鍵ごとに違う値を返す device で、報告が操作の鍵の値に従い、入口の報告と一致する。
#[test]
fn query_route_follows_per_key_answer() {
    // TASK-920
    let keys = [
        FEATURE_SHADER_F16,
        FEATURE_SUBGROUP,
        FEATURE_SHADER_FLOAT32_ATOMIC,
    ];
    for weak in keys {
        let dev = Scripted::insufficient(weak);
        let exec = Exec::deny_by_default(&dev);
        for op in OPS {
            let ctx = format!("{weak:?} {op:?}");
            let Ok(r) = exec.query_route(op, Some(C_ONLY)) else {
                // 経路が届く SubgroupSumF32・AtomicAdd は Unsupported。
                assert!(
                    matches!(op, OpId::SubgroupSumF32 | OpId::AtomicAdd),
                    "{ctx}"
                );
                continue;
            };
            // 操作の鍵が弱い鍵なら NotSupported、ほかの鍵は表に無いので UnknownFeature。
            let reason = if key_of(op) == weak {
                UnsupportedReason::NotSupported
            } else {
                UnsupportedReason::UnknownFeature
            };
            assert_eq!(r.route, Some(Route::Cpu), "{ctx}");
            assert_eq!(
                r.native_reason,
                Some(UnavailableReason::CapabilityUnsupported(Some(reason))),
                "{ctx}"
            );
        }
        for op in OPS {
            let ctx = format!("{weak:?} {op:?}");
            let (entry, _) = call_entry(&exec, Some(NONE), op);
            let route = exec.query_route(op, Some(NONE));
            match entry {
                Ok(Outcome::Unavailable { report }) => {
                    assert_eq!(route.unwrap(), report, "{ctx}");
                }
                other => panic!("unexpected entry result {other:?}: {ctx}"),
            }
        }
    }
    // 同じ操作でも、弱い鍵が違えば報告が違う。
    let report_under = |weak, op| {
        let dev = Scripted::insufficient(weak);
        let exec = Exec::deny_by_default(&dev);
        exec.query_route(op, Some(NONE)).unwrap()
    };
    assert_ne!(
        report_under(FEATURE_SHADER_F16, OpId::F16Fma),
        report_under(FEATURE_SUBGROUP, OpId::F16Fma)
    );
}

/// 既定の印は上書き `None` の `query_route` に効き、`Some` の上書きは既定を置き換える。
#[test]
fn query_route_none_override_uses_default_opt_in() {
    // TASK-920
    let dev = Scripted::insufficient(FEATURE_SHADER_F16);
    for (default, want) in [
        (NONE, None),
        (G_ONLY, Some(Route::GpuEmulated)),
        (C_ONLY, Some(Route::Cpu)),
        (BOTH, Some(Route::GpuEmulated)),
    ] {
        let exec = Exec::new(&dev, default);
        let r = exec.query_route(OpId::F16Fma, None).unwrap();
        assert_eq!(r.route, want, "{default:?}");
        let fallback = want.is_none().then_some(UnavailableReason::OptInNotGiven);
        assert_eq!(r.fallback_reason, fallback, "{default:?}");
        // 上書きが `Some` なら既定は使わない。
        let other = if default == NONE { BOTH } else { NONE };
        assert_eq!(
            exec.query_route(OpId::F16Fma, Some(other)).unwrap(),
            Exec::new(&dev, other)
                .query_route(OpId::F16Fma, None)
                .unwrap(),
            "{default:?}"
        );
        assert_eq!(exec.query_route(OpId::F16Fma, None).unwrap(), r);
    }
}
