//! 使えない理由の値の区別と、f32 の非正規化数の項の試験（TASK-919）。
//!
//! 公開のパスだけを使い、`Exec` の入口を試験用の device（`common::Scripted`）に向けて呼ぶ。
//! 経路の無い報告で理由の値が別々に届くことと、f32 の非正規化数の項が空であることを照らす。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::CapabilityLevel;
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::feature_key::FEATURE_SUBGROUP;
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OpId, OptIn, Outcome, SubgroupSumF32Operands,
    SubgroupSumU32Operands, UnavailableReason,
};

/// 4 つの入口を host の形の列で 1 回ずつ呼び、操作と結果の組を返す。
fn call_four(exec: &Exec<'_>, over: Option<OptIn>) -> [(OpId, Result<Outcome, ContractError>); 4] {
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
        (OpId::F16Fma, r16),
        (OpId::SubgroupSumU32, ru),
        (OpId::SubgroupSumF32, rf),
        (OpId::AtomicAdd, ra),
    ]
}

#[test]
fn reasons_are_distinct() {
    // FB-23
    let all = [
        UnavailableReason::CapabilityUnsupported(None),
        UnavailableReason::CapabilityUnsupported(Some(UnsupportedReason::NoApi)),
        UnavailableReason::CapabilityUnsupported(Some(UnsupportedReason::NotSupported)),
        UnavailableReason::CapabilityUnsupported(Some(UnsupportedReason::UnknownFeature)),
        UnavailableReason::CapabilityUnknown,
        UnavailableReason::OptInNotGiven,
        UnavailableReason::SemanticsNotPreservable,
        UnavailableReason::F32DenormalBehaviorUnknown,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            assert_eq!(i == j, a == b, "{i} {j}");
        }
    }
}

#[test]
fn unavailable_report_reasons_are_told_apart() {
    let bare = Scripted::bare();
    let fails = Scripted::query_fails();
    let ex_bare = Exec::deny_by_default(&bare);
    let ex_fails = Exec::deny_by_default(&fails);
    let from_bare = call_four(&ex_bare, None);
    let from_fails = call_four(&ex_fails, None);
    for ((op, a), (_, b)) in from_bare.iter().zip(from_fails.iter()) {
        let Ok(Outcome::Unavailable { report: ra }) = a else {
            panic!("expected Unavailable for {op:?}");
        };
        let Ok(Outcome::Unavailable { report: rb }) = b else {
            panic!("expected Unavailable for {op:?}");
        };
        assert!(
            matches!(
                ra.native_reason,
                Some(UnavailableReason::CapabilityUnsupported(_))
            ),
            "{op:?}"
        );
        assert_eq!(
            rb.native_reason,
            Some(UnavailableReason::CapabilityUnknown),
            "{op:?}"
        );
        assert_ne!(ra.native_reason, rb.native_reason, "{op:?}");
        assert_eq!(
            ra.fallback_reason,
            Some(UnavailableReason::OptInNotGiven),
            "{op:?}"
        );
        assert_eq!(
            rb.fallback_reason,
            Some(UnavailableReason::OptInNotGiven),
            "{op:?}"
        );
    }
}

#[test]
fn f32_denormal_absent_on_unavailable() {
    // FB-56
    let devices = [
        Scripted::bare(),
        Scripted::insufficient(FEATURE_SUBGROUP),
        Scripted::query_fails(),
    ];
    for dev in &devices {
        let exec = Exec::deny_by_default(dev);
        for over in [None, Some(OptIn::default())] {
            let calls = call_four(&exec, over);
            let ops: Vec<OpId> = calls.iter().map(|(op, _)| *op).collect();
            assert!(ops.contains(&OpId::SubgroupSumF32));
            assert!(ops.contains(&OpId::AtomicAdd));
            for (op, result) in &calls {
                let Ok(Outcome::Unavailable { report }) = result else {
                    panic!("expected Unavailable for {op:?}");
                };
                assert_eq!(report.op, *op);
                assert_eq!(report.f32_denormal, None, "{op:?}");
            }
        }
    }
}

#[test]
fn report_items_are_readable() {
    let dev = Scripted::bare();
    let exec = Exec::deny_by_default(&dev);
    for (op, result) in call_four(&exec, None) {
        let outcome = result.expect("Ok expected");
        let report = outcome.report();
        assert_eq!(report.op, op);
        assert_eq!(report.route, None);
        assert_eq!(
            report.native_reason,
            Some(UnavailableReason::CapabilityUnsupported(Some(
                UnsupportedReason::UnknownFeature
            )))
        );
        assert_eq!(
            report.fallback_reason,
            Some(UnavailableReason::OptInNotGiven)
        );
        assert_eq!(report.observed_level, Some(CapabilityLevel::Unsupported));
        assert_eq!(report.f32_denormal, None);
    }
}
