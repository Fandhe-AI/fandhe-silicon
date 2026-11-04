//! opt-in の既定と呼び出しごとの上書きの試験（TASK-919）。
//!
//! 公開のパスだけを使い、`Exec` の 7 つの入口を試験用の device（`common::Scripted`）に
//! 向けて呼ぶ。出力の列に定数を入れて呼び、`Ok(Outcome::Unavailable { .. })` を返した呼び出しと、
//! `f16_fma` 以外の入口が `Err(ContractError::Unsupported { .. })` を返した呼び出しでは、呼んだ後も同じ値であることを照らす。
//! `f16_fma` が経路 `Route::Cpu` で host の形なら出力に書くので、その呼び出しは値が変わることを照らす。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, FeatureId};
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

/// 入口 1 回分の結果。
struct Call {
    op: OpId,
    result: Result<Outcome, ContractError>,
    /// 出力の列が呼ぶ前のままか。
    untouched: bool,
}

/// 4 つの入口のうち `entry` 番目（0: f16_fma・1: subgroup_sum_u32・2: subgroup_sum_f32・3: atomic_add）だけを 1 回呼ぶ。
fn call_one(exec: &Exec<'_>, over: Option<OptIn>, entry: usize) {
    let in16 = [1u16; 2];
    let in_u = [1u32; 2];
    let in_f = [1.0f32; 2];
    let (mut o16, mut ou, mut of, mut oa) = ([7u16; 2], [7u32; 2], [7.0f32; 2], [7.0f32; 2]);
    match entry {
        0 => {
            let _ = exec.f16_fma(
                over,
                2,
                F16FmaOperands::Host {
                    a: &in16,
                    b: &in16,
                    c: &in16,
                    out: &mut o16,
                },
            );
        }
        1 => {
            let _ = exec.subgroup_sum_u32(
                over,
                2,
                SubgroupSumU32Operands::Host {
                    x: &in_u,
                    out: &mut ou,
                },
            );
        }
        2 => {
            let _ = exec.subgroup_sum_f32(
                over,
                2,
                SubgroupSumF32Operands::Host {
                    x: &in_f,
                    out: &mut of,
                },
            );
        }
        _ => {
            let _ = exec.atomic_add(
                over,
                2,
                1,
                AtomicAddOperands::Host {
                    x: &in_f,
                    out: &mut oa,
                },
            );
        }
    }
}

/// 4 つの入口を 1 回ずつ呼ぶ。
fn call_four(exec: &Exec<'_>, over: Option<OptIn>) -> [Call; 4] {
    let in16 = [1u16; 2];
    let in_u = [1u32; 2];
    let in_f = [1.0f32; 2];
    let (mut o16, mut ou, mut of, mut oa) = ([7u16; 2], [7u32; 2], [7.0f32; 2], [7.0f32; 2]);
    let r16 = exec.f16_fma(
        over,
        2,
        F16FmaOperands::Host {
            a: &in16,
            b: &in16,
            c: &in16,
            out: &mut o16,
        },
    );
    let ru = exec.subgroup_sum_u32(
        over,
        2,
        SubgroupSumU32Operands::Host {
            x: &in_u,
            out: &mut ou,
        },
    );
    let rf = exec.subgroup_sum_f32(
        over,
        2,
        SubgroupSumF32Operands::Host {
            x: &in_f,
            out: &mut of,
        },
    );
    let ra = exec.atomic_add(
        over,
        2,
        1,
        AtomicAddOperands::Host {
            x: &in_f,
            out: &mut oa,
        },
    );
    [
        Call {
            op: OpId::F16Fma,
            result: r16,
            untouched: o16 == [7; 2],
        },
        Call {
            op: OpId::SubgroupSumU32,
            result: ru,
            untouched: ou == [7; 2],
        },
        Call {
            op: OpId::SubgroupSumF32,
            result: rf,
            untouched: of == [7.0; 2],
        },
        Call {
            op: OpId::AtomicAdd,
            result: ra,
            untouched: oa == [7.0; 2],
        },
    ]
}

fn is_unavailable(r: &Result<Outcome, ContractError>) -> bool {
    matches!(r, Ok(Outcome::Unavailable { .. }))
}

fn is_unsupported(r: &Result<Outcome, ContractError>) -> bool {
    matches!(r, Err(ContractError::Unsupported { .. }))
}

/// 入口の操作に対応する機能の鍵。
fn feature_of(op: OpId) -> FeatureId {
    match op {
        OpId::F16Fma => FEATURE_SHADER_F16,
        OpId::AtomicAdd => FEATURE_SHADER_FLOAT32_ATOMIC,
        _ => FEATURE_SUBGROUP,
    }
}

/// Unavailable で、代わりの実行の理由が印なしであることを照らす。
fn assert_opt_in_not_given(call: &Call) {
    assert!(call.untouched, "{:?}", call.op);
    let Ok(Outcome::Unavailable { report }) = &call.result else {
        panic!("expected Unavailable for {:?}", call.op);
    };
    assert_eq!(report.op, call.op);
    assert_eq!(report.route, None);
    assert_eq!(
        report.fallback_reason,
        Some(UnavailableReason::OptInNotGiven)
    );
}

#[test]
fn deny_default_without_override_is_unavailable() {
    // FB-14
    for (i, op) in OPS.into_iter().enumerate() {
        let devices = [
            (
                Scripted::bare(),
                UnavailableReason::CapabilityUnsupported(Some(UnsupportedReason::UnknownFeature)),
                CapabilityLevel::Unsupported,
            ),
            (
                Scripted::insufficient(feature_of(op)),
                UnavailableReason::CapabilityUnsupported(Some(UnsupportedReason::NotSupported)),
                CapabilityLevel::Unsupported,
            ),
            (
                Scripted::query_fails(),
                UnavailableReason::CapabilityUnknown,
                CapabilityLevel::S0,
            ),
        ];
        for (dev, native, level) in devices {
            let exec = Exec::deny_by_default(&dev);
            let calls = call_four(&exec, None);
            let call = &calls[i];
            assert_eq!(call.op, op);
            assert_opt_in_not_given(call);
            let Ok(Outcome::Unavailable { report }) = &call.result else {
                unreachable!();
            };
            assert_eq!(report.native_reason, Some(native));
            assert_eq!(report.observed_level, Some(level));
        }
    }
}

#[test]
fn override_applies_to_that_call_only() {
    let dev = Scripted::bare();

    // 既定が拒否のとき、許す上書きはその呼び出しだけに効く。
    let exec = Exec::deny_by_default(&dev);
    for _ in 0..2 {
        for call in call_four(&exec, Some(C_ONLY)) {
            assert!(!is_unavailable(&call.result), "{:?}", call.op);
        }
        call_four(&exec, None)
            .iter()
            .for_each(assert_opt_in_not_given);
        assert_eq!(exec.default_opt_in(), OptIn::default());
    }

    // 既定が許すとき、拒否の上書きは既定を丸ごと置き換え、その呼び出しだけに効く。
    let exec = Exec::new(&dev, BOTH);
    call_four(&exec, Some(NONE))
        .iter()
        .for_each(assert_opt_in_not_given);
    assert_eq!(exec.default_opt_in(), BOTH);
    for call in call_four(&exec, None) {
        assert!(!is_unavailable(&call.result), "{:?}", call.op);
    }
}

#[test]
fn each_flag_reaches_route_choice() {
    // FB-15
    let devices = [
        Scripted::bare(),
        Scripted::insufficient(FEATURE_SUBGROUP),
        Scripted::query_fails(),
    ];
    for dev in &devices {
        let exec = Exec::deny_by_default(dev);
        for over in [G_ONLY, C_ONLY, BOTH] {
            for call in call_four(&exec, Some(over)) {
                assert!(!is_unavailable(&call.result), "{:?} with {over:?}", call.op);
            }
        }
        call_four(&exec, Some(NONE))
            .iter()
            .for_each(assert_opt_in_not_given);
    }
}

#[test]
fn chosen_route_does_not_write_output() {
    let dev = Scripted::bare();
    let exec = Exec::deny_by_default(&dev);
    for call in call_four(&exec, Some(C_ONLY))
        .into_iter()
        .filter(|call| call.op != OpId::F16Fma)
    {
        assert!(is_unsupported(&call.result), "{:?}", call.op);
        assert!(call.untouched, "{:?}", call.op);
    }
    for over in [None, Some(NONE), Some(G_ONLY), Some(C_ONLY), Some(BOTH)] {
        assert!(is_unsupported(&exec.external_interop_shared_memory(over)));
        assert!(is_unsupported(&exec.hardware_ray_tracing(over)));
        assert!(is_unsupported(&exec.unified_memory_sharing(over)));
    }
}

#[test]
fn cpu_route_writes_f16_output() {
    let dev = Scripted::bare();
    let exec = Exec::deny_by_default(&dev);
    let in16 = [1u16; 2];
    let mut out = [7u16; 2];
    let r = exec.f16_fma(
        Some(C_ONLY),
        2,
        F16FmaOperands::Host {
            a: &in16,
            b: &in16,
            c: &in16,
            out: &mut out,
        },
    );
    match r {
        Ok(Outcome::Ran { report }) => assert_eq!(report.route, Some(Route::Cpu)),
        other => panic!("expected Ran: {other:?}"),
    }
    assert_ne!(out, [7u16; 2]);
    assert_eq!(dev.memory_calls(), 0);
    assert_eq!(dev.queue_calls(), 0);
}

#[test]
fn entries_query_once_and_skip_memory_and_queue() {
    // 4 つの入口は 1 回の呼び出しにつき能力を 1 回だけ問い合わせる（入口ごとに別の device で数える）。
    for over in [None, Some(C_ONLY)] {
        for entry in 0..4 {
            let dev = Scripted::bare();
            let exec = Exec::deny_by_default(&dev);
            call_one(&exec, over, entry);
            assert_eq!(dev.query_ex_calls(), 1, "entry {entry} with {over:?}");
            assert_eq!(dev.memory_calls(), 0, "entry {entry} with {over:?}");
            assert_eq!(dev.queue_calls(), 0, "entry {entry} with {over:?}");
        }
    }
    // 3 つの入口は能力も問い合わせない。
    for over in [None, Some(NONE), Some(G_ONLY), Some(C_ONLY), Some(BOTH)] {
        let dev = Scripted::bare();
        let exec = Exec::deny_by_default(&dev);
        let _ = exec.external_interop_shared_memory(over);
        let _ = exec.hardware_ray_tracing(over);
        let _ = exec.unified_memory_sharing(over);
        assert_eq!(dev.query_ex_calls(), 0);
        assert_eq!(dev.memory_calls(), 0);
        assert_eq!(dev.queue_calls(), 0);
    }
}
