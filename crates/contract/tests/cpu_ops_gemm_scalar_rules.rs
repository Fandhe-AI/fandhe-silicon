//! `cpu_ops` の `gemm_scalar` の積和の順と、NaN・±Inf・±0 を含む入力の出力を確かめる外の試験（TASK-2445 のポインタ）。
//! feature `cpu-ops` と `test-support` が有効な組でだけ走る。参照は `reference_gemm_f32`（TASK-24 のポインタ）、
//! 特殊値の入力は `CaseId::G01`〜`G13`（TASK-977 のポインタ）、比べは `compare_f32_with_special_values`（TASK-25 のポインタ）。

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, ReferenceCpuOps};
use fandhe_silicon_core::parity::case_inputs::{
    CaseId, CaseInput, GemmCase, OutputClass, classify,
};
use fandhe_silicon_core::parity::cpu_reference::{
    reference_gemm_f32, reference_gemm_f32_descending_k,
};
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, SplitMix64, generate_f32,
};
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};

/// 出力を埋める番兵のビット列。
const SENTINEL_BITS: u32 = 0x7fc0_beef;

/// 特殊値を含む GEMM の 13 ケース。
const SPECIAL_CASES: [CaseId; 13] = [
    CaseId::G01,
    CaseId::G02,
    CaseId::G03,
    CaseId::G04,
    CaseId::G05,
    CaseId::G06,
    CaseId::G07,
    CaseId::G08,
    CaseId::G09,
    CaseId::G10,
    CaseId::G11,
    CaseId::G12,
    CaseId::G13,
];

/// 1 つの GEMM の入力（`a` は m×k、`b` は k×n、行優先）。
struct Gemm {
    m: usize,
    k: usize,
    n: usize,
    a: Vec<f32>,
    b: Vec<f32>,
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// 固定の seed の生成器を `offset` 回進めた位置から `a`、続けて `b` を作る正方の入力。
fn square(size: usize, offset: u64) -> Gemm {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    rng.skip(offset);
    let a = (0..size * size)
        .map(|_| rng.next_signed_unit_f32())
        .collect();
    let b = (0..size * size)
        .map(|_| rng.next_signed_unit_f32())
        .collect();
    Gemm {
        m: size,
        k: size,
        n: size,
        a,
        b,
    }
}

/// 64³・256³・37×41×53 の 3 つの入力。64³ は生成器の位置 0、256³ は位置 `2 * 64 * 64` から作り、
/// 37×41×53 は `GemmGateRow::Odd37x41x53`（TASK-1005 のポインタ）の入力。
fn finite_cases() -> Vec<Gemm> {
    let row = GemmGateRow::Odd37x41x53;
    assert_eq!(row.a_offset(), 2 * 64 * 64 + 2 * 256 * 256);
    let odd = generate_f32(row).unwrap();
    vec![
        square(64, 0),
        square(256, 2 * 64 * 64),
        Gemm {
            m: row.m(),
            k: row.k(),
            n: row.n(),
            a: odd.a().to_vec(),
            b: odd.b().to_vec(),
        },
    ]
}

fn special_case(id: CaseId) -> GemmCase {
    let CaseInput::Gemm(g) = id.input().unwrap() else {
        panic!("{} is not gemm", id.name())
    };
    g
}

/// 出力を番兵で埋めてから `gemm_scalar` を呼ぶ。
fn run<T: CpuOps>(ops: &T, a: &[f32], b: &[f32], m: usize, k: usize, n: usize) -> Vec<f32> {
    let mut out = vec![f32::from_bits(SENTINEL_BITS); m * n];
    ops.gemm_scalar(a, b, m, k, n, &mut out).unwrap();
    out
}

/// `DefaultCpuOps`・`ReferenceCpuOps` の 2 つの型の `gemm_scalar` の出力。
fn run_both(a: &[f32], b: &[f32], m: usize, k: usize, n: usize) -> [(&'static str, Vec<f32>); 2] {
    [
        ("default", run(&DefaultCpuOps, a, b, m, k, n)),
        ("reference", run(&ReferenceCpuOps, a, b, m, k, n)),
    ]
}

/// 語の件数を NaN・+Inf・-Inf・+0・-0 の順に数える。
fn special_counts(v: &[f32]) -> [usize; 5] {
    let count = |f: fn(&f32) -> bool| v.iter().filter(|x| f(x)).count();
    [
        count(|x| x.is_nan()),
        count(|x| *x == f32::INFINITY),
        count(|x| *x == f32::NEG_INFINITY),
        count(|x| x.to_bits() == 0.0_f32.to_bits()),
        count(|x| x.to_bits() == (-0.0_f32).to_bits()),
    ]
}

/// 2 つの型の `gemm_scalar` が、3 つの入力で k の昇順の参照と T1 で一致し、ビットまで一致する（CPU-1）。
/// 同じ入力の k の降順の参照とはビットが違う。
#[test]
fn gemm_scalar_follows_ascending_k() {
    for c in finite_cases() {
        let ascending = reference_gemm_f32(&c.a, &c.b, c.m, c.k, c.n).unwrap();
        let descending = reference_gemm_f32_descending_k(&c.a, &c.b, c.m, c.k, c.n).unwrap();
        assert_ne!(
            bits(&ascending),
            bits(&descending),
            "{}x{}x{}",
            c.m,
            c.k,
            c.n
        );
        for (name, out) in run_both(&c.a, &c.b, c.m, c.k, c.n) {
            let cmp = compare_f32(&ascending, &out, T1).unwrap();
            assert_eq!(cmp.compared(), c.m * c.n, "{name} {}x{}x{}", c.m, c.k, c.n);
            assert!(cmp.is_match(), "{name} {}x{}x{}", c.m, c.k, c.n);
            assert_eq!(
                bits(&out),
                bits(&ascending),
                "{name} {}x{}x{}",
                c.m,
                c.k,
                c.n
            );
        }
    }
}

/// 13 ケースの入力に NaN・+Inf・-Inf・+0・-0 がある。
#[test]
fn special_inputs_hold_special_values() {
    let mut total = [0usize; 5];
    for id in SPECIAL_CASES {
        let g = special_case(id);
        for (i, c) in special_counts(g.a())
            .into_iter()
            .zip(special_counts(g.b()))
            .map(|(x, y)| x + y)
            .enumerate()
        {
            total[i] += c;
        }
    }
    assert_eq!(total, [3, 6, 3, 59, 60]);
}

/// 2 つの型の `gemm_scalar` の出力の各要素の区分が、ケースの設計の区分と同じ（CPU-7）。
#[test]
fn gemm_scalar_output_classes_match_design() {
    for id in SPECIAL_CASES {
        let g = special_case(id);
        for (name, out) in run_both(g.a(), g.b(), g.m(), g.k(), g.n()) {
            assert_eq!(out.len(), g.design().len(), "{name} {}", id.name());
            for (i, (x, want)) in out.iter().zip(g.design()).enumerate() {
                assert_ne!(x.to_bits(), SENTINEL_BITS, "{name} {} [{i}]", id.name());
                assert_eq!(classify(*x), *want, "{name} {} [{i}]", id.name());
            }
        }
    }
}

/// 1 要素のケースの出力の区分: +Inf と -Inf の和・0 と Inf の積は NaN、Inf の積は B の符号を通る。
#[test]
fn gemm_scalar_single_element_classes() {
    let want = [
        (CaseId::G01, OutputClass::Nan),
        (CaseId::G02, OutputClass::PosInf),
        (CaseId::G03, OutputClass::NegInf),
        (CaseId::G04, OutputClass::NegInf),
        (CaseId::G05, OutputClass::Nan),
        (CaseId::G06, OutputClass::Nan),
        (CaseId::G07, OutputClass::Nan),
        (CaseId::G13, OutputClass::Nan),
    ];
    for (id, class) in want {
        let g = special_case(id);
        for (name, out) in run_both(g.a(), g.b(), g.m(), g.k(), g.n()) {
            assert_eq!(out.len(), 1, "{name} {}", id.name());
            assert_eq!(classify(out[0]), class, "{name} {}", id.name());
        }
    }
}

/// 2 つの型の `gemm_scalar` の出力が、13 ケースの CPU 参照と規則で一致する。
#[test]
fn gemm_scalar_matches_case_reference_by_rule() {
    for id in SPECIAL_CASES {
        let g = special_case(id);
        let reference = id.reference().unwrap();
        let reference = reference.as_f32().unwrap();
        for (name, out) in run_both(g.a(), g.b(), g.m(), g.k(), g.n()) {
            let cmp = compare_f32_with_special_values(reference, &out, T1).unwrap();
            assert_eq!(cmp.compared(), g.m() * g.n(), "{name} {}", id.name());
            assert_eq!(cmp.exceeded(), 0, "{name} {}", id.name());
            assert_eq!(cmp.non_finite_mismatch(), 0, "{name} {}", id.name());
            assert!(cmp.is_match(), "{name} {}", id.name());
        }
    }
}

/// 比べの陽性対照: NaN を `+0.0` に、+Inf を -Inf に置き換えた出力は、規則でも区分でも一致しない。
#[test]
fn rule_detects_replaced_non_finite() {
    for (id, replaced) in [(CaseId::G05, 0.0_f32), (CaseId::G02, f32::NEG_INFINITY)] {
        let g = special_case(id);
        let reference = id.reference().unwrap();
        let reference = reference.as_f32().unwrap();
        let mut out = run(&DefaultCpuOps, g.a(), g.b(), g.m(), g.k(), g.n());
        out[0] = replaced;
        let cmp = compare_f32_with_special_values(reference, &out, T1).unwrap();
        assert_eq!(cmp.non_finite_mismatch(), 1, "{}", id.name());
        assert!(!cmp.is_match(), "{}", id.name());
        assert_ne!(classify(out[0]), g.design()[0], "{}", id.name());
    }
}
