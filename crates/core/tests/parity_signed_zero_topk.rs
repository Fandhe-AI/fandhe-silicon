//! ±0 の扱いの「比べの側」の確認（spec TASK-976 の 2/6。対応する項目は PARITY-4 のポインタのみ）。
//! 要素ごとの比べでは符号付きの零を同じとみなし、Top-k の比べではビット列で区別することを、
//! feature `test-support` の裏にだけ存在する検証用の道具で確かめる。
//! 参照の並び自体は `parity_topk.rs` の `positive_zero_precedes_negative_zero` が持つので、ここでは重ねない。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, compare_f32_with_special_values, judge_element_with_special_values,
};
use fandhe_silicon_core::parity::tolerance::{ElementVerdict, T1, compare_f32, judge_element};
use fandhe_silicon_core::parity::topk::{TopKEntry, compare_topk, reference_topk_f32};

fn e(slot: u32, score: f32) -> TopKEntry {
    TopKEntry::new(slot, score)
}

fn pairs(v: &[TopKEntry]) -> Vec<(u32, u32)> {
    v.iter().map(|x| (x.slot(), x.score_bits())).collect()
}

/// 両方の要素ごとの比べ方が一致と判定することをまとめて確かめる。
fn assert_elementwise_match(reference: &[f32], actual: &[f32]) {
    assert!(compare_f32(reference, actual, T1).unwrap().is_match());
    assert!(
        compare_f32_with_special_values(reference, actual, T1)
            .unwrap()
            .is_match()
    );
}

#[test]
fn gemm_reference_negative_zero_matches_positive_zero_actual() {
    // 前提: 融合積和だと -0.0、融合なしだと +0.0 になる入力。崩れたら入力を選び直す。
    let reference = reference_gemm_f32(&[-1e-30], &[1e-30], 1, 1, 1).unwrap();
    assert_eq!(reference[0].to_bits(), 0x8000_0000);
    let unfused = std::hint::black_box(-1e-30_f32) * 1e-30_f32 + 0.0_f32;
    assert_eq!(unfused.to_bits(), 0);
    assert_elementwise_match(&reference, &[unfused]);
}

#[test]
fn gemm_reference_positive_zero_matches_negative_zero_actual() {
    let reference = reference_gemm_f32(&[-0.0], &[1.0], 1, 1, 1).unwrap();
    assert_eq!(reference[0].to_bits(), 0);
    assert_elementwise_match(&reference, &[-0.0]);
}

#[test]
fn elementwise_like_output_treats_signed_zeros_as_equal() {
    // ReLU 風の出力を直書きし、零の位置の符号だけを入れ替える。
    let reference = [0.0_f32, -0.0, 2.0, 0.0, 3.5];
    let actual = [-0.0_f32, 0.0, 2.0, -0.0, 3.5];
    assert_elementwise_match(&reference, &actual);

    // 零でない外れ値（絶対誤差の上限の 2 倍）を 1 要素入れると不一致になる。
    let outlier = (T1.absolute() * 2.0) as f32;
    let mut bad = actual;
    bad[3] = outlier;
    let c = compare_f32(&reference, &bad, T1).unwrap();
    assert!(!c.is_match());
    assert_eq!(c.first_mismatch(), Some(3));
    let s = compare_f32_with_special_values(&reference, &bad, T1).unwrap();
    assert!(!s.is_match());
    assert_eq!(s.first_mismatch(), Some(3));
}

#[test]
#[allow(clippy::float_cmp)]
fn ieee_ordered_topk_is_a_mismatch_against_total_order_reference() {
    // IEEE の == では区別できない入力であることの確認。
    let (neg, pos) = (
        std::hint::black_box(-0.0_f32),
        std::hint::black_box(0.0_f32),
    );
    assert!(neg == pos);

    let reference = reference_topk_f32(&[-0.0, 0.0], 2).unwrap();
    // IEEE の比較で並べた実測に見立てる（-0.0 が小さいスロットのまま先に来る）。
    let ieee_ordered = [e(0, -0.0), e(1, 0.0)];
    let c = compare_topk(&reference, &ieee_ordered).unwrap();
    assert!(!c.is_match());
    assert_eq!(c.mismatched(), 2);
    assert_eq!(c.first_mismatch(), Some(0));

    let total_ordered = [e(1, 0.0), e(0, -0.0)];
    assert!(compare_topk(&reference, &total_ordered).unwrap().is_match());
}

#[test]
fn signed_zero_mismatch_is_localized_within_longer_topk() {
    // -0.0（スロット 1）を +0.0（スロット 3）より小さいスロットに置き、別の同点も混ぜる。
    let scores = [3.0, -0.0, 5.0, 0.0, 3.0, 1.0, -2.0];
    let reference = reference_topk_f32(&scores, 6).unwrap();
    assert_eq!(
        pairs(&reference),
        vec![
            (2, 5.0_f32.to_bits()),
            (0, 3.0_f32.to_bits()),
            (4, 3.0_f32.to_bits()),
            (5, 1.0_f32.to_bits()),
            (3, 0.0_f32.to_bits()),
            (1, (-0.0_f32).to_bits()),
        ]
    );
    // 零の 2 件だけを IEEE の順に入れ替える。
    let mut actual = reference.clone();
    actual.swap(4, 5);
    let c = compare_topk(&reference, &actual).unwrap();
    assert_eq!(c.compared(), 6);
    assert_eq!(c.mismatched(), 2);
    assert_eq!(c.first_mismatch(), Some(4));
}

#[test]
fn same_signed_zero_pair_matches_elementwise_but_not_in_topk() {
    for (r, a) in [(0.0_f32, -0.0_f32), (-0.0, 0.0)] {
        assert_eq!(judge_element(r, a, T1), ElementVerdict::Within);
        assert_eq!(
            judge_element_with_special_values(r, a, T1),
            SpecialVerdict::Within
        );
        let c = compare_topk(&[e(0, r)], &[e(0, a)]).unwrap();
        assert!(!c.is_match());
    }
}
