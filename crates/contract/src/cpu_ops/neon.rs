//! `cpu_ops` の aarch64 の `neon` 版の計算（非公開）。
//!
//! aarch64 で `neon` が有効なビルドでだけコンパイルされ、`DefaultCpuOps` の `*_simd` から呼ばれる。
//! `unsafe` は `gemm`・`elementwise` の 2 か所の呼び出しだけで、生のポインタを作らない。
//! 行列積は k を 4 つずつ 4 レーンの積和に蓄え、水平和を取る（spec CPU-2 のポインタ）。
//! 端数の k は昇順の `f32::mul_add` で足す（spec CPU-6 のポインタ）。
//! 要素ごと演算は 4 要素ずつ処理し、端数は scalar 版と同じ式で書く（spec CPU-20 のポインタ）。
//! 実行時に CPU の機能を調べない。

use super::UnaryElementwiseOp;
use super::scalar::{elementwise_lengths_ok, gemm_lengths_ok, relu};
use core::arch::aarch64::{
    float32x4_t, vaddq_f32, vaddvq_f32, vandq_u32, vcgtq_f32, vdupq_n_f32, vfmaq_f32,
    vgetq_lane_f32, vreinterpretq_f32_u32, vreinterpretq_u32_f32, vsetq_lane_f32,
};
use fandhe_silicon_core::error::ContractError;

/// 長さ 4 の配列から 4 レーンの値を作る。
#[inline]
#[target_feature(enable = "neon")]
fn load(x: &[f32; 4]) -> float32x4_t {
    let v = vdupq_n_f32(x[0]);
    let v = vsetq_lane_f32::<1>(x[1], v);
    let v = vsetq_lane_f32::<2>(x[2], v);
    vsetq_lane_f32::<3>(x[3], v)
}

/// 4 レーンの値を長さ 4 の配列にする。
#[inline]
#[target_feature(enable = "neon")]
fn store(v: float32x4_t) -> [f32; 4] {
    [
        vgetq_lane_f32::<0>(v),
        vgetq_lane_f32::<1>(v),
        vgetq_lane_f32::<2>(v),
        vgetq_lane_f32::<3>(v),
    ]
}

/// 長さが検査済みの行列積を 4 レーンで書く。
#[target_feature(enable = "neon")]
fn gemm_lanes(a: &[f32], b: &[f32], m: usize, k: usize, n: usize, out: &mut [f32]) {
    let k4 = k - k % 4;
    for i in 0..m {
        for j in 0..n {
            let mut acc = vdupq_n_f32(0.0);
            for p in (0..k4).step_by(4) {
                let va = load(&[
                    a[i * k + p],
                    a[i * k + p + 1],
                    a[i * k + p + 2],
                    a[i * k + p + 3],
                ]);
                let vb = load(&[
                    b[p * n + j],
                    b[(p + 1) * n + j],
                    b[(p + 2) * n + j],
                    b[(p + 3) * n + j],
                ]);
                acc = vfmaq_f32(acc, va, vb);
            }
            let mut s = vaddvq_f32(acc);
            for p in k4..k {
                s = a[i * k + p].mul_add(b[p * n + j], s);
            }
            out[i * n + j] = s;
        }
    }
}

/// 長さが検査済みの単項の要素ごと演算を 4 要素ずつ書く。
#[target_feature(enable = "neon")]
fn elementwise_lanes(op: UnaryElementwiseOp, a: &[f32], out: &mut [f32]) {
    let (oc, orest) = out.as_chunks_mut::<4>();
    let (ac, arest) = a.as_chunks::<4>();
    for (o, x) in oc.iter_mut().zip(ac) {
        let v = load(x);
        let r = match op {
            UnaryElementwiseOp::Relu => {
                let mask = vcgtq_f32(v, vdupq_n_f32(0.0));
                vreinterpretq_f32_u32(vandq_u32(vreinterpretq_u32_f32(v), mask))
            }
            UnaryElementwiseOp::AddScalar(c) => vaddq_f32(v, vdupq_n_f32(c)),
        };
        *o = store(r);
    }
    for (o, &x) in orest.iter_mut().zip(arest) {
        *o = match op {
            UnaryElementwiseOp::Relu => relu(x),
            UnaryElementwiseOp::AddScalar(c) => x + c,
        };
    }
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
    // SAFETY: この module は `cfg(all(target_arch = "aarch64", target_feature = "neon"))` の裏にだけあり、
    // ビルド全体で neon が有効なので `gemm_lanes` の target feature を満たす。
    unsafe { gemm_lanes(a, b, m, k, n, out) };
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
    // SAFETY: `gemm` と同じ。この module は neon が有効なビルドにだけあり、`elementwise_lanes` の
    // target feature を満たす。
    unsafe { elementwise_lanes(op, a, out) };
    Ok(())
}
