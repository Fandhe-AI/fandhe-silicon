//! wasm32 向けの glue の cdylib（spec TASK-22 のポインタ）。
//!
//! 別 workspace `wasm/` の唯一の member。製品の crate `fandhe-silicon-contract`
//! （feature は `contract-core`・`cpu-isa`・`cpu-ops` のみ）に path で依存し、
//! wasm32-unknown-unknown でビルドした `.wasm` の入口になる。
//! 依存の向きは「本 crate → 製品」だけで、製品の crate は本 crate を知らない。
//!
//! 手で書く unsafe は持たない。配列の受け渡しは wasm-bindgen の型の変換に任せる。

// 製品の crate をリンクに入れる。今の CPU 向けの feature はコードを持たないので、使うシンボルは無い。
use fandhe_silicon_contract as _;

/// 受け取った `f32` の列を、同じ値・同じ順序で新しい `Vec` に写して返す。
///
/// JS 側から `Float32Array` を渡して往復を確かめられる最小の口。
/// 値は変換せずビット列のまま写すので、`-0.0`・無限大・NaN もそのまま返る。
/// wasm32 のときだけ wasm-bindgen の export になり、ホストでは通常の関数としてテストできる。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn roundtrip_f32(input: &[f32]) -> Vec<f32> {
    input.to_vec()
}

#[cfg(test)]
mod tests {
    use super::roundtrip_f32;

    fn bits(v: &[f32]) -> Vec<u32> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(roundtrip_f32(&[]).is_empty());
    }

    #[test]
    fn finite_values_keep_value_and_order() {
        let input = [1.0_f32, -2.5, 0.0, f32::MAX, f32::MIN_POSITIVE];
        assert_eq!(bits(&roundtrip_f32(&input)), bits(&input));
    }

    #[test]
    fn non_finite_values_and_negative_zero_keep_their_bits() {
        let input = [f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -0.0_f32];
        assert_eq!(bits(&roundtrip_f32(&input)), bits(&input));
    }
}
