//! 下-2 の公開のパスから `f16_fma` を host の形で呼び、列の長さが `n` と違えば `InvalidArgument` を返し、
//! 能力の問合せ・確保・投入の数えが 0 で出力が番兵のままのこと、長さが揃えば能力の問合せに届くことを確かめる（TASK-930）。
//! `subgroup_sum_u32`・`subgroup_sum_f32` の `x`・`out` も同じ形で確かめる（TASK-931）。
//! `atomic_add` の `x`・`out` も同じ形で確かめる（TASK-932）。
//! 置き場の説明は docs/exec-cpu-route.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::route_evidence::{SENTINEL_BYTE, is_untouched};
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OptIn, Outcome, SubgroupSumF32Operands,
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
const DEVICES: usize = 3;
/// 要素数。
const N: u64 = 4;

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

#[test]
fn mismatched_host_column_is_rejected() {
    let s16 = u16::from_ne_bytes([SENTINEL_BYTE; 2]);
    for bad in 0..4 {
        for bad_len in [3usize, 5] {
            for_each_combo(|over, d, dev_i| {
                let len = |i: usize| if i == bad { bad_len } else { N as usize };
                let (a, b, c) = (vec![1u16; len(0)], vec![1u16; len(1)], vec![1u16; len(2)]);
                let mut out = vec![s16; len(3)];
                let dev = make_device(dev_i);
                let exec = Exec::new(&dev, d);
                let r = exec.f16_fma(
                    over,
                    N,
                    F16FmaOperands::Host {
                        a: &a,
                        b: &b,
                        c: &c,
                        out: &mut out,
                    },
                );
                let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_ne_bytes()).collect();
                let tag = format!("{bad} {bad_len} {over:?} {d:?} {dev_i}");
                assert!(
                    matches!(r, Err(ContractError::InvalidArgument(_))),
                    "{tag}: {r:?}"
                );
                assert_eq!(dev.query_ex_calls(), 0, "{tag}");
                assert_eq!(dev.memory_calls(), 0, "{tag}");
                assert_eq!(dev.queue_calls(), 0, "{tag}");
                assert!(is_untouched(&bytes), "{tag}");
            });
        }
    }
}

#[test]
fn matching_host_columns_reach_route_choice() {
    let s16 = u16::from_ne_bytes([SENTINEL_BYTE; 2]);
    for_each_combo(|over, d, dev_i| {
        let n = N as usize;
        let (a, b, c) = (vec![1u16; n], vec![1u16; n], vec![1u16; n]);
        let mut out = vec![s16; n];
        let dev = make_device(dev_i);
        let exec = Exec::new(&dev, d);
        let r = exec.f16_fma(
            over,
            N,
            F16FmaOperands::Host {
                a: &a,
                b: &b,
                c: &c,
                out: &mut out,
            },
        );
        let tag = format!("{over:?} {d:?} {dev_i}");
        assert!(
            !matches!(r, Err(ContractError::InvalidArgument(_))),
            "{tag}: {r:?}"
        );
        assert_eq!(dev.query_ex_calls(), 1, "{tag}");
    });
}

/// subgroup の総和の 2 つの入口を host の形で 1 回呼ぶ。`x` の長さ `x_len`、`out` の長さ `out_len`（番兵で埋める）。
/// 戻り値は結果と、`out` が番兵のままかどうか。
fn call_subgroup(
    exec: &Exec<'_>,
    over: Option<OptIn>,
    f32_entry: bool,
    n: u64,
    x_len: usize,
    out_len: usize,
) -> (Result<Outcome, ContractError>, bool) {
    let s32 = u32::from_ne_bytes([SENTINEL_BYTE; 4]);
    if f32_entry {
        let x = vec![1.0f32; x_len];
        let mut out = vec![f32::from_bits(s32); out_len];
        let r = exec.subgroup_sum_f32(
            over,
            n,
            SubgroupSumF32Operands::Host {
                x: &x,
                out: &mut out,
            },
        );
        let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_bits().to_ne_bytes()).collect();
        (r, is_untouched(&bytes))
    } else {
        let x = vec![1u32; x_len];
        let mut out = vec![s32; out_len];
        let r = exec.subgroup_sum_u32(
            over,
            n,
            SubgroupSumU32Operands::Host {
                x: &x,
                out: &mut out,
            },
        );
        let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_ne_bytes()).collect();
        (r, is_untouched(&bytes))
    }
}

#[test]
fn mismatched_subgroup_columns_are_rejected() {
    for (x_len, out_len) in [(256usize, 2usize), (258, 2), (257, 1), (257, 3)] {
        for f32_entry in [false, true] {
            for_each_combo(|over, d, dev_i| {
                let dev = make_device(dev_i);
                let exec = Exec::new(&dev, d);
                let (r, untouched) = call_subgroup(&exec, over, f32_entry, 257, x_len, out_len);
                let tag = format!("{x_len} {out_len} {f32_entry} {over:?} {d:?} {dev_i}");
                assert!(
                    matches!(r, Err(ContractError::InvalidArgument(_))),
                    "{tag}: {r:?}"
                );
                assert_eq!(dev.query_ex_calls(), 0, "{tag}");
                assert_eq!(dev.memory_calls(), 0, "{tag}");
                assert_eq!(dev.queue_calls(), 0, "{tag}");
                assert!(untouched, "{tag}");
            });
        }
    }
}

#[test]
fn matching_subgroup_columns_reach_route_choice() {
    for f32_entry in [false, true] {
        for_each_combo(|over, d, dev_i| {
            let dev = make_device(dev_i);
            let exec = Exec::new(&dev, d);
            let (r, _) = call_subgroup(&exec, over, f32_entry, 257, 257, 2);
            let tag = format!("{f32_entry} {over:?} {d:?} {dev_i}");
            assert!(
                !matches!(r, Err(ContractError::InvalidArgument(_))),
                "{tag}: {r:?}"
            );
            assert_eq!(dev.query_ex_calls(), 1, "{tag}");
        });
    }
}

/// `atomic_add` を host の形で 1 回呼ぶ。`x` の長さ `x_len`、`out` の長さ `out_len`（番兵で埋める）。
/// 戻り値は結果と、`out` が番兵のままかどうか。
fn call_atomic(
    exec: &Exec<'_>,
    over: Option<OptIn>,
    n: u64,
    m: u64,
    x_len: usize,
    out_len: usize,
) -> (Result<Outcome, ContractError>, bool) {
    let s32 = u32::from_ne_bytes([SENTINEL_BYTE; 4]);
    let x = vec![1.0f32; x_len];
    let mut out = vec![f32::from_bits(s32); out_len];
    let r = exec.atomic_add(
        over,
        n,
        m,
        AtomicAddOperands::Host {
            x: &x,
            out: &mut out,
        },
    );
    let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_bits().to_ne_bytes()).collect();
    (r, is_untouched(&bytes))
}

#[test]
fn mismatched_atomic_columns_are_rejected() {
    for (x_len, out_len) in [(7usize, 3usize), (9, 3), (8, 2), (8, 4)] {
        for_each_combo(|over, d, dev_i| {
            let dev = make_device(dev_i);
            let exec = Exec::new(&dev, d);
            let (r, untouched) = call_atomic(&exec, over, 8, 3, x_len, out_len);
            let tag = format!("{x_len} {out_len} {over:?} {d:?} {dev_i}");
            assert!(
                matches!(r, Err(ContractError::InvalidArgument(_))),
                "{tag}: {r:?}"
            );
            assert_eq!(dev.query_ex_calls(), 0, "{tag}");
            assert_eq!(dev.memory_calls(), 0, "{tag}");
            assert_eq!(dev.queue_calls(), 0, "{tag}");
            assert!(untouched, "{tag}");
        });
    }
}

#[test]
fn matching_atomic_columns_reach_route_choice() {
    for (n, m) in [(8u64, 3u64), (2, 5)] {
        for_each_combo(|over, d, dev_i| {
            let dev = make_device(dev_i);
            let exec = Exec::new(&dev, d);
            let x_len = usize::try_from(n).unwrap();
            let out_len = usize::try_from(m).unwrap();
            let (r, _) = call_atomic(&exec, over, n, m, x_len, out_len);
            let tag = format!("{n} {m} {over:?} {d:?} {dev_i}");
            assert!(
                !matches!(r, Err(ContractError::InvalidArgument(_))),
                "{tag}: {r:?}"
            );
            assert_eq!(dev.query_ex_calls(), 1, "{tag}");
        });
    }
}
