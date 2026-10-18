//! 代わりの実行の試験の CPU の参照（spec TASK-929 の 4/6。対象の項目は FB-43・FB-44 のポインタ）。
//!
//! `f16-fma`・`subgroup-sum`・`atomic-add` の各経路の試験と、後段の背後実装の比べから呼ばれる。
//! 入力は [`super::fallback_inputs`] が作り、出力は [`super::fallback_compare`] の比べ方に渡す。
//! 標準ライブラリだけで書き、外部依存・`unsafe`・panic を持たない。確保は `try_reserve_exact` で、
//! 失敗は [`ReferenceError::AllocationFailed`] に写す。長さ 0 や出力の数 0 の拒否は下-2 の入口の
//! 受け持ちで、ここでは空の結果を返す。置き場の文書は `docs/parity-fallback-inputs.md`。

use core::ops::Range;

use super::binary16::{binary16_bits_to_f64, f64_to_binary16_bits};
use super::cpu_reference::ReferenceError;
use super::fallback_compare::{F32SubnormalHandling, reference_sum_f32};
use super::fallback_inputs::{F16FmaInputs, SUBGROUP_SUM_BLOCK};

/// 結果が NaN のときに返す、正規の静かな NaN の binary16 のビット列。
const CANONICAL_NAN_BITS: u16 = 0x7e00;

/// binary16 の積和 `a × b + c` の参照。3 つを f64 に正確に広げ、`f64::mul_add` で 1 回の丸めで
/// 計算し、[`f64_to_binary16_bits`] で binary16 へ 1 回だけ丸める。
///
/// `a * b + c` と分けて書くと丸めが増えるので使わない。`f64::mul_add` は IEEE の融合積和で結果が
/// 一意に決まり、環境によって変わらない。結果が NaN のときは NaN のペイロードと符号が環境で
/// 変わりうるので、正規の静かな NaN（`0x7e00`）に揃える（比べ方は NaN を区分で比べる）。
pub fn reference_f16_fma(a: u16, b: u16, c: u16) -> u16 {
    let r = binary16_bits_to_f64(a).mul_add(binary16_bits_to_f64(b), binary16_bits_to_f64(c));
    if r.is_nan() {
        CANONICAL_NAN_BITS
    } else {
        f64_to_binary16_bits(r)
    }
}

/// [`F16FmaInputs`] の各要素に [`reference_f16_fma`] をかけた列を返す。
///
/// `b`・`c` の長さが `a` と違えば [`ReferenceError::LengthMismatch`]（`input` は `"b"` か `"c"`）を
/// 返し、黙って切り詰めない。空の入力は空の列を返す。
pub fn reference_f16_fma_slice(inputs: &F16FmaInputs) -> Result<Vec<u16>, ReferenceError> {
    let n = inputs.a.len();
    for (name, len) in [("b", inputs.b.len()), ("c", inputs.c.len())] {
        if len != n {
            return Err(ReferenceError::LengthMismatch {
                input: name,
                expected: n,
                actual: len,
            });
        }
    }
    let mut out = Vec::new();
    out.try_reserve_exact(n)?;
    out.extend(
        inputs
            .a
            .iter()
            .zip(&inputs.b)
            .zip(&inputs.c)
            .map(|((&a, &b), &c)| reference_f16_fma(a, b, c)),
    );
    Ok(out)
}

/// `subgroup-sum` の出力の数。入力の長さ `n` を [`SUBGROUP_SUM_BLOCK`] ごとのブロックに分けた数
/// （端数のブロックを含む）。
pub fn subgroup_sum_output_len(n: usize) -> usize {
    n.div_ceil(SUBGROUP_SUM_BLOCK)
}

/// 出力の要素 `j` に足される入力の範囲。`256j..min(256j+256, n)`。
///
/// `j` が出力の数以上なら `None`。添字の積が `usize` に収まらないときも `None`。
pub fn subgroup_sum_block_range(n: usize, j: usize) -> Option<Range<usize>> {
    if j >= subgroup_sum_output_len(n) {
        return None;
    }
    let start = j.checked_mul(SUBGROUP_SUM_BLOCK)?;
    let end = start.checked_add(SUBGROUP_SUM_BLOCK)?.min(n);
    Some(start..end)
}

/// u32 の `subgroup-sum` の参照。ブロックごとの和を 2^32 を法として（`wrapping_add`）求める。
pub fn reference_subgroup_sum_u32(x: &[u32]) -> Result<Vec<u32>, ReferenceError> {
    let mut out = Vec::new();
    out.try_reserve_exact(subgroup_sum_output_len(x.len()))?;
    out.extend(
        x.chunks(SUBGROUP_SUM_BLOCK)
            .map(|block| block.iter().fold(0u32, |acc, &v| acc.wrapping_add(v))),
    );
    Ok(out)
}

/// f32 の和の参照の列。出力の要素ごとの参照の値と Σ|x| を、同じ添字で持つ。
///
/// [`super::fallback_compare::compare_relative_sum_f32`] の `reference` と `abs_sums` に、
/// [`Self::values`] と [`Self::abs_sums`] をそのまま渡せる。
#[derive(Debug, Clone, PartialEq)]
pub struct F32SumReferences {
    values: Vec<f32>,
    abs_sums: Vec<f64>,
}

impl F32SumReferences {
    /// 要素ごとの参照の和（f32）。
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// 要素ごとの Σ|x|（f64）。
    pub fn abs_sums(&self) -> &[f64] {
        &self.abs_sums
    }

    /// 出力の要素の数。
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 出力の要素が無いか。
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// `count` 個の出力それぞれについて、`inputs_of(j)` が返す入力の列から参照を作る。
    fn build<I>(
        count: usize,
        mut inputs_of: impl FnMut(usize) -> I,
        handling: F32SubnormalHandling,
    ) -> Result<Self, ReferenceError>
    where
        I: IntoIterator<Item = f32>,
    {
        let mut values = Vec::new();
        let mut abs_sums = Vec::new();
        values.try_reserve_exact(count)?;
        abs_sums.try_reserve_exact(count)?;
        for j in 0..count {
            let r = reference_sum_f32(inputs_of(j), handling);
            values.push(r.value());
            abs_sums.push(r.abs_sum());
        }
        Ok(Self { values, abs_sums })
    }
}

/// f32 の `subgroup-sum` の参照。ブロックごとに [`reference_sum_f32`] をかける
/// （f64 で列の順に足し、f32 へ 1 回丸める）。
pub fn reference_subgroup_sum_f32(
    x: &[f32],
    handling: F32SubnormalHandling,
) -> Result<F32SumReferences, ReferenceError> {
    F32SumReferences::build(
        subgroup_sum_output_len(x.len()),
        |j| {
            let block: &[f32] = match subgroup_sum_block_range(x.len(), j) {
                Some(r) => &x[r],
                None => &[],
            };
            block.iter().copied()
        },
        handling,
    )
}

/// f32 の `atomic-add` の参照。長さ `m` の出力で、要素 `j` は `i mod m == j` を満たす入力 `x[i]` を
/// 添字の順に足した和（`m` 個の出力の初期値は +0.0）。
///
/// 足される入力が無い要素（`x.len() < m` の後ろ側）は +0.0・Σ|x| 0。`m == 0` は空の結果を返す。
pub fn reference_atomic_add_f32(
    x: &[f32],
    m: usize,
    handling: F32SubnormalHandling,
) -> Result<F32SumReferences, ReferenceError> {
    if m == 0 {
        return Ok(F32SumReferences {
            values: Vec::new(),
            abs_sums: Vec::new(),
        });
    }
    F32SumReferences::build(m, |j| x.iter().copied().skip(j).step_by(m), handling)
}
