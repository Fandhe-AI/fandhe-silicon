//! `cpu_ops` の scalar 版の計算と長さの判定（非公開）。
//!
//! `DefaultCpuOps` と `ReferenceCpuOps` の `*_scalar` から呼ばれる。GEMM は各出力要素を k の昇順の
//! `f32::mul_add` で積む（spec CPU-1 のポインタ）。長さの判定は添字で触る前に置き、合わなければ `out` に何も書かずに `InvalidArgument` を返す。
//! 部分 Top-k の計算も `DefaultCpuOps`・`ReferenceCpuOps` の両方から呼ばれる。

use super::UnaryElementwiseOp;
use core::cmp::Ordering;
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

/// 部分 Top-k が `out` の埋まらない位置に書く値。
pub(super) const TOPK_SENTINEL: (u32, f32) = (u32::MAX, f32::NEG_INFINITY);

/// `scores` の長さで、添字がスロット番号（`u32`）に収まるか。
pub(super) fn topk_slots_fit(scores_len: usize) -> bool {
    u32::try_from(scores_len).is_ok()
}

/// `a` が `b` より前に並ぶか（スコアの降順、同点はスロット番号の昇順）。
fn topk_before(a: &(u32, f32), b: &(u32, f32)) -> bool {
    match b.1.total_cmp(&a.1) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => a.0 < b.0,
    }
}

/// 有限のスコアの上位を `out` の先頭に並べて書き、残りの位置を番兵で埋める。
/// ヒープを確保しない。スロット番号が `u32` に収まらない長さのときは、添字に触れる前に
/// `out` に何も書かず `InvalidArgument` を返す。
pub(super) fn topk_partial(
    scores: &[f32],
    k: usize,
    out: &mut [(u32, f32)],
) -> Result<(), ContractError> {
    if !topk_slots_fit(scores.len()) {
        return Err(ContractError::InvalidArgument(
            "topk_partial scores length exceeds the u32 slot range".into(),
        ));
    }
    let cap = k.min(out.len());
    let mut filled = 0_usize;
    // スコアを先に引く順にして、長さがちょうど `u32::MAX` でも範囲の側が余分に進まないようにする。
    for (&score, slot) in scores.iter().zip(0_u32..) {
        if !score.is_finite() {
            continue;
        }
        let cand = (slot, score);
        let pos = out
            .get(..filled)
            .map_or(0, |s| s.partition_point(|e| topk_before(e, &cand)));
        if pos >= cap {
            continue;
        }
        let end = filled.saturating_add(1).min(cap);
        if let Some(window) = out.get_mut(pos..end) {
            window.rotate_right(1);
            if let Some(first) = window.first_mut() {
                *first = cand;
            }
        }
        if filled < cap {
            filled += 1;
        }
    }
    if let Some(rest) = out.get_mut(filled..) {
        rest.fill(TOPK_SENTINEL);
    }
    Ok(())
}
