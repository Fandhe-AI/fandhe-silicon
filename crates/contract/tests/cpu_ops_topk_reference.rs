//! `cpu_ops` の部分 Top-k を、共通の crate の CPU 参照（`fandhe_silicon_core::parity::topk`）と
//! 比べる外の試験（TASK-1352 のポインタ）。feature `cpu-ops` と `test-support` が有効な組でだけ走る。
//! 公開のパスだけを使い、入力は固定の seed から決定的に作る。

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, ReferenceCpuOps};
use fandhe_silicon_core::parity::gemm_inputs::{GEMM_INPUT_SEED, SplitMix64};
use fandhe_silicon_core::parity::topk::{TopKEntry, compare_topk, reference_topk_f32};
use fandhe_silicon_core::parity::topk_inputs::SIGNED_ZERO_INPUTS;

/// 比べる 1 件の入力（名前・スコアの列・k）。
type Case = (&'static str, Vec<f32>, usize);

/// 呼ぶメソッド。
#[derive(Clone, Copy)]
enum Method {
    Scalar,
    Simd,
}

const METHODS: [Method; 2] = [Method::Scalar, Method::Simd];

fn batch_input() -> Vec<f32> {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let v: Vec<f32> = (0..20_000).map(|_| rng.next_signed_unit_f32()).collect();
    assert!(v.iter().all(|x| x.is_finite()));
    v
}

fn ties_input() -> Vec<f32> {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let v: Vec<f32> = (0..200usize)
        .map(|i| {
            if i % 10 == 3 {
                0.0
            } else {
                ((rng.next_u64() >> 61) as f32) + 1.0
            }
        })
        .collect();
    assert!(v.iter().all(|x| x.is_finite()));
    assert_eq!(v.iter().filter(|x| x.to_bits() == 0x0000_0000).count(), 20);
    assert_eq!(v.iter().filter(|x| x.to_bits() == 0x8000_0000).count(), 0);
    let r = reference_topk_f32(&v, 11).unwrap();
    assert_eq!(r[9].score_bits(), r[10].score_bits());
    v
}

fn non_finite_input() -> Vec<f32> {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let mut v: Vec<f32> = (0..200).map(|_| rng.next_signed_unit_f32()).collect();
    v[5] = f32::NAN;
    v[17] = f32::from_bits(0xffc0_0000);
    v[30] = f32::INFINITY;
    v[77] = f32::NEG_INFINITY;
    assert_eq!(v.iter().filter(|x| x.is_nan()).count(), 2);
    assert_eq!(v.iter().filter(|x| **x == f32::INFINITY).count(), 1);
    assert_eq!(v.iter().filter(|x| **x == f32::NEG_INFINITY).count(), 1);
    v
}

fn cases() -> Vec<Case> {
    vec![
        ("batch", batch_input(), 64),
        ("ties", ties_input(), 10),
        ("non_finite", non_finite_input(), 10),
    ]
}

/// 4 つの組（2 型 × 2 メソッド）の名前、実装、メソッド。
fn combos() -> Vec<(&'static str, Box<dyn CpuOps>, Method)> {
    let mut v: Vec<(&'static str, Box<dyn CpuOps>, Method)> = Vec::new();
    for m in METHODS {
        v.push(("default", Box::new(DefaultCpuOps), m));
        v.push(("reference", Box::new(ReferenceCpuOps), m));
    }
    v
}

fn run(ops: &dyn CpuOps, method: Method, scores: &[f32], k: usize) -> Vec<(u32, f32)> {
    let mut out = vec![(7u32, 7.0f32); k];
    let r = match method {
        Method::Scalar => ops.topk_partial_scalar(scores, k, &mut out),
        Method::Simd => ops.topk_partial_simd(scores, k, &mut out),
    };
    assert!(r.is_ok());
    out
}

fn to_bits(out: &[(u32, f32)]) -> Vec<(u32, u32)> {
    out.iter().map(|(s, v)| (*s, v.to_bits())).collect()
}

fn to_entries(out: &[(u32, f32)]) -> Vec<TopKEntry> {
    out.iter().map(|(s, v)| TopKEntry::new(*s, *v)).collect()
}

fn assert_matches_reference(name: &str, scores: &[f32], k: usize, out: &[(u32, f32)]) {
    let r = reference_topk_f32(scores, k).unwrap();
    assert_eq!(r.len(), k, "{name}");
    assert!(
        compare_topk(&r, &to_entries(out)).unwrap().is_match(),
        "{name}"
    );
}

#[test]
fn topk_scalar_matches_cpu_reference() {
    // CPU-10
    for (name, scores, k) in cases() {
        let out = run(&DefaultCpuOps, Method::Scalar, &scores, k);
        assert_matches_reference(name, &scores, k, &out);
    }
    for input in SIGNED_ZERO_INPUTS {
        let (scores, k) = (input.scores(), input.k());
        let out = run(&DefaultCpuOps, Method::Scalar, scores, k);
        assert_matches_reference(input.name(), scores, k, &out);
        let expected = input.expected();
        assert!(
            compare_topk(&expected, &to_entries(&out))
                .unwrap()
                .is_match(),
            "{}",
            input.name()
        );
    }
}

#[test]
fn topk_excludes_non_finite() {
    // CPU-11: この入力はどれも有限の件数が k 以上で、out に番兵（スコアが -Inf）が入らない。
    for (name, scores, k) in cases() {
        for (label, ops, method) in combos() {
            let out = run(ops.as_ref(), method, &scores, k);
            assert!(out.iter().all(|(_, v)| v.is_finite()), "{name} {label}");
            if name == "non_finite" {
                for bad in [5u32, 17, 30, 77] {
                    assert!(out.iter().all(|(s, _)| *s != bad), "{name} {label}");
                }
            }
        }
    }
}

#[test]
fn topk_fills_first_k() {
    // CPU-14
    for (name, scores, k) in cases() {
        for (label, ops, method) in combos() {
            let out = run(ops.as_ref(), method, &scores, k);
            assert_eq!(out.len(), k);
            for (s, v) in &out {
                assert!(v.is_finite(), "{name} {label}");
                assert_ne!(*s, ReferenceCpuOps::SENTINEL.0, "{name} {label}");
            }
        }
    }
}

#[test]
fn topk_simd_matches_scalar_bytes() {
    // CPU-16
    let mut all: Vec<(String, Vec<f32>, usize)> = cases()
        .into_iter()
        .map(|(n, s, k)| (n.to_string(), s, k))
        .collect();
    for input in SIGNED_ZERO_INPUTS {
        all.push((input.name().to_string(), input.scores().to_vec(), input.k()));
    }
    for (name, scores, k) in all {
        let ops_list: [(&str, Box<dyn CpuOps>); 2] = [
            ("default", Box::new(DefaultCpuOps)),
            ("reference", Box::new(ReferenceCpuOps)),
        ];
        for (label, ops) in ops_list {
            let a = run(ops.as_ref(), Method::Scalar, &scores, k);
            let b = run(ops.as_ref(), Method::Simd, &scores, k);
            assert_eq!(to_bits(&a), to_bits(&b), "{name} {label}");
        }
    }
}

#[test]
fn topk_reference_ops_follow_scalar_path() {
    // CPU-27
    for (name, scores, k) in cases() {
        let base = to_bits(&run(&DefaultCpuOps, Method::Scalar, &scores, k));
        for m in METHODS {
            let out = run(&ReferenceCpuOps, m, &scores, k);
            assert_eq!(to_bits(&out), base, "{name}");
        }
    }
}

#[test]
fn topk_longer_out_keeps_sentinel_tail() {
    // CPU-14: out が k より長く、有限のスコアの件数が k を超える入力。
    // 先頭の k 件は CPU 参照と一致し、k 件目より後の位置はすべて番兵のまま。
    let sentinel = ReferenceCpuOps::SENTINEL;
    for (name, scores, k) in [
        ("batch", batch_input(), 8usize),
        ("ties", ties_input(), 5),
        ("non_finite", non_finite_input(), 3),
    ] {
        let finite = scores.iter().filter(|x| x.is_finite()).count();
        assert!(finite > k, "{name}");
        let out_len = k + 7;
        for (label, ops, method) in combos() {
            let mut out = vec![(7u32, 7.0f32); out_len];
            let r = match method {
                Method::Scalar => ops.topk_partial_scalar(&scores, k, &mut out),
                Method::Simd => ops.topk_partial_simd(&scores, k, &mut out),
            };
            assert!(r.is_ok(), "{name} {label}");
            let (head, tail) = out.split_at(k);
            assert_matches_reference(name, &scores, k, head);
            for (s, v) in tail {
                assert_eq!(*s, sentinel.0, "{name} {label}");
                assert_eq!(v.to_bits(), sentinel.1.to_bits(), "{name} {label}");
            }
        }
    }
}
