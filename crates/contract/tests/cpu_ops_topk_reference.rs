//! `cpu_ops` の部分 Top-k を、共通の crate の CPU 参照（`fandhe_silicon_core::parity::topk`）と
//! 比べる外の試験（TASK-1352 のポインタ）。feature `cpu-ops` と `test-support` が有効な組でだけ走る。
//! 公開のパスだけを使い、入力は固定の seed から決定的に作る。
//! 期待の出力を CPU 参照から作らず、出力に並びの規則を直に当てる試験も置く。
//! SIMD 版と scalar 版の出力をバイトの列にして比べ、違うバイトの数を数える試験も置く。
//! -0.0 と +0.0 を含む入力を、scalar 版と SIMD 版の出力に当てる試験も置く。

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, ReferenceCpuOps};
use fandhe_silicon_core::parity::case_inputs::{CaseId, CaseInput};
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

/// 出力の各件が入力の（添字, スコア）の組で、並びの規則と上位の集合の規則を満たすことを、
/// CPU 参照を呼ばずに入力から直に確かめる。
fn assert_order_rule(name: &str, scores: &[f32], k: usize, out: &[(u32, f32)]) {
    assert!(k > 0, "{name}");
    assert_eq!(out.len(), k, "{name}");
    let mut seen = vec![false; scores.len()];
    for (slot, score) in out {
        let i = usize::try_from(*slot).unwrap();
        assert!(i < scores.len(), "{name} slot {slot}");
        assert_eq!(scores[i].to_bits(), score.to_bits(), "{name} slot {slot}");
        assert!(score.is_finite(), "{name} slot {slot}");
        assert!(!seen[i], "{name} duplicate slot {slot}");
        seen[i] = true;
    }
    for pair in out.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        match a.1.total_cmp(&b.1) {
            std::cmp::Ordering::Greater => {}
            std::cmp::Ordering::Equal => assert!(a.0 < b.0, "{name} {a:?} {b:?}"),
            std::cmp::Ordering::Less => panic!("{name} {a:?} {b:?}"),
        }
    }
    let last = out[k - 1];
    let last_slot = usize::try_from(last.0).unwrap();
    for (j, score) in scores.iter().enumerate() {
        if seen[j] || !score.is_finite() {
            continue;
        }
        match last.1.total_cmp(score) {
            std::cmp::Ordering::Greater => {}
            std::cmp::Ordering::Equal => assert!(last_slot < j, "{name} slot {j}"),
            std::cmp::Ordering::Less => panic!("{name} slot {j} outranks last"),
        }
    }
}

#[test]
fn topk_order_rule_holds_directly() {
    // CPU-10
    let mut all: Vec<(String, Vec<f32>, usize)> = vec![
        ("batch".to_string(), batch_input(), 64),
        ("ties".to_string(), ties_input(), 10),
    ];
    for input in SIGNED_ZERO_INPUTS {
        all.push((input.name().to_string(), input.scores().to_vec(), input.k()));
    }
    let ops_list: [(&str, Box<dyn CpuOps>); 2] = [
        ("default", Box::new(DefaultCpuOps)),
        ("reference", Box::new(ReferenceCpuOps)),
    ];
    for (name, scores, k) in &all {
        for (label, ops) in &ops_list {
            let out = run(ops.as_ref(), Method::Scalar, scores, *k);
            assert_order_rule(&format!("{name} {label}"), scores, *k, &out);
        }
    }
}

#[test]
fn topk_ties_cross_k_boundary_in_slot_order() {
    // CPU-10
    let scores = ties_input();
    let k = 10;
    let ops_list: [(&str, Box<dyn CpuOps>); 2] = [
        ("default", Box::new(DefaultCpuOps)),
        ("reference", Box::new(ReferenceCpuOps)),
    ];
    for (label, ops) in &ops_list {
        let out = run(ops.as_ref(), Method::Scalar, &scores, k);
        let boundary = out[k - 1].1.to_bits();
        let inside: Vec<usize> = out
            .iter()
            .filter(|(_, v)| v.to_bits() == boundary)
            .map(|(s, _)| usize::try_from(*s).unwrap())
            .collect();
        let outside: Vec<usize> = scores
            .iter()
            .enumerate()
            .filter(|(i, v)| v.to_bits() == boundary && !inside.contains(i))
            .map(|(i, _)| i)
            .collect();
        assert!(inside.len() >= 2, "{label}");
        assert!(!outside.is_empty(), "{label}");
        assert!(inside.windows(2).all(|w| w[0] < w[1]), "{label}");
        assert!(inside.iter().max() < outside.iter().min(), "{label}");
    }
}

#[test]
fn topk_non_finite_slots_absent_directly() {
    // CPU-11
    let scores = non_finite_input();
    let k = 10;
    let ops_list: [(&str, Box<dyn CpuOps>); 2] = [
        ("default", Box::new(DefaultCpuOps)),
        ("reference", Box::new(ReferenceCpuOps)),
    ];
    for (label, ops) in &ops_list {
        let out = run(ops.as_ref(), Method::Scalar, &scores, k);
        assert_order_rule(label, &scores, k, &out);
        for (slot, _) in &out {
            let i = usize::try_from(*slot).unwrap();
            assert!(scores[i].is_finite(), "{label} slot {slot}");
        }
    }
}

#[test]
fn topk_first_k_are_input_pairs() {
    // CPU-14
    let ops_list: [(&str, Box<dyn CpuOps>); 2] = [
        ("default", Box::new(DefaultCpuOps)),
        ("reference", Box::new(ReferenceCpuOps)),
    ];
    for (name, scores, k) in cases() {
        for (label, ops) in &ops_list {
            let out = run(ops.as_ref(), Method::Scalar, &scores, k);
            assert_eq!(out.len(), k, "{name} {label}");
            for (slot, score) in &out {
                assert_ne!(*slot, ReferenceCpuOps::SENTINEL.0, "{name} {label}");
                let i = usize::try_from(*slot).unwrap();
                assert_eq!(scores[i].to_bits(), score.to_bits(), "{name} {label}");
            }
        }
    }
}

/// scalar 版の呼び出しで `out` を前詰めする値。スコアは NaN。
const FILL_SCALAR: (u32, f32) = (0xA5A5_A5A5, f32::from_bits(0x7fc0_00a5));

/// SIMD 版の呼び出しで `out` を前詰めする値。スコアは NaN。
const FILL_SIMD: (u32, f32) = (0x5A5A_5A5A, f32::from_bits(0x7fc0_005a));

/// `fill` で前詰めした長さ k の `out` を作り、`method` のメソッドを呼んで出力を返す。
fn run_filled(
    ops: &dyn CpuOps,
    method: Method,
    scores: &[f32],
    k: usize,
    fill: (u32, f32),
) -> Vec<(u32, f32)> {
    let mut out = vec![fill; k];
    let r = match method {
        Method::Scalar => ops.topk_partial_scalar(scores, k, &mut out),
        Method::Simd => ops.topk_partial_simd(scores, k, &mut out),
    };
    assert!(r.is_ok());
    out
}

/// 出力の各件のスロット番号とスコアのビットを、この順に、この環境のバイト順で並べたバイトの列。
fn out_bytes(out: &[(u32, f32)]) -> Vec<u8> {
    let mut v = Vec::with_capacity(out.len() * (size_of::<u32>() + size_of::<f32>()));
    for (slot, score) in out {
        v.extend_from_slice(&slot.to_ne_bytes());
        v.extend_from_slice(&score.to_bits().to_ne_bytes());
    }
    v
}

/// 2 つのバイトの列で同じ位置の値が違うバイトの数。長さが違えば `None`。
fn differing_bytes(a: &[u8], b: &[u8]) -> Option<usize> {
    if a.len() != b.len() {
        return None;
    }
    Some(a.iter().zip(b).filter(|(x, y)| x != y).count())
}

/// 出力のどの件も、前詰めの値のスロット番号・スコアのビットを残していないことを確かめる。
fn assert_overwritten(name: &str, out: &[(u32, f32)], fill: (u32, f32)) {
    for (i, (slot, score)) in out.iter().enumerate() {
        assert_ne!(*slot, fill.0, "{name} slot at {i}");
        assert_ne!(score.to_bits(), fill.1.to_bits(), "{name} score at {i}");
    }
}

#[test]
fn topk_simd_scalar_byte_sequences_match() {
    // CPU-16
    let ops_list: [(&str, Box<dyn CpuOps>); 2] = [
        ("default", Box::new(DefaultCpuOps)),
        ("reference", Box::new(ReferenceCpuOps)),
    ];
    let entry_bytes = size_of::<u32>() + size_of::<f32>();
    let mut compared = 0usize;
    for (name, scores, k) in cases() {
        for (label, ops) in &ops_list {
            let tag = format!("{name} {label}");
            let scalar = run_filled(ops.as_ref(), Method::Scalar, &scores, k, FILL_SCALAR);
            let simd = run_filled(ops.as_ref(), Method::Simd, &scores, k, FILL_SIMD);
            assert_overwritten(&format!("{tag} scalar"), &scalar, FILL_SCALAR);
            assert_overwritten(&format!("{tag} simd"), &simd, FILL_SIMD);
            let a = out_bytes(&scalar);
            let b = out_bytes(&simd);
            assert_eq!(a.len(), k * entry_bytes, "{tag} scalar len");
            assert_eq!(b.len(), k * entry_bytes, "{tag} simd len");
            assert_eq!(differing_bytes(&a, &b), Some(0), "{tag}");
            compared += a.len();
        }
    }
    assert_eq!(compared, 2 * (64 + 10 + 10) * entry_bytes);
}

#[test]
fn topk_byte_count_detects_divergence() {
    // CPU-16
    let entry_bytes = size_of::<u32>() + size_of::<f32>();
    for (name, scores, k) in cases() {
        let base_out = run_filled(&DefaultCpuOps, Method::Scalar, &scores, k, FILL_SCALAR);
        let base = out_bytes(&base_out);
        assert_eq!(differing_bytes(&base, &base), Some(0), "{name} self");

        let mut flipped_out = base_out.clone();
        let last = flipped_out.last_mut().unwrap();
        last.1 = f32::from_bits(last.1.to_bits() ^ 1);
        let flipped = out_bytes(&flipped_out);
        assert_eq!(differing_bytes(&base, &flipped), Some(1), "{name} bit");

        let mut swapped_out = base_out.clone();
        swapped_out.swap(0, 1);
        let swapped = out_bytes(&swapped_out);
        let n = differing_bytes(&base, &swapped);
        assert!(n.is_some_and(|n| n > 0), "{name} swap");

        let dropped = &base[..base.len() - entry_bytes];
        assert_eq!(differing_bytes(&base, dropped), None, "{name} len");
    }
}

/// `CaseId::E4` の入力（スコアの列と k）。
fn case_e4() -> (Vec<f32>, usize) {
    match CaseId::E4.input().unwrap() {
        CaseInput::TopK { scores, k } => (scores, k),
        _ => panic!("CaseId::E4 is not a top-k input"),
    }
}

/// -0.0 と +0.0 を含む入力の 5 件（`CaseId::E4` と `SIGNED_ZERO_INPUTS` の 4 件）。
fn signed_zero_cases() -> Vec<(String, Vec<f32>, usize)> {
    let (scores, k) = case_e4();
    let mut all = vec![("case_e4".to_string(), scores, k)];
    for input in SIGNED_ZERO_INPUTS {
        all.push((input.name().to_string(), input.scores().to_vec(), input.k()));
    }
    all
}

/// 2 つの出力で、スロット番号が違うか `f32::total_cmp` が `Equal` でない位置の数。長さが違えば `None`。
fn total_order_mismatches(a: &[(u32, f32)], b: &[(u32, f32)]) -> Option<usize> {
    if a.len() != b.len() {
        return None;
    }
    Some(
        a.iter()
            .zip(b)
            .filter(|(x, y)| x.0 != y.0 || x.1.total_cmp(&y.1) != std::cmp::Ordering::Equal)
            .count(),
    )
}

/// 有限のスコアを `partial_cmp` の降順（同点はスロット番号の昇順）に並べた先頭 k 件。
/// 試験の中の比べの相手で、製品の出力の期待には使わない。
fn ieee_ordered_topk(scores: &[f32], k: usize) -> Vec<(u32, f32)> {
    let mut v: Vec<(u32, f32)> = (0_u32..)
        .zip(scores)
        .filter(|(_, x)| x.is_finite())
        .map(|(i, x)| (i, *x))
        .collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    v.truncate(k);
    v
}

#[test]
fn topk_case_e4_matches_case_reference() {
    // CPU-12
    let (scores, k) = case_e4();
    assert_eq!(k, 8);
    let reference = CaseId::E4.reference().unwrap();
    let expected = reference.as_topk().unwrap();
    for (label, ops, method) in combos() {
        let out = run(ops.as_ref(), method, &scores, k);
        assert!(
            compare_topk(expected, &to_entries(&out))
                .unwrap()
                .is_match(),
            "{label}"
        );
        let slots: Vec<u32> = out.iter().map(|(s, _)| *s).collect();
        assert_eq!(slots, (5..13).collect::<Vec<u32>>(), "{label}");
        assert!(
            out.iter().all(|(_, v)| v.to_bits() == 1.0_f32.to_bits()),
            "{label}"
        );
    }
}

#[test]
fn topk_case_e4_order_rule_holds_directly() {
    // CPU-12
    let (scores, k) = case_e4();
    for (label, ops, method) in combos() {
        let out = run(ops.as_ref(), method, &scores, k);
        assert_order_rule(&format!("case_e4 {label}"), &scores, k, &out);
    }
}

#[test]
fn topk_positive_zero_precedes_negative_zero() {
    // CPU-12
    for input in SIGNED_ZERO_INPUTS {
        let (scores, k) = (input.scores(), input.k());
        for (label, ops, method) in combos() {
            let name = format!("{} {label}", input.name());
            let out = run(ops.as_ref(), method, scores, k);
            assert_order_rule(&name, scores, k, &out);
            let pos: Vec<usize> = (0..out.len())
                .filter(|i| out[*i].1.to_bits() == 0x0000_0000)
                .collect();
            let neg: Vec<usize> = (0..out.len())
                .filter(|i| out[*i].1.to_bits() == 0x8000_0000)
                .collect();
            assert!(!pos.is_empty(), "{name}");
            if !neg.is_empty() {
                assert!(pos.last() < neg.first(), "{name}");
            }
        }
    }
}

#[test]
fn topk_simd_signed_zero_total_order_matches_scalar() {
    // CPU-13
    let ops_list: [(&str, Box<dyn CpuOps>); 2] = [
        ("default", Box::new(DefaultCpuOps)),
        ("reference", Box::new(ReferenceCpuOps)),
    ];
    let mut compared = 0usize;
    for (name, scores, k) in signed_zero_cases() {
        assert!(scores.iter().any(|x| x.to_bits() == 0x8000_0000), "{name}");
        let scalar = run(&DefaultCpuOps, Method::Scalar, &scores, k);
        for (label, ops) in &ops_list {
            let simd = run(ops.as_ref(), Method::Simd, &scores, k);
            assert_eq!(
                total_order_mismatches(&scalar, &simd),
                Some(0),
                "{name} {label}"
            );
            compared += scalar.len();
        }
    }
    assert_eq!(compared, 2 * (8 + 2 + 4 + 2 + 4));
}

#[test]
fn topk_signed_zero_inputs_differ_from_ieee_order() {
    // CPU-13
    assert_eq!(SIGNED_ZERO_INPUTS.len(), 4);
    let expected = [Some(2), Some(2), Some(1), Some(4)];
    for (input, want) in SIGNED_ZERO_INPUTS.iter().zip(expected) {
        let (scores, k) = (input.scores(), input.k());
        let out = run(&DefaultCpuOps, Method::Scalar, scores, k);
        let ieee = ieee_ordered_topk(scores, k);
        assert_eq!(
            total_order_mismatches(&out, &ieee),
            want,
            "{}",
            input.name()
        );
    }
}

#[test]
fn topk_total_order_compare_detects_divergence() {
    // CPU-13
    let base = vec![(1_u32, 0.0_f32), (0, -0.0_f32)];
    assert_eq!(total_order_mismatches(&base, &base), Some(0));

    let mut flipped = base.clone();
    flipped[0].1 = -0.0;
    assert_eq!(total_order_mismatches(&base, &flipped), Some(1));

    let mut swapped = base.clone();
    swapped.swap(0, 1);
    assert_eq!(total_order_mismatches(&base, &swapped), Some(2));

    // スコアのビットが同じでスロット番号だけが違う組は、スロットの比べで数えられる。
    let mut slot_only = base.clone();
    slot_only[0].0 = 7;
    assert_eq!(total_order_mismatches(&base, &slot_only), Some(1));

    assert_eq!(total_order_mismatches(&base, &base[..1]), None);
}
