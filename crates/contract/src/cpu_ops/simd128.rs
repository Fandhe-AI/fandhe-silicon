//! `cpu_ops` の wasm32 の `simd128` 版の計算（非公開）。
//!
//! wasm32 で `simd128` が有効なビルドでだけコンパイルされ、`DefaultCpuOps` の `*_simd` から呼ばれる。
//! 長さの判定は scalar 版と同じ関数を添字で触る前に置き、合わなければ `out` に何も書かずに
//! `InvalidArgument` を返す。GEMM は k を 4 つずつ 4 レーンの積と和（別の命令）で積み、4 レーンを
//! 先頭から順に足す（spec CPU-3 のポインタ）。端数の k は昇順の `f32::mul_add` で足す（spec CPU-6 のポインタ）。
//! 要素ごと演算は 4 要素ずつ処理し、端数は scalar 版と同じ式で書く（spec CPU-20 のポインタ）。
//! 実行時に CPU の機能を調べず、unsafe も使わない。

use super::UnaryElementwiseOp;
use super::scalar::{elementwise_lengths_ok, gemm_lengths_ok, relu};
use core::arch::wasm32::{
    f32x4, f32x4_add, f32x4_extract_lane, f32x4_gt, f32x4_mul, f32x4_splat, v128, v128_and,
};
use fandhe_silicon_core::error::ContractError;

/// 4 レーンの値を先頭から順に足す。
fn sum_lanes(v: v128) -> f32 {
    ((f32x4_extract_lane::<0>(v) + f32x4_extract_lane::<1>(v)) + f32x4_extract_lane::<2>(v))
        + f32x4_extract_lane::<3>(v)
}

/// 行優先の f32 の行列積を 4 レーンで書く。長さが合わないときは `out` に触れず `InvalidArgument` を返す。
pub(super) fn gemm(
    a: &[f32],
    b: &[f32],
    m: usize,
    k: usize,
    n: usize,
    out: &mut [f32],
) -> Result<(), ContractError> {
    if !gemm_lengths_ok(a.len(), b.len(), out.len(), m, k, n) {
        return Err(ContractError::InvalidArgument(
            "gemm slice lengths do not match m, k, n".into(),
        ));
    }
    let k4 = k - k % 4;
    for i in 0..m {
        for j in 0..n {
            let mut acc = f32x4_splat(0.0);
            for p in (0..k4).step_by(4) {
                let va = f32x4(
                    a[i * k + p],
                    a[i * k + p + 1],
                    a[i * k + p + 2],
                    a[i * k + p + 3],
                );
                let vb = f32x4(
                    b[p * n + j],
                    b[(p + 1) * n + j],
                    b[(p + 2) * n + j],
                    b[(p + 3) * n + j],
                );
                acc = f32x4_add(acc, f32x4_mul(va, vb));
            }
            let mut s = sum_lanes(acc);
            for p in k4..k {
                s = a[i * k + p].mul_add(b[p * n + j], s);
            }
            out[i * n + j] = s;
        }
    }
    Ok(())
}

/// 単項の要素ごと演算を 4 要素ずつ書く。長さが違えば `out` に触れず `InvalidArgument` を返す。
pub(super) fn elementwise(
    op: UnaryElementwiseOp,
    a: &[f32],
    out: &mut [f32],
) -> Result<(), ContractError> {
    if !elementwise_lengths_ok(a.len(), out.len()) {
        return Err(ContractError::InvalidArgument(
            "elementwise input and output lengths differ".into(),
        ));
    }
    let (oc, orest) = out.as_chunks_mut::<4>();
    let (ac, arest) = a.as_chunks::<4>();
    for (o, x) in oc.iter_mut().zip(ac) {
        let v = f32x4(x[0], x[1], x[2], x[3]);
        let r = match op {
            // 比較の真偽のマスクで選ぶ。NaN は比較が偽になり 0.0 へ落ちるので、端数の式と揃う。
            UnaryElementwiseOp::Relu => v128_and(v, f32x4_gt(v, f32x4_splat(0.0))),
            UnaryElementwiseOp::AddScalar(c) => f32x4_add(v, f32x4_splat(c)),
        };
        *o = [
            f32x4_extract_lane::<0>(r),
            f32x4_extract_lane::<1>(r),
            f32x4_extract_lane::<2>(r),
            f32x4_extract_lane::<3>(r),
        ];
    }
    for (o, &x) in orest.iter_mut().zip(arest) {
        *o = match op {
            UnaryElementwiseOp::Relu => relu(x),
            UnaryElementwiseOp::AddScalar(c) => x + c,
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu_ops::scalar;

    /// 決定的な疑似乱数の列（外部依存なし）。
    fn fill(len: usize, seed: u32) -> Vec<f32> {
        let mut s = seed;
        (0..len)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((s >> 16) as f32 / 65_536.0) - 0.5
            })
            .collect()
    }

    #[test]
    fn gemm_matches_scalar_within_tolerance() {
        for (m, k, n) in [
            (1, 1, 1),
            (2, 3, 2),
            (3, 4, 3),
            (2, 5, 3),
            (3, 7, 2),
            (2, 9, 4),
        ] {
            let a = fill(m * k, 1);
            let b = fill(k * n, 2);
            let mut want = vec![0.0_f32; m * n];
            let mut got = vec![0.0_f32; m * n];
            scalar::gemm(&a, &b, m, k, n, &mut want).unwrap();
            gemm(&a, &b, m, k, n, &mut got).unwrap();
            for (w, g) in want.iter().zip(&got) {
                assert!((w - g).abs() <= 1e-5, "m={m} k={k} n={n}: {w} vs {g}");
            }
        }
    }

    #[test]
    fn gemm_exact_on_small_integers() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let mut out = [0.0_f32; 4];
        gemm(&a, &b, 2, 3, 2, &mut out).unwrap();
        assert_eq!(out, [58.0, 64.0, 139.0, 154.0]);
    }

    #[test]
    fn gemm_rejects_mismatch_without_writing() {
        let a = [1.0_f32; 6];
        let b = [1.0_f32; 6];
        let mut out = [7.0_f32; 3];
        assert!(matches!(
            gemm(&a, &b, 2, 3, 2, &mut out),
            Err(ContractError::InvalidArgument(_))
        ));
        assert_eq!(out, [7.0; 3]);
        let mut out = [7.0_f32; 4];
        assert!(gemm(&a, &b, usize::MAX, 2, 2, &mut out).is_err());
        assert_eq!(out, [7.0; 4]);
    }

    #[test]
    fn elementwise_matches_scalar_for_all_lengths() {
        for len in 0..=9 {
            let a = fill(len, 3);
            for op in [
                UnaryElementwiseOp::Relu,
                UnaryElementwiseOp::AddScalar(0.25),
            ] {
                // 出力を番兵で埋めてから呼ぶ。
                let mut want = vec![f32::from_bits(0x7fc0_dead); len];
                let mut got = vec![f32::from_bits(0x7fc0_dead); len];
                scalar::elementwise(op, &a, &mut want).unwrap();
                elementwise(op, &a, &mut got).unwrap();
                let wb: Vec<u32> = want.iter().map(|x| x.to_bits()).collect();
                let gb: Vec<u32> = got.iter().map(|x| x.to_bits()).collect();
                assert_eq!(wb, gb, "len={len}");
            }
        }
    }

    #[test]
    fn relu_nan_matches_scalar_in_lanes_and_remainder() {
        for len in [4, 5, 9] {
            let a = vec![f32::NAN; len];
            let mut want = vec![7.0_f32; len];
            let mut got = vec![7.0_f32; len];
            scalar::elementwise(UnaryElementwiseOp::Relu, &a, &mut want).unwrap();
            elementwise(UnaryElementwiseOp::Relu, &a, &mut got).unwrap();
            let wb: Vec<u32> = want.iter().map(|x| x.to_bits()).collect();
            let gb: Vec<u32> = got.iter().map(|x| x.to_bits()).collect();
            assert_eq!(wb, gb, "len={len}");
            assert!(got.iter().all(|v| *v == 0.0));
        }
    }

    #[test]
    fn elementwise_rejects_mismatch_without_writing() {
        let a = [1.0_f32; 5];
        for olen in [4, 6] {
            let mut out = vec![7.0_f32; olen];
            assert!(matches!(
                elementwise(UnaryElementwiseOp::Relu, &a, &mut out),
                Err(ContractError::InvalidArgument(_))
            ));
            assert!(out.iter().all(|v| *v == 7.0));
        }
    }
}
