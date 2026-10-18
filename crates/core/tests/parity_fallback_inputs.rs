//! `f16-fma` の試験の入力の生成の確認（spec TASK-929 の 2/6）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。
//!
//! sha256 の値は、本リポの製品のコードが出す既知の出力として固定する
//! （`parity_gemm_inputs.rs` と同じ流儀）。

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::binary16::{binary16_bits_to_f32, f32_to_binary16_bits};
use fandhe_silicon_core::parity::fallback_inputs::{
    E2_LEN, E2_PAIRS, F16FmaInputs, FALLBACK_INPUT_SEED, FALLBACK_LENGTHS, FallbackInputKind,
    S_LEN, S_VALUES, generate_f16_fma_e1, generate_f16_fma_e2, generate_f16_fma_r,
    generate_f16_fma_s,
};
use fandhe_silicon_core::parity::gemm_inputs::GEMM_INPUT_SEED;

const KINDS: [FallbackInputKind; 4] = [
    FallbackInputKind::F16FmaE1,
    FallbackInputKind::F16FmaE2,
    FallbackInputKind::F16FmaS,
    FallbackInputKind::F16FmaR,
];

fn digest(inputs: &F16FmaInputs) -> String {
    let mut bytes = Vec::with_capacity(inputs.len() * 6);
    for v in [&inputs.a, &inputs.b, &inputs.c] {
        bytes.extend(v.iter().flat_map(|x| x.to_le_bytes()));
    }
    sha256_bytes(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn lengths_table_is_fixed() {
    assert_eq!(
        FALLBACK_LENGTHS,
        [
            1, 31, 32, 33, 255, 256, 257, 1000, 65_539, 4096, 65_536, 262_144, 4_194_304
        ]
    );
}

#[test]
fn seed_base_matches_gemm_seed() {
    assert_eq!(FALLBACK_INPUT_SEED, GEMM_INPUT_SEED);
}

#[test]
fn seed_differs_across_kinds_and_lengths() {
    let mut seeds = Vec::new();
    for kind in KINDS {
        for n in FALLBACK_LENGTHS {
            seeds.push(kind.seed_for(n));
        }
    }
    let total = seeds.len();
    seeds.sort_unstable();
    seeds.dedup();
    assert_eq!(seeds.len(), total);
}

#[test]
fn kind_names_are_fixed() {
    let names: Vec<_> = KINDS.iter().map(|k| k.name()).collect();
    assert_eq!(
        names,
        ["f16-fma-e1", "f16-fma-e2", "f16-fma-s", "f16-fma-r"]
    );
}

#[test]
fn e1_values_are_integers_in_range() {
    for n in FALLBACK_LENGTHS {
        let i = generate_f16_fma_e1(n).unwrap();
        assert_eq!((i.a.len(), i.b.len(), i.c.len()), (n, n, n));
        for k in 0..n {
            let (a, b, c) = (
                binary16_bits_to_f32(i.a[k]),
                binary16_bits_to_f32(i.b[k]),
                binary16_bits_to_f32(i.c[k]),
            );
            for (v, lim) in [(a, 32.0), (b, 32.0), (c, 1024.0)] {
                assert_eq!(v, v.trunc(), "n={n} k={k}");
                assert!((-lim..=lim).contains(&v), "n={n} k={k} v={v}");
            }
            let exact = a * b + c;
            assert!(exact.abs() <= 2048.0);
            assert_eq!(
                binary16_bits_to_f32(f32_to_binary16_bits(exact)),
                exact,
                "n={n} k={k}: not exact in binary16"
            );
        }
    }
}

#[test]
fn r_excludes_non_finite() {
    for n in FALLBACK_LENGTHS {
        let i = generate_f16_fma_r(n).unwrap();
        assert_eq!((i.a.len(), i.b.len(), i.c.len()), (n, n, n));
        for v in [&i.a, &i.b, &i.c] {
            assert!(v.iter().all(|x| x & 0x7C00 != 0x7C00), "n={n}");
        }
    }
}

#[test]
fn e2_enumeration() {
    let i = generate_f16_fma_e2().unwrap();
    assert_eq!(E2_LEN, 1_048_576);
    assert_eq!((i.a.len(), i.b.len(), i.c.len()), (E2_LEN, E2_LEN, E2_LEN));
    for (k, &(pb, pc)) in E2_PAIRS.iter().enumerate() {
        assert_eq!((i.a[k], i.b[k], i.c[k]), (0, pb, pc));
        let l = E2_LEN - 16 + k;
        assert_eq!((i.a[l], i.b[l], i.c[l]), (0xFFFF, pb, pc));
    }
    for (idx, a) in i.a.iter().enumerate() {
        assert_eq!(*a as usize, idx / 16);
        assert_eq!((i.b[idx], i.c[idx]), E2_PAIRS[idx % 16]);
    }
}

#[test]
fn e2_pairs_are_distinct_and_hand_checked() {
    let mut pairs = E2_PAIRS.to_vec();
    pairs.sort_unstable();
    pairs.dedup();
    assert_eq!(pairs.len(), 16);
    // 手で確かめる値: (1, 2^-24)・(2^-14, +0)・(1, 65504)
    assert_eq!(binary16_bits_to_f32(0x3C00), 1.0);
    assert_eq!(binary16_bits_to_f32(0x0001), 2.0_f32.powi(-24));
    assert_eq!(binary16_bits_to_f32(0x0400), 2.0_f32.powi(-14));
    assert_eq!(binary16_bits_to_f32(0x7BFF), 65504.0);
    assert_eq!(binary16_bits_to_f32(0x3BFF), 1.0 - 2.0_f32.powi(-11));
    assert_eq!(binary16_bits_to_f32(0x3C01), 1.0 + 2.0_f32.powi(-10));
}

#[test]
fn s_enumeration() {
    let i = generate_f16_fma_s().unwrap();
    assert_eq!(S_LEN, 3375);
    assert_eq!(i.len(), 3375);
    let mut set = S_VALUES.to_vec();
    set.sort_unstable();
    set.dedup();
    assert_eq!(set.len(), 15);
    assert!(S_VALUES.contains(&0x7E00));
    for idx in 0..S_LEN {
        assert_eq!(i.a[idx], S_VALUES[idx / 225]);
        assert_eq!(i.b[idx], S_VALUES[(idx / 15) % 15]);
        assert_eq!(i.c[idx], S_VALUES[idx % 15]);
    }
}

#[test]
fn s_values_decode_as_expected() {
    let f = |i: usize| binary16_bits_to_f32(S_VALUES[i]);
    assert_eq!(f(0).to_bits(), 0.0_f32.to_bits());
    assert_eq!(f(1).to_bits(), (-0.0_f32).to_bits());
    assert_eq!(f(2), 2.0_f32.powi(-24));
    assert_eq!(f(4), 2.0_f32.powi(-14) - 2.0_f32.powi(-24));
    assert_eq!(f(6), 2.0_f32.powi(-14));
    assert_eq!(f(8), 1.0);
    assert_eq!(f(10), 65504.0);
    assert_eq!(f(12), f32::INFINITY);
    assert_eq!(f(13), f32::NEG_INFINITY);
    assert!(f(14).is_nan());
}

#[test]
fn generation_is_deterministic() {
    for n in [1, 33, 1000] {
        assert_eq!(
            generate_f16_fma_e1(n).unwrap(),
            generate_f16_fma_e1(n).unwrap()
        );
        assert_eq!(
            generate_f16_fma_r(n).unwrap(),
            generate_f16_fma_r(n).unwrap()
        );
    }
    assert_eq!(generate_f16_fma_s().unwrap(), generate_f16_fma_s().unwrap());
}

#[test]
fn empty_length_returns_empty() {
    assert!(generate_f16_fma_e1(0).unwrap().is_empty());
    assert!(generate_f16_fma_r(0).unwrap().is_empty());
}

#[test]
fn known_digests() {
    let got: Vec<String> = [1, 33, 1000]
        .iter()
        .flat_map(|&n| {
            [
                digest(&generate_f16_fma_e1(n).unwrap()),
                digest(&generate_f16_fma_r(n).unwrap()),
            ]
        })
        .chain([digest(&generate_f16_fma_s().unwrap())])
        .collect();
    let want = [
        "89e4cd19d09ca8f3dd82210280bf106e3cc24afa58f60a79b3904ba2044fb933",
        "50332d999dc48e0855d36898de32677b050d0c23070953e0661acf1c6ea662c0",
        "089415ed95dbf00091600e97bcb74c16d208bfc41762eb04c5a6f4fa586cfaa0",
        "292daf89d93c3a0522815a2688d102f45953760f9f835dc1793f8ea7e4c59a86",
        "2f0b416f0c7e88b327945fe371607ac5c6716e79e8f01134979b18acf4bf1496",
        "d66c5fa85a13995b64db70ad6e48f49f33a85721e07bf7d7d558901e07baa307",
        "98a2d7fb2672e39ccc06dc9bd98a22d3b2fe030cd02c1af4c603a948dfc0da8b",
    ];
    assert_eq!(got, want);
}

#[test]
fn e2_digest() {
    assert_eq!(
        digest(&generate_f16_fma_e2().unwrap()),
        "6c97fda5c9f6d4b74baa2a0d084a06e46d14eaa9f8a81e8753fa642448676927"
    );
}
