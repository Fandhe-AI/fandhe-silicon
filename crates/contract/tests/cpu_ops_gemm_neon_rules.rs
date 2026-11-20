//! `cpu_ops` の `gemm_simd` の aarch64 の `neon` 版の和の順と k の端数を確かめる外の試験（TASK-2446 のポインタ）。
//! feature `cpu-ops` と `test-support` が有効な組でだけ走る。`neon` の版を呼ぶ 2 本は aarch64 で `neon` が有効な
//! ビルドでだけコンパイルされ、ほかの 3 本は `cfg` を持たない。入力の生成器は `fandhe_silicon_core::parity::gemm_inputs`、
//! 比べは `compare_f32`（TASK-24 のポインタ）。

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps};
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, SplitMix64, generate_f32,
};
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};

/// 出力を埋める番兵のビット列。
const SENTINEL_BITS: u32 = 0x7fc0_beef;

/// 1 つの GEMM の入力（`a` は m×k、`b` は k×n、行優先）。
struct Gemm {
    m: usize,
    k: usize,
    n: usize,
    a: Vec<f32>,
    b: Vec<f32>,
}

/// 4 レーンの和の式の形。
#[derive(Clone, Copy)]
enum Form {
    /// レーンごとに `mul_add` で積み、`(l0 + l1) + (l2 + l3)` で足し、端数の k を `mul_add` で足す。
    Lanes,
    /// レーンごとの積と和を別の演算にする。
    SplitLaneProduct,
    /// 4 レーンを `((l0 + l1) + l2) + l3` で足す。
    SerialLaneSum,
    /// 端数の k の積と和を別の演算にする。
    SplitTailProduct,
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
fn three_inputs() -> Vec<Gemm> {
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

/// k を 4 つずつ 4 レーンに積み、レーンの和を取り、端数の k を足す式を `form` の形で書いた参照。
fn lane_reference(c: &Gemm, form: Form) -> Vec<f32> {
    let (m, k, n) = (c.m, c.k, c.n);
    let k4 = k - k % 4;
    let mut out = vec![0.0_f32; m * n];
    for i in 0..m {
        for j in 0..n {
            let mut l = [0.0_f32; 4];
            for p in (0..k4).step_by(4) {
                for (q, lane) in l.iter_mut().enumerate() {
                    let (x, y) = (c.a[i * k + p + q], c.b[(p + q) * n + j]);
                    *lane = match form {
                        Form::SplitLaneProduct => *lane + x * y,
                        _ => x.mul_add(y, *lane),
                    };
                }
            }
            let mut s = match form {
                Form::SerialLaneSum => ((l[0] + l[1]) + l[2]) + l[3],
                _ => (l[0] + l[1]) + (l[2] + l[3]),
            };
            for p in k4..k {
                let (x, y) = (c.a[i * k + p], c.b[p * n + j]);
                s = match form {
                    Form::SplitTailProduct => s + x * y,
                    _ => x.mul_add(y, s),
                };
            }
            out[i * n + j] = s;
        }
    }
    out
}

/// 出力を番兵で埋めてから `DefaultCpuOps` の `gemm_scalar`（`simd` が偽）または `gemm_simd` を呼ぶ。
fn run(c: &Gemm, simd: bool) -> Vec<f32> {
    let mut out = vec![f32::from_bits(SENTINEL_BITS); c.m * c.n];
    if simd {
        DefaultCpuOps
            .gemm_simd(&c.a, &c.b, c.m, c.k, c.n, &mut out)
            .unwrap();
    } else {
        DefaultCpuOps
            .gemm_scalar(&c.a, &c.b, c.m, c.k, c.n, &mut out)
            .unwrap();
    }
    out
}

/// 3 つの入力の (m, k, n) と、k を 4 で割った余りと、値が有限であること。
#[test]
fn three_inputs_have_the_expected_shapes() {
    let shapes: Vec<(usize, usize, usize, usize)> = three_inputs()
        .iter()
        .map(|c| {
            assert_eq!(c.a.len(), c.m * c.k);
            assert_eq!(c.b.len(), c.k * c.n);
            assert!(c.a.iter().chain(&c.b).all(|x| x.is_finite()));
            (c.m, c.k, c.n, c.k % 4)
        })
        .collect();
    assert_eq!(
        shapes,
        [(64, 64, 64, 0), (256, 256, 256, 0), (37, 53, 41, 1)]
    );
}

/// 4 レーンの参照が、3 つの入力で `gemm_scalar` と T1 で一致してビットが違い、レーンの積和・レーンの和の形を
/// 変えた式とビットが違う。端数の積和の形を変えた式とは、37×41×53 でビットが違い、64³・256³ で同じ。
#[test]
fn lane_reference_differs_from_scalar_and_other_forms() {
    for c in three_inputs() {
        let shape = format!("{}x{}x{}", c.m, c.k, c.n);
        let lanes = lane_reference(&c, Form::Lanes);
        let scalar = run(&c, false);
        let cmp = compare_f32(&scalar, &lanes, T1).unwrap();
        assert_eq!(cmp.compared(), c.m * c.n, "{shape}");
        assert!(cmp.is_match(), "{shape}");
        assert_ne!(bits(&lanes), bits(&scalar), "{shape}");
        for form in [Form::SplitLaneProduct, Form::SerialLaneSum] {
            assert_ne!(bits(&lanes), bits(&lane_reference(&c, form)), "{shape}");
        }
        let split_tail = bits(&lane_reference(&c, Form::SplitTailProduct));
        if c.k % 4 == 0 {
            assert_eq!(bits(&lanes), split_tail, "{shape}");
        } else {
            assert_ne!(bits(&lanes), split_tail, "{shape}");
        }
    }
}

/// `gemm_simd` が 3 つの入力で出力の全要素を書く。番兵のビットが残らず、全要素が有限（CPU-6）。
#[test]
fn gemm_simd_writes_every_element_of_three_inputs() {
    for c in three_inputs() {
        let shape = format!("{}x{}x{}", c.m, c.k, c.n);
        let out = run(&c, true);
        assert_eq!(out.len(), c.m * c.n, "{shape}");
        let left = out.iter().filter(|x| x.to_bits() == SENTINEL_BITS).count();
        assert_eq!(left, 0, "{shape}: sentinel count");
        assert!(out.iter().all(|x| x.is_finite()), "{shape}");
    }
}

/// aarch64 で `neon` が有効なビルドの `gemm_simd` が、3 つの入力で `gemm_scalar` と T1 で一致し、
/// ビットが違う（CPU-2）。
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[test]
fn neon_gemm_simd_matches_scalar_by_t1() {
    for c in three_inputs() {
        let shape = format!("{}x{}x{}", c.m, c.k, c.n);
        let simd = run(&c, true);
        let scalar = run(&c, false);
        let cmp = compare_f32(&scalar, &simd, T1).unwrap();
        assert_eq!(cmp.compared(), c.m * c.n, "{shape}");
        assert!(cmp.is_match(), "{shape}");
        assert_ne!(bits(&simd), bits(&scalar), "{shape}");
    }
}

/// aarch64 で `neon` が有効なビルドの `gemm_simd` が、3 つの入力で 4 レーンの参照とビットまで一致する。
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[test]
fn neon_gemm_simd_equals_lane_reference_bits() {
    for c in three_inputs() {
        let shape = format!("{}x{}x{}", c.m, c.k, c.n);
        assert_eq!(
            bits(&run(&c, true)),
            bits(&lane_reference(&c, Form::Lanes)),
            "{shape}"
        );
    }
}
