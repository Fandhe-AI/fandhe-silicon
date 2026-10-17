//! T-1 の要素ごとの規則の確認（spec TASK-975 の 2/6。内容の (1)。対応する項目は PARITY-1 のポインタのみ）。
//! 比較元が融合積和の CPU 参照であること、一致が「相対誤差が内、または絶対誤差が内」の
//! 要素ごとの判定であること、1 要素の外れで列全体が不一致になることを確かめる。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::{ElementVerdict, T1, compare_f32, judge_element};

/// 長さ `n` の単純な有限の参照列（外れ値の土台に使う）。
fn long_reference(n: usize) -> Vec<f32> {
    (0..n).map(|i| 1.0 + (i % 7) as f32).collect()
}

#[test]
fn reference_is_built_with_fused_multiply_add() {
    // 融合の有無の差が絶対誤差の上限を超えるよう、積を 2^10 倍ずつ拡大した入力。
    let e = 2.0_f32.powi(-12);
    let x = 1.0 + e;
    let neg = -(1.0 + 2.0 * e);
    let s = 2.0_f32.powi(10);
    let a = [s, x * s];
    let b = [neg * s, x * s];
    let reference = reference_gemm_f32(&a, &b, 1, 2, 1).unwrap();
    // 融合なしの手順（積を丸めてから足す）。
    let unfused = [(x * s) * (x * s) + (s * (neg * s))];
    // 想定どおりに両者が離れていることを先に確かめる（崩れたら入力を選び直す）。
    assert_eq!(reference[0], 2.0_f32.powi(-4));
    assert_eq!(unfused[0], 0.0);

    let against_unfused = compare_f32(&reference, &unfused, T1).unwrap();
    assert!(!against_unfused.is_match());
    assert_eq!(against_unfused.exceeded(), 1);
    // 融合ありの値と同じ列は一致する。
    let against_fused = compare_f32(&reference, &reference, T1).unwrap();
    assert!(against_fused.is_match());
}

#[test]
fn match_requires_relative_or_absolute_bound() {
    let rel = T1.relative();
    let abs = T1.absolute();

    // 参照が大きい: 絶対誤差は外だが、相対誤差は内。
    let big = 1000.0_f32;
    let big_actual = big + (rel * f64::from(big) * 0.5) as f32;
    assert!(f64::from(big_actual - big) > abs);
    assert_eq!(judge_element(big, big_actual, T1), ElementVerdict::Within);

    // 参照が小さい: 相対誤差は外だが、絶対誤差は内。
    let small = (abs * 100.0) as f32;
    let small_actual = small + (abs * 0.5) as f32;
    assert!(f64::from(small_actual - small) / f64::from(small) > rel);
    assert_eq!(
        judge_element(small, small_actual, T1),
        ElementVerdict::Within
    );

    // 両方内。
    let one_actual = 1.0 + (abs * 0.1) as f32;
    assert_eq!(judge_element(1.0, one_actual, T1), ElementVerdict::Within);

    // 両方外。
    let both_out = 1.0 + (rel * 10.0) as f32;
    assert_eq!(judge_element(1.0, both_out, T1), ElementVerdict::Exceeded);
}

#[test]
fn max_value_comparison_would_pass_but_elementwise_fails() {
    let cases: [([f32; 4], [f32; 4]); 2] = [
        ([1.0, 2.0, 3.0, 4.0], [4.0, 3.0, 2.0, 1.0]),
        ([100.0, 1.0, 2.0, 0.5], [100.0, 2.0, 1.0, 0.5]),
    ];
    for (reference, actual) in cases {
        // 最大値どうしを比べれば内に入る（最大値比較では区別できない入力）。
        let max_of = |v: &[f32]| v.iter().copied().fold(f32::MIN, f32::max);
        assert_eq!(
            judge_element(max_of(&reference), max_of(&actual), T1),
            ElementVerdict::Within
        );

        let result = compare_f32(&reference, &actual, T1).unwrap();
        let expected: Vec<usize> = (0..reference.len())
            .filter(|&i| judge_element(reference[i], actual[i], T1) == ElementVerdict::Exceeded)
            .collect();
        assert!(!result.is_match());
        assert_eq!(result.exceeded(), expected.len());
        assert_eq!(result.first_mismatch(), expected.first().copied());
    }
}

#[test]
fn single_outlier_in_a_long_sequence_is_a_mismatch() {
    let n = 1024;
    let reference = long_reference(n);
    for pos in [0, n / 2, n - 1] {
        let mut actual = reference.clone();
        // 1 要素だけ相対誤差の上限の 10 倍ずらす。平均誤差にすれば上限の内に収まる。
        actual[pos] = reference[pos] * (1.0 + (T1.relative() * 10.0) as f32);
        let mean_error = actual
            .iter()
            .zip(&reference)
            .map(|(a, r)| f64::from((a - r).abs()) / f64::from(*r))
            .sum::<f64>()
            / n as f64;
        assert!(mean_error < T1.relative());

        let result = compare_f32(&reference, &actual, T1).unwrap();
        assert!(!result.is_match());
        assert_eq!(result.exceeded(), 1);
        assert_eq!(result.first_mismatch(), Some(pos));
    }
}

#[test]
fn finite_rule_is_the_same_through_special_values_comparator() {
    let n = 1024;
    let mut inputs: Vec<(Vec<f32>, Vec<f32>)> = vec![
        (vec![1.0, 2.0, 3.0, 4.0], vec![4.0, 3.0, 2.0, 1.0]),
        (vec![100.0, 1.0, 2.0], vec![100.0, 2.0, 1.0]),
    ];
    let reference = long_reference(n);
    for pos in [0, n / 2, n - 1] {
        let mut actual = reference.clone();
        actual[pos] = reference[pos] * (1.0 + (T1.relative() * 10.0) as f32);
        inputs.push((reference.clone(), actual));
    }
    inputs.push((reference.clone(), reference.clone()));

    for (r, a) in &inputs {
        let plain = compare_f32(r, a, T1).unwrap();
        let special = compare_f32_with_special_values(r, a, T1).unwrap();
        assert_eq!(special.is_match(), plain.is_match());
        assert_eq!(special.exceeded(), plain.exceeded());
        assert_eq!(special.first_mismatch(), plain.first_mismatch());
    }
}

#[test]
fn elementwise_like_output_is_judged_per_element() {
    // 要素ごと演算（ReLU 相当）の出力に見立てた列。CPU 参照は core に無いので直書きする。
    let input = [-2.0_f32, -0.5, 0.0, 0.5, 2.0, 8.0];
    let reference: Vec<f32> = input.iter().map(|v| v.max(0.0)).collect();
    assert!(compare_f32(&reference, &reference, T1).unwrap().is_match());

    let mut actual = reference.clone();
    actual[4] = 2.0 * (1.0 + (T1.relative() * 10.0) as f32);
    let result = compare_f32(&reference, &actual, T1).unwrap();
    assert!(!result.is_match());
    assert_eq!(result.exceeded(), 1);
    assert_eq!(result.first_mismatch(), Some(4));
}
