//! `subgroup-sum`・`atomic-add` の CPU の参照の出力の形の確認（spec TASK-929 の 5/6。対象の項目は
//! FB-43・FB-44 のポインタ）。feature `test-support` の裏にだけ存在する検証用の道具を対象にする。
//!
//! 参照の実装（`skip(j).step_by(m)`・ブロック範囲の関数）とは別に、規則をそのまま書いた散布の計算
//! （`s[i / 256] += x[i]`・`o[i % m] += x[i]`）と、手計算した小さな入力で、出力の長さ・
//! どの入力がどの出力に足されるか・足される入力が無い要素の初期値を確かめる。

use fandhe_silicon_core::parity::fallback_compare::{F32SubnormalHandling, flush_subnormal_f32};
use fandhe_silicon_core::parity::fallback_inputs::{
    ATOMIC_ADD_OUTPUT_COUNTS, FALLBACK_LENGTHS, SUBGROUP_SUM_BLOCK, ax_pair_allowed,
    generate_atomic_add_ag, generate_atomic_add_as, generate_atomic_add_ax,
    generate_subgroup_sum_f32_fg, generate_subgroup_sum_f32_fs, generate_subgroup_sum_f32_fx,
    generate_subgroup_sum_u32_u,
};
use fandhe_silicon_core::parity::fallback_reference::{
    reference_atomic_add_f32, reference_subgroup_sum_f32, reference_subgroup_sum_u32,
    subgroup_sum_output_len,
};

const PRESERVE: F32SubnormalHandling = F32SubnormalHandling::Preserve;
const FLUSH: F32SubnormalHandling = F32SubnormalHandling::FlushToZero;
const HANDLINGS: [F32SubnormalHandling; 2] = [PRESERVE, FLUSH];

/// 値の一致を確かめる入力の長さの上限（debug の試験の実行時間を抑える。これより長い入力は長さだけ確かめる）。
const VALUE_CHECK_MAX_LEN: usize = 65_539;

fn norm_bits(x: f32) -> u32 {
    if x.is_nan() { 0x7fc0_0000 } else { x.to_bits() }
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|&v| norm_bits(v)).collect()
}

/// 規則をそのまま書いた散布。長さ `len` の f64 の列を +0.0 から始め、`slot_of(i)` に足す。
fn scatter_f32(
    x: &[f32],
    len: usize,
    slot_of: impl Fn(usize) -> usize,
    handling: F32SubnormalHandling,
) -> (Vec<f32>, Vec<f64>) {
    let flush = handling == FLUSH;
    let mut sums = vec![0.0_f64; len];
    let mut abs = vec![0.0_f64; len];
    for (i, &v) in x.iter().enumerate() {
        let v = if flush { flush_subnormal_f32(v) } else { v };
        let v = f64::from(v);
        sums[slot_of(i)] += v;
        abs[slot_of(i)] += v.abs();
    }
    let out = sums
        .iter()
        .map(|&s| {
            let r = s as f32;
            if flush { flush_subnormal_f32(r) } else { r }
        })
        .collect();
    (out, abs)
}

fn scatter_subgroup_f32(x: &[f32], h: F32SubnormalHandling) -> (Vec<f32>, Vec<f64>) {
    scatter_f32(x, x.len().div_ceil(256), |i| i / 256, h)
}

fn scatter_atomic_f32(x: &[f32], m: usize, h: F32SubnormalHandling) -> (Vec<f32>, Vec<f64>) {
    scatter_f32(x, m, |i| i % m, h)
}

fn scatter_subgroup_u32(x: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; x.len().div_ceil(256)];
    for (i, &v) in x.iter().enumerate() {
        out[i / 256] = out[i / 256].wrapping_add(v);
    }
    out
}

/// 整数値の f32 の入力用。i64 で足してから f32 にする（和が 2^24 未満の前提）。
fn scatter_int_f32(x: &[f32], len: usize, slot_of: impl Fn(usize) -> usize) -> Vec<f32> {
    let mut sums = vec![0i64; len];
    for (i, &v) in x.iter().enumerate() {
        sums[slot_of(i)] += v as i64;
    }
    sums.iter().map(|&s| s as f32).collect()
}

fn check_f32_against_scatter(
    actual: &(Vec<f32>, Vec<f64>),
    expected: &(Vec<f32>, Vec<f64>),
    what: &str,
) {
    assert_eq!(actual.0.len(), expected.0.len(), "length: {what}");
    assert_eq!(bits(&actual.0), bits(&expected.0), "values: {what}");
    let a: Vec<u64> = actual.1.iter().map(|v| v.to_bits()).collect();
    let e: Vec<u64> = expected.1.iter().map(|v| v.to_bits()).collect();
    assert_eq!(a, e, "abs sums: {what}");
}

fn sub_f32(x: &[f32], h: F32SubnormalHandling) -> (Vec<f32>, Vec<f64>) {
    let r = reference_subgroup_sum_f32(x, h).unwrap();
    (r.values().to_vec(), r.abs_sums().to_vec())
}

fn atom_f32(x: &[f32], m: usize, h: F32SubnormalHandling) -> (Vec<f32>, Vec<f64>) {
    let r = reference_atomic_add_f32(x, m, h).unwrap();
    (r.values().to_vec(), r.abs_sums().to_vec())
}

fn series_u32(n: usize) -> Vec<u32> {
    (1..=n as u32).collect()
}

fn series_f32(n: usize) -> Vec<f32> {
    (1..=n).map(|i| i as f32).collect()
}

fn one_hot_f32(n: usize, p: usize) -> Vec<f32> {
    let mut x = vec![0.0; n];
    x[p] = 1.0;
    x
}

#[test]
fn subgroup_sum_output_len_hand_checked() {
    let cases = [
        (0, 0),
        (1, 1),
        (31, 1),
        (32, 1),
        (33, 1),
        (64, 1),
        (65, 1),
        (255, 1),
        (256, 1),
        (257, 2),
        (511, 2),
        (512, 2),
        (513, 3),
    ];
    assert_eq!(SUBGROUP_SUM_BLOCK, 256);
    for (n, len) in cases {
        assert_eq!(subgroup_sum_output_len(n), len, "n={n}");
        assert_eq!(
            reference_subgroup_sum_u32(&vec![1u32; n]).unwrap().len(),
            len
        );
        for h in HANDLINGS {
            let r = reference_subgroup_sum_f32(&vec![1.0f32; n], h).unwrap();
            assert_eq!(r.len(), len, "n={n}");
            assert_eq!(r.abs_sums().len(), len, "n={n}");
        }
    }
}

#[test]
fn subgroup_sum_one_hot_marks_block() {
    for n in [1usize, 255, 256, 257, 513] {
        for p in [0usize, 255, 256, 257, n - 1] {
            if p >= n {
                continue;
            }
            let mut xu = vec![0u32; n];
            xu[p] = 1;
            let got = reference_subgroup_sum_u32(&xu).unwrap();
            for (j, &v) in got.iter().enumerate() {
                assert_eq!(v, u32::from(j == p / 256), "u32 n={n} p={p} j={j}");
            }
            for h in HANDLINGS {
                let r = reference_subgroup_sum_f32(&one_hot_f32(n, p), h).unwrap();
                for (j, &v) in r.values().iter().enumerate() {
                    let want = if j == p / 256 { 1.0 } else { 0.0 };
                    assert_eq!(v, want, "f32 n={n} p={p} j={j}");
                    assert_eq!(v.to_bits() >> 31, 0, "sign n={n} p={p} j={j}");
                }
            }
        }
    }
}

#[test]
fn subgroup_sum_arithmetic_series_hand_checked() {
    let cases: [(usize, &[u32]); 5] = [
        (1, &[1]),
        (255, &[32640]),
        (256, &[32896]),
        (257, &[32896, 257]),
        (513, &[32896, 98432, 513]),
    ];
    for (n, expected) in cases {
        assert_eq!(
            reference_subgroup_sum_u32(&series_u32(n)).unwrap(),
            expected,
            "u32 n={n}"
        );
        let ef: Vec<f32> = expected.iter().map(|&v| v as f32).collect();
        let ea: Vec<f64> = expected.iter().map(|&v| f64::from(v)).collect();
        for h in HANDLINGS {
            let r = reference_subgroup_sum_f32(&series_f32(n), h).unwrap();
            assert_eq!(r.values(), ef.as_slice(), "f32 n={n}");
            assert_eq!(r.abs_sums(), ea.as_slice(), "abs n={n}");
        }
    }
}

#[test]
fn atomic_add_small_hand_examples() {
    let x = [1.0f32, 2.0, 3.0, 4.0, 5.0];
    let cases: [(usize, &[f32]); 5] = [
        (1, &[15.0]),
        (2, &[9.0, 6.0]),
        (3, &[5.0, 7.0, 3.0]),
        (5, &[1.0, 2.0, 3.0, 4.0, 5.0]),
        (7, &[1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0.0]),
    ];
    for (m, expected) in cases {
        let ea: Vec<f64> = expected.iter().map(|&v| f64::from(v)).collect();
        for h in HANDLINGS {
            let r = reference_atomic_add_f32(&x, m, h).unwrap();
            assert_eq!(r.values(), expected, "m={m}");
            // 入力がすべて正なので Σ|x| は和と同じ。
            assert_eq!(r.abs_sums(), ea.as_slice(), "abs m={m}");
        }
    }
}

#[test]
fn atomic_add_length_is_m_and_starts_from_positive_zero() {
    for h in HANDLINGS {
        // n < m: 末尾は足される入力が無く、+0.0 のまま。
        let r = reference_atomic_add_f32(&[7.0], 4096, h).unwrap();
        assert_eq!(r.len(), 4096);
        assert_eq!(r.values()[0], 7.0);
        assert!(r.values()[1..].iter().all(|v| v.to_bits() == 0));
        assert!(r.abs_sums()[1..].iter().all(|v| v.to_bits() == 0));

        // n > m: 先頭の slot に x[0] + x[64] が足される。
        let x: Vec<f32> = (0..65).map(|i| (i + 1) as f32).collect();
        let r = reference_atomic_add_f32(&x, 64, h).unwrap();
        assert_eq!(r.len(), 64);
        assert_eq!(r.values()[0], 1.0 + 65.0);
        assert_eq!(&r.values()[1..], &x[1..64]);

        // n = m: 各 slot に 1 つずつ。
        let r = reference_atomic_add_f32(&x[..64], 64, h).unwrap();
        assert_eq!(r.values(), &x[..64]);

        // すべて -0.0 でも出力は +0.0（初期値 +0.0 に足すため）。
        let r = reference_atomic_add_f32(&[-0.0; 5], 2, h).unwrap();
        assert_eq!(r.len(), 2);
        assert!(r.values().iter().all(|v| v.to_bits() == 0));
    }
}

#[test]
fn atomic_add_one_hot_marks_slot() {
    for n in [1usize, 65, 257] {
        for m in [1usize, 2, 64, 4096] {
            for p in [0, n / 2, n - 1] {
                for h in HANDLINGS {
                    let r = reference_atomic_add_f32(&one_hot_f32(n, p), m, h).unwrap();
                    assert_eq!(r.len(), m);
                    for (j, &v) in r.values().iter().enumerate() {
                        let want = if j == p % m { 1.0 } else { 0.0 };
                        assert_eq!(v, want, "n={n} m={m} p={p} j={j}");
                        assert_eq!(v.to_bits() >> 31, 0, "sign n={n} m={m} p={p} j={j}");
                    }
                }
            }
        }
    }
}

#[test]
fn subgroup_sum_matches_scatter_for_generated_inputs() {
    for n in FALLBACK_LENGTHS {
        let len = n.div_ceil(256);
        let u = generate_subgroup_sum_u32_u(n).unwrap();
        let got = reference_subgroup_sum_u32(&u).unwrap();
        assert_eq!(got.len(), len, "U n={n}");
        if n > VALUE_CHECK_MAX_LEN {
            continue;
        }
        assert_eq!(got, scatter_subgroup_u32(&u), "U n={n}");
        let fx = generate_subgroup_sum_f32_fx(n).unwrap();
        let exp = scatter_int_f32(&fx, len, |i| i / 256);
        let fg = generate_subgroup_sum_f32_fg(n).unwrap();
        let fs = generate_subgroup_sum_f32_fs(n).unwrap();
        for h in HANDLINGS {
            let r = sub_f32(&fx, h);
            assert_eq!(bits(&r.0), bits(&exp), "FX n={n}");
            check_f32_against_scatter(&r, &scatter_subgroup_f32(&fx, h), "FX");
            check_f32_against_scatter(&sub_f32(&fg, h), &scatter_subgroup_f32(&fg, h), "FG");
            check_f32_against_scatter(&sub_f32(&fs, h), &scatter_subgroup_f32(&fs, h), "FS");
        }
    }
}

#[test]
fn atomic_add_matches_scatter_for_generated_inputs() {
    for n in FALLBACK_LENGTHS {
        let ag = generate_atomic_add_ag(n).unwrap();
        if n > VALUE_CHECK_MAX_LEN {
            for m in ATOMIC_ADD_OUTPUT_COUNTS {
                for h in HANDLINGS {
                    assert_eq!(atom_f32(&ag, m, h).0.len(), m, "AG n={n} m={m}");
                }
            }
            continue;
        }
        let ax = generate_atomic_add_ax(n).unwrap();
        for m in ATOMIC_ADD_OUTPUT_COUNTS {
            let a_s = generate_atomic_add_as(n, m).unwrap();
            for h in HANDLINGS {
                if ax_pair_allowed(n, m) {
                    let exp = scatter_int_f32(&ax, m, |i| i % m);
                    let r = atom_f32(&ax, m, h);
                    assert_eq!(bits(&r.0), bits(&exp), "AX n={n} m={m}");
                }
                let r = atom_f32(&ag, m, h);
                check_f32_against_scatter(&r, &scatter_atomic_f32(&ag, m, h), "AG");
                let r = atom_f32(&a_s, m, h);
                check_f32_against_scatter(&r, &scatter_atomic_f32(&a_s, m, h), "AS");
            }
        }
    }
}
