//! CPU の演算の trait と型の置き場（feature `cpu-ops` の下にだけある。TASK-1351 のポインタ）。
//!
//! 上位の層から呼ばれる行列積と単項の要素ごと演算の口 [`CpuOps`] と、それを実装する
//! [`DefaultCpuOps`]・[`ReferenceCpuOps`] を持つ。計算の本体は非公開の `scalar` module にある。
//!
//! `DefaultCpuOps` の `*_simd` の版はビルド時の `cfg` で選ぶ。wasm32 で `simd128` が有効なビルドでは
//! 4 レーンの命令の版を使い、それ以外のビルドでは `UnsupportedExtensionDefault` を返す。
//! 実行時に CPU の機能を調べない（spec CPU-26 のポインタ）。

mod scalar;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod simd128;

use fandhe_silicon_core::error::ContractError;

/// 単項の要素ごと演算の種類。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryElementwiseOp {
    /// 各要素を `max(x, 0.0)` にする（spec CPU-18 のポインタ）。
    Relu,
    /// 各要素に定数を足す（spec CPU-19 のポインタ）。
    AddScalar(f32),
}

/// CPU の演算の口。行優先の f32 の行列積 `a`（m×k）×`b`（k×n）を `out`（m×n）に書くことと、
/// 単項の要素ごと演算を `a` から `out` に書くことを持つ（spec CPU-25 のポインタ）。
/// 長さが合わない入力は `ContractError::InvalidArgument` で返し、`out` には触れない。
/// ただし wasm32 で `simd128` が有効でないビルドでは、[`DefaultCpuOps`] の `*_simd` は入力を見ずに
/// `UnsupportedExtensionDefault` を返す（未対応を優先する）。
pub trait CpuOps {
    /// scalar 版の行列積。`a`・`b` を読み、`out` に書く。
    fn gemm_scalar(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [f32],
    ) -> Result<(), ContractError>;
    /// simd 版の行列積。`a`・`b` を読み、`out` に書く。対応しない実装は `Err` を返す。
    fn gemm_simd(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [f32],
    ) -> Result<(), ContractError>;
    /// scalar 版の要素ごと演算。`a` を読み、`out` に書く。
    fn elementwise_scalar(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
        out: &mut [f32],
    ) -> Result<(), ContractError>;
    /// simd 版の要素ごと演算。`a` を読み、`out` に書く。対応しない実装は `Err` を返す。
    fn elementwise_simd(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
        out: &mut [f32],
    ) -> Result<(), ContractError>;
}

/// `DefaultCpuOps::gemm_simd` の版（wasm32 で `simd128` が有効なビルド）。
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
fn gemm_simd_impl(
    a: &[f32],
    b: &[f32],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [f32],
) -> Result<(), ContractError> {
    simd128::gemm(a, b, m, k, n, out)
}

/// `DefaultCpuOps::gemm_simd` の版（それ以外のビルド。未対応を返す）。
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn gemm_simd_impl(
    _a: &[f32],
    _b: &[f32],
    _m: usize,
    _k: usize,
    _n: usize,
    _out: &mut [f32],
) -> Result<(), ContractError> {
    Err(ContractError::UnsupportedExtensionDefault { op: "gemm_simd" })
}

/// `DefaultCpuOps::elementwise_simd` の版（wasm32 で `simd128` が有効なビルド）。
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
fn elementwise_simd_impl(
    op: UnaryElementwiseOp,
    a: &[f32],
    out: &mut [f32],
) -> Result<(), ContractError> {
    simd128::elementwise(op, a, out)
}

/// `DefaultCpuOps::elementwise_simd` の版（それ以外のビルド。未対応を返す）。
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn elementwise_simd_impl(
    _op: UnaryElementwiseOp,
    _a: &[f32],
    _out: &mut [f32],
) -> Result<(), ContractError> {
    Err(ContractError::UnsupportedExtensionDefault {
        op: "elementwise_simd",
    })
}

/// [`CpuOps`] を実装する型。`*_simd` は、wasm32 で `simd128` が有効なビルドでは 4 レーンの版
/// （長さが合わなければ `InvalidArgument`）を呼び、それ以外のビルドでは `UnsupportedExtensionDefault` を返す。
#[derive(Debug, Clone, Copy)]
pub struct DefaultCpuOps;

impl CpuOps for DefaultCpuOps {
    fn gemm_scalar(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        scalar::gemm(a, b, m, k, n, out)
    }
    fn gemm_simd(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        gemm_simd_impl(a, b, m, k, n, out)
    }
    fn elementwise_scalar(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        scalar::elementwise(op, a, out)
    }
    fn elementwise_simd(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        elementwise_simd_impl(op, a, out)
    }
}

/// どのメソッドも `*_scalar` と同じ計算をする [`CpuOps`] の実装（spec CPU-27 のポインタ）。
#[derive(Debug, Clone, Copy)]
pub struct ReferenceCpuOps;

impl CpuOps for ReferenceCpuOps {
    fn gemm_scalar(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        scalar::gemm(a, b, m, k, n, out)
    }
    fn gemm_simd(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        k: usize,
        n: usize,
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        scalar::gemm(a, b, m, k, n, out)
    }
    fn elementwise_scalar(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        scalar::elementwise(op, a, out)
    }
    fn elementwise_simd(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
        out: &mut [f32],
    ) -> Result<(), ContractError> {
        scalar::elementwise(op, a, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(v: &[f32]) -> Vec<u32> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    #[test]
    fn gemm_values() {
        let mut out = [0.0_f32; 1];
        DefaultCpuOps
            .gemm_scalar(&[2.0], &[3.0], 1, 1, 1, &mut out)
            .unwrap();
        assert_eq!(bits(&out), bits(&[6.0]));

        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let mut out = [0.0_f32; 4];
        DefaultCpuOps
            .gemm_scalar(&a, &b, 2, 3, 2, &mut out)
            .unwrap();
        assert_eq!(bits(&out), bits(&[58.0, 64.0, 139.0, 154.0]));
    }

    #[test]
    fn elementwise_values() {
        let a = [-1.5, 0.0, 2.25, 3.0];
        let mut out = [9.0_f32; 4];
        DefaultCpuOps
            .elementwise_scalar(UnaryElementwiseOp::Relu, &a, &mut out)
            .unwrap();
        assert_eq!(bits(&out), bits(&[0.0, 0.0, 2.25, 3.0]));
        DefaultCpuOps
            .elementwise_scalar(UnaryElementwiseOp::AddScalar(0.5), &a, &mut out)
            .unwrap();
        assert_eq!(bits(&out), bits(&[-1.0, 0.5, 2.75, 3.5]));
    }

    #[test]
    fn reference_agrees_and_default_simd_unsupported() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let mut want = [0.0_f32; 4];
        DefaultCpuOps
            .gemm_scalar(&a, &b, 2, 3, 2, &mut want)
            .unwrap();
        let mut o = [0.0_f32; 4];
        assert_eq!(
            DefaultCpuOps.gemm_simd(&a, &b, 2, 3, 2, &mut o),
            Err(ContractError::UnsupportedExtensionDefault { op: "gemm_simd" })
        );
        let mut o = [0.0_f32; 4];
        ReferenceCpuOps
            .gemm_scalar(&a, &b, 2, 3, 2, &mut o)
            .unwrap();
        assert_eq!(bits(&o), bits(&want));
        let mut o = [0.0_f32; 4];
        ReferenceCpuOps.gemm_simd(&a, &b, 2, 3, 2, &mut o).unwrap();
        assert_eq!(bits(&o), bits(&want));

        let x = [-1.5, 0.0, 2.25, 3.0];
        let op = UnaryElementwiseOp::AddScalar(0.5);
        let mut want = [0.0_f32; 4];
        DefaultCpuOps.elementwise_scalar(op, &x, &mut want).unwrap();
        let mut o = [0.0_f32; 4];
        assert_eq!(
            DefaultCpuOps.elementwise_simd(op, &x, &mut o),
            Err(ContractError::UnsupportedExtensionDefault {
                op: "elementwise_simd"
            })
        );
        let mut o = [0.0_f32; 4];
        ReferenceCpuOps.elementwise_scalar(op, &x, &mut o).unwrap();
        assert_eq!(bits(&o), bits(&want));
        let mut o = [0.0_f32; 4];
        ReferenceCpuOps.elementwise_simd(op, &x, &mut o).unwrap();
        assert_eq!(bits(&o), bits(&want));
    }

    fn is_invalid(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    fn gemm_all(m: usize, k: usize, n: usize, a: usize, b: usize, o: usize) {
        let (av, bv) = (vec![1.0_f32; a], vec![1.0_f32; b]);
        let mut ov = vec![7.0_f32; o];
        assert!(is_invalid(
            DefaultCpuOps.gemm_scalar(&av, &bv, m, k, n, &mut ov)
        ));
        assert!(is_invalid(
            ReferenceCpuOps.gemm_scalar(&av, &bv, m, k, n, &mut ov)
        ));
        assert!(is_invalid(
            ReferenceCpuOps.gemm_simd(&av, &bv, m, k, n, &mut ov)
        ));
        assert!(ov.iter().all(|v| v.to_bits() == 7.0_f32.to_bits()));
    }

    #[test]
    fn mismatched_gemm_lengths_are_rejected() {
        gemm_all(2, 3, 2, 5, 6, 4);
        gemm_all(2, 3, 2, 7, 6, 4);
        gemm_all(2, 3, 2, 6, 5, 4);
        gemm_all(2, 3, 2, 6, 6, 3);
        gemm_all(2, 3, 2, 6, 6, 5);
        gemm_all(usize::MAX, 2, 2, 4, 4, 4);
    }

    #[test]
    fn mismatched_elementwise_lengths_are_rejected() {
        for (alen, olen) in [(4, 3), (4, 5)] {
            let a = vec![1.0_f32; alen];
            let mut o = vec![7.0_f32; olen];
            for op in [UnaryElementwiseOp::Relu, UnaryElementwiseOp::AddScalar(1.0)] {
                assert!(is_invalid(DefaultCpuOps.elementwise_scalar(op, &a, &mut o)));
                assert!(is_invalid(
                    ReferenceCpuOps.elementwise_scalar(op, &a, &mut o)
                ));
                assert!(is_invalid(ReferenceCpuOps.elementwise_simd(op, &a, &mut o)));
                assert!(o.iter().all(|v| v.to_bits() == 7.0_f32.to_bits()));
            }
        }
    }
}
