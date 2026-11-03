//! `cpu_ops` の GEMM と要素ごと演算を、共通の crate の CPU 参照（`fandhe_silicon_core::parity`）と
//! 比べる外の試験（TASK-1351 のポインタ）。feature `cpu-ops` と `test-support` が有効な組でだけ走る。
//! 公開のパスだけを使い、入力は固定の seed から決定的に作る。

use fandhe_silicon_contract::cpu_ops::{
    CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, SplitMix64, generate_f32,
};
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};

/// 要素ごと演算の入力の長さ。
const ELEMENTWISE_LEN: usize = 1_000_003;

/// GEMM の 1 ケース（`a` は m×k、`b` は k×n、行優先）。
struct GemmCase {
    m: usize,
    k: usize,
    n: usize,
    a: Vec<f32>,
    b: Vec<f32>,
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn assert_finite(v: &[f32]) {
    assert!(v.iter().all(|x| x.is_finite()));
}

/// 同じ seed の生成器から `a`、続けて `b` を作る正方のケース。
fn square_case(size: usize) -> GemmCase {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let a: Vec<f32> = (0..size * size)
        .map(|_| rng.next_signed_unit_f32())
        .collect();
    let b: Vec<f32> = (0..size * size)
        .map(|_| rng.next_signed_unit_f32())
        .collect();
    assert_finite(&a);
    assert_finite(&b);
    GemmCase {
        m: size,
        k: size,
        n: size,
        a,
        b,
    }
}

fn odd_case() -> GemmCase {
    let row = GemmGateRow::Odd37x41x53;
    let inputs = generate_f32(row).unwrap();
    assert_finite(inputs.a());
    assert_finite(inputs.b());
    GemmCase {
        m: row.m(),
        k: row.k(),
        n: row.n(),
        a: inputs.a().to_vec(),
        b: inputs.b().to_vec(),
    }
}

/// 64³・256³・37×41×53 の 3 ケース。
fn gemm_cases() -> Vec<GemmCase> {
    vec![square_case(64), square_case(256), odd_case()]
}

/// 長さ 1,000,003 の有限で 0 を含まない入力。
fn elementwise_input() -> Vec<f32> {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let v: Vec<f32> = (0..ELEMENTWISE_LEN)
        .map(|_| {
            loop {
                let x = rng.next_signed_unit_f32();
                if x != 0.0 {
                    break x;
                }
            }
        })
        .collect();
    assert_eq!(v.len(), ELEMENTWISE_LEN);
    assert_finite(&v);
    assert!(v.iter().all(|x| *x != 0.0));
    v
}

fn ops() -> [UnaryElementwiseOp; 2] {
    [UnaryElementwiseOp::Relu, UnaryElementwiseOp::AddScalar(0.5)]
}

/// 実装の型ごとの呼び分けを 1 つの関数にそろえる補助。
trait CpuOpsDyn {
    fn scalar_gemm(&self, c: &GemmCase) -> Result<Vec<f32>, ContractError>;
    fn simd_gemm(&self, c: &GemmCase) -> Result<Vec<f32>, ContractError>;
    fn scalar_elementwise(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
    ) -> Result<Vec<f32>, ContractError>;
    fn simd_elementwise(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
    ) -> Result<Vec<f32>, ContractError>;
}

impl<T: CpuOps> CpuOpsDyn for T {
    fn scalar_gemm(&self, c: &GemmCase) -> Result<Vec<f32>, ContractError> {
        let mut out = vec![0.0_f32; c.m * c.n];
        self.gemm_scalar(&c.a, &c.b, c.m, c.k, c.n, &mut out)?;
        Ok(out)
    }
    fn simd_gemm(&self, c: &GemmCase) -> Result<Vec<f32>, ContractError> {
        let mut out = vec![0.0_f32; c.m * c.n];
        self.gemm_simd(&c.a, &c.b, c.m, c.k, c.n, &mut out)?;
        Ok(out)
    }
    fn scalar_elementwise(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
    ) -> Result<Vec<f32>, ContractError> {
        // 出力を番兵で埋めてから呼ぶ。
        let mut out = vec![f32::from_bits(0x7fc0_dead); a.len()];
        self.elementwise_scalar(op, a, &mut out)?;
        Ok(out)
    }
    fn simd_elementwise(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
    ) -> Result<Vec<f32>, ContractError> {
        // 出力を番兵で埋めてから呼ぶ。
        let mut out = vec![f32::from_bits(0x7fc0_dead); a.len()];
        self.elementwise_simd(op, a, &mut out)?;
        Ok(out)
    }
}

/// scalar 版の GEMM が CPU 参照と T1 で一致し、積の順も同じでビットまで一致する（CPU-1）。
#[test]
fn gemm_scalar_matches_cpu_reference() {
    for c in gemm_cases() {
        let reference = reference_gemm_f32(&c.a, &c.b, c.m, c.k, c.n).unwrap();
        let out = DefaultCpuOps.scalar_gemm(&c).unwrap();
        assert!(compare_f32(&reference, &out, T1).unwrap().is_match());
        assert_eq!(bits(&reference), bits(&out));
    }
}

/// `DefaultCpuOps` の `gemm_simd` が CPU 参照と T1 で一致する（CPU-4）。
#[test]
fn gemm_simd_matches_cpu_reference() {
    for c in gemm_cases() {
        let reference = reference_gemm_f32(&c.a, &c.b, c.m, c.k, c.n).unwrap();
        let out = DefaultCpuOps.simd_gemm(&c).unwrap();
        assert!(compare_f32(&reference, &out, T1).unwrap().is_match());
    }
}

/// `ReferenceCpuOps` は 4 メソッドとも `DefaultCpuOps` の scalar とビットまで一致する（CPU-27）。
#[test]
fn reference_ops_follow_scalar_path() {
    for c in gemm_cases() {
        let want = bits(&DefaultCpuOps.scalar_gemm(&c).unwrap());
        assert_eq!(bits(&ReferenceCpuOps.scalar_gemm(&c).unwrap()), want);
        assert_eq!(bits(&ReferenceCpuOps.simd_gemm(&c).unwrap()), want);
    }
    let a = elementwise_input();
    for op in ops() {
        let want = bits(&DefaultCpuOps.scalar_elementwise(op, &a).unwrap());
        assert_eq!(
            bits(&ReferenceCpuOps.scalar_elementwise(op, &a).unwrap()),
            want
        );
        assert_eq!(
            bits(&ReferenceCpuOps.simd_elementwise(op, &a).unwrap()),
            want
        );
    }
}

/// `Relu` は各要素が `max(x, 0.0)` とビットまで一致する（CPU-18）。
#[test]
fn elementwise_scalar_relu() {
    let a = elementwise_input();
    let out = DefaultCpuOps
        .scalar_elementwise(UnaryElementwiseOp::Relu, &a)
        .unwrap();
    let want: Vec<f32> = a.iter().map(|x| x.max(0.0)).collect();
    assert_eq!(bits(&out), bits(&want));
}

/// `AddScalar(0.5)` は各要素が `x + 0.5` とビットまで一致する（CPU-19）。
#[test]
fn elementwise_scalar_add_scalar() {
    let a = elementwise_input();
    let out = DefaultCpuOps
        .scalar_elementwise(UnaryElementwiseOp::AddScalar(0.5), &a)
        .unwrap();
    let want: Vec<f32> = a.iter().map(|x| x + 0.5).collect();
    assert_eq!(bits(&out), bits(&want));
}

/// `elementwise_simd` が scalar とビットまで一致する（CPU-20）。
#[test]
fn elementwise_simd_matches_scalar_bits() {
    let full = elementwise_input();
    let mut inputs: Vec<&[f32]> = vec![&full];
    for len in 0..10 {
        inputs.push(&full[..len]);
    }
    for a in inputs {
        for op in ops() {
            let want = DefaultCpuOps.scalar_elementwise(op, a).unwrap();
            let got = DefaultCpuOps.simd_elementwise(op, a).unwrap();
            assert_eq!(bits(&got), bits(&want));
        }
    }
}

fn is_invalid(r: Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

/// 長さが合わない・積があふれる入力でも、4 メソッド × 2 型が panic せず `InvalidArgument` を返し、`out` に触れない。
#[test]
fn mismatched_lengths_do_not_panic() {
    // (m, k, n, a の長さ, b の長さ, out の長さ)
    let gemm_shapes = [
        (2, 3, 2, 5, 6, 4),
        (2, 3, 2, 7, 6, 4),
        (2, 3, 2, 6, 5, 4),
        (2, 3, 2, 6, 6, 3),
        (2, 3, 2, 6, 6, 5),
        (usize::MAX, 2, 2, 4, 4, 4),
    ];
    for (m, k, n, al, bl, ol) in gemm_shapes {
        let (a, b) = (vec![1.0_f32; al], vec![1.0_f32; bl]);
        let mut o = vec![7.0_f32; ol];
        assert!(is_invalid(
            DefaultCpuOps.gemm_scalar(&a, &b, m, k, n, &mut o)
        ));
        assert!(is_invalid(
            ReferenceCpuOps.gemm_scalar(&a, &b, m, k, n, &mut o)
        ));
        assert!(is_invalid(
            ReferenceCpuOps.gemm_simd(&a, &b, m, k, n, &mut o)
        ));
        assert!(is_invalid(DefaultCpuOps.gemm_simd(&a, &b, m, k, n, &mut o)));
        assert!(o.iter().all(|v| v.to_bits() == 7.0_f32.to_bits()));
    }
    for (al, ol) in [(4, 3), (4, 5)] {
        let a = vec![1.0_f32; al];
        let mut o = vec![7.0_f32; ol];
        for op in ops() {
            assert!(is_invalid(DefaultCpuOps.elementwise_scalar(op, &a, &mut o)));
            assert!(is_invalid(
                ReferenceCpuOps.elementwise_scalar(op, &a, &mut o)
            ));
            assert!(is_invalid(ReferenceCpuOps.elementwise_simd(op, &a, &mut o)));
            assert!(is_invalid(DefaultCpuOps.elementwise_simd(op, &a, &mut o)));
            assert!(o.iter().all(|v| v.to_bits() == 7.0_f32.to_bits()));
        }
    }
}

/// `AddScalar(c)` は 0.5 以外の定数でも各要素が `x + c` とビットまで一致する。
/// `DefaultCpuOps` と `ReferenceCpuOps` の scalar 版・SIMD 版を、4 の倍数と端数のある長さで比べる（CPU-19）。
#[test]
fn elementwise_add_scalar_other_constants() {
    let full = elementwise_input();
    let lens = [0, 1, 3, 4, 5, 7, 8, 13, 1024, 1027, ELEMENTWISE_LEN];
    for c in [-2.5_f32, 1.0, 3.75, 0.3, -0.125, 1.0e6] {
        let op = UnaryElementwiseOp::AddScalar(c);
        for len in lens {
            let a = &full[..len];
            let want: Vec<f32> = a.iter().map(|x| x + c).collect();
            let want = bits(&want);
            assert_eq!(
                bits(&DefaultCpuOps.scalar_elementwise(op, a).unwrap()),
                want
            );
            assert_eq!(bits(&DefaultCpuOps.simd_elementwise(op, a).unwrap()), want);
            assert_eq!(
                bits(&ReferenceCpuOps.scalar_elementwise(op, a).unwrap()),
                want
            );
            assert_eq!(
                bits(&ReferenceCpuOps.simd_elementwise(op, a).unwrap()),
                want
            );
        }
    }
}
