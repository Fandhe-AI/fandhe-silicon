//! ±0 の扱いの「比べの側」と、部分 Top-k の位置ごとの比べの確認
//! （spec TASK-976 の 2/6・3/6。対応する項目は PARITY-4・PARITY-6 のポインタのみ）。
//! 要素ごとの比べでは符号付きの零を同じとみなし、Top-k の比べではビット列で区別することを、
//! feature `test-support` の裏にだけ存在する検証用の道具で確かめる。
//! 部分 Top-k の側では、k 件の出力を同じスコア列から作った `reference_topk_f32` と位置ごとに比べ、
//! 足りない位置を埋める値の位置も同じビット規則で比べることを確かめる。
//! 比べる相手は同じスコア列への CPU 参照で、全量 readback の出力どうしの比べはここでは扱わない（PARITY-13 のポインタのみ）。
//! 参照の並び自体は `parity_topk.rs` の `positive_zero_precedes_negative_zero` が持つので、ここでは重ねない。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, compare_f32_with_special_values, judge_element_with_special_values,
};
use fandhe_silicon_core::parity::tolerance::{
    CompareError, ElementVerdict, T1, compare_f32, judge_element,
};
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
// ---- 部分 Top-k の位置ごとの比べ（TASK-976 の 3/6） ----

/// 埋める値の候補。本リポは埋める値を定めないので、どの候補でも同じ規則で比べられることを確かめる。
fn pad_candidates() -> Vec<TopKEntry> {
    vec![
        e(u32::MAX, f32::NEG_INFINITY),
        e(u32::MAX, f32::from_bits(0x7fc0_0001)),
        e(u32::MAX, -0.0),
        e(7, 1.5),
    ]
}

/// 呼び出し側の責務として、参照を `k` 件に `pad` でそろえる（参照は `min(k, 有限の件数)` 件しか返さない）。
fn pad_to_k(mut entries: Vec<TopKEntry>, k: usize, pad: TopKEntry) -> Vec<TopKEntry> {
    while entries.len() < k {
        entries.push(pad);
    }
    entries
}

#[test]
fn tie_pair_with_swapped_slots_is_a_mismatch() {
    let reference = reference_topk_f32(&[2.0, 5.0, 2.0, 1.0], 4).unwrap();
    assert_eq!(
        pairs(&reference),
        vec![
            (1, 5.0_f32.to_bits()),
            (0, 2.0_f32.to_bits()),
            (2, 2.0_f32.to_bits()),
            (3, 1.0_f32.to_bits()),
        ]
    );
    // スコアのビット列は同じまま、同点 2 件のスロットだけを入れ替える。
    let actual = [e(1, 5.0), e(2, 2.0), e(0, 2.0), e(3, 1.0)];
    let c = compare_topk(&reference, &actual).unwrap();
    assert_eq!(c.compared(), 4);
    assert_eq!(c.mismatched(), 2);
    assert_eq!(c.first_mismatch(), Some(1));
}

#[test]
fn padded_reference_matches_identically_padded_actual() {
    let reference = reference_topk_f32(&[1.0, f32::NAN, 3.0], 5).unwrap();
    assert_eq!(
        pairs(&reference),
        vec![(2, 3.0_f32.to_bits()), (0, 1.0_f32.to_bits())]
    );
    for pad in pad_candidates() {
        let padded_ref = pad_to_k(reference.clone(), 5, pad);
        let padded_actual = pad_to_k(vec![e(2, 3.0), e(0, 1.0)], 5, pad);
        let c = compare_topk(&padded_ref, &padded_actual).unwrap();
        assert_eq!(c.compared(), 5);
        assert!(c.is_match());
    }
}

#[test]
fn real_entry_in_pad_position_is_a_mismatch() {
    let reference = reference_topk_f32(&[1.0, f32::INFINITY], 3).unwrap();
    assert_eq!(pairs(&reference), vec![(0, 1.0_f32.to_bits())]);
    for pad in pad_candidates() {
        let padded_ref = pad_to_k(reference.clone(), 3, pad);
        // 位置 1 は埋める位置。そこに実の 1 件（有限のスロット・スコア）が入っている実測。
        let actual = [e(0, 1.0), e(1, 0.5), padded_ref[2]];
        let c = compare_topk(&padded_ref, &actual).unwrap();
        assert_eq!(c.mismatched(), 1);
        assert_eq!(c.first_mismatch(), Some(1));
    }
}

#[test]
fn pad_with_different_bits_is_a_mismatch() {
    let reference = reference_topk_f32(&[4.0], 3).unwrap();
    let pad = e(u32::MAX, f32::from_bits(0x7fc0_0001));
    let padded_ref = pad_to_k(reference, 3, pad);
    // スロットは同じで、スコアのビット列だけが違う（NaN の payload 違い）。
    let other_payload = e(u32::MAX, f32::from_bits(0x7fc0_0002));
    let actual = [padded_ref[0], padded_ref[1], other_payload];
    let c = compare_topk(&padded_ref, &actual).unwrap();
    assert_eq!(c.mismatched(), 1);
    assert_eq!(c.first_mismatch(), Some(2));

    // 埋める値が 0.0 と -0.0 の違いでも区別される。
    let zero_ref = pad_to_k(vec![e(0, 4.0)], 2, e(u32::MAX, 0.0));
    let zero_actual = pad_to_k(vec![e(0, 4.0)], 2, e(u32::MAX, -0.0));
    let c = compare_topk(&zero_ref, &zero_actual).unwrap();
    assert_eq!(c.mismatched(), 1);
    assert_eq!(c.first_mismatch(), Some(1));
}

#[test]
fn pad_placed_before_real_entries_is_a_mismatch() {
    let reference = reference_topk_f32(&[3.0, 2.0], 4).unwrap();
    let pad = e(u32::MAX, f32::NEG_INFINITY);
    let padded_ref = pad_to_k(reference, 4, pad);
    // 埋める値を実の件より前に置く（実の件が後ろへずれる）。
    let actual = [pad, pad, e(0, 3.0), e(1, 2.0)];
    let c = compare_topk(&padded_ref, &actual).unwrap();
    assert_eq!(c.compared(), 4);
    assert_eq!(c.mismatched(), 4);
    assert_eq!(c.first_mismatch(), Some(0));
}

#[test]
fn unpadded_reference_against_k_length_actual_is_length_mismatch() {
    let reference = reference_topk_f32(&[1.0, f32::NAN], 3).unwrap();
    assert_eq!(reference.len(), 1);
    let pad = e(u32::MAX, f32::NEG_INFINITY);
    let actual = pad_to_k(vec![e(0, 1.0)], 3, pad);
    // 黙って一致にはならない。そろえるのは呼び出し側。
    assert_eq!(
        compare_topk(&reference, &actual).unwrap_err(),
        CompareError::LengthMismatch {
            reference: 1,
            actual: 3
        }
    );
}

#[test]
fn non_finite_scores_are_absent_and_selecting_them_is_a_mismatch() {
    // IEEE の > だと +Inf は最上位に来る。NaN と -Inf も混ぜる。
    let scores = [f32::NAN, 2.0, f32::INFINITY, 1.0, f32::NEG_INFINITY];
    let reference = reference_topk_f32(&scores, 5).unwrap();
    assert_eq!(
        pairs(&reference),
        vec![(1, 2.0_f32.to_bits()), (3, 1.0_f32.to_bits())]
    );
    assert!(reference.iter().all(|x| x.score().is_finite()));
    assert!(reference.iter().all(|x| ![0, 2, 4].contains(&x.slot())));

    // +Inf のスロットを先頭に選んだ実測。
    let picks_inf = [e(2, f32::INFINITY), e(1, 2.0)];
    let c = compare_topk(&reference, &picks_inf).unwrap();
    assert!(!c.is_match());
    assert_eq!(c.first_mismatch(), Some(0));
    // NaN のスロットを先頭に選んだ実測。
    let picks_nan = [e(0, f32::NAN), e(1, 2.0)];
    let c = compare_topk(&reference, &picks_nan).unwrap();
    assert_eq!(c.first_mismatch(), Some(0));
}

#[test]
fn mixed_input_with_ties_non_finite_and_signed_zeros() {
    // 同点（3.0 がスロット 0 と 5）、有限でない値、-0.0（スロット 1）と +0.0（スロット 4）を混ぜる。
    let scores = [3.0, -0.0, f32::NAN, f32::INFINITY, 0.0, 3.0, -1.0];
    let full = reference_topk_f32(&scores, 6).unwrap();
    assert_eq!(
        pairs(&full),
        vec![
            (0, 3.0_f32.to_bits()),
            (5, 3.0_f32.to_bits()),
            (4, 0.0_f32.to_bits()),
            (1, (-0.0_f32).to_bits()),
            (6, (-1.0_f32).to_bits()),
        ]
    );

    // k が有限の件数より小さい: 零の手前で切れ、そろえは要らない。
    let small = reference_topk_f32(&scores, 3).unwrap();
    assert_eq!(small.len(), 3);
    let actual_small = [e(0, 3.0), e(5, 3.0), e(4, 0.0)];
    assert!(compare_topk(&small, &actual_small).unwrap().is_match());
    // 零の符号だけが違う実測は不一致。
    let wrong_sign = [e(0, 3.0), e(5, 3.0), e(4, -0.0)];
    let c = compare_topk(&small, &wrong_sign).unwrap();
    assert_eq!(c.first_mismatch(), Some(2));

    // k が有限の件数より大きい: 末尾を埋める値でそろえ、同じ並びなら一致する。
    for pad in pad_candidates() {
        let padded_ref = pad_to_k(reference_topk_f32(&scores, 8).unwrap(), 8, pad);
        assert_eq!(padded_ref.len(), 8);
        let mut actual = pad_to_k(full.clone(), 8, pad);
        assert!(compare_topk(&padded_ref, &actual).unwrap().is_match());
        // 零の 2 件の順を入れ替えると、その 2 位置だけが不一致。
        actual.swap(2, 3);
        let c = compare_topk(&padded_ref, &actual).unwrap();
        assert_eq!(c.mismatched(), 2);
        assert_eq!(c.first_mismatch(), Some(2));
    }
}

/// テスト内の独立した選び方（繰り返しの最大探索）。有限でない値を飛ばし、同点は小さいスロットを先に取る。
fn independent_topk(scores: &[f32], k: usize) -> Vec<TopKEntry> {
    let mut taken = vec![false; scores.len()];
    let mut out = Vec::new();
    while out.len() < k {
        let mut best: Option<usize> = None;
        for (i, s) in scores.iter().enumerate() {
            if taken[i] || !s.is_finite() {
                continue;
            }
            // 厳密に大きいときだけ置き換えるので、同点では先に見た小さいスロットが残る。
            if best.is_none_or(|b| s.total_cmp(&scores[b]).is_gt()) {
                best = Some(i);
            }
        }
        let Some(b) = best else { break };
        taken[b] = true;
        out.push(e(u32::try_from(b).unwrap(), scores[b]));
    }
    out
}

#[test]
fn independent_selection_over_same_scores_equals_reference() {
    let scores = [
        0.0,
        -0.0,
        2.5,
        f32::NAN,
        2.5,
        f32::INFINITY,
        -3.0,
        0.0,
        f32::NEG_INFINITY,
    ];
    for k in [1, 3, 6, 10] {
        let reference = reference_topk_f32(&scores, k).unwrap();
        let actual = independent_topk(&scores, k);
        let c = compare_topk(&reference, &actual).unwrap();
        assert!(c.is_match(), "k = {k}");
    }
}
