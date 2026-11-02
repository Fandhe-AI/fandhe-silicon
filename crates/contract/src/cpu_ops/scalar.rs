//! `cpu_ops` の scalar 版の計算と長さの判定（非公開）。
//!
//! `DefaultCpuOps` と `ReferenceCpuOps` の `*_scalar` から呼ばれる。GEMM は各出力要素を k の昇順の
//! `f32::mul_add` で積む（spec CPU-1 のポインタ）。長さの判定は添字で触る前に置き、合わなければ `out` に何も書かずに `InvalidArgument` を返す。

use super::UnaryElementwiseOp;
use fandhe_silicon_core::error::ContractError;

/// GEMM の 3 つの slice の長さが `m`・`k`・`n` と合うか。積があふれる入力は偽。
pub(super) fn gemm_lengths_ok(
    a_len: usize,
    b_len: usize,
    out_len: usize,
    m: usize,
    k: usize,
    n: usize,
) -> bool {
    let (Some(mk), Some(kn), Some(mn)) = (m.checked_mul(k), k.checked_mul(n), m.checked_mul(n))
    else {
        return false;
    };
    a_len == mk && b_len == kn && out_len == mn
}

/// 要素ごと演算の入出力の長さが等しいか。
pub(super) fn elementwise_lengths_ok(a_len: usize, out_len: usize) -> bool {
    a_len == out_len
}

/// 行優先の f32 の行列積 `a`（m×k）×`b`（k×n）を `out`（m×n）に書く。
/// 長さが合わない・次元の積があふれるときは `out` に触れず `InvalidArgument` を返す。
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
    for i in 0..m {
        for j in 0..n {
            let mut acc = 0.0_f32;
            for p in 0..k {
                acc = a[i * k + p].mul_add(b[p * n + j], acc);
            }
            out[i * n + j] = acc;
        }
    }
    Ok(())
}

/// 単項の要素ごと演算を `a` から `out` に書く。長さが違えば `out` に触れず `InvalidArgument` を返す。
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
    for (o, &x) in out.iter_mut().zip(a) {
        *o = match op {
            UnaryElementwiseOp::Relu => x.max(0.0),
            UnaryElementwiseOp::AddScalar(c) => x + c,
        };
    }
    Ok(())
}
