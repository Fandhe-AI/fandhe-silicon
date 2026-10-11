//! Top-k の CPU 参照（`reference_topk_f32`）と T-2 の比べ（`compare_topk`）の振る舞いの確認（spec TASK-24 の 5/8）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::tolerance::CompareError;
use fandhe_silicon_core::parity::topk::{
    TopKEntry, TopKReferenceError, compare_topk, reference_topk_f32,
};

fn pairs(v: &[TopKEntry]) -> Vec<(u32, u32)> {
    v.iter().map(|e| (e.slot(), e.score_bits())).collect()
}

fn e(slot: u32, score: f32) -> TopKEntry {
    TopKEntry::new(slot, score)
}

#[test]
fn descending_order_matches_hand_computed_result() {
    let r = reference_topk_f32(&[1.0, 5.0, 3.0, 4.0, 2.0], 3).unwrap();
    assert_eq!(
        pairs(&r),
        vec![
            (1, 5.0_f32.to_bits()),
            (3, 4.0_f32.to_bits()),
            (2, 3.0_f32.to_bits())
        ]
    );
}

#[test]
fn ties_are_ordered_by_ascending_slot() {
    let r = reference_topk_f32(&[2.0, 7.0, 2.0, 7.0], 4).unwrap();
    let slots: Vec<u32> = r.iter().map(|x| x.slot()).collect();
    assert_eq!(slots, vec![1, 3, 0, 2]);
}

#[test]
fn positive_zero_precedes_negative_zero() {
    let r = reference_topk_f32(&[-0.0, 0.0], 2).unwrap();
    assert_eq!(
        pairs(&r),
        vec![(1, 0.0_f32.to_bits()), (0, (-0.0_f32).to_bits())]
    );
}

#[test]
fn non_finite_scores_never_appear() {
    let s = [f32::NAN, f32::INFINITY, 1.0, f32::NEG_INFINITY, 2.0];
    let r = reference_topk_f32(&s, 10).unwrap();
    assert_eq!(
        pairs(&r),
        vec![(4, 2.0_f32.to_bits()), (2, 1.0_f32.to_bits())]
    );
    let all_bad = [f32::NAN, f32::INFINITY];
    assert!(reference_topk_f32(&all_bad, 2).unwrap().is_empty());
}

#[test]
fn fewer_finite_than_k_returns_only_finite_count() {
    let r = reference_topk_f32(&[1.0, f32::NAN, 2.0], 5).unwrap();
    assert_eq!(r.len(), 2);
}

#[test]
fn zero_k_and_empty_scores_give_empty_ok() {
    assert!(reference_topk_f32(&[1.0, 2.0], 0).unwrap().is_empty());
    assert!(reference_topk_f32(&[], 3).unwrap().is_empty());
}

#[test]
fn identical_lists_match() {
    let r = reference_topk_f32(&[3.0, 1.0, 2.0], 3).unwrap();
    let c = compare_topk(&r, &r).unwrap();
    assert!(c.is_match());
    assert_eq!(c.compared(), 3);
    assert_eq!(c.mismatched(), 0);
    assert_eq!(c.first_mismatch(), None);
}

#[test]
fn slot_difference_and_one_ulp_difference_are_mismatches() {
    let r = [e(0, 3.0), e(1, 2.0), e(2, 1.0)];
    let slot_diff = [e(0, 3.0), e(5, 2.0), e(2, 1.0)];
    let c = compare_topk(&r, &slot_diff).unwrap();
    assert!(!c.is_match());
    assert_eq!((c.mismatched(), c.first_mismatch()), (1, Some(1)));

    let ulp = f32::from_bits(1.0_f32.to_bits() + 1);
    let ulp_diff = [e(0, 3.0), e(1, 2.0), e(2, ulp)];
    let c = compare_topk(&r, &ulp_diff).unwrap();
    assert_eq!((c.mismatched(), c.first_mismatch()), (1, Some(2)));
}

#[test]
#[allow(clippy::float_cmp)]
fn signed_zero_difference_is_caught() {
    // f32 の == では見逃す差であることの確認。
    let (neg, pos) = (
        std::hint::black_box(-0.0_f32),
        std::hint::black_box(0.0_f32),
    );
    assert!(neg == pos);
    let c = compare_topk(&[e(0, -0.0)], &[e(0, 0.0)]).unwrap();
    assert!(!c.is_match());
}

#[test]
fn non_finite_entries_follow_the_same_bitwise_rule() {
    let s = e(u32::MAX, f32::NEG_INFINITY);
    assert!(compare_topk(&[s], &[s]).unwrap().is_match());
    assert!(!compare_topk(&[s], &[e(0, 1.0)]).unwrap().is_match());
}

#[test]
fn length_mismatch_and_empty_are_errors() {
    assert_eq!(
        compare_topk(&[e(0, 1.0)], &[]).unwrap_err(),
        CompareError::LengthMismatch {
            reference: 1,
            actual: 0
        }
    );
    assert_eq!(
        compare_topk(&[], &[]).unwrap_err(),
        CompareError::EmptyInput
    );
}

#[test]
fn reference_error_display_is_non_empty_ascii() {
    for err in [
        TopKReferenceError::SlotOverflow,
        TopKReferenceError::AllocationFailed,
    ] {
        let text = err.to_string();
        assert!(!text.is_empty() && text.is_ascii());
    }
}

#[test]
fn gemm_output_feeds_topk_and_compares_equal() {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let c = reference_gemm_f32(&a, &b, 2, 3, 2).unwrap();
    let x = reference_topk_f32(&c, 3).unwrap();
    let y = reference_topk_f32(&c, 3).unwrap();
    assert!(compare_topk(&x, &y).unwrap().is_match());
}
