//! 結果の一致を確かめるときの比較元になる、CPU 上の参照実装（spec TASK-24）。
//!
//! 背後実装の出力と比べる側（検証用のテスト・測定）から呼ばれる。標準ライブラリだけで書き、
//! 背後実装・アーキテクチャに依らず同じ手順で計算する。

use std::collections::TryReserveError;
use std::fmt;

/// 参照の計算に渡した入力が使えなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceError {
    /// 次元から要素数を求める積が `usize` に収まらなかった。
    SizeOverflow,
    /// 入力の長さが次元から求めた要素数と一致しなかった。
    LengthMismatch {
        /// 長さが合わなかった入力（`"a"`・`"b"`・`"c"` のいずれか）。
        input: &'static str,
        /// 次元から求めた要素数。
        expected: usize,
        /// 実際に渡された長さ。
        actual: usize,
    },
    /// 出力の領域を確保できなかった。
    AllocationFailed,
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeOverflow => write!(f, "matrix dimensions overflow usize"),
            Self::LengthMismatch {
                input,
                expected,
                actual,
            } => write!(f, "input {input} has length {actual}, expected {expected}"),
            Self::AllocationFailed => write!(f, "failed to allocate the output buffer"),
        }
    }
}

impl std::error::Error for ReferenceError {}

impl From<TryReserveError> for ReferenceError {
    fn from(_: TryReserveError) -> Self {
        Self::AllocationFailed
    }
}

/// f32 の行列積 `a (m×k) * b (k×n)` を CPU で計算し、`m×n` の行優先の結果を返す。
///
/// - `a`・`b` は行優先で隙間なく並べた `f32`。長さがそれぞれ `m*k`・`k*n` でなければ
///   [`ReferenceError::LengthMismatch`]、積が `usize` を超えれば [`ReferenceError::SizeOverflow`]
///   を返す。検査が済むまで確保も添字アクセスもしない。
/// - 各要素は初期値 `0.0` から、`k` の昇順に `f32::mul_add` で積み上げる。
/// - 出力の確保に失敗したら [`ReferenceError::AllocationFailed`] を返す。
pub fn reference_gemm_f32(
    a: &[f32],
    b: &[f32],
    m: usize,
    k: usize,
    n: usize,
) -> Result<Vec<f32>, ReferenceError> {
    let a_len = m.checked_mul(k).ok_or(ReferenceError::SizeOverflow)?;
    let b_len = k.checked_mul(n).ok_or(ReferenceError::SizeOverflow)?;
    let c_len = m.checked_mul(n).ok_or(ReferenceError::SizeOverflow)?;
    if a.len() != a_len {
        return Err(ReferenceError::LengthMismatch {
            input: "a",
            expected: a_len,
            actual: a.len(),
        });
    }
    if b.len() != b_len {
        return Err(ReferenceError::LengthMismatch {
            input: "b",
            expected: b_len,
            actual: b.len(),
        });
    }

    let mut c = Vec::new();
    c.try_reserve_exact(c_len)?;
    for i in 0..m {
        for j in 0..n {
            let mut acc = 0.0_f32;
            for p in 0..k {
                // 長さを検査済みなので i*k+p < m*k、p*n+j < k*n で範囲内。
                acc = a[i * k + p].mul_add(b[p * n + j], acc);
            }
            c.push(acc);
        }
    }
    Ok(c)
}

/// [`reference_gemm_f32`] と同じ入力・同じ検査で、累積の順だけを `k` の降順にした対照（spec TASK-1005 の 4/6）。
///
/// 判定には使わない。各要素は初期値 `0.0` から `p = k-1, k-2, …, 0` の順に `f32::mul_add` で積み上げる。
/// 昇順の参照との違いは累積の順だけで、長さの検査・溢れ・確保の失敗は [`reference_gemm_f32`] と同じ
/// 順・同じ [`ReferenceError`] で返す（検査が済むまで確保も添字アクセスもしない）。
pub fn reference_gemm_f32_descending_k(
    a: &[f32],
    b: &[f32],
    m: usize,
    k: usize,
    n: usize,
) -> Result<Vec<f32>, ReferenceError> {
    let a_len = m.checked_mul(k).ok_or(ReferenceError::SizeOverflow)?;
    let b_len = k.checked_mul(n).ok_or(ReferenceError::SizeOverflow)?;
    let c_len = m.checked_mul(n).ok_or(ReferenceError::SizeOverflow)?;
    if a.len() != a_len {
        return Err(ReferenceError::LengthMismatch {
            input: "a",
            expected: a_len,
            actual: a.len(),
        });
    }
    if b.len() != b_len {
        return Err(ReferenceError::LengthMismatch {
            input: "b",
            expected: b_len,
            actual: b.len(),
        });
    }

    let mut c = Vec::new();
    c.try_reserve_exact(c_len)?;
    for i in 0..m {
        for j in 0..n {
            let mut acc = 0.0_f32;
            for p in (0..k).rev() {
                // 長さを検査済みなので i*k+p < m*k、p*n+j < k*n で範囲内。
                acc = a[i * k + p].mul_add(b[p * n + j], acc);
            }
            c.push(acc);
        }
    }
    Ok(c)
}

/// binary16 のビット列の入力 `a (m×k) * b (k×n)` を、f32 に広げてから [`reference_gemm_f32`] で計算する。
///
/// 累積の手順は [`reference_gemm_f32`] と同じ（新しく書かない）。長さの検査・溢れ・確保の失敗は
/// 同じ [`ReferenceError`] で返す。広げる前に長さを検査するので、不一致の入力は確保しない。
pub fn reference_gemm_f16acc(
    a_bits: &[u16],
    b_bits: &[u16],
    m: usize,
    k: usize,
    n: usize,
) -> Result<Vec<f32>, ReferenceError> {
    let a_len = m.checked_mul(k).ok_or(ReferenceError::SizeOverflow)?;
    let b_len = k.checked_mul(n).ok_or(ReferenceError::SizeOverflow)?;
    m.checked_mul(n).ok_or(ReferenceError::SizeOverflow)?;
    if a_bits.len() != a_len {
        return Err(ReferenceError::LengthMismatch {
            input: "a",
            expected: a_len,
            actual: a_bits.len(),
        });
    }
    if b_bits.len() != b_len {
        return Err(ReferenceError::LengthMismatch {
            input: "b",
            expected: b_len,
            actual: b_bits.len(),
        });
    }
    let a = super::binary16::widen_binary16_slice(a_bits)?;
    let b = super::binary16::widen_binary16_slice(b_bits)?;
    reference_gemm_f32(&a, &b, m, k, n)
}
