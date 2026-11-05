//! wasm32 向けの glue の cdylib（spec TASK-22・TASK-2957 のポインタ）。
//!
//! 別 workspace `wasm/` の member。製品の crate `fandhe-silicon-contract`
//! （feature は `contract-core`・`cpu-isa`・`cpu-ops` のみ）に path で依存し（試験だけが `fandhe-silicon-core` にも path で依存する）、
//! wasm32-unknown-unknown でビルドした `.wasm` の入口になる。
//! 依存の向きは「本 crate → 製品」だけで、製品の crate は本 crate を知らない。
//!
//! 手で書く unsafe は持たない。配列の受け渡しは wasm-bindgen の型の変換に任せる。
//! 契約の crate の `cpu_ops` の `DefaultCpuOps` を呼ぶ関数を export として持ち、
//! 出力は呼び出し元が渡す領域に書いて成否を `u32` で返す（spec BUILD-42 のポインタ）。

// 契約の crate の CPU の演算を呼ぶ。
use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, UnaryElementwiseOp};

/// 成功の戻り値。
const OK: u32 = 0;
/// 拒否の戻り値。拒否の種類では値を分けない。
const REJECTED: u32 = 1;

/// 受け取った `f32` の列を、同じ値・同じ順序で新しい `Vec` に写して返す。
///
/// JS 側から `Float32Array` を渡して往復を確かめられる最小の口。
/// 値は変換せずビット列のまま写すので、`-0.0`・無限大・NaN もそのまま返る。
/// wasm32 のときだけ wasm-bindgen の export になり、ホストでは通常の関数としてテストできる。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn roundtrip_f32(input: &[f32]) -> Vec<f32> {
    input.to_vec()
}

/// 行優先の `a`（m×k）と `b`（k×n）の積を `out`（m×n）に書き、0 を返す。
///
/// 長さが `m`・`k`・`n` と合わない、または次元の積が `usize` に収まらないときは
/// `out` を変えずに 1 を返す。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn gemm_scalar(a: &[f32], b: &[f32], m: usize, k: usize, n: usize, out: &mut [f32]) -> u32 {
    if DefaultCpuOps.gemm_scalar(a, b, m, k, n, out).is_ok() {
        OK
    } else {
        REJECTED
    }
}

/// `gemm_scalar` と同じ入力・戻り値の行列積。
///
/// wasm32 で `simd128` が有効なビルドでは 4 レーンの版を、それ以外のビルドでは
/// `gemm_scalar` と同じ計算を使う。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn gemm_simd(a: &[f32], b: &[f32], m: usize, k: usize, n: usize, out: &mut [f32]) -> u32 {
    if DefaultCpuOps.gemm_simd(a, b, m, k, n, out).is_ok() {
        OK
    } else {
        REJECTED
    }
}

/// `all(target_arch = "wasm32", target_feature = "simd128")` の `cfg` が真のビルドで 1、
/// 偽のビルドで 0 を返す。bit0 以外は立てない。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn detect_isa_bits() -> u32 {
    u32::from(cfg!(all(
        target_arch = "wasm32",
        target_feature = "simd128"
    )))
}

/// `op_code` と定数 `c` を契約の crate の単項演算に読み替える。未割当の `op_code` は `None`。
fn elementwise_op(op_code: u32, c: f32) -> Option<UnaryElementwiseOp> {
    match op_code {
        0 => Some(UnaryElementwiseOp::Relu),
        1 => Some(UnaryElementwiseOp::AddScalar(c)),
        _ => None,
    }
}

/// `a` の各要素に `op_code` の演算をかけた値を `out` に書き、0 を返す。
///
/// `op_code` 0 は各要素を `max(x, 0.0)` にする（`c` は使わない）。1 は各要素に `c` を足す。
/// `op_code` が 2 以上、または `a` と `out` の長さが違うときは `out` を変えずに 1 を返す。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn elementwise_scalar(op_code: u32, c: f32, a: &[f32], out: &mut [f32]) -> u32 {
    let Some(op) = elementwise_op(op_code, c) else {
        return REJECTED;
    };
    if DefaultCpuOps.elementwise_scalar(op, a, out).is_ok() {
        OK
    } else {
        REJECTED
    }
}

/// `elementwise_scalar` と同じ入力・戻り値の要素ごとの演算。
///
/// wasm32 で `simd128` が有効なビルドでは 4 レーンの版を、それ以外のビルドでは
/// `elementwise_scalar` と同じ計算を使う。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn elementwise_simd(op_code: u32, c: f32, a: &[f32], out: &mut [f32]) -> u32 {
    let Some(op) = elementwise_op(op_code, c) else {
        return REJECTED;
    };
    if DefaultCpuOps.elementwise_simd(op, a, out).is_ok() {
        OK
    } else {
        REJECTED
    }
}

/// 部分 Top-k の結果を `out` に並べる共通の補助。`topk_scalar`・`topk_simd` が演算だけを替えて呼ぶ。
///
/// `out` の長さが `k`×8 バイトでなければ（`k`×8 が `usize` に収まらないときも）何も触れずに拒否する。
/// 作業の列は長さの照合の後に確保し、その長さは `k` 件。
fn topk_into_bytes(k: usize, out: &mut [u8], run: impl FnOnce(&mut [(u32, f32)]) -> bool) -> u32 {
    let Some(len) = k.checked_mul(8) else {
        return REJECTED;
    };
    if out.len() != len {
        return REJECTED;
    }
    let mut work = vec![(0_u32, 0.0_f32); k];
    if !run(&mut work) {
        return REJECTED;
    }
    let (chunks, _) = out.as_chunks_mut::<8>();
    for (chunk, (slot, score)) in chunks.iter_mut().zip(&work) {
        let [a0, a1, a2, a3] = slot.to_le_bytes();
        let [b0, b1, b2, b3] = score.to_le_bytes();
        *chunk = [a0, a1, a2, a3, b0, b1, b2, b3];
    }
    OK
}

/// 有限のスコアの上位 `k` 件を `out`（`k`×8 バイト）に書き、0 を返す。
///
/// 1 件を、u32 のスロット番号と f32 のスコアをリトルエンディアンで並べた 8 バイトにする。
/// 件数・並び・埋まらない位置に書く値は `DefaultCpuOps::topk_partial_scalar` と同じ。
/// `k`×8 が `usize` に収まらない、または `out` の長さが `k`×8 バイトでないときは
/// `out` を変えずに 1 を返す。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn topk_scalar(scores: &[f32], k: usize, out: &mut [u8]) -> u32 {
    topk_into_bytes(k, out, |work| {
        DefaultCpuOps.topk_partial_scalar(scores, k, work).is_ok()
    })
}

/// `topk_scalar` と同じ入力・戻り値の部分 Top-k。
///
/// 契約の crate の `topk_partial_simd` を呼ぶ。どのビルドでも `topk_scalar` と同じ計算になる。
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn topk_simd(scores: &[f32], k: usize, out: &mut [u8]) -> u32 {
    topk_into_bytes(k, out, |work| {
        DefaultCpuOps.topk_partial_simd(scores, k, work).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CpuOps, DefaultCpuOps, UnaryElementwiseOp, elementwise_scalar, elementwise_simd,
        gemm_scalar, gemm_simd, roundtrip_f32, topk_scalar, topk_simd,
    };

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

    type GemmFn = fn(&[f32], &[f32], usize, usize, usize, &mut [f32]) -> u32;

    const GEMMS: [GemmFn; 2] = [gemm_scalar, gemm_simd];

    #[test]
    fn gemm_matches_default_cpu_ops() {
        let cases = [
            (
                vec![1.0, -2.0, 3.0, -0.0, 5.0, 6.0],
                vec![1.0, 0.0, -1.0, 2.0, 3.0, -4.0],
                2,
                3,
                2,
            ),
            (vec![-3.0], vec![2.0], 1, 1, 1),
            (vec![], vec![1.0, 2.0], 0, 1, 2),
        ];
        for (a, b, m, k, n) in &cases {
            let mut want_scalar = vec![7.0_f32; m * n];
            let mut want_simd = vec![7.0_f32; m * n];
            DefaultCpuOps
                .gemm_scalar(a, b, *m, *k, *n, &mut want_scalar)
                .unwrap();
            DefaultCpuOps
                .gemm_simd(a, b, *m, *k, *n, &mut want_simd)
                .unwrap();
            let mut got = vec![7.0_f32; m * n];
            assert_eq!(gemm_scalar(a, b, *m, *k, *n, &mut got), 0);
            assert_eq!(bits(&got), bits(&want_scalar));
            let mut got = vec![7.0_f32; m * n];
            assert_eq!(gemm_simd(a, b, *m, *k, *n, &mut got), 0);
            assert_eq!(bits(&got), bits(&want_simd));
        }
        let mut empty: Vec<f32> = vec![];
        for f in GEMMS {
            assert_eq!(f(&[1.0, 2.0], &[], 2, 1, 0, &mut empty), 0);
        }
    }

    #[test]
    fn gemm_rejects_mismatched_lengths() {
        let a = [1.0_f32; 6];
        let b = [1.0_f32; 6];
        for f in GEMMS {
            for (la, lb, lo) in [
                (5, 6, 4),
                (7, 6, 4),
                (6, 5, 4),
                (6, 7, 4),
                (6, 6, 3),
                (6, 6, 5),
            ] {
                let a2 = vec![1.0_f32; la];
                let b2 = vec![1.0_f32; lb];
                let mut out = vec![7.0_f32; lo];
                assert_eq!(f(&a2, &b2, 2, 3, 2, &mut out), 1);
                assert_eq!(bits(&out), bits(&vec![7.0_f32; lo]));
            }
            let mut out = [7.0_f32; 4];
            assert_eq!(f(&a, &b, 2, 3, 2, &mut out), 0);
        }
    }

    #[test]
    fn gemm_rejects_overflowing_dimensions() {
        for f in GEMMS {
            let mut out = [7.0_f32; 4];
            assert_eq!(f(&[], &[], usize::MAX, 0, 2, &mut out), 1);
            assert_eq!(bits(&out), bits(&[7.0_f32; 4]));
        }
    }

    type ElementwiseFn = fn(u32, f32, &[f32], &mut [f32]) -> u32;

    const ELEMENTWISES: [ElementwiseFn; 2] = [elementwise_scalar, elementwise_simd];

    #[test]
    fn elementwise_matches_default_cpu_ops() {
        let input = [
            -0.0_f32,
            -1.5,
            2.5,
            0.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            3.0,
            -7.0,
            0.25,
        ];
        let c = 0.5_f32;
        for (op_code, op) in [
            (0_u32, UnaryElementwiseOp::Relu),
            (1_u32, UnaryElementwiseOp::AddScalar(c)),
        ] {
            let mut want_scalar = vec![7.0_f32; input.len()];
            let mut want_simd = vec![7.0_f32; input.len()];
            DefaultCpuOps
                .elementwise_scalar(op, &input, &mut want_scalar)
                .unwrap();
            DefaultCpuOps
                .elementwise_simd(op, &input, &mut want_simd)
                .unwrap();
            let mut got = vec![7.0_f32; input.len()];
            assert_eq!(elementwise_scalar(op_code, c, &input, &mut got), 0);
            assert_eq!(bits(&got), bits(&want_scalar));
            let mut got = vec![7.0_f32; input.len()];
            assert_eq!(elementwise_simd(op_code, c, &input, &mut got), 0);
            assert_eq!(bits(&got), bits(&want_simd));
        }
        for f in ELEMENTWISES {
            let mut with_c = vec![7.0_f32; input.len()];
            let mut without_c = vec![7.0_f32; input.len()];
            assert_eq!(f(0, 123.0, &input, &mut with_c), 0);
            assert_eq!(f(0, 0.0, &input, &mut without_c), 0);
            assert_eq!(bits(&with_c), bits(&without_c));
            let mut empty: Vec<f32> = vec![];
            assert_eq!(f(0, c, &[], &mut empty), 0);
            assert_eq!(f(1, c, &[], &mut empty), 0);
        }
    }

    #[test]
    fn elementwise_rejects_unassigned_op_code() {
        for f in ELEMENTWISES {
            for op_code in [2_u32, 3, u32::MAX] {
                for (la, lo) in [(4, 4), (4, 3), (0, 0)] {
                    let a = vec![1.0_f32; la];
                    let mut out = vec![7.0_f32; lo];
                    assert_eq!(f(op_code, 0.5, &a, &mut out), 1);
                    assert_eq!(bits(&out), bits(&vec![7.0_f32; lo]));
                }
            }
        }
    }

    #[test]
    fn elementwise_rejects_mismatched_lengths() {
        for f in ELEMENTWISES {
            for op_code in [0_u32, 1] {
                for (la, lo) in [(4, 3), (4, 5), (0, 1), (1, 0)] {
                    let a = vec![1.0_f32; la];
                    let mut out = vec![7.0_f32; lo];
                    assert_eq!(f(op_code, 0.5, &a, &mut out), 1);
                    assert_eq!(bits(&out), bits(&vec![7.0_f32; lo]));
                }
            }
        }
    }

    type TopkFn = fn(&[f32], usize, &mut [u8]) -> u32;

    const TOPKS: [TopkFn; 2] = [topk_scalar, topk_simd];

    /// 試験専用。`(スロット番号, スコア)` の列をリトルエンディアンの 8 バイトずつに並べる。
    fn pairs_to_bytes(pairs: &[(u32, f32)]) -> Vec<u8> {
        pairs
            .iter()
            .flat_map(|(slot, score)| slot.to_le_bytes().into_iter().chain(score.to_le_bytes()))
            .collect()
    }

    #[test]
    fn topk_matches_default_cpu_ops() {
        let inputs: [Vec<f32>; 3] = [
            vec![
                1.0,
                3.0,
                3.0,
                -0.0,
                0.0,
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                2.0,
            ],
            vec![5.0, 4.0],
            vec![],
        ];
        for scores in &inputs {
            for k in [0_usize, 1, 3, scores.len(), scores.len() + 3] {
                let mut want_scalar = vec![(0_u32, 0.0_f32); k];
                let mut want_simd = vec![(0_u32, 0.0_f32); k];
                DefaultCpuOps
                    .topk_partial_scalar(scores, k, &mut want_scalar)
                    .unwrap();
                DefaultCpuOps
                    .topk_partial_simd(scores, k, &mut want_simd)
                    .unwrap();
                let mut got = vec![0xAA_u8; k * 8];
                assert_eq!(topk_scalar(scores, k, &mut got), 0);
                assert_eq!(got, pairs_to_bytes(&want_scalar));
                let mut got = vec![0xAA_u8; k * 8];
                assert_eq!(topk_simd(scores, k, &mut got), 0);
                assert_eq!(got, pairs_to_bytes(&want_simd));
            }
        }
    }

    #[test]
    fn topk_bytes_layout() {
        for f in TOPKS {
            let mut out = [0xAA_u8; 16];
            assert_eq!(f(&[1.0, 3.0, 2.0], 2, &mut out), 0);
            assert_eq!(
                out,
                [
                    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x40, //
                    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40,
                ]
            );
            let mut out = [0xAA_u8; 32];
            assert_eq!(f(&[1.0, 3.0, 2.0], 4, &mut out), 0);
            assert_eq!(out[24..], [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x80, 0xFF]);
        }
    }

    #[test]
    fn topk_rejects_output_length_mismatch() {
        let scores = [1.0_f32, 3.0, 2.0];
        for f in TOPKS {
            for lo in [8_usize, 24, 15, 17, 0] {
                let mut out = vec![0xAA_u8; lo];
                assert_eq!(f(&scores, 2, &mut out), 1);
                assert_eq!(out, vec![0xAA_u8; lo]);
            }
            let mut out = [0xAA_u8; 16];
            assert_eq!(f(&scores, 2, &mut out), 0);
        }
    }

    #[test]
    fn topk_rejects_overflowing_count() {
        for f in TOPKS {
            let mut out = [0xAA_u8; 16];
            assert_eq!(f(&[1.0], usize::MAX / 8 + 1, &mut out), 1);
            assert_eq!(out, [0xAA_u8; 16]);
            // 空の `out` は、あふれた積が 0 に回っても長さが合ってしまう入力。
            let mut empty: [u8; 0] = [];
            assert_eq!(f(&[1.0], usize::MAX / 8 + 1, &mut empty), 1);
            assert_eq!(f(&[], 1 << (usize::BITS - 3), &mut empty), 1);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn detect_isa_bits_is_zero_on_host() {
        use super::detect_isa_bits;
        assert_eq!(detect_isa_bits(), 0);
    }

    /// ロストの通知先の関数の型（DEV-136）。wasm32 では `Send` でない関数（`Rc` を捕まえる）が登録でき、
    /// ホストでは `Send` が要る。wasm32 の clippy（`--all-targets`）が型検査として通す。
    mod lost_callback_type {
        use fandhe_silicon_core::device_lost::{DeviceLostNotify, LostCallback};

        struct Sink;
        impl DeviceLostNotify for Sink {
            fn set_lost_callback(&self, _callback: LostCallback) {}
        }

        #[cfg(target_arch = "wasm32")]
        #[test]
        fn wasm32_accepts_a_closure_that_is_not_send() {
            let seen = std::rc::Rc::new(std::cell::Cell::new(0_u32));
            Sink.set_lost_callback(Box::new(move |_| seen.set(1)));
        }

        #[cfg(not(target_arch = "wasm32"))]
        #[test]
        fn host_requires_a_send_callback() {
            fn assert_send<T: Send>() {}
            assert_send::<LostCallback>();
            Sink.set_lost_callback(Box::new(|_| {}));
        }
    }
}
