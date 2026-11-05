//! CPU の演算の trait と型の置き場（feature `cpu-ops` の下にだけある。TASK-1351 のポインタ）。
//!
//! 上位の層から呼ばれる行列積・単項の要素ごと演算・部分 Top-k の口 [`CpuOps`] と、それを実装する
//! [`DefaultCpuOps`]・[`ReferenceCpuOps`] を持つ。計算の本体は非公開の `scalar` module にある。
//!
//! `DefaultCpuOps` の `gemm_simd`・`elementwise_simd` の版はビルド時の `cfg` で選ぶ。wasm32 で `simd128` が
//! 有効なビルドでは 4 レーンの命令の版を使い、それ以外のビルドでは `*_scalar` と同じ計算をする。
//! `topk_partial_simd` はどのビルドでも `topk_partial_scalar` と同じ計算をする。
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
/// 単項の要素ごと演算を `a` から `out` に書くことと、部分 Top-k を `out` に書くことを持つ（spec CPU-25 のポインタ）。
/// 行列積と要素ごと演算では、長さが合わない入力は `ContractError::InvalidArgument` で返し、`out` には触れない。
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
    /// simd 版の行列積。`a`・`b` を読み、`out` に書く。
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
    /// simd 版の要素ごと演算。`a` を読み、`out` に書く。
    fn elementwise_simd(
        &self,
        op: UnaryElementwiseOp,
        a: &[f32],
        out: &mut [f32],
    ) -> Result<(), ContractError>;
    /// scalar 版の部分 Top-k。有限のスコアだけを候補に、`f32::total_cmp` の降順（`-0.0` は `+0.0` より
    /// 小さい）、同点は `scores` の添字（スロット番号）の昇順で並べ、上位を `(スロット番号, スコア)` で
    /// `out` の先頭に書く。書く件数は `k`・有限のスコアの件数・`out` の長さの最小で、残りの位置には
    /// [`ReferenceCpuOps::SENTINEL`] を書く。`k` と `out` の長さの組では `Err` を返さない
    /// （spec CPU-10・CPU-15 のポインタ）。
    fn topk_partial_scalar(
        &self,
        scores: &[f32],
        k: usize,
        out: &mut [(u32, f32)],
    ) -> Result<(), ContractError>;
    /// simd 版の部分 Top-k。どの実装・どのビルドでも `topk_partial_scalar` と同じ計算をする
    /// （spec CPU-16 のポインタ）。
    fn topk_partial_simd(
        &self,
        scores: &[f32],
        k: usize,
        out: &mut [(u32, f32)],
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

/// `DefaultCpuOps::gemm_simd` の版（それ以外のビルド。scalar 版と同じ計算をする）。
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn gemm_simd_impl(
    a: &[f32],
    b: &[f32],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [f32],
) -> Result<(), ContractError> {
    scalar::gemm(a, b, m, k, n, out)
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

/// `DefaultCpuOps::elementwise_simd` の版（それ以外のビルド。scalar 版と同じ計算をする）。
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn elementwise_simd_impl(
    op: UnaryElementwiseOp,
    a: &[f32],
    out: &mut [f32],
) -> Result<(), ContractError> {
    scalar::elementwise(op, a, out)
}

/// [`CpuOps`] を実装する型。`gemm_simd`・`elementwise_simd` は、wasm32 で `simd128` が有効なビルドでは
/// 4 レーンの版を呼び、それ以外のビルドでは `*_scalar` と同じ計算をする。長さが合わなければどのビルドでも
/// `InvalidArgument`。`topk_partial_simd` はどのビルドでも `topk_partial_scalar` と同じ計算をする。
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
    fn topk_partial_scalar(
        &self,
        scores: &[f32],
        k: usize,
        out: &mut [(u32, f32)],
    ) -> Result<(), ContractError> {
        scalar::topk_partial(scores, k, out)
    }
    fn topk_partial_simd(
        &self,
        scores: &[f32],
        k: usize,
        out: &mut [(u32, f32)],
    ) -> Result<(), ContractError> {
        scalar::topk_partial(scores, k, out)
    }
}

/// どのメソッドも `*_scalar` と同じ計算をする [`CpuOps`] の実装（spec CPU-27 のポインタ）。
#[derive(Debug, Clone, Copy)]
pub struct ReferenceCpuOps;

impl ReferenceCpuOps {
    /// 部分 Top-k が `out` の埋まらない位置に書く値（spec CPU-27 のポインタ）。
    pub const SENTINEL: (u32, f32) = scalar::TOPK_SENTINEL;
}

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
    fn topk_partial_scalar(
        &self,
        scores: &[f32],
        k: usize,
        out: &mut [(u32, f32)],
    ) -> Result<(), ContractError> {
        scalar::topk_partial(scores, k, out)
    }
    fn topk_partial_simd(
        &self,
        scores: &[f32],
        k: usize,
        out: &mut [(u32, f32)],
    ) -> Result<(), ContractError> {
        scalar::topk_partial(scores, k, out)
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

    /// `DefaultCpuOps` の `*_simd` が、どのビルドでも計算した値を返す。
    #[test]
    fn default_simd_values() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let mut o = [0.0_f32; 4];
        DefaultCpuOps.gemm_simd(&a, &b, 2, 3, 2, &mut o).unwrap();
        assert_eq!(bits(&o), bits(&[58.0, 64.0, 139.0, 154.0]));
        let x = [-1.5, 0.0, 2.25, 3.0, -4.0];
        for op in [UnaryElementwiseOp::Relu, UnaryElementwiseOp::AddScalar(0.5)] {
            let mut want = [9.0_f32; 5];
            let mut got = [9.0_f32; 5];
            DefaultCpuOps.elementwise_scalar(op, &x, &mut want).unwrap();
            DefaultCpuOps.elementwise_simd(op, &x, &mut got).unwrap();
            assert_eq!(bits(&got), bits(&want));
        }
    }

    #[test]
    fn reference_agrees_with_scalar() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let mut want = [0.0_f32; 4];
        DefaultCpuOps
            .gemm_scalar(&a, &b, 2, 3, 2, &mut want)
            .unwrap();
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
            DefaultCpuOps.gemm_simd(&av, &bv, m, k, n, &mut ov)
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
        // あふれた積が一周して 0 になり、渡した長さとちょうど一致する入力。
        // 素の wrapping_mul に崩すと長さの検査をすり抜けるので、checked_mul の拒否を固定する。
        let half = 1_usize << (usize::BITS - 1);
        gemm_all(half, 2, 2, 0, 4, 0); // m×k と m×n が一周して 0
        gemm_all(2, half, 2, 0, 0, 4); // m×k と k×n が一周して 0
        gemm_all(0, half, 2, 0, 0, 0); // k×n だけが一周して 0
    }

    #[test]
    fn mismatched_elementwise_lengths_are_rejected() {
        for (alen, olen) in [(4, 3), (4, 5)] {
            let a = vec![1.0_f32; alen];
            let mut o = vec![7.0_f32; olen];
            for op in [UnaryElementwiseOp::Relu, UnaryElementwiseOp::AddScalar(1.0)] {
                assert!(is_invalid(DefaultCpuOps.elementwise_scalar(op, &a, &mut o)));
                assert!(is_invalid(DefaultCpuOps.elementwise_simd(op, &a, &mut o)));
                assert!(is_invalid(
                    ReferenceCpuOps.elementwise_scalar(op, &a, &mut o)
                ));
                assert!(is_invalid(ReferenceCpuOps.elementwise_simd(op, &a, &mut o)));
                assert!(o.iter().all(|v| v.to_bits() == 7.0_f32.to_bits()));
            }
        }
    }

    /// 4 通り（2 実装 × 2 メソッド）の結果を `(スロット, スコアのビット)` の列で返す。4 通りは同じでなければならない。
    fn topk_all(scores: &[f32], k: usize, out_len: usize) -> Vec<(u32, u32)> {
        let mut results = Vec::new();
        for which in 0..4 {
            let mut out = vec![(7_u32, 7.0_f32); out_len];
            let r = match which {
                0 => DefaultCpuOps.topk_partial_scalar(scores, k, &mut out),
                1 => DefaultCpuOps.topk_partial_simd(scores, k, &mut out),
                2 => ReferenceCpuOps.topk_partial_scalar(scores, k, &mut out),
                _ => ReferenceCpuOps.topk_partial_simd(scores, k, &mut out),
            };
            assert!(r.is_ok());
            results.push(
                out.iter()
                    .map(|&(s, v)| (s, v.to_bits()))
                    .collect::<Vec<_>>(),
            );
        }
        assert!(results.windows(2).all(|w| w[0] == w[1]));
        results.swap_remove(0)
    }

    fn pair(slot: u32, v: f32) -> (u32, u32) {
        (slot, v.to_bits())
    }

    const TOPK_INPUT: [f32; 7] = [1.0, f32::NAN, 3.0, 3.0, -0.0, 0.0, f32::INFINITY];

    #[test]
    fn topk_values() {
        let s = ReferenceCpuOps::SENTINEL;
        assert_eq!(
            topk_all(&TOPK_INPUT, 5, 6),
            vec![
                pair(2, 3.0),
                pair(3, 3.0),
                pair(0, 1.0),
                pair(5, 0.0),
                pair(4, -0.0),
                pair(s.0, s.1)
            ]
        );
    }

    #[test]
    fn topk_short_out() {
        assert_eq!(
            topk_all(&TOPK_INPUT, 5, 2),
            vec![pair(2, 3.0), pair(3, 3.0)]
        );
    }

    #[test]
    fn topk_k_zero_fills_sentinel() {
        let s = ReferenceCpuOps::SENTINEL;
        assert_eq!(topk_all(&TOPK_INPUT, 0, 3), vec![pair(s.0, s.1); 3]);
    }

    #[test]
    fn topk_empty_scores_fills_sentinel() {
        let s = ReferenceCpuOps::SENTINEL;
        assert_eq!(topk_all(&[], 4, 3), vec![pair(s.0, s.1); 3]);
    }

    #[test]
    fn topk_empty_out_is_ok() {
        assert!(topk_all(&TOPK_INPUT, 3, 0).is_empty());
    }

    #[test]
    fn topk_slot_range() {
        assert!(scalar::topk_slots_fit(u32::MAX as usize));
        #[cfg(target_pointer_width = "64")]
        assert!(!scalar::topk_slots_fit(u32::MAX as usize + 1));
    }

    #[test]
    fn sentinel_value() {
        assert_eq!(ReferenceCpuOps::SENTINEL.0, u32::MAX);
        assert_eq!(
            ReferenceCpuOps::SENTINEL.1.to_bits(),
            f32::NEG_INFINITY.to_bits()
        );
    }
}
