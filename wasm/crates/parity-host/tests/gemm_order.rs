//! 行列積の和の順の確かめの 3 ケースの入力・2 つの参照・書き出しの試験（spec TASK-2447 のポインタ）。
//! 期待の値は生成器・core の CPU 参照・`DefaultCpuOps` の呼び出しと、試験の中で書いた式で作り、値を手で写さない。
//! ただし手で選んだ 1 ケースの入力と答えは、式の順を独立に確かめるため字句で置く。

use std::fs;
use std::path::{Path, PathBuf};

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, SplitMix64, generate_f32,
};
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};
use wasm_parity_host::{
    GEMM_ORDER_CASES, GemmCase, GemmCaseSource, GemmCaseSpec, GenError, build_case, build_cases,
    build_order_cases, gemm_cpu_reference, generate, simd128_order,
};

/// 試験ごとの一時ディレクトリ。破棄で消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "wasm-parity-host-order-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        TempDir(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

fn differing(a: &[f32], b: &[f32]) -> usize {
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count()
}

/// 和の順の参照と比べる、順を変えた式。
#[derive(Debug, Clone, Copy)]
enum Variant {
    /// 4 レーンの積と和を `mul_add` 1 回で積む。
    FusedLanes,
    /// 4 レーンの和を 3 番から 0 番の順に足す。
    ReversedLanes,
    /// 4 レーンの和を (0 番 + 1 番) + (2 番 + 3 番) で足す。
    PairwiseLanes,
    /// k の端数を足さない。
    NoTail,
}

fn variant(case: &GemmCase, v: Variant) -> Vec<f32> {
    let (m, k, n) = (case.m, case.k, case.n);
    let k4 = k - k % 4;
    let mut out = vec![0.0f32; m * n];
    for i in 0..m {
        for j in 0..n {
            let mut l = [0.0f32; 4];
            for p in (0..k4).step_by(4) {
                for (q, lane) in l.iter_mut().enumerate() {
                    let (x, y) = (case.a[i * k + p + q], case.b[(p + q) * n + j]);
                    *lane = match v {
                        Variant::FusedLanes => x.mul_add(y, *lane),
                        _ => *lane + x * y,
                    };
                }
            }
            let mut s = match v {
                Variant::ReversedLanes => ((l[3] + l[2]) + l[1]) + l[0],
                Variant::PairwiseLanes => (l[0] + l[1]) + (l[2] + l[3]),
                _ => ((l[0] + l[1]) + l[2]) + l[3],
            };
            if !matches!(v, Variant::NoTail) {
                for p in k4..k {
                    s = case.a[i * k + p].mul_add(case.b[p * n + j], s);
                }
            }
            out[i * n + j] = s;
        }
    }
    out
}

#[test]
fn order_cases_follow_generator_positions() {
    let cases = build_order_cases().unwrap();
    assert_eq!(cases.len(), 3);
    assert_eq!(
        GEMM_ORDER_CASES.map(|c| c.name),
        ["order-gemm-64", "order-gemm-256", "order-gemm-37x41x53"]
    );
    // 64³ は生成器の先頭から、256³ は 2*64*64 語の後から a、続けて b を取る。
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let head: Vec<f32> = (0..2 * 64 * 64 + 2 * 256 * 256)
        .map(|_| rng.next_signed_unit_f32())
        .collect();
    for (case, (size, start)) in cases.iter().zip([(64usize, 0usize), (256, 2 * 64 * 64)]) {
        assert_eq!((case.m, case.k, case.n), (size, size, size));
        let mut actual = case.a.clone();
        actual.extend_from_slice(&case.b);
        assert_eq!(
            bits(&actual),
            bits(&head[start..start + 2 * size * size]),
            "{}",
            case.name
        );
    }
    let row = GemmGateRow::Odd37x41x53;
    let gate = generate_f32(row).unwrap();
    assert_eq!(
        (cases[2].m, cases[2].k, cases[2].n),
        (row.m(), row.k(), row.n())
    );
    assert_eq!(bits(&cases[2].a), bits(gate.a()));
    assert_eq!(bits(&cases[2].b), bits(gate.b()));
}

#[test]
fn order_cases_share_inputs_with_existing_cases_except_256() {
    let order = build_order_cases().unwrap();
    let existing = build_cases().unwrap();
    assert_eq!(bits(&order[0].a), bits(&existing[0].a));
    assert_eq!(bits(&order[0].b), bits(&existing[0].b));
    assert_ne!(bits(&order[1].a), bits(&existing[1].a));
    assert_eq!(bits(&order[2].a), bits(&existing[2].a));
    assert_eq!(bits(&order[2].b), bits(&existing[2].b));
}

/// 手で選んだ 3 ケース（m=1・n=1）: 4 レーンの和を 0 番から順に足すと 3.0 が 3 回とも丸めで消え、端数の 2 つも消える。
/// 端数の 4.0・5.0 を昇順に足すと `100000008.0`（降順なら `100000016.0`）。端数の積和を 1 回の丸めで足すと
/// 2^-11 + 2^-24（積と和を別に丸めると 2^-11）。
#[test]
fn simd128_order_hand_case() {
    let case = GemmCase {
        name: "hand",
        m: 1,
        k: 6,
        n: 1,
        a: vec![1.0e8, 3.0, 3.0, 3.0, 3.0, 3.0],
        b: vec![1.0; 6],
    };
    assert_eq!(simd128_order(&case).unwrap(), vec![1.0e8]);
    assert_eq!(variant(&case, Variant::ReversedLanes), vec![100_000_008.0]);
    let tail_order = GemmCase {
        name: "hand-tail-order",
        m: 1,
        k: 6,
        n: 1,
        a: vec![1.0e8, 0.0, 0.0, 0.0, 4.0, 5.0],
        b: vec![1.0; 6],
    };
    assert_eq!(simd128_order(&tail_order).unwrap(), vec![100_000_008.0]);
    let x = 1.0 + 2.0f32.powi(-12);
    let tail_fused = GemmCase {
        name: "hand-tail-fused",
        m: 1,
        k: 5,
        n: 1,
        a: vec![-1.0, 0.0, 0.0, 0.0, x],
        b: vec![1.0, 1.0, 1.0, 1.0, x],
    };
    assert_eq!(
        simd128_order(&tail_fused).unwrap(),
        vec![2.0f32.powi(-11) + 2.0f32.powi(-24)]
    );
}

#[test]
fn simd128_order_rejects_bad_lengths() {
    let case = GemmCase {
        name: "bad",
        m: 2,
        k: 3,
        n: 2,
        a: vec![0.0; 5],
        b: vec![0.0; 6],
    };
    assert!(simd128_order(&case).is_err());
}

#[test]
fn simd128_order_rejects_unallocatable_output() {
    let case = GemmCase {
        name: "huge",
        m: usize::MAX,
        k: 0,
        n: 1,
        a: Vec::new(),
        b: Vec::new(),
    };
    assert_eq!(simd128_order(&case).unwrap_err(), GenError::InputFailed);
}

#[test]
fn square_from_rejects_unallocatable_inputs() {
    // 要素数（1 辺の 2 乗）は usize に収まり、f32 のバイト数は isize::MAX を超える寸法。
    let size = 1usize << (usize::BITS / 2 - 1);
    let spec = GemmCaseSpec {
        name: "huge",
        source: GemmCaseSource::SquareFrom(size, 0),
    };
    assert_eq!(build_case(&spec).unwrap_err(), GenError::InputFailed);
}

#[test]
fn cpu_reference_matches_core_and_default_scalar() {
    for case in build_order_cases().unwrap() {
        let reference = gemm_cpu_reference(&case).unwrap();
        let core = reference_gemm_f32(&case.a, &case.b, case.m, case.k, case.n).unwrap();
        assert_eq!(bits(&reference), bits(&core), "{}", case.name);
        let mut scalar = vec![0.0f32; case.m * case.n];
        DefaultCpuOps
            .gemm_scalar(&case.a, &case.b, case.m, case.k, case.n, &mut scalar)
            .unwrap();
        assert_eq!(bits(&reference), bits(&scalar), "{}", case.name);
    }
}

/// 和の順の参照は CPU 参照と語ごとのビットで違うが、`T1` の閾値の内に入る。
#[test]
fn simd128_order_differs_from_cpu_reference_within_t1() {
    for case in build_order_cases().unwrap() {
        let order = simd128_order(&case).unwrap();
        let reference = gemm_cpu_reference(&case).unwrap();
        assert!(differing(&order, &reference) > 0, "{}", case.name);
        assert!(
            compare_f32(&reference, &order, T1).unwrap().is_match(),
            "{}",
            case.name
        );
    }
}

/// 順を変えた式の出力は、どのケースでも和の順の参照と語ごとのビットで違う（端数を足さない式は端数のあるケースだけ）。
#[test]
fn variants_differ_from_simd128_order_by_bits() {
    for case in build_order_cases().unwrap() {
        let order = simd128_order(&case).unwrap();
        for v in [
            Variant::FusedLanes,
            Variant::ReversedLanes,
            Variant::PairwiseLanes,
        ] {
            assert!(
                differing(&order, &variant(&case, v)) > 0,
                "{} {v:?}",
                case.name
            );
        }
        let no_tail = differing(&order, &variant(&case, Variant::NoTail));
        if case.k % 4 == 0 {
            assert_eq!(no_tail, 0, "{}", case.name);
        } else {
            assert_eq!(no_tail, case.m * case.n, "{}", case.name);
        }
    }
}

#[test]
fn order_manifest_entries_match_files() {
    let dir = TempDir::new("manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let start = text.find("\"order-gemm-64\"").unwrap();
    let tail = &text[text[..start].rfind("    {").unwrap()..];
    let entries: Vec<&str> = tail
        .split("\n    }")
        .filter(|e| e.contains("\"name\""))
        .collect();
    assert_eq!(entries.len(), 3);
    let keys = [
        "name",
        "kind",
        "m",
        "k",
        "n",
        "a_file",
        "b_file",
        "native_scalar_file",
        "cpu_reference_file",
        "simd128_order_file",
        "a_sha256",
        "b_sha256",
        "native_scalar_sha256",
        "cpu_reference_sha256",
        "simd128_order_sha256",
    ];
    for (entry, case) in entries.iter().zip(build_order_cases().unwrap()) {
        let got: Vec<(&str, &str)> = entry
            .lines()
            .filter_map(|l| l.trim().strip_prefix('"'))
            .map(|l| {
                let (k, v) = l.split_once("\": ").unwrap();
                (k, v.trim_end_matches(',').trim_matches('"'))
            })
            .collect();
        assert_eq!(got.iter().map(|(k, _)| *k).collect::<Vec<_>>(), keys);
        let value = |k: &str| got.iter().find(|(key, _)| *key == k).unwrap().1;
        assert_eq!(value("name"), case.name);
        assert_eq!(value("kind"), "gemm-order");
        assert_eq!(value("m"), case.m.to_string());
        assert_eq!(value("k"), case.k.to_string());
        assert_eq!(value("n"), case.n.to_string());
        let mut native = vec![0.0f32; case.m * case.n];
        DefaultCpuOps
            .gemm_scalar(&case.a, &case.b, case.m, case.k, case.n, &mut native)
            .unwrap();
        for (file_key, hash_key, suffix, want) in [
            ("a_file", "a_sha256", "a", case.a.clone()),
            ("b_file", "b_sha256", "b", case.b.clone()),
            (
                "native_scalar_file",
                "native_scalar_sha256",
                "native-scalar",
                native.clone(),
            ),
            (
                "cpu_reference_file",
                "cpu_reference_sha256",
                "cpu-reference",
                gemm_cpu_reference(&case).unwrap(),
            ),
            (
                "simd128_order_file",
                "simd128_order_sha256",
                "simd128-order",
                simd128_order(&case).unwrap(),
            ),
        ] {
            assert_eq!(value(file_key), format!("{}.{suffix}.bin", case.name));
            let bytes = fs::read(dir.path().join(value(file_key))).unwrap();
            let expected: Vec<u8> = want.iter().flat_map(|v| v.to_le_bytes()).collect();
            assert_eq!(bytes, expected, "{} {file_key}", case.name);
            assert_eq!(value(hash_key), hex(&sha256_bytes(&bytes)));
        }
    }
}
