//! T-1 の要素ごとの規則への追加の入力（spec TASK-975 の 5/6。対応する項目は
//! PARITY-1・PARITY-2・PARITY-3・PARITY-5 のポインタのみ）。
//! 項目だけが求める 3 種類の入力（最大値どうしの比べでは通るが要素ごとには外れる組、
//! NaN の payload だけが違う組、参照が零で実測が微小な有限の組）を名前付きの組として置き、
//! `compare_f32` と `compare_f32_with_special_values` の 2 つの比べ方に通して、
//! 要素ごとの判定と列の集計が合うことを確かめる。
//! 規則そのものの確かめは `parity_element_rules.rs` にある。ここでは規則を確かめ直さない。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にし、入力はこの場で組む。

use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, compare_f32_with_special_values, judge_element_with_special_values,
};
use fandhe_silicon_core::parity::tolerance::{ElementVerdict, T1, compare_f32, judge_element};

/// 追加の入力の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputKind {
    MaxPassesElementwiseFails,
    NanPayloadOnly,
    ZeroReferenceTinyFinite,
}

/// 名前の付いた入力の組（参照と実測は同じ長さ）。
struct InputCase {
    kind: InputKind,
    label: &'static str,
    reference: Vec<f32>,
    actual: Vec<f32>,
}

/// 上限未満で最大の `f32`（正）。
fn largest_below_absolute() -> f32 {
    let mut x = T1.absolute() as f32;
    while f64::from(x) >= T1.absolute() {
        x = f32::from_bits(x.to_bits() - 1);
    }
    loop {
        let next = f32::from_bits(x.to_bits() + 1);
        if f64::from(next) >= T1.absolute() {
            return x;
        }
        x = next;
    }
}

/// 上限以上で最小の `f32`（正）。
fn smallest_at_or_above_absolute() -> f32 {
    f32::from_bits(largest_below_absolute().to_bits() + 1)
}

fn max_of(v: &[f32]) -> f32 {
    v.iter().copied().fold(f32::MIN, f32::max)
}

/// 追加の入力の組を全部返す。
fn additional_inputs() -> Vec<InputCase> {
    let over = smallest_at_or_above_absolute();
    let under = largest_below_absolute();
    let tiny = f32::from_bits(1);
    let qnan = f32::from_bits(0x7fc0_0000);
    let qnan_payload = f32::from_bits(0x7fc0_1234);
    let qnan_negative = f32::from_bits(0xffc0_0000);
    let snan = f32::from_bits(0x7f80_0001);
    let snan_negative = f32::from_bits(0xff80_4321);

    // 最大の要素は同じで、中間の 1 要素だけを上限の外へずらす。
    let mut shifted = vec![1.0, 2.0, 3.0, 4.0];
    shifted[2] += 0.5;

    vec![
        InputCase {
            kind: InputKind::MaxPassesElementwiseFails,
            label: "max-same-middle-shifted",
            reference: vec![1.0, 2.0, 3.0, 4.0],
            actual: shifted,
        },
        InputCase {
            kind: InputKind::MaxPassesElementwiseFails,
            label: "max-same-permuted",
            reference: vec![1.0, 2.0, 3.0, 4.0],
            actual: vec![4.0, 3.0, 2.0, 1.0],
        },
        InputCase {
            kind: InputKind::NanPayloadOnly,
            label: "nan-payload-differs",
            reference: vec![qnan, qnan, snan],
            actual: vec![qnan_payload, snan, qnan_payload],
        },
        InputCase {
            kind: InputKind::NanPayloadOnly,
            label: "nan-sign-bit-differs",
            reference: vec![qnan, qnan_negative, snan],
            actual: vec![qnan_negative, qnan, snan_negative],
        },
        InputCase {
            kind: InputKind::ZeroReferenceTinyFinite,
            label: "zero-reference-tiny-inside",
            reference: vec![0.0, -0.0, 0.0, -0.0, 0.0, -0.0],
            actual: vec![tiny, -tiny, under, -under, -tiny, tiny],
        },
        InputCase {
            kind: InputKind::ZeroReferenceTinyFinite,
            label: "zero-reference-adjacent-outside",
            reference: vec![0.0, -0.0, 0.0],
            actual: vec![over, -over, under],
        },
    ]
}

/// 要素ごとの判定から数えた不一致の件数と最初の添字（有限でない値の規則つき）。
fn count_special(reference: &[f32], actual: &[f32]) -> (usize, Option<usize>) {
    let bad: Vec<usize> = (0..reference.len())
        .filter(|&i| !judge_element_with_special_values(reference[i], actual[i], T1).is_match())
        .collect();
    (bad.len(), bad.first().copied())
}

#[test]
fn every_additional_input_kind_is_present() {
    let cases = additional_inputs();
    for kind in [
        InputKind::MaxPassesElementwiseFails,
        InputKind::NanPayloadOnly,
        InputKind::ZeroReferenceTinyFinite,
    ] {
        assert!(cases.iter().any(|c| c.kind == kind), "{kind:?}");
    }
    let mut labels: Vec<&str> = cases.iter().map(|c| c.label).collect();
    labels.sort_unstable();
    let before = labels.len();
    labels.dedup();
    assert_eq!(before, labels.len());
    for c in &cases {
        assert_eq!(c.reference.len(), c.actual.len(), "{}", c.label);
    }
}

#[test]
fn max_passing_but_elementwise_failing_inputs_are_mismatches() {
    for c in additional_inputs()
        .iter()
        .filter(|c| c.kind == InputKind::MaxPassesElementwiseFails)
    {
        // 最大値どうしは内に入る。
        assert_eq!(
            judge_element(max_of(&c.reference), max_of(&c.actual), T1),
            ElementVerdict::Within,
            "{}",
            c.label
        );
        let bad: Vec<usize> = (0..c.reference.len())
            .filter(|&i| judge_element(c.reference[i], c.actual[i], T1) != ElementVerdict::Within)
            .collect();
        assert!(!bad.is_empty(), "{}", c.label);
        let plain = compare_f32(&c.reference, &c.actual, T1).unwrap();
        assert!(!plain.is_match(), "{}", c.label);
        assert_eq!(plain.exceeded(), bad.len(), "{}", c.label);
        assert_eq!(plain.first_mismatch(), bad.first().copied(), "{}", c.label);
        let special = compare_f32_with_special_values(&c.reference, &c.actual, T1).unwrap();
        assert!(!special.is_match(), "{}", c.label);
        assert_eq!(special.exceeded(), bad.len(), "{}", c.label);
        assert_eq!(
            special.first_mismatch(),
            bad.first().copied(),
            "{}",
            c.label
        );
    }
}

#[test]
fn nan_payload_only_inputs_match_under_special_values_rule() {
    for c in additional_inputs()
        .iter()
        .filter(|c| c.kind == InputKind::NanPayloadOnly)
    {
        // 入力の前提: 両方とも NaN で、ビット列は異なる。
        for i in 0..c.reference.len() {
            assert!(
                c.reference[i].is_nan() && c.actual[i].is_nan(),
                "{}",
                c.label
            );
            assert_ne!(
                c.reference[i].to_bits(),
                c.actual[i].to_bits(),
                "{}",
                c.label
            );
            assert_eq!(
                judge_element_with_special_values(c.reference[i], c.actual[i], T1),
                SpecialVerdict::BothNan,
                "{}",
                c.label
            );
        }
        let special = compare_f32_with_special_values(&c.reference, &c.actual, T1).unwrap();
        assert!(special.is_match(), "{}", c.label);
        assert_eq!(special.first_mismatch(), None, "{}", c.label);
        // 既存の関係の固定: 規則なしの比べ方では有限でない値として数えられ、一致にならない。
        let plain = compare_f32(&c.reference, &c.actual, T1).unwrap();
        assert!(!plain.is_match(), "{}", c.label);
        assert_eq!(plain.non_finite(), c.reference.len(), "{}", c.label);

        // NaN の一致は、別の添字の有限の外れを隠さない。
        let mut reference = c.reference.clone();
        let mut actual = c.actual.clone();
        reference.push(1.0);
        actual.push(2.0);
        let outlier = reference.len() - 1;
        let mixed = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
        assert!(!mixed.is_match(), "{}", c.label);
        assert_eq!(mixed.exceeded(), 1, "{}", c.label);
        assert_eq!(mixed.first_mismatch(), Some(outlier), "{}", c.label);
    }
}

#[test]
fn zero_reference_tiny_finite_inputs_follow_absolute_bound() {
    for c in additional_inputs()
        .iter()
        .filter(|c| c.kind == InputKind::ZeroReferenceTinyFinite)
    {
        let mut inside = 0;
        for i in 0..c.reference.len() {
            assert_eq!(c.reference[i], 0.0, "{}", c.label);
            let within = f64::from(c.actual[i]).abs() < T1.absolute();
            let verdict = judge_element(c.reference[i], c.actual[i], T1);
            assert_eq!(
                verdict,
                if within {
                    ElementVerdict::Within
                } else {
                    ElementVerdict::Exceeded
                },
                "{}",
                c.label
            );
            inside += usize::from(within);
        }
        let outside = c.reference.len() - inside;
        let plain = compare_f32(&c.reference, &c.actual, T1).unwrap();
        let special = compare_f32_with_special_values(&c.reference, &c.actual, T1).unwrap();
        assert_eq!(plain.exceeded(), outside, "{}", c.label);
        assert_eq!(special.exceeded(), outside, "{}", c.label);
        assert_eq!(plain.is_match(), outside == 0, "{}", c.label);
        assert_eq!(special.is_match(), outside == 0, "{}", c.label);
        assert_eq!(
            plain.first_mismatch(),
            special.first_mismatch(),
            "{}",
            c.label
        );
    }
    // 組の側が、上限の内の組と外の組の両方を持つこと。
    let cases = additional_inputs();
    let inside_case = cases
        .iter()
        .find(|c| c.label == "zero-reference-tiny-inside")
        .unwrap();
    assert!(
        compare_f32(&inside_case.reference, &inside_case.actual, T1)
            .unwrap()
            .is_match()
    );
    let outside_case = cases
        .iter()
        .find(|c| c.label == "zero-reference-adjacent-outside")
        .unwrap();
    assert_eq!(
        compare_f32(&outside_case.reference, &outside_case.actual, T1)
            .unwrap()
            .first_mismatch(),
        Some(0)
    );

    // 対照: 同じ大きさの誤差が、零でない参照には相対誤差で内に入る。
    let d = smallest_at_or_above_absolute();
    let r = (f64::from(d) / (T1.relative() * 0.5)) as f32;
    assert!(f64::from((r + d) - r) >= T1.absolute());
    assert_eq!(judge_element(r, r + d, T1), ElementVerdict::Within);
    assert_eq!(judge_element(0.0, d, T1), ElementVerdict::Exceeded);
}

#[test]
fn concatenated_additional_inputs_are_judged_per_element() {
    let mut reference = Vec::new();
    let mut actual = Vec::new();
    for c in additional_inputs() {
        reference.extend(c.reference);
        actual.extend(c.actual);
    }
    let (count, first) = count_special(&reference, &actual);
    assert!(count > 0);
    let special = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert_eq!(special.compared(), reference.len());
    assert_eq!(special.exceeded() + special.non_finite_mismatch(), count);
    assert_eq!(special.first_mismatch(), first);
    assert_eq!(special.is_match(), count == 0);
}

#[test]
fn both_comparators_agree_on_finite_only_additional_inputs() {
    for c in additional_inputs()
        .iter()
        .filter(|c| c.reference.iter().chain(&c.actual).all(|v| v.is_finite()))
    {
        let plain = compare_f32(&c.reference, &c.actual, T1).unwrap();
        let special = compare_f32_with_special_values(&c.reference, &c.actual, T1).unwrap();
        assert_eq!(plain.compared(), special.compared(), "{}", c.label);
        assert_eq!(plain.exceeded(), special.exceeded(), "{}", c.label);
        assert_eq!(
            plain.first_mismatch(),
            special.first_mismatch(),
            "{}",
            c.label
        );
        assert_eq!(plain.is_match(), special.is_match(), "{}", c.label);
    }
}
