//! `cpu_ops` の wasm32 の `simd128` 版の計算（非公開）。
//!
//! wasm32 で `simd128` が有効なビルドでだけコンパイルされ、`DefaultCpuOps` の `*_simd` から呼ばれる。
//! 長さの判定は scalar 版と同じ関数を添字で触る前に置き、合わなければ `out` に何も書かずに
//! `InvalidArgument` を返す。GEMM は k を 4 つずつ 4 レーンの積と和（別の命令）で積み、4 レーンを
//! 先頭から順に足して、端数の k を昇順の `f32::mul_add` で足す（spec CPU-3 のポインタ）。
//! 要素ごと演算は 4 要素ずつ処理し、端数は scalar 版と同じ式で書く（spec CPU-6 のポインタ）。
//! 実行時に CPU の機能を調べず、unsafe も使わない（spec CPU-20 のポインタ）。

use super::UnaryElementwiseOp;
use super::scalar::{elementwise_lengths_ok, gemm_lengths_ok};
use core::arch::wasm32::{
    f32x4, f32x4_add, f32x4_extract_lane, f32x4_max, f32x4_mul, f32x4_splat, v128,
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
            UnaryElementwiseOp::Relu => f32x4_max(v, f32x4_splat(0.0)),
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
            UnaryElementwiseOp::Relu => x.max(0.0),
            UnaryElementwiseOp::AddScalar(c) => x + c,
        };
    }
    Ok(())
}
