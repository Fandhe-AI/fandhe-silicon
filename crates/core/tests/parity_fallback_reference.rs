//! `f16-fma`・`subgroup-sum`・`atomic-add` の CPU の参照と f64 から binary16 への変換の確認
//! （spec TASK-929 の 4/6）。feature `test-support` の裏にだけ存在する検証用の道具を対象にする。
//!
//! sha256 の値は、本リポの製品のコードが出す既知の出力として固定する
//! （`parity_fallback_inputs.rs` と同じ流儀）。

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::binary16::{
    binary16_bits_to_f32, binary16_bits_to_f64, f32_to_binary16_bits, f64_to_binary16_bits,
};
use fandhe_silicon_core::parity::cpu_reference::ReferenceError;
use fandhe_silicon_core::parity::fallback_compare::{F32SubnormalHandling, reference_sum_f32};
use fandhe_silicon_core::parity::fallback_inputs::{
    ATOMIC_ADD_OUTPUT_COUNTS, F16FmaInputs, FALLBACK_INPUT_SEED, FALLBACK_LENGTHS,
    SUBGROUP_SUM_BLOCK, generate_atomic_add_as, generate_f16_fma_e1, generate_f16_fma_e2,
    generate_f16_fma_r, generate_f16_fma_s, generate_subgroup_sum_f32_fs,
    generate_subgroup_sum_u32_u,
};
use fandhe_silicon_core::parity::fallback_reference::{
    reference_atomic_add_f32, reference_f16_fma, reference_f16_fma_slice,
    reference_subgroup_sum_f32, reference_subgroup_sum_u32, subgroup_sum_block_range,
    subgroup_sum_output_len,
};
use fandhe_silicon_core::parity::gemm_inputs::SplitMix64;

const PRESERVE: F32SubnormalHandling = F32SubnormalHandling::Preserve;
const FLUSH: F32SubnormalHandling = F32SubnormalHandling::FlushToZero;

fn hex(bytes: &[u8]) -> String {
    sha256_bytes(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn u16_digest(values: &[u16]) -> String {
    let bytes: Vec<u8> = values.iter().flat_map(|x| x.to_le_bytes()).collect();
    hex(&bytes)
}

fn u32_digest(values: &[u32]) -> String {
    let bytes: Vec<u8> = values.iter().flat_map(|x| x.to_le_bytes()).collect();
    hex(&bytes)
}

/// NaN のペイロードは環境で変わりうるので、NaN は 1 つの値に揃えてから数える。
fn f32_digest(values: &[f32]) -> String {
    let bytes: Vec<u8> = values
        .iter()
        .flat_map(|x| {
            let bits = if x.is_nan() { 0x7fc0_0000 } else { x.to_bits() };
            bits.to_le_bytes()
        })
        .collect();
    hex(&bytes)
}

#[test]
fn f64_conversion_round_trips_every_binary16() {
    for b in 0..=u16::MAX {
        let wide = binary16_bits_to_f64(b);
        if wide.is_nan() {
            assert!(f64_to_binary16_bits(wide) & 0x7c00 == 0x7c00);
            assert!(f64_to_binary16_bits(wide) & 0x03ff != 0);
        } else {
            assert_eq!(f64_to_binary16_bits(wide), b, "bits {b:#06x}");
        }
    }
}

#[test]
fn f64_conversion_matches_f32_conversion_on_f32_values() {
    let mut inputs: Vec<f32> = Vec::new();
    // 全 binary16 の値と、隣り合う値の中点。
    for b in 0..=u16::MAX {
        let v = binary16_bits_to_f32(b);
        if v.is_nan() {
            continue;
        }
        inputs.push(v);
        let next = binary16_bits_to_f32(b.wrapping_add(1));
        if next.is_finite() && v.is_finite() {
            inputs.push((v + next) / 2.0);
        }
    }
    let edges = [
        65504.0f32,
        65519.996,
        65520.0,
        65536.0,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        0.0,
        -0.0,
        2.0f32.powi(-24),
        2.0f32.powi(-25),
        f32::from_bits(2.0f32.powi(-25).to_bits() + 1),
        2.0f32.powi(-14),
        f32::from_bits(2.0f32.powi(-14).to_bits() - 1),
        f32::MIN_POSITIVE,
        f32::from_bits(1),
    ];
    for e in edges {
        inputs.push(e);
        inputs.push(-e);
    }
    let mut rng = SplitMix64::new(FALLBACK_INPUT_SEED);
    for _ in 0..200_000 {
        let v = f32::from_bits(rng.next_u64() as u32);
        if !v.is_nan() {
            inputs.push(v);
        }
    }
    for v in inputs {
        assert_eq!(
            f64_to_binary16_bits(f64::from(v)),
            f32_to_binary16_bits(v),
            "value bits {:#010x}",
            v.to_bits()
        );
    }
}

#[test]
fn f64_conversion_rounds_once() {
    // 1 + 2^-11 + 2^-40 は binary16 の 1 と 1+2^-10 の中点を僅かに超える。直接なら 1+2^-10 へ上がる。
    let x = 1.0 + 2.0f64.powi(-11) + 2.0f64.powi(-40);
    assert_eq!(f64_to_binary16_bits(x), 0x3c01);
    // f32 を経由すると中点ちょうどに丸まってから偶数側の 1.0 へ落ちる（丸めが 2 回になる）。
    assert_eq!(f32_to_binary16_bits(x as f32), 0x3c00);
}

#[test]
fn f64_conversion_special_values() {
    assert_eq!(f64_to_binary16_bits(0.0), 0x0000);
    assert_eq!(f64_to_binary16_bits(-0.0), 0x8000);
    assert_eq!(f64_to_binary16_bits(f64::INFINITY), 0x7c00);
    assert_eq!(f64_to_binary16_bits(f64::NEG_INFINITY), 0xfc00);
    assert_eq!(f64_to_binary16_bits(f64::MAX), 0x7c00);
    assert_eq!(f64_to_binary16_bits(-f64::MAX), 0xfc00);
    // f64 の非正規化数は零へ。符号は保つ。
    assert_eq!(f64_to_binary16_bits(f64::from_bits(1)), 0x0000);
    assert_eq!(f64_to_binary16_bits(-f64::from_bits(1)), 0x8000);
    assert_eq!(f64_to_binary16_bits(f64::MIN_POSITIVE), 0x0000);
    // 2^-25 ちょうどは偶数側の 0、超えれば最小の非正規化数。
    assert_eq!(f64_to_binary16_bits(2.0f64.powi(-25)), 0x0000);
    assert_eq!(
        f64_to_binary16_bits(2.0f64.powi(-25) * (1.0 + 2.0f64.powi(-40))),
        0x0001
    );
    assert_eq!(f64_to_binary16_bits(2.0f64.powi(-24)), 0x0001);
    // 65520 から無限大。
    assert_eq!(f64_to_binary16_bits(65519.0), 0x7bff);
    assert_eq!(f64_to_binary16_bits(65520.0), 0x7c00);
    assert_eq!(f64_to_binary16_bits(-65520.0), 0xfc00);
    // NaN は NaN のまま。符号を保ち、静かな NaN の bit が立つ。
    let nan = f64_to_binary16_bits(f64::NAN);
    assert_eq!(nan & 0x7e00, 0x7e00);
    let neg_nan = f64_to_binary16_bits(f64::from_bits(f64::NAN.to_bits() | (1 << 63)));
    assert_eq!(neg_nan & 0x8000, 0x8000);
    // 信号 NaN の bit だけが立つ NaN も NaN のまま（無限大にならない）。
    let snan = f64_to_binary16_bits(f64::from_bits(0x7ff0_0000_0000_0001));
    assert_eq!(snan & 0x7c00, 0x7c00);
    assert_ne!(snan & 0x03ff, 0);
}

#[test]
fn f16_fma_hand_checked() {
    let h = |x: f32| f32_to_binary16_bits(x);
    // 整数の積和。
    assert_eq!(reference_f16_fma(h(2.0), h(3.0), h(1.0)), h(7.0));
    assert_eq!(reference_f16_fma(h(-2.0), h(3.0), h(1.0)), h(-5.0));
    // 零の符号。(+0)×(−1)+(+0) は +0、(−0)×(+1)+(−0) は −0。
    assert_eq!(reference_f16_fma(h(0.0), h(-1.0), h(0.0)), 0x0000);
    assert_eq!(reference_f16_fma(h(-0.0), h(1.0), h(-0.0)), 0x8000);
    // 非正規化数の結果: 2^-14 × 2^-10 + 0 = 2^-24。
    assert_eq!(reference_f16_fma(0x0400, h(2.0f32.powi(-10)), 0), 0x0001);
    // 桁あふれは無限大。
    assert_eq!(reference_f16_fma(h(65504.0), h(2.0), 0), 0x7c00);
    assert_eq!(reference_f16_fma(h(-65504.0), h(2.0), 0), 0xfc00);
    // 無限大 × 0 と、逆符号の無限大の和は NaN（正規の静かな NaN）。
    assert_eq!(reference_f16_fma(0x7c00, 0, 0), 0x7e00);
    assert_eq!(reference_f16_fma(0x7c00, h(1.0), 0xfc00), 0x7e00);
    // NaN の入力は NaN に揃う。
    assert_eq!(reference_f16_fma(0xfe01, h(1.0), 0), 0x7e00);
    // 積を丸めずに足す（融合）: (1+2^-10)^2 = 1 + 2^-9 + 2^-20 に −(1 + 2^-9) を足すと 2^-20。
    let a = 0x3c01;
    assert_eq!(
        reference_f16_fma(a, a, h(-(1.0 + 2.0f32.powi(-9)))),
        h(2.0f32.powi(-20))
    );
}

#[test]
fn f16_fma_e1_matches_exact_integer_arithmetic() {
    for n in FALLBACK_LENGTHS {
        if n > 65_539 {
            continue;
        }
        let inputs = generate_f16_fma_e1(n).unwrap();
        let out = reference_f16_fma_slice(&inputs).unwrap();
        assert_eq!(out.len(), n);
        for (i, &got) in out.iter().enumerate() {
            let a = binary16_bits_to_f32(inputs.a[i]) as i64;
            let b = binary16_bits_to_f32(inputs.b[i]) as i64;
            let c = binary16_bits_to_f32(inputs.c[i]) as i64;
            let exact = a * b + c;
            assert_eq!(got, f32_to_binary16_bits(exact as f32), "n {n} index {i}");
        }
    }
}

#[test]
fn f16_fma_slice_checks_lengths_and_empty() {
    let mut inputs = F16FmaInputs {
        a: vec![0, 0],
        b: vec![0],
        c: vec![0, 0],
    };
    assert_eq!(
        reference_f16_fma_slice(&inputs),
        Err(ReferenceError::LengthMismatch {
            input: "b",
            expected: 2,
            actual: 1
        })
    );
    inputs.b = vec![0, 0];
    inputs.c = vec![0, 0, 0];
    assert_eq!(
        reference_f16_fma_slice(&inputs),
        Err(ReferenceError::LengthMismatch {
            input: "c",
            expected: 2,
            actual: 3
        })
    );
    let empty = F16FmaInputs {
        a: vec![],
        b: vec![],
        c: vec![],
    };
    assert_eq!(reference_f16_fma_slice(&empty), Ok(vec![]));
}

#[test]
fn f16_fma_slice_matches_element_function() {
    let inputs = generate_f16_fma_r(1000).unwrap();
    let out = reference_f16_fma_slice(&inputs).unwrap();
    for (i, &got) in out.iter().enumerate() {
        assert_eq!(
            got,
            reference_f16_fma(inputs.a[i], inputs.b[i], inputs.c[i])
        );
    }
    // E2・S は NaN・無限大を含み、結果の NaN は正規の静かな NaN に揃う。
    for inputs in [
        generate_f16_fma_e2().unwrap(),
        generate_f16_fma_s().unwrap(),
    ] {
        let out = reference_f16_fma_slice(&inputs).unwrap();
        for v in out {
            if v & 0x7c00 == 0x7c00 && v & 0x03ff != 0 {
                assert_eq!(v, 0x7e00);
            }
        }
    }
}

#[test]
fn block_ranges_cover_input() {
    for n in FALLBACK_LENGTHS {
        let len = subgroup_sum_output_len(n);
        assert_eq!(len, n.div_ceil(256));
        let mut next = 0;
        for j in 0..len {
            let r = subgroup_sum_block_range(n, j).unwrap();
            assert_eq!(r.start, next);
            assert_eq!(r.end, (r.start + SUBGROUP_SUM_BLOCK).min(n));
            next = r.end;
        }
        assert_eq!(next, n);
        assert_eq!(subgroup_sum_block_range(n, len), None);
        assert_eq!(subgroup_sum_block_range(n, usize::MAX), None);
    }
    assert_eq!(subgroup_sum_output_len(0), 0);
    assert_eq!(subgroup_sum_block_range(0, 0), None);
    assert_eq!(subgroup_sum_block_range(257, 1), Some(256..257));
}

#[test]
fn subgroup_sum_u32_wraps_and_matches_blocks() {
    let mut x = vec![0u32; 257];
    x[0] = u32::MAX;
    x[1] = 3;
    x[256] = 9;
    assert_eq!(reference_subgroup_sum_u32(&x), Ok(vec![2, 9]));
    assert_eq!(reference_subgroup_sum_u32(&[]), Ok(vec![]));
    let x = generate_subgroup_sum_u32_u(1000).unwrap();
    let out = reference_subgroup_sum_u32(&x).unwrap();
    assert_eq!(out.len(), 4);
    for (j, v) in out.iter().enumerate() {
        let r = subgroup_sum_block_range(1000, j).unwrap();
        let sum = x[r].iter().fold(0u64, |a, &v| a + u64::from(v));
        assert_eq!(*v, sum as u32);
    }
}

#[test]
fn subgroup_sum_f32_matches_reference_sum_per_block() {
    for handling in [PRESERVE, FLUSH] {
        for n in [1usize, 255, 256, 257, 1000, 4096] {
            let x = generate_subgroup_sum_f32_fs(n).unwrap();
            let r = reference_subgroup_sum_f32(&x, handling).unwrap();
            assert_eq!(r.len(), subgroup_sum_output_len(n));
            assert_eq!(r.values().len(), r.abs_sums().len());
            for j in 0..r.len() {
                let range = subgroup_sum_block_range(n, j).unwrap();
                let direct = reference_sum_f32(x[range].iter().copied(), handling);
                assert_eq!(r.values()[j].to_bits(), direct.value().to_bits());
                assert_eq!(r.abs_sums()[j].to_bits(), direct.abs_sum().to_bits());
            }
        }
    }
    let empty = reference_subgroup_sum_f32(&[], PRESERVE).unwrap();
    assert!(empty.is_empty());
}

#[test]
fn atomic_add_matches_strided_sums() {
    for handling in [PRESERVE, FLUSH] {
        for n in [1usize, 31, 257, 1000] {
            for m in ATOMIC_ADD_OUTPUT_COUNTS {
                let x = generate_atomic_add_as(n, m).unwrap();
                let r = reference_atomic_add_f32(&x, m, handling).unwrap();
                assert_eq!(r.len(), m);
                for j in 0..m {
                    let direct = reference_sum_f32(x.iter().copied().skip(j).step_by(m), handling);
                    assert_eq!(r.values()[j].to_bits(), direct.value().to_bits());
                    assert_eq!(r.abs_sums()[j].to_bits(), direct.abs_sum().to_bits());
                    if j >= n {
                        assert_eq!(r.values()[j].to_bits(), 0.0f32.to_bits());
                        assert_eq!(r.abs_sums()[j], 0.0);
                    }
                }
            }
        }
    }
}

#[test]
fn atomic_add_small_hand_example_and_zero_outputs() {
    let x = [1.0f32, 2.0, 3.0, 4.0, 5.0];
    let r = reference_atomic_add_f32(&x, 2, PRESERVE).unwrap();
    assert_eq!(r.values(), &[9.0, 6.0]);
    assert_eq!(r.abs_sums(), &[9.0, 6.0]);
    let r = reference_atomic_add_f32(&x, 7, PRESERVE).unwrap();
    assert_eq!(r.values(), &[1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0.0]);
    assert!(
        reference_atomic_add_f32(&x, 0, PRESERVE)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn references_are_deterministic() {
    let inputs = generate_f16_fma_r(1000).unwrap();
    assert_eq!(
        reference_f16_fma_slice(&inputs),
        reference_f16_fma_slice(&inputs)
    );
    let x = generate_subgroup_sum_f32_fs(1000).unwrap();
    // NaN は == で等しくならないので、ビット列で比べる。
    let a = reference_subgroup_sum_f32(&x, PRESERVE).unwrap();
    let b = reference_subgroup_sum_f32(&x, PRESERVE).unwrap();
    assert_eq!(f32_digest(a.values()), f32_digest(b.values()));
    assert_eq!(a.abs_sums().len(), b.abs_sums().len());
    for (p, q) in a.abs_sums().iter().zip(b.abs_sums()) {
        assert_eq!(p.to_bits(), q.to_bits());
    }
}

#[test]
fn known_digests() {
    let e1 = generate_f16_fma_e1(1000).unwrap();
    let r = generate_f16_fma_r(1000).unwrap();
    let s = generate_f16_fma_s().unwrap();
    let e2 = generate_f16_fma_e2().unwrap();
    let u = generate_subgroup_sum_u32_u(1000).unwrap();
    let fs = generate_subgroup_sum_f32_fs(1000).unwrap();
    let a_s = generate_atomic_add_as(1000, 64).unwrap();
    let got = [
        u16_digest(&reference_f16_fma_slice(&e1).unwrap()),
        u16_digest(&reference_f16_fma_slice(&r).unwrap()),
        u16_digest(&reference_f16_fma_slice(&s).unwrap()),
        u16_digest(&reference_f16_fma_slice(&e2).unwrap()),
        u32_digest(&reference_subgroup_sum_u32(&u).unwrap()),
        f32_digest(reference_subgroup_sum_f32(&fs, PRESERVE).unwrap().values()),
        f32_digest(reference_atomic_add_f32(&a_s, 64, FLUSH).unwrap().values()),
    ];
    let want: [&str; 7] = [
        "c8382502043f4ebd3254344b89ad5d970463bb7721b15a6cda39464d9eb8b6af",
        "15f9d3aeb1e84adbc22a0bf181d968cdf958cd0fb8fc85960be6117c796f6a0e",
        "696b9ab9dc4f25232ec93be33083b0f2093628c245764f23db1a88ad1b3864f2",
        "79aeeaf4676e29698d59b59d8d1c66b1e3ccf9dd9e7a4546222f9f691b45dd4c",
        "6d67c0f3d880e145efd6d5ee7158b79cea63eb948d0a75493532207e3c19a00e",
        "cf033069677bcd279820b46a5cead684b689ea8f10f9414510a6ed83fa9fcf26",
        "476925045208872886683bf4e1965ddeb571c8b2d32cd34ca8aa1dd8fc2ebaa2",
    ];
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "digest {i}");
    }
}
