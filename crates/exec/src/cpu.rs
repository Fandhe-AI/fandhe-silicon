//! 経路が `Route::Cpu` に決まった入口（`f16_fma` と、u32・f32 の subgroup 総和）が、host の列で計算する本体の置き場（`docs/spec/05-tasks.md` の TASK-930・TASK-931）。
//!
//! `Exec` の入口（`entry`）から呼ばれる。下-1 の `memory()`・`queue()` を呼ばない。

use crate::binary16::{from_f32, to_f32};

/// f16 の融合積和。要素ごとに f16 を f32 へ広げ、`f32::mul_add` の結果を f16 へ 1 回丸めて `out` に書く。
/// 4 つの列の長さは呼び手（入口の照合）が揃える。
pub(crate) fn f16_fma(a: &[u16], b: &[u16], c: &[u16], out: &mut [u16]) {
    for (((o, &x), &y), &z) in out.iter_mut().zip(a).zip(b).zip(c) {
        *o = from_f32(to_f32(x).mul_add(to_f32(y), to_f32(z)));
    }
}

/// subgroup の総和で 1 ブロックに入れる要素数。
const BLOCK: usize = 256;

/// u32 の subgroup の総和。`x` を 256 要素ずつのブロックに分け、ブロックごとに添字の順に 0 から足した値
/// （2^32 を法とする）を `out` に書く。列の長さは呼び手（入口の照合）が揃える。
pub(crate) fn subgroup_sum_u32(x: &[u32], out: &mut [u32]) {
    for (o, block) in out.iter_mut().zip(x.chunks(BLOCK)) {
        *o = block.iter().fold(0u32, |s, &v| s.wrapping_add(v));
    }
}

/// f32 の subgroup の総和。`x` を 256 要素ずつのブロックに分け、ブロックごとに添字の順に +0.0 から
/// f32 で足した値を `out` に書く。列の長さは呼び手（入口の照合）が揃える。
pub(crate) fn subgroup_sum_f32(x: &[f32], out: &mut [f32]) {
    for (o, block) in out.iter_mut().zip(x.chunks(BLOCK)) {
        *o = block.iter().fold(0.0f32, |s, &v| s + v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_silicon_core::parity::fallback_compare::binary16_order_distance;
    use fandhe_silicon_core::parity::fallback_reference::reference_f16_fma;

    fn one(a: u16, b: u16, c: u16) -> u16 {
        let mut out = [0u16; 1];
        f16_fma(&[a], &[b], &[c], &mut out);
        out[0]
    }

    #[test]
    fn hand_computed_cases() {
        assert_eq!(one(0x3C00, 0x3C00, 0x3C00), 0x4000);
        assert_eq!(one(0x3C00, 0x0001, 0x0000), 0x0001);
        assert_eq!(one(0x7BFF, 0x4000, 0x0000), 0x7C00);
        assert_eq!(one(0xFBFF, 0x4000, 0x0000), 0xFC00);
        assert_eq!(one(0xBC00, 0x3C00, 0x3C00), 0x0000);
        assert!(to_f32(one(0x7C00, 0x0000, 0x3C00)).is_nan());
    }

    #[test]
    fn single_rounding_after_f32_fma() {
        // 入力 (0x0642, 0x251D, 0x9EB9): 本体は 0x9EB8、core の参照は 0x9EB9。
        assert_eq!(one(0x0642, 0x251D, 0x9EB9), 0x9EB8);
        assert_eq!(reference_f16_fma(0x0642, 0x251D, 0x9EB9), 0x9EB9);
        assert_eq!(binary16_order_distance(0x9EB9, 0x9EB8), 1);
    }

    #[test]
    fn subgroup_sum_u32_blocks_and_wraps() {
        let mut out = [7u32; 1];
        subgroup_sum_u32(&[u32::MAX, 1], &mut out);
        assert_eq!(out, [0]);
        let mut out = [7u32; 2];
        subgroup_sum_u32(&[1u32; 257], &mut out);
        assert_eq!(out, [256, 1]);
    }

    #[test]
    fn subgroup_sum_f32_blocks_in_index_order() {
        let mut out = [7.0f32; 2];
        subgroup_sum_f32(&[1.0f32; 300], &mut out);
        assert_eq!(out, [256.0, 44.0]);
        let mut out = [7.0f32; 1];
        subgroup_sum_f32(&[1.0e8, 1.0, -1.0e8], &mut out);
        assert_eq!(out, [0.0]);
        subgroup_sum_f32(&[1.0e8, -1.0e8, 1.0], &mut out);
        assert_eq!(out, [1.0]);
        subgroup_sum_f32(&[-0.0f32; 3], &mut out);
        assert_eq!(out[0].to_bits(), 0);
    }
}
