//! 下-2 の公開のパスから 4 つの入口を長さ 0（`atomic_add` は出力の数 0 も）で呼び、
//! `InvalidArgument` を返すこと、試験用の device（`common::Scripted`）の能力の問合せ・確保・投入の数えが
//! すべて 0 のこと、番兵で埋めた出力の列が変わらないこと、通る長さが能力の問合せに届くこと、
//! 拒否の呼び出しの後も `query_route` の報告が変わらないことを確かめる（TASK-924）。
//! 置き場の説明は docs/exec-length-check.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::route_evidence::{SENTINEL_BYTE, is_untouched};
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OpId, OptIn, Outcome, SubgroupSumF32Operands,
    SubgroupSumU32Operands,
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

/// 呼び出しごとの上書き（`None` は既定のまま）。
const OVERRIDES: [Option<OptIn>; 5] = [None, Some(NONE), Some(G_ONLY), Some(C_ONLY), Some(BOTH)];
/// `Exec::new` に渡す既定の印。
const DEFAULTS: [OptIn; 4] = [NONE, G_ONLY, C_ONLY, BOTH];
/// 試験の対象の入口。
const OPS: [OpId; 4] = [
    OpId::F16Fma,
    OpId::SubgroupSumU32,
    OpId::SubgroupSumF32,
    OpId::AtomicAdd,
];
/// device の答えの通り数。
const DEVICES: usize = 3;

fn q(level: CapabilityLevel) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: None,
        evidence: None,
    }
}

/// 答えの添字から新しい試験用の device を作る（数えを混ぜないため呼び出しごとに作る）。
fn make_device(i: usize) -> Scripted {
    match i {
        0 => Scripted::answering(q(CapabilityLevel::S2)),
        1 => Scripted::answering(q(CapabilityLevel::Unsupported)),
        _ => Scripted::query_fails(),
    }
}

/// 入口を 1 回呼び、結果と、出力の列が番兵のままかを返す。
/// `m` は `atomic_add` だけが使う。
fn call(
    exec: &Exec<'_>,
    op: OpId,
    over: Option<OptIn>,
    n: u64,
    m: u64,
    in_len: usize,
    out_len: usize,
) -> (Result<Outcome, ContractError>, bool) {
    let s16 = u16::from_ne_bytes([SENTINEL_BYTE; 2]);
    let s32 = u32::from_ne_bytes([SENTINEL_BYTE; 4]);
    let sf = f32::from_bits(s32);
    let in16 = vec![1u16; in_len];
    let in_u = vec![1u32; in_len];
    let in_f = vec![1.0f32; in_len];
    let (mut o16, mut ou, mut of, mut oa) = (
        vec![s16; out_len],
        vec![s32; out_len],
        vec![sf; out_len],
        vec![sf; out_len],
    );
    let (result, bytes): (_, Vec<u8>) = match op {
        OpId::F16Fma => {
            let r = exec.f16_fma(
                over,
                n,
                F16FmaOperands::Host {
                    a: &in16,
                    b: &in16,
                    c: &in16,
                    out: &mut o16,
                },
            );
            (r, o16.iter().flat_map(|v| v.to_ne_bytes()).collect())
        }
        OpId::SubgroupSumU32 => {
            let r = exec.subgroup_sum_u32(
                over,
                n,
                SubgroupSumU32Operands::Host {
                    x: &in_u,
                    out: &mut ou,
                },
            );
            (r, ou.iter().flat_map(|v| v.to_ne_bytes()).collect())
        }
        OpId::SubgroupSumF32 => {
            let r = exec.subgroup_sum_f32(
                over,
                n,
                SubgroupSumF32Operands::Host {
                    x: &in_f,
                    out: &mut of,
                },
            );
            (
                r,
                of.iter().flat_map(|v| v.to_bits().to_ne_bytes()).collect(),
            )
        }
        OpId::AtomicAdd => {
            let r = exec.atomic_add(
                over,
                n,
                m,
                AtomicAddOperands::Host {
                    x: &in_f,
                    out: &mut oa,
                },
            );
            (
                r,
                oa.iter().flat_map(|v| v.to_bits().to_ne_bytes()).collect(),
            )
        }
        _ => unreachable!(),
    };
    (result, is_untouched(&bytes))
}

/// 全組（上書き × 既定の印 × device の答え）に `f` を流す。
fn for_each_combo(mut f: impl FnMut(OpId, Option<OptIn>, OptIn, usize)) {
    for op in OPS {
        for over in OVERRIDES {
            for d in DEFAULTS {
                for dev_i in 0..DEVICES {
                    f(op, over, d, dev_i);
                }
            }
        }
    }
}

/// 拒否の結果か。
fn is_rejected(r: &Result<Outcome, ContractError>) -> bool {
    // FB-57: 引数の不正の種類だけを照らし、文言は照らさない。
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

#[test]
fn zero_length_is_rejected_on_every_entry() {
    // FB-40
    for_each_combo(|op, over, d, dev_i| {
        let dev = make_device(dev_i);
        let exec = Exec::new(&dev, d);
        let m = if op == OpId::AtomicAdd { 1 } else { 0 };
        let (r, _) = call(&exec, op, over, 0, m, 0, 0);
        let tag = format!("{op:?} {over:?} {d:?} {dev_i}");
        assert!(is_rejected(&r), "{tag}: {r:?}");
        assert_eq!(dev.query_ex_calls(), 0, "{tag}");
        assert_eq!(dev.memory_calls(), 0, "{tag}");
        assert_eq!(dev.queue_calls(), 0, "{tag}");
    });
}

#[test]
fn zero_length_with_sentinel_outputs_is_untouched() {
    for_each_combo(|op, over, d, dev_i| {
        let dev = make_device(dev_i);
        let exec = Exec::new(&dev, d);
        let m = if op == OpId::AtomicAdd { 1 } else { 0 };
        let (r, untouched) = call(&exec, op, over, 0, m, 0, 2);
        let tag = format!("{op:?} {over:?} {d:?} {dev_i}");
        assert!(is_rejected(&r), "{tag}: {r:?}");
        assert!(untouched, "{tag}");
    });
}

#[test]
fn zero_outputs_is_rejected_for_atomic_add() {
    // FB-59
    for over in OVERRIDES {
        for d in DEFAULTS {
            for dev_i in 0..DEVICES {
                for out_len in [0usize, 2] {
                    let dev = make_device(dev_i);
                    let exec = Exec::new(&dev, d);
                    let (r, untouched) = call(&exec, OpId::AtomicAdd, over, 8, 0, 8, out_len);
                    let tag = format!("{over:?} {d:?} {dev_i} {out_len}");
                    assert!(is_rejected(&r), "{tag}: {r:?}");
                    assert_eq!(dev.query_ex_calls(), 0, "{tag}");
                    assert_eq!(dev.memory_calls(), 0, "{tag}");
                    assert_eq!(dev.queue_calls(), 0, "{tag}");
                    assert!(untouched, "{tag}");
                }
            }
        }
    }
}

#[test]
fn valid_length_reaches_route_choice() {
    for_each_combo(|op, over, d, dev_i| {
        let dev = make_device(dev_i);
        let exec = Exec::new(&dev, d);
        let m = if op == OpId::AtomicAdd { 1 } else { 0 };
        let out_len = if op == OpId::F16Fma { 2 } else { 1 };
        let (r, _) = call(&exec, op, over, 2, m, 2, out_len);
        let tag = format!("{op:?} {over:?} {d:?} {dev_i}");
        assert!(!is_rejected(&r), "{tag}: {r:?}");
        assert_eq!(dev.query_ex_calls(), 1, "{tag}");
    });
}

#[test]
fn query_route_is_unaffected() {
    for_each_combo(|op, over, d, dev_i| {
        let tag = format!("{op:?} {over:?} {d:?} {dev_i}");
        let dev = make_device(dev_i);
        let exec = Exec::new(&dev, d);
        let (r, _) = call(&exec, op, over, 0, 0, 0, 0);
        assert!(is_rejected(&r), "{tag}: {r:?}");
        if op == OpId::AtomicAdd {
            let (r, _) = call(&exec, op, over, 4, 0, 4, 0);
            assert!(is_rejected(&r), "{tag}: {r:?}");
        }
        let after = exec.query_route(op, over).ok();

        let fresh_dev = make_device(dev_i);
        let fresh = Exec::new(&fresh_dev, d);
        let before = fresh.query_route(op, over).ok();
        assert_eq!(after, before, "{tag}");
    });
}
