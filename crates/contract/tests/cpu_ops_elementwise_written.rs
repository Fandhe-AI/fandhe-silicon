//! `cpu_ops` の要素ごと演算が、端数を含む長さの入力で出力の全要素を書き、各要素が式とビットまで
//! 一致することを確かめる外の試験（TASK-2455 のポインタ）。feature `cpu-ops` と `test-support` が
//! 有効な組でだけ走る。入力の生成器は `fandhe_silicon_core::parity::gemm_inputs`（TASK-24 のポインタ）。

use fandhe_silicon_contract::cpu_ops::{
    CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp,
};
use fandhe_silicon_core::parity::gemm_inputs::{GEMM_INPUT_SEED, SplitMix64};

/// 入力の長さ（4 で割って 3 余る）。
const LEN: usize = 1_000_003;

/// 出力を埋める番兵のビット列。
const SENTINEL_BITS: u32 = 0x7fc0_beef;

/// `0.0` と等しい値を飛ばしながら `LEN` 個の入力を作る。
fn input() -> Vec<f32> {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let mut v = Vec::with_capacity(LEN);
    while v.len() < LEN {
        let x = rng.next_signed_unit_f32();
        if x != 0.0 {
            v.push(x);
        }
    }
    v
}

/// 出力を番兵で埋めてから `elementwise_scalar`（`simd` が偽）または `elementwise_simd` を呼ぶ。
fn run<T: CpuOps>(ops: &T, simd: bool, op: UnaryElementwiseOp, a: &[f32]) -> Vec<f32> {
    let mut out = vec![f32::from_bits(SENTINEL_BITS); a.len()];
    if simd {
        ops.elementwise_simd(op, a, &mut out).unwrap();
    } else {
        ops.elementwise_scalar(op, a, &mut out).unwrap();
    }
    out
}

/// 型 2 つ × メソッド 2 つの 4 通りの出力。
fn run_all(op: UnaryElementwiseOp, a: &[f32]) -> Vec<(&'static str, Vec<f32>)> {
    vec![
        ("default-scalar", run(&DefaultCpuOps, false, op, a)),
        ("default-simd", run(&DefaultCpuOps, true, op, a)),
        ("reference-scalar", run(&ReferenceCpuOps, false, op, a)),
        ("reference-simd", run(&ReferenceCpuOps, true, op, a)),
    ]
}

fn relu(x: f32) -> f32 {
    x.max(0.0)
}

fn add_half(x: f32) -> f32 {
    x + 0.5
}

/// 番兵が 0 件で、全要素が式とビットまで一致することを確かめる。
fn assert_all_written(label: &str, a: &[f32], out: &[f32], f: fn(f32) -> f32) {
    assert_eq!(out.len(), a.len(), "{label}: len");
    let left = out.iter().filter(|x| x.to_bits() == SENTINEL_BITS).count();
    assert_eq!(left, 0, "{label}: sentinel count at len {}", a.len());
    for (i, (x, y)) in a.iter().zip(out).enumerate() {
        assert_eq!(
            y.to_bits(),
            f(*x).to_bits(),
            "{label}: index {i} input {x} output {y}"
        );
    }
}

/// 端数 0〜3 の長さと 0〜9 の接頭辞の長さ。
fn prefix_lens() -> Vec<usize> {
    let mut v: Vec<usize> = (0..=9).collect();
    v.extend([1_000_000, 1_000_001, 1_000_002, 1_000_003]);
    v
}

/// 演算と、その式。
type Case = (UnaryElementwiseOp, fn(f32) -> f32);

/// 入力が長さ 1,000,003 で、端数が 3 で、特殊な値を含まない。
#[test]
fn input_has_no_special_values() {
    let a = input();
    assert_eq!(a.len(), 1_000_003);
    assert_eq!(a.len(), 4 * 250_000 + 3);
    assert_eq!(a.len() % 4, 3);
    assert!(a.iter().all(|x| x.is_finite()));
    assert_eq!(a.iter().filter(|x| **x == 0.0).count(), 0);
}

/// `Relu` の 4 通りで、番兵が 0 件で `x.max(0.0)` とビットまで一致する（CPU-18）。
#[test]
fn relu_writes_every_element() {
    let a = input();
    for n in prefix_lens() {
        for (label, out) in run_all(UnaryElementwiseOp::Relu, &a[..n]) {
            assert_all_written(label, &a[..n], &out, relu);
        }
    }
}

/// `AddScalar(0.5)` の 4 通りで、番兵が 0 件で `x + 0.5` とビットまで一致する（CPU-19）。
#[test]
fn add_scalar_writes_every_element() {
    let a = input();
    for n in prefix_lens() {
        for (label, out) in run_all(UnaryElementwiseOp::AddScalar(0.5), &a[..n]) {
            assert_all_written(label, &a[..n], &out, add_half);
        }
    }
}

/// 長さ 1,000,003 の末尾 3 要素と、長さ 6・7 の接頭辞の末尾の要素が書かれている（CPU-20）。
#[test]
fn last_elements_are_written() {
    let a = input();
    let cases: [(usize, std::ops::Range<usize>); 3] =
        [(1_000_003, 1_000_000..1_000_003), (6, 4..6), (7, 4..7)];
    let ops: [Case; 2] = [
        (UnaryElementwiseOp::Relu, relu),
        (UnaryElementwiseOp::AddScalar(0.5), add_half),
    ];
    for (n, range) in cases {
        for (op, f) in ops {
            for (label, out) in run_all(op, &a[..n]) {
                for i in range.clone() {
                    assert_ne!(out[i].to_bits(), SENTINEL_BITS, "{label}: index {i}");
                    assert_eq!(
                        out[i].to_bits(),
                        f(a[i]).to_bits(),
                        "{label}: index {i} len {n}"
                    );
                }
            }
        }
    }
}
