//! 下-2 の公開のパスから 4 つの入口を、上限を差し替えた試験用の device（`common::Scripted`）で呼び、
//! 上限を超える長さと必要バイト数のあふれで `InvalidArgument` を返すこと、能力の問合せ・確保・投入の数えが
//! 0 のこと、出力の列が番兵のままのこと、通る境界の長さが能力の問合せに届くことを確かめる（TASK-924）。
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
/// device の答えの通り数。
const DEVICES: usize = 2;
/// 差し替える上限。
const LIMIT: u64 = 64;

fn q(level: CapabilityLevel) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: None,
        evidence: None,
    }
}

/// 答えの添字と上限から新しい試験用の device を作る（数えを混ぜないため呼び出しごとに作る）。
fn make_device(i: usize, limit: u64) -> Scripted {
    match i {
        0 => Scripted::answering(q(CapabilityLevel::S2)),
        _ => Scripted::answering(q(CapabilityLevel::Unsupported)),
    }
    .with_max_buffer_bytes(limit)
}

/// 入口を 1 回呼び、結果と、出力の列（番兵）が番兵のままかを返す。
/// `f16_fma` の列と subgroup の 2 つの入口の `x` は長さ `n.min(64)`（最小 1）、subgroup の `out` は `k` を 256 で割って切り上げた長さ、
/// `atomic_add` の `out` は長さ `m.min(64)`（最小 1）。`m` は `atomic_add` だけが使う。
fn call(
    exec: &Exec<'_>,
    op: OpId,
    over: Option<OptIn>,
    n: u64,
    m: u64,
) -> (Result<Outcome, ContractError>, bool) {
    let s16 = u16::from_ne_bytes([SENTINEL_BYTE; 2]);
    let s32 = u32::from_ne_bytes([SENTINEL_BYTE; 4]);
    let sf = f32::from_bits(s32);
    let k = usize::try_from(n.min(64)).unwrap().max(1);
    let in16 = vec![1u16; k];
    let in_u = vec![1u32; k];
    let in_f = vec![1.0f32; k];
    let (mut o16, mut ou, mut of, mut oa) = (
        vec![s16; k],
        vec![s32; k.div_ceil(256)],
        vec![sf; k.div_ceil(256)],
        vec![sf; usize::try_from(m.min(64)).unwrap().max(1)],
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
fn for_each_combo(mut f: impl FnMut(Option<OptIn>, OptIn, usize)) {
    for over in OVERRIDES {
        for d in DEFAULTS {
            for dev_i in 0..DEVICES {
                f(over, d, dev_i);
            }
        }
    }
}

/// 拒否の結果か。
fn is_rejected(r: &Result<Outcome, ContractError>) -> bool {
    // FB-57: 引数の不正の種類だけを照らし、文言は照らさない。
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

/// 入口ごとの (入口, n, m) の組。`f16_fma` は `f16_n`、ほかは `word_n` を要素数とし、
/// `atomic_add` は出力の数を 1 にした組と、要素数 1 で出力の数を `word_m` にした組を持つ。
fn cases(f16_n: u64, word_n: u64, word_m: u64) -> Vec<(OpId, u64, u64)> {
    vec![
        (OpId::F16Fma, f16_n, 0),
        (OpId::SubgroupSumU32, word_n, 0),
        (OpId::SubgroupSumF32, word_n, 0),
        (OpId::AtomicAdd, word_n, 1),
        (OpId::AtomicAdd, 1, word_m),
    ]
}

#[test]
fn smallest_length_over_limit_is_rejected_through_entry() {
    // FB-58
    for (op, n, m) in cases(33, 17, 17) {
        for_each_combo(|over, d, dev_i| {
            let dev = make_device(dev_i, LIMIT);
            let exec = Exec::new(&dev, d);
            let (r, untouched) = call(&exec, op, over, n, m);
            let tag = format!("{op:?} {n} {m} {over:?} {d:?} {dev_i}");
            assert!(is_rejected(&r), "{tag}: {r:?}");
            assert_eq!(dev.query_ex_calls(), 0, "{tag}");
            assert_eq!(dev.memory_calls(), 0, "{tag}");
            assert_eq!(dev.queue_calls(), 0, "{tag}");
            assert!(untouched, "{tag}");
        });
    }
}

#[test]
fn largest_length_within_limit_reaches_route_choice() {
    for (op, n, m) in cases(32, 16, 16) {
        for_each_combo(|over, d, dev_i| {
            let dev = make_device(dev_i, LIMIT);
            let exec = Exec::new(&dev, d);
            let (r, _) = call(&exec, op, over, n, m);
            let tag = format!("{op:?} {n} {m} {over:?} {d:?} {dev_i}");
            assert!(!is_rejected(&r), "{tag}: {r:?}");
            assert_eq!(dev.query_ex_calls(), 1, "{tag}");
        });
    }
}

#[test]
fn byte_count_overflow_is_rejected_through_entry() {
    // FB-58: 上限を最大にして、あふれだけで落ちることを見る。
    let mut overflow = cases(u64::MAX / 2 + 1, u64::MAX / 4 + 1, u64::MAX / 4 + 1);
    overflow.push((OpId::F16Fma, u64::MAX, 0));
    for (op, n, m) in overflow {
        for_each_combo(|over, d, dev_i| {
            let dev = make_device(dev_i, u64::MAX);
            let exec = Exec::new(&dev, d);
            let (r, untouched) = call(&exec, op, over, n, m);
            let tag = format!("{op:?} {n} {m} {over:?} {d:?} {dev_i}");
            assert!(is_rejected(&r), "{tag}: {r:?}");
            assert_eq!(dev.query_ex_calls(), 0, "{tag}");
            assert_eq!(dev.memory_calls(), 0, "{tag}");
            assert_eq!(dev.queue_calls(), 0, "{tag}");
            assert!(untouched, "{tag}");
        });
    }
    // 一つ手前は、どの入口も host の列の長さが `n`（または `m`）と違うので `InvalidArgument`。
    for (op, n, m) in cases(u64::MAX / 2, u64::MAX / 4, u64::MAX / 4) {
        for_each_combo(|over, d, dev_i| {
            let dev = make_device(dev_i, u64::MAX);
            let exec = Exec::new(&dev, d);
            let (r, untouched) = call(&exec, op, over, n, m);
            let tag = format!("{op:?} {n} {m} {over:?} {d:?} {dev_i}");
            assert!(is_rejected(&r), "{tag}: {r:?}");
            assert_eq!(dev.query_ex_calls(), 0, "{tag}");
            assert_eq!(dev.memory_calls(), 0, "{tag}");
            assert_eq!(dev.queue_calls(), 0, "{tag}");
            assert!(untouched, "{tag}");
        });
    }
}

#[test]
fn rejected_call_does_not_block_next_call() {
    for (bad, good) in cases(33, 17, 17).into_iter().zip(cases(32, 16, 16)) {
        let dev = make_device(0, LIMIT);
        let exec = Exec::new(&dev, BOTH);
        let tag = format!("{:?} {} {}", bad.0, bad.1, bad.2);
        let (r, _) = call(&exec, bad.0, None, bad.1, bad.2);
        assert!(is_rejected(&r), "{tag}: {r:?}");
        let (r, _) = call(&exec, good.0, None, good.1, good.2);
        assert!(!is_rejected(&r), "{tag}: {r:?}");
        assert_eq!(dev.query_ex_calls(), 1, "{tag}");
    }
}
