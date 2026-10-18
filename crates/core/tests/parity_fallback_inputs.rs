//! `f16-fma`・`subgroup-sum`・`atomic-add` の試験の入力の生成の確認（spec TASK-929 の 2/6・3/6）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。
//!
//! sha256 の値は、本リポの製品のコードが出す既知の出力として固定する
//! （`parity_gemm_inputs.rs` と同じ流儀）。

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::binary16::{binary16_bits_to_f32, f32_to_binary16_bits};
use fandhe_silicon_core::parity::fallback_inputs::{
    ATOMIC_ADD_OUTPUT_COUNTS, E2_LEN, E2_PAIRS, F16FmaInputs, FALLBACK_INPUT_SEED,
    FALLBACK_LENGTHS, FallbackInputKind, S_LEN, S_VALUES, SUBGROUP_SUM_BLOCK, ax_pair_allowed,
    generate_atomic_add_ag, generate_atomic_add_as, generate_atomic_add_ax, generate_f16_fma_e1,
    generate_f16_fma_e2, generate_f16_fma_r, generate_f16_fma_s, generate_subgroup_sum_f32_fg,
    generate_subgroup_sum_f32_fs, generate_subgroup_sum_f32_fx, generate_subgroup_sum_u32_u,
};
use fandhe_silicon_core::parity::gemm_inputs::GEMM_INPUT_SEED;

const KINDS: [FallbackInputKind; 11] = [
    FallbackInputKind::F16FmaE1,
    FallbackInputKind::F16FmaE2,
    FallbackInputKind::F16FmaS,
    FallbackInputKind::F16FmaR,
    FallbackInputKind::SubgroupSumU32U,
    FallbackInputKind::SubgroupSumF32Fx,
    FallbackInputKind::SubgroupSumF32Fg,
    FallbackInputKind::SubgroupSumF32Fs,
    FallbackInputKind::AtomicAddAx,
    FallbackInputKind::AtomicAddAg,
    FallbackInputKind::AtomicAddAs,
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
    for n in FALLBACK_LENGTHS {
        for m in ATOMIC_ADD_OUTPUT_COUNTS {
            seeds.push(FallbackInputKind::AtomicAddAs.seed_for_outputs(n, m));
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
        [
            "f16-fma-e1",
            "f16-fma-e2",
            "f16-fma-s",
            "f16-fma-r",
            "subgroup-sum-u32-u",
            "subgroup-sum-f32-fx",
            "subgroup-sum-f32-fg",
            "subgroup-sum-f32-fs",
            "atomic-add-ax",
            "atomic-add-ag",
            "atomic-add-as",
        ]
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
    for n in [1, 33, 1000] {
        assert_eq!(
            bits(&generate_subgroup_sum_f32_fs(n)),
            bits(&generate_subgroup_sum_f32_fs(n))
        );
        assert_eq!(
            bits(&generate_subgroup_sum_f32_fg(n)),
            bits(&generate_subgroup_sum_f32_fg(n))
        );
        assert_eq!(
            bits(&generate_atomic_add_ag(n)),
            bits(&generate_atomic_add_ag(n))
        );
        assert_eq!(
            generate_subgroup_sum_u32_u(n).unwrap(),
            generate_subgroup_sum_u32_u(n).unwrap()
        );
        for m in ATOMIC_ADD_OUTPUT_COUNTS {
            assert_eq!(
                bits(&generate_atomic_add_as(n, m)),
                bits(&generate_atomic_add_as(n, m))
            );
        }
    }
}

#[test]
fn empty_length_returns_empty() {
    assert!(generate_f16_fma_e1(0).unwrap().is_empty());
    assert!(generate_f16_fma_r(0).unwrap().is_empty());
    assert!(generate_subgroup_sum_u32_u(0).unwrap().is_empty());
    assert!(generate_subgroup_sum_f32_fx(0).unwrap().is_empty());
    assert!(generate_subgroup_sum_f32_fg(0).unwrap().is_empty());
    assert!(generate_subgroup_sum_f32_fs(0).unwrap().is_empty());
    assert!(generate_atomic_add_ax(0).unwrap().is_empty());
    assert!(generate_atomic_add_ag(0).unwrap().is_empty());
    for m in ATOMIC_ADD_OUTPUT_COUNTS {
        assert!(generate_atomic_add_as(0, m).unwrap().is_empty());
    }
    assert!(generate_atomic_add_as(10, 0).unwrap().is_empty());
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
fn bits(
    r: &Result<Vec<f32>, fandhe_silicon_core::parity::cpu_reference::ReferenceError>,
) -> Vec<u32> {
    r.as_ref().unwrap().iter().map(|x| x.to_bits()).collect()
}

fn digest_f32(v: &[f32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect();
    sha256_bytes(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn digest_u32(v: &[u32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    sha256_bytes(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn outputs_table_and_block_are_fixed() {
    assert_eq!(ATOMIC_ADD_OUTPUT_COUNTS, [1, 64, 4096]);
    assert_eq!(SUBGROUP_SUM_BLOCK, 256);
}

#[test]
fn ax_pairs_are_computed() {
    let count = |m: usize| {
        FALLBACK_LENGTHS
            .iter()
            .filter(|&&n| ax_pair_allowed(n, m))
            .count()
    };
    assert_eq!(count(1), 9);
    assert_eq!(count(64), 12);
    assert_eq!(count(4096), 13);
    assert!(ax_pair_allowed(4096, 1));
    assert!(ax_pair_allowed(65_535, 1));
    assert!(!ax_pair_allowed(65_536, 1));
    assert!(!ax_pair_allowed(65_539, 1));
    assert!(ax_pair_allowed(1 << 18, 64));
    assert!(!ax_pair_allowed(1 << 22, 64));
    assert!(!ax_pair_allowed(1, 0));
    assert!(!ax_pair_allowed(usize::MAX, 1));
}

#[test]
fn u_is_uniform_over_high_bits() {
    let v = generate_subgroup_sum_u32_u(1 << 16).unwrap();
    assert_eq!(v.len(), 1 << 16);
    assert!(v.iter().any(|&x| x >> 31 == 1) && v.iter().any(|&x| x >> 31 == 0));
    assert!(v.iter().any(|&x| x > u32::MAX / 2 + u32::MAX / 4));
}

#[test]
fn fx_ax_are_integers_in_range() {
    for n in FALLBACK_LENGTHS {
        for v in [
            generate_subgroup_sum_f32_fx(n).unwrap(),
            generate_atomic_add_ax(n).unwrap(),
        ] {
            assert_eq!(v.len(), n);
            assert!(
                v.iter()
                    .all(|x| x.fract() == 0.0 && (-256.0..=256.0).contains(x)),
                "n={n}"
            );
        }
    }
    // 使える組では、出力の要素ごとの |和| が 2^24 未満に収まる
    let ax = generate_atomic_add_ax(1000).unwrap();
    for m in ATOMIC_ADD_OUTPUT_COUNTS {
        assert!(ax_pair_allowed(1000, m));
        let mut sums = vec![0.0_f64; m];
        for (i, x) in ax.iter().enumerate() {
            sums[i % m] += f64::from(x.abs());
        }
        assert!(sums.iter().all(|&s| s < 16_777_216.0));
    }
}

#[test]
fn fg_ag_magnitudes_in_range() {
    for n in FALLBACK_LENGTHS {
        for v in [
            generate_subgroup_sum_f32_fg(n).unwrap(),
            generate_atomic_add_ag(n).unwrap(),
        ] {
            assert_eq!(v.len(), n);
            assert!(v.iter().all(|x| {
                let a = x.abs();
                a.is_finite() && (2.0_f32.powi(-20)..=2.0_f32.powi(20)).contains(&a)
            }));
        }
    }
    let big = generate_subgroup_sum_f32_fg(1 << 22).unwrap();
    let total: f64 = big.iter().map(|x| f64::from(x.abs())).sum();
    assert!(total <= 2.0_f64.powi(42));
    assert!(big.iter().any(|x| x.is_sign_negative()) && big.iter().any(|x| x.is_sign_positive()));
    for v in [
        generate_subgroup_sum_f32_fg(1 << 16).unwrap(),
        generate_atomic_add_ag(1 << 16).unwrap(),
    ] {
        let mut seen = [false; 40];
        for x in &v {
            let e = x.abs().log2().floor() as i32;
            if (-20..20).contains(&e) {
                seen[(e + 20) as usize] = true;
            }
        }
        assert!(seen.iter().all(|&b| b));
    }
}

fn is_subnormal(x: f32) -> bool {
    x != 0.0 && x.is_finite() && x.abs() < f32::MIN_POSITIVE
}

/// クラス（長さ L の列）が型 `t` の規則に合うかを数えて確かめる。
fn check_class(t: usize, class: &[f32], ctx: &str) {
    let nan = class.iter().filter(|x| x.is_nan()).count();
    let pinf = class.iter().filter(|&&x| x == f32::INFINITY).count();
    let ninf = class.iter().filter(|&&x| x == f32::NEG_INFINITY).count();
    let l = class.len();
    match t {
        0 => assert_eq!((nan, pinf, ninf), (1, 0, 0), "{ctx}"),
        1 => assert_eq!((nan, pinf, ninf), (0, 1, 0), "{ctx}"),
        2 => assert_eq!((nan, pinf, ninf), (0, 0, 1), "{ctx}"),
        3 => assert_eq!((nan, pinf, ninf), (0, 1, usize::from(l >= 2)), "{ctx}"),
        4 => assert!(
            class.iter().all(|x| x.to_bits() == (-0.0_f32).to_bits()),
            "{ctx}"
        ),
        5 => {
            assert!(class.iter().all(|&x| x == 0.0), "{ctx}");
            if l >= 64 {
                assert!(class.iter().any(|x| x.is_sign_negative()));
                assert!(class.iter().any(|x| x.is_sign_positive()));
            }
        }
        6 => assert!(class.iter().all(|&x| is_subnormal(x)), "{ctx}"),
        _ => {
            for (r, &x) in class.iter().enumerate() {
                if r % 2 == 0 {
                    assert!(is_subnormal(x), "{ctx} r={r}");
                } else {
                    assert!(x.is_finite() && x.abs() >= f32::MIN_POSITIVE, "{ctx} r={r}");
                }
            }
        }
    }
    if t <= 3 {
        assert!(
            class
                .iter()
                .filter(|x| x.is_finite())
                .all(|x| x.abs() <= 2.0_f32.powi(20)),
            "{ctx}"
        );
    }
}

#[test]
fn fs_patterns_by_block() {
    for n in [
        1,
        257,
        SUBGROUP_SUM_BLOCK * 8,
        SUBGROUP_SUM_BLOCK * 8 + 3,
        256 * 17 + 1,
    ] {
        let v = generate_subgroup_sum_f32_fs(n).unwrap();
        assert_eq!(v.len(), n);
        for (j, class) in v.chunks(SUBGROUP_SUM_BLOCK).enumerate() {
            check_class(j % 8, class, &format!("n={n} block={j}"));
        }
    }
    // 長さ 1 のクラス: 型 (3) は +Inf だけ、型 (7) は非正規化数 1 つ
    let v = generate_subgroup_sum_f32_fs(256 * 3 + 1).unwrap();
    assert_eq!(v[768], f32::INFINITY);
    let v = generate_subgroup_sum_f32_fs(256 * 7 + 1).unwrap();
    assert!(is_subnormal(v[256 * 7]));
}

#[test]
fn as_patterns_by_output_element() {
    for m in ATOMIC_ADD_OUTPUT_COUNTS {
        for n in [1, 64 * 8, 1000, 4096 + 5] {
            let v = generate_atomic_add_as(n, m).unwrap();
            assert_eq!(v.len(), n);
            for j in 0..m.min(n) {
                let class: Vec<f32> = v.iter().copied().skip(j).step_by(m).collect();
                check_class(j % 8, &class, &format!("n={n} m={m} j={j}"));
            }
        }
    }
    // j >= n の出力の要素は入力を受けず、長さは n のまま
    assert_eq!(generate_atomic_add_as(3, 4096).unwrap().len(), 3);
}

#[test]
fn fs_as_finite_parts_bound() {
    let mut all = generate_subgroup_sum_f32_fs(1 << 16).unwrap();
    all.extend(generate_atomic_add_as(1 << 16, 64).unwrap());
    assert!(
        all.iter()
            .filter(|x| x.is_finite())
            .all(|x| x.abs() <= 2.0_f32.powi(20))
    );
}

#[test]
fn known_digests_subgroup_atomic() {
    let mut got = Vec::new();
    for n in [33, 257, 1000] {
        got.push(digest_u32(&generate_subgroup_sum_u32_u(n).unwrap()));
        got.push(digest_f32(&generate_subgroup_sum_f32_fx(n).unwrap()));
        got.push(digest_f32(&generate_subgroup_sum_f32_fg(n).unwrap()));
        got.push(digest_f32(&generate_subgroup_sum_f32_fs(n).unwrap()));
        got.push(digest_f32(&generate_atomic_add_ax(n).unwrap()));
        got.push(digest_f32(&generate_atomic_add_ag(n).unwrap()));
        got.push(digest_f32(&generate_atomic_add_as(n, 64).unwrap()));
    }
    let want: [&str; 21] = [
        "008366d84c260be9bbccc8e289ee581183a7e71525fa835f7a4337b7500c3bed",
        "a3c3109fb75777157f2fb72a14e558e8a9dfa3ffb419643a6c931a8a77786246",
        "07a503b9b65e0c122b35e4dfe78d60bc94dda2d80677e00ac8f96455e1f37f44",
        "d4cfce31c9f6a5c609074675e90301ca8a1a4597da95fa75c3c12465ec02bb84",
        "ef0e389770ebadea623501b8c32b34be4cb6dedca54de99df474ee1cdaf1af18",
        "bafc121feaf9ff96adc151fa5ca6731a2486209ffe88915d9d06da5181b77882",
        "b12340055928843d5fd995f0a08b8bdee7dc8543ef0dbd495d230c88e2af8c20",
        "69b18113859168b7cde3b4a1b09f137726009df44e213d7991deed6002a887d1",
        "981a450b8a40e9e279de3080dfef923a1eb6c5f487d44e85deb9a153d2cd2327",
        "9697146ca6491eaf0a92430437833d79733420eb08efdcd576788d535f20ed89",
        "f2cb98b3a2e483a60a424d54431f331328ba61d8bf54e0c0f7aaec6ceee809f6",
        "d08daca464739f15a178015a641b248f4da59a5d258336d1d531658fc29713e7",
        "1a874d701e571e2b50d7dd5aae107e567b4c4d2c50e3157fe0693109b4eee608",
        "bfb63ae804536abd58144477dab06d27d9f7098c27df32e60f7455ff743c4f67",
        "6227badeefdae0ed62425dcea685ab6744f18794269d9fef4571c370ceccac93",
        "20f357e2ea1f745cdafad6003c4dfb400e1785177f92df4671e7dbcdece717f0",
        "5775e572d6e2937a5d4443367de5dd96b0939976446b3ab2dbf53d0d537d29a5",
        "790ccac62a7f9ed967e8255e5ef6eabae8fb13c23b66f25d7d3f616433ddd833",
        "d986a82e914c1b7c9bae371686cc47d1e3f2562da6f19efa017e4a07d8f4063a",
        "31383a2a9ad60a54402f2046f691d28adf79dc2f2eef2e4cc6ed3a770eefb32c",
        "1178688b41012f4b27693fa887011c853d2b6f1ce6d70562a03e3dde4da241ec",
    ];
    assert_eq!(got, want);
}
