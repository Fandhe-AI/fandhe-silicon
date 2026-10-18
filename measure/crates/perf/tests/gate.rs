//! `core` の実物の比べの道具の結果を門（`measure_perf::gate`）に通す、端から端までの試験
//! （spec TASK-1026 の 3/4 のポインタ）。
//!
//! 本 crate の lib は `core` に依存しない。この試験だけが dev-dependency で `core`（feature `test-support`）を呼び、
//! 比べの結果を `GateVerdict` へ写す呼び手の役を務める。比べがエラー（長さ 0・長さ違い・大きさ違い）のときは
//! 結果なし（`None`）に写し、門が fail-closed で採らないことを確かめる。
//! GPU の実機は要らず、ファイルにも環境変数にもネットワークにも触れない。

use fandhe_silicon_core::parity::pixel::{PixelView, T3, compare_pixels, compare_pixels_exact};
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};
use fandhe_silicon_core::parity::topk::{TopKEntry, compare_topk, reference_topk_f32};
use measure_perf::gate::{
    Decision, GateInput, GateOutcome, GateResult, GateSource, GateVerdict, NotTakenReason, apply,
    render,
};
use measure_perf::record::{Id, Name};

fn verdict(is_match: bool) -> GateVerdict {
    if is_match {
        GateVerdict::Match
    } else {
        GateVerdict::Mismatch
    }
}

/// 結果の一致の道具の結果（`is_match` と、エラーかどうか）を門の入力の結果へ写す。
fn result_parity(is_match: Option<bool>) -> Option<GateResult> {
    is_match.map(|m| GateResult {
        source: GateSource::ResultParity,
        verdict: verdict(m),
    })
}

/// 画素の一致の道具の結果を門の入力の結果へ写す。
fn pixel_parity(is_match: Option<bool>) -> Option<GateResult> {
    is_match.map(|m| GateResult {
        source: GateSource::PixelParity,
        verdict: verdict(m),
    })
}

fn inp(arm: &str, row: &str, id: &str, result: Option<GateResult>) -> GateInput {
    GateInput {
        arm: Name::parse(arm).unwrap(),
        row: Name::parse(row).unwrap(),
        run_id: Id::parse(id).unwrap(),
        result,
    }
}

fn decision(o: &GateOutcome, id: &str) -> Decision {
    o.decisions()
        .iter()
        .find(|d| d.run_id().as_str() == id)
        .unwrap()
        .decision()
}

fn count(o: &GateOutcome, r: NotTakenReason) -> u64 {
    o.reason_counts().iter().find(|(x, _)| *x == r).unwrap().1
}

// ---- 結果の一致の道具（f32 の列）の結果 ----

const REFERENCE: [f32; 4] = [1.0, -2.0, 0.5, 100.0];

fn f32_gate(actual: &[f32]) -> Option<GateResult> {
    result_parity(
        compare_f32(&REFERENCE, actual, T1)
            .ok()
            .map(|c| c.is_match()),
    )
}

/// 1 要素に、相対・絶対の両方の上限を十分に超える誤差を足した列。
fn perturbed() -> [f32; 4] {
    let mut v = REFERENCE;
    v[3] += 1.0; // 100 に対し相対誤差 1e-2 は T1 の相対の上限（1e-3）を超える
    v
}

#[test]
fn result_parity_exact_input_is_taken() {
    let o = apply(&[inp("a", "r", "run-1", f32_gate(&REFERENCE))]).unwrap();
    assert_eq!(decision(&o, "run-1"), Decision::Taken);
    assert_eq!(o.taken_run_ids().len(), 1);
}

#[test]
fn result_parity_perturbed_input_is_not_taken_and_blocks_pair() {
    let o = apply(&[
        inp("a", "r", "run-1", f32_gate(&REFERENCE)),
        inp("a", "r", "run-2", f32_gate(&perturbed())),
    ])
    .unwrap();
    assert_eq!(
        decision(&o, "run-2"),
        Decision::NotTaken(NotTakenReason::ResultParityMismatch)
    );
    assert_eq!(
        decision(&o, "run-1"),
        Decision::NotTaken(NotTakenReason::PairNotTaken)
    );
    assert!(o.taken_run_ids().is_empty());
    assert_eq!(count(&o, NotTakenReason::ResultParityMismatch), 1);
    assert_eq!(count(&o, NotTakenReason::PairNotTaken), 1);
}

// ---- Top-k（ビット同一の比べ） ----

fn topk_gate(actual: &[TopKEntry], reference: &[TopKEntry]) -> Option<GateResult> {
    result_parity(compare_topk(reference, actual).ok().map(|c| c.is_match()))
}

#[test]
fn topk_identical_taken_and_bit_flip_or_swap_not_taken() {
    let scores = [0.25f32, 3.0, -1.0, 7.5, 2.0];
    let reference = reference_topk_f32(&scores, 3).unwrap();

    // スコアの 1 ビットだけ違う
    let mut flipped = reference.clone();
    flipped[0] = TopKEntry::new(
        flipped[0].slot(),
        f32::from_bits(flipped[0].score_bits() ^ 1),
    );
    // スロット番号の入れ替え
    let mut swapped = reference.clone();
    swapped.swap(0, 1);

    let o = apply(&[
        inp("a", "same", "run-1", topk_gate(&reference, &reference)),
        inp("a", "flip", "run-2", topk_gate(&flipped, &reference)),
        inp("a", "swap", "run-3", topk_gate(&swapped, &reference)),
    ])
    .unwrap();
    assert_eq!(decision(&o, "run-1"), Decision::Taken);
    for id in ["run-2", "run-3"] {
        assert_eq!(
            decision(&o, id),
            Decision::NotTaken(NotTakenReason::ResultParityMismatch)
        );
    }
    assert_eq!(o.taken_run_count(), 1);
}

// ---- 有限でない値の規則 ----

#[test]
fn non_finite_in_finite_reference_position_is_not_taken() {
    let special = |actual: &[f32]| {
        result_parity(
            compare_f32_with_special_values(&REFERENCE, actual, T1)
                .ok()
                .map(|c| c.is_match()),
        )
    };
    let mut with_nan = REFERENCE;
    with_nan[1] = f32::NAN; // 参照が有限の位置に NaN
    let o = apply(&[
        inp("a", "ok", "run-1", special(&REFERENCE)),
        inp("a", "nan", "run-2", special(&with_nan)),
    ])
    .unwrap();
    assert_eq!(decision(&o, "run-1"), Decision::Taken);
    assert_eq!(
        decision(&o, "run-2"),
        Decision::NotTaken(NotTakenReason::ResultParityMismatch)
    );
}

// ---- 画素の一致の道具 ----

const W: usize = 4;
const H: usize = 4;

fn image() -> Vec<u8> {
    (0..W * H * 4).map(|i| (i % 200) as u8).collect()
}

fn view(data: &[u8]) -> PixelView<'_> {
    PixelView::new(data, W, H).unwrap()
}

fn t3_gate(reference: &[u8], actual: &[u8]) -> Option<GateResult> {
    pixel_parity(
        compare_pixels(view(reference), view(actual), T3)
            .ok()
            .map(|c| c.is_match()),
    )
}

fn exact_gate(reference: &[u8], actual: &[u8]) -> Option<GateResult> {
    pixel_parity(
        compare_pixels_exact(view(reference), view(actual))
            .ok()
            .map(|c| c.is_match()),
    )
}

#[test]
fn pixel_parity_same_image_taken_and_changed_pixel_not_taken() {
    let reference = image();
    // 1 画素の 1 チャネルを、チャネル差の上限（T3 は 2）を超える 3 だけ変える
    let mut changed = reference.clone();
    changed[5] = changed[5].wrapping_add(3);

    let o = apply(&[
        inp("a", "same", "run-1", t3_gate(&reference, &reference)),
        inp("a", "diff", "run-2", t3_gate(&reference, &changed)),
    ])
    .unwrap();
    assert_eq!(decision(&o, "run-1"), Decision::Taken);
    assert_eq!(
        decision(&o, "run-2"),
        Decision::NotTaken(NotTakenReason::PixelParityMismatch)
    );
}

#[test]
fn pixel_parity_exact_rejects_difference_of_one() {
    let reference = image();
    let mut changed = reference.clone();
    changed[0] = changed[0].wrapping_add(1);
    let o = apply(&[
        inp("a", "same", "run-1", exact_gate(&reference, &reference)),
        inp("a", "diff", "run-2", exact_gate(&reference, &changed)),
    ])
    .unwrap();
    assert_eq!(decision(&o, "run-1"), Decision::Taken);
    assert_eq!(
        decision(&o, "run-2"),
        Decision::NotTaken(NotTakenReason::PixelParityMismatch)
    );
}

// ---- 比べがエラー ----

#[test]
fn compare_error_maps_to_missing_and_is_not_taken() {
    let short = [1.0f32, -2.0];
    assert!(compare_f32(&REFERENCE, &short, T1).is_err());
    let small = vec![0u8; 2 * 2 * 4];
    assert!(compare_pixels(view(&image()), PixelView::new(&small, 2, 2).unwrap(), T3).is_err());

    let o = apply(&[inp("a", "r", "run-1", f32_gate(&short))]).unwrap();
    assert_eq!(
        decision(&o, "run-1"),
        Decision::NotTaken(NotTakenReason::ResultMissing)
    );
    assert!(o.taken_run_ids().is_empty());
}

// ---- 混ぜた入力・全部一致の入力 ----

#[test]
fn mixed_inputs_leave_reasons_and_counts() {
    let img = image();
    let mut img_bad = img.clone();
    img_bad[5] = img_bad[5].wrapping_add(3);
    let short = [1.0f32];

    let o = apply(&[
        // 組 A: 一致 + 結果の不一致
        inp("a", "r1", "run-a1", f32_gate(&REFERENCE)),
        inp("a", "r1", "run-a2", f32_gate(&perturbed())),
        // 組 B: 一致 + 画素の不一致
        inp("b", "r1", "run-b1", t3_gate(&img, &img)),
        inp("b", "r1", "run-b2", t3_gate(&img, &img_bad)),
        // 組 C: すべて一致
        inp("c", "r1", "run-c1", f32_gate(&REFERENCE)),
        inp("c", "r1", "run-c2", f32_gate(&REFERENCE)),
        // 組 D: 一致 + 比べのエラー
        inp("d", "r1", "run-d1", f32_gate(&REFERENCE)),
        inp("d", "r1", "run-d2", f32_gate(&short)),
    ])
    .unwrap();

    assert_eq!(
        decision(&o, "run-a2"),
        Decision::NotTaken(NotTakenReason::ResultParityMismatch)
    );
    assert_eq!(
        decision(&o, "run-b2"),
        Decision::NotTaken(NotTakenReason::PixelParityMismatch)
    );
    assert_eq!(
        decision(&o, "run-d2"),
        Decision::NotTaken(NotTakenReason::ResultMissing)
    );
    for id in ["run-a1", "run-b1", "run-d1"] {
        assert_eq!(
            decision(&o, id),
            Decision::NotTaken(NotTakenReason::PairNotTaken)
        );
    }
    assert_eq!(decision(&o, "run-c1"), Decision::Taken);
    assert_eq!(decision(&o, "run-c2"), Decision::Taken);

    assert_eq!(o.taken_run_count(), 2);
    assert_eq!(o.not_taken_run_count(), 6);
    assert_eq!(o.taken_pair_count(), 1);
    assert_eq!(o.not_taken_pair_count(), 3);
    assert_eq!(count(&o, NotTakenReason::ResultParityMismatch), 1);
    assert_eq!(count(&o, NotTakenReason::PixelParityMismatch), 1);
    assert_eq!(count(&o, NotTakenReason::ResultMissing), 1);
    assert_eq!(count(&o, NotTakenReason::PairNotTaken), 3);
    let ids: Vec<&str> = o.taken_run_ids().iter().map(|i| i.as_str()).collect();
    assert_eq!(ids, ["run-c1", "run-c2"]);

    let s = render(&o);
    assert!(s.contains(
        "\"not_taken_by_reason\":{\"result-parity-mismatch\":1,\"pixel-parity-mismatch\":1,\"result-missing\":1,\"pair-not-taken\":3}"
    ));
    assert!(s.contains("\"taken_run_count\":2,\"not_taken_run_count\":6"));
    assert!(s.contains(
        "\"run_id\":\"run-a2\",\"decision\":\"not-taken\",\"reason\":\"result-parity-mismatch\""
    ));
    assert!(s.contains("\"run_id\":\"run-c1\",\"decision\":\"taken\"}"));
}

#[test]
fn all_match_inputs_are_all_taken() {
    let img = image();
    let reference = reference_topk_f32(&[1.0, 5.0, 3.0], 2).unwrap();
    let o = apply(&[
        inp("a", "r1", "run-1", f32_gate(&REFERENCE)),
        inp("a", "r2", "run-2", topk_gate(&reference, &reference)),
        inp("b", "r1", "run-3", t3_gate(&img, &img)),
        inp("b", "r2", "run-4", exact_gate(&img, &img)),
    ])
    .unwrap();
    assert_eq!(o.taken_run_count(), 4);
    assert_eq!(o.not_taken_run_count(), 0);
    assert_eq!(o.not_taken_pair_count(), 0);
    assert!(o.reason_counts().iter().all(|(_, n)| *n == 0));
    assert_eq!(o.taken_run_ids().len(), 4);
}
