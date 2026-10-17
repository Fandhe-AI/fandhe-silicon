//! T-1 の要素ごとの規則の確認（spec TASK-975 の 2/6〜4/6。内容の (1)〜(4)。対応する項目は
//! PARITY-1・PARITY-2・PARITY-3・PARITY-5 のポインタのみ）。
//! 比較元が融合積和の CPU 参照であること、一致が「相対誤差が内、または絶対誤差が内」の
//! 要素ごとの判定であること、1 要素の外れで列全体が不一致になることを確かめる。
//! 参照が零の要素では相対誤差が効かず、絶対誤差の上限だけで決まることも確かめる。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, compare_f32_with_special_values, judge_element_with_special_values,
};
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

/// 有限でない側の値を要素単位で判定する（T1）。
fn judge_sv(reference: f32, actual: f32) -> SpecialVerdict {
    judge_element_with_special_values(reference, actual, T1)
}

/// 要素単位の判定から数えた、有限でない値の不一致の添字。
fn non_finite_mismatch_positions(reference: &[f32], actual: &[f32]) -> Vec<usize> {
    (0..reference.len())
        .filter(|&i| judge_sv(reference[i], actual[i]) == SpecialVerdict::NonFiniteMismatch)
        .collect()
}

// ---- 内容の (2)・(3)（TASK-975 の 3/6。PARITY-2・PARITY-3 のポインタのみ） ----

#[test]
fn nan_reference_matches_nan_with_payload_only_difference() {
    let r = f32::NAN;
    let payload_only = f32::from_bits(r.to_bits() ^ 1);
    let sign_only = f32::from_bits(r.to_bits() ^ 0x8000_0000);
    // 入力が崩れたら気づけるよう、NaN でありビット列が違うことを先に確かめる。
    for v in [payload_only, sign_only] {
        assert!(v.is_nan());
        assert_ne!(v.to_bits(), r.to_bits());
        assert_eq!(judge_sv(r, v), SpecialVerdict::BothNan);
    }

    let reference = [1.0, r, 2.0, r, 3.0];
    let actual = [1.0, payload_only, 2.0, sign_only, 3.0];
    let result = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert!(result.is_match());
    assert_eq!(result.non_finite_mismatch(), 0);
    assert_eq!(result.exceeded(), 0);
    assert_eq!(result.first_mismatch(), None);

    // 既存の比べ方では NaN は区別されず、有限でない値として数えられる。
    let plain = compare_f32(&reference, &actual, T1).unwrap();
    assert!(plain.non_finite() > 0);
}

#[test]
fn nan_from_cpu_reference_matches_nan_with_another_payload() {
    // +Inf と -Inf の和が NaN になる入力。2 列目は有限。
    let a = [1.0_f32, 1.0];
    let b = [f32::INFINITY, 2.0, f32::NEG_INFINITY, 3.0];
    let reference = reference_gemm_f32(&a, &b, 1, 2, 2).unwrap();
    assert!(reference[0].is_nan());
    assert!(reference[1].is_finite());

    let mut actual = reference.clone();
    actual[0] = f32::from_bits(reference[0].to_bits() ^ 1);
    assert_ne!(actual[0].to_bits(), reference[0].to_bits());
    let result = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert!(result.is_match());
    assert_eq!(result.first_mismatch(), None);
}

#[test]
fn only_one_side_nan_is_a_mismatch() {
    let nan = f32::NAN;
    let finites = [0.0_f32, -0.0, 1.0, f32::MAX];
    for f in finites {
        assert_eq!(judge_sv(nan, f), SpecialVerdict::NonFiniteMismatch);
        assert_eq!(judge_sv(f, nan), SpecialVerdict::NonFiniteMismatch);
    }
    for inf in [f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(judge_sv(nan, inf), SpecialVerdict::NonFiniteMismatch);
        assert_eq!(judge_sv(inf, nan), SpecialVerdict::NonFiniteMismatch);
    }

    let reference = [nan, 1.0, nan, f32::INFINITY, 2.0, nan];
    let actual = [1.0, nan, f32::INFINITY, nan, 2.0, nan];
    let expected = non_finite_mismatch_positions(&reference, &actual);
    assert_eq!(expected, vec![0, 1, 2, 3]);
    let result = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert!(!result.is_match());
    assert_eq!(result.non_finite_mismatch(), expected.len());
    assert_eq!(result.first_mismatch(), expected.first().copied());
}

#[test]
fn payload_only_nan_match_does_not_hide_other_mismatch() {
    let nan = f32::NAN;
    let other = f32::from_bits(nan.to_bits() ^ 1);
    assert!(other.is_nan());
    let reference = vec![nan, 1.0, nan, 2.0, 3.0];

    // 別の添字の NaN 片側。
    let mut one_sided = vec![other, 1.0, other, 2.0, 3.0];
    one_sided[3] = nan;
    let result = compare_f32_with_special_values(&reference, &one_sided, T1).unwrap();
    assert!(!result.is_match());
    assert_eq!(result.non_finite_mismatch(), 1);
    assert_eq!(result.first_mismatch(), Some(3));

    // 別の添字の有限の外れ。
    let mut exceeded = vec![other, 1.0, other, 2.0, 3.0];
    exceeded[4] = 3.0 * (1.0 + (T1.relative() * 10.0) as f32);
    let result = compare_f32_with_special_values(&reference, &exceeded, T1).unwrap();
    assert!(!result.is_match());
    assert_eq!(result.exceeded(), 1);
    assert_eq!(result.non_finite_mismatch(), 0);
    assert_eq!(result.first_mismatch(), Some(4));
}

#[test]
fn infinite_reference_matches_only_same_sign_infinity() {
    let nans = [
        f32::NAN,
        f32::from_bits(f32::NAN.to_bits() ^ 1),
        f32::from_bits(f32::NAN.to_bits() ^ 0x8000_0000),
    ];
    let finites = [0.0_f32, -0.0, 1.0, f32::MAX, f32::MIN];
    for (inf, opposite) in [
        (f32::INFINITY, f32::NEG_INFINITY),
        (f32::NEG_INFINITY, f32::INFINITY),
    ] {
        assert_eq!(judge_sv(inf, inf), SpecialVerdict::SameInfinity);
        assert_eq!(judge_sv(inf, opposite), SpecialVerdict::NonFiniteMismatch);
        for v in nans.into_iter().chain(finites) {
            assert!(!v.is_infinite());
            assert_eq!(judge_sv(inf, v), SpecialVerdict::NonFiniteMismatch);
        }
    }
}

#[test]
fn finite_reference_against_nan_or_infinity_is_a_mismatch() {
    let finites = [0.0_f32, -0.0, 1e-30, 1.0, 1e30, f32::MAX, f32::MIN];
    let non_finites = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    for r in finites {
        for a in non_finites {
            assert_eq!(judge_sv(r, a), SpecialVerdict::NonFiniteMismatch);
        }
    }
    // 誤差で見れば近い組（最大値と無限大）も不一致。
    assert_eq!(
        judge_sv(f32::MAX, f32::INFINITY),
        SpecialVerdict::NonFiniteMismatch
    );
    assert_eq!(
        judge_sv(f32::MIN, f32::NEG_INFINITY),
        SpecialVerdict::NonFiniteMismatch
    );
}

#[test]
fn infinity_from_cpu_reference_is_judged_per_element() {
    // 1x1 に 1x4 を掛け、出力に +Inf・-Inf・有限・+Inf を並べる。
    let a = [1.0_f32];
    let b = [f32::INFINITY, f32::NEG_INFINITY, 2.0, f32::INFINITY];
    let reference = reference_gemm_f32(&a, &b, 1, 1, 4).unwrap();
    let infinite: Vec<usize> = (0..reference.len())
        .filter(|&i| reference[i].is_infinite())
        .collect();
    assert_eq!(infinite, vec![0, 1, 3]);

    let same = compare_f32_with_special_values(&reference, &reference, T1).unwrap();
    assert!(same.is_match());

    // Inf 要素を符号反転・NaN・有限へ置き換える。
    let mut actual = reference.clone();
    actual[1] = f32::INFINITY;
    actual[3] = f32::MAX;
    let replaced = [1usize, 3];
    let result = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert!(!result.is_match());
    assert_eq!(result.non_finite_mismatch(), replaced.len());
    assert_eq!(result.first_mismatch(), Some(1));

    let mut with_nan = reference.clone();
    with_nan[0] = f32::NAN;
    let result = compare_f32_with_special_values(&reference, &with_nan, T1).unwrap();
    assert_eq!(result.non_finite_mismatch(), 1);
    assert_eq!(result.first_mismatch(), Some(0));
}

// ---- 内容の (4)（TASK-975 の 4/6。PARITY-5 のポインタのみ） ----

/// 絶対誤差の上限を `f32` に写した値（`T1` の getter から導く）。
fn zero_bound() -> f32 {
    T1.absolute() as f32
}

/// 絶対誤差が上限未満か（厳密な `<`）。`f64` で計算する期待値。
fn inside_absolute_bound(x: f32) -> bool {
    f64::from(x.abs()) < T1.absolute()
}

/// 要素単位の判定から数えた、外れの添字。
fn exceeded_positions(reference: &[f32], actual: &[f32]) -> Vec<usize> {
    (0..reference.len())
        .filter(|&i| judge_element(reference[i], actual[i], T1) == ElementVerdict::Exceeded)
        .collect()
}

#[test]
fn zero_from_cpu_reference_matches_zero_of_either_sign() {
    // 打ち消しで零になる要素（添字 0・4）と零でない有限の要素を混ぜる。積は丸めなしで表せる。
    let a = [3.0_f32, -3.0, 3.0, 1.0, 2.0, -2.0];
    let b = [0.5_f32, 1.0, 0.5, 2.0];
    let reference = reference_gemm_f32(&a, &b, 3, 2, 2).unwrap();
    let zeros: Vec<usize> = (0..reference.len())
        .filter(|&i| reference[i] == 0.0)
        .collect();
    assert_eq!(zeros, vec![0, 4]);

    // 零の符号は固定しない。どちらの符号の零に置き換えても一致する。
    for z in [0.0_f32, -0.0] {
        let mut actual = reference.clone();
        for &i in &zeros {
            actual[i] = z;
        }
        let plain = compare_f32(&reference, &actual, T1).unwrap();
        assert!(plain.is_match());
        assert_eq!(plain.exceeded(), 0);
        let sv = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
        assert!(sv.is_match());
        assert_eq!(sv.first_mismatch(), None);
    }
}

#[test]
fn zero_reference_with_tiny_finite_inside_absolute_bound_matches() {
    let bound = zero_bound();
    let tiny = [
        f32::from_bits(1),
        (T1.absolute() * 0.5) as f32,
        bound.next_down(),
    ];
    for t in tiny {
        // 入力が想定どおりか（零でない有限で、上限未満）を先に確かめる。
        assert!(t != 0.0 && t.is_finite());
        assert!(inside_absolute_bound(t));
    }
    // 参照の零は CPU 参照の零と直書きの -0.0・+0.0 の両方。
    let cpu = reference_gemm_f32(&[3.0, -3.0], &[0.5, 0.5], 1, 2, 1).unwrap();
    assert!(cpu[0] == 0.0);
    let reference = vec![cpu[0], -0.0, 0.0, 5.0];
    for sign in [1.0_f32, -1.0] {
        let actual = vec![sign * tiny[0], sign * tiny[1], sign * tiny[2], 5.0];
        let plain = compare_f32(&reference, &actual, T1).unwrap();
        assert!(plain.is_match());
        assert_eq!(plain.exceeded(), 0);
        assert_eq!(plain.first_mismatch(), None);
        let sv = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
        assert!(sv.is_match());
        assert_eq!(sv.exceeded(), 0);
        assert_eq!(sv.first_mismatch(), None);
    }
}

#[test]
fn zero_reference_with_finite_outside_absolute_bound_is_a_mismatch() {
    let bound = zero_bound();
    let reference = vec![1.0, 0.0, -0.0, 2.0, 0.0, -0.0, 0.0];
    let actual = vec![
        1.0,
        bound,            // 上限ちょうど（厳密な `<` なので外）
        -bound.next_up(), // その次の値
        2.0,
        bound.next_down(), // 内
        bound * 8.0,       // 上限の数倍
        -0.0,
    ];
    // 期待は f64 の厳密な `<` から導く（上限ちょうどの内外を直書きで仮定しない）。
    let expected: Vec<usize> = (0..reference.len())
        .filter(|&i| reference[i] == 0.0 && !inside_absolute_bound(actual[i]))
        .collect();
    assert!(expected.contains(&5));
    assert!(expected.len() >= 2);
    assert_eq!(exceeded_positions(&reference, &actual), expected);

    let plain = compare_f32(&reference, &actual, T1).unwrap();
    assert!(!plain.is_match());
    assert_eq!(plain.exceeded(), expected.len());
    assert_eq!(plain.first_mismatch(), expected.first().copied());
    let sv = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert!(!sv.is_match());
    assert_eq!(sv.exceeded(), expected.len());
    assert_eq!(sv.first_mismatch(), expected.first().copied());
}

#[test]
fn relative_error_is_not_used_for_zero_reference() {
    // 絶対誤差の上限以上で、零でない参照には相対誤差だけで内に入る大きさの誤差。
    let d = (T1.absolute() * 4.0) as f32;
    assert!(f64::from(d) >= T1.absolute());
    let r = (f64::from(d) / (T1.relative() * 0.5)) as f32;
    for sign in [1.0_f32, -1.0] {
        let delta = sign * d;
        // 零でない参照では内（相対誤差だけで内）。
        assert!(f64::from((r + delta) - r).abs() >= T1.absolute());
        assert_eq!(judge_element(r, r + delta, T1), ElementVerdict::Within);
        // 零の参照では、同じ大きさの誤差は外。
        for z in [0.0_f32, -0.0] {
            assert_eq!(judge_element(z, delta, T1), ElementVerdict::Exceeded);
        }
    }
    // 1 本の列にまとめる。外れは零の参照の位置だけ。
    let reference = vec![r, 0.0, r, -0.0, r, 0.0];
    let actual = vec![r + d, d, r - d, -d, r + d, -d];
    let plain = compare_f32(&reference, &actual, T1).unwrap();
    assert!(!plain.is_match());
    assert_eq!(plain.exceeded(), 3);
    assert_eq!(plain.first_mismatch(), Some(1));
    let sv = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert_eq!(sv.exceeded(), 3);
    assert_eq!(sv.first_mismatch(), Some(1));
}

#[test]
fn zero_reference_rule_does_not_hide_non_finite_mismatch() {
    let tiny = f32::from_bits(1);
    let reference = vec![0.0_f32, -0.0, 0.0, -0.0, 0.0];
    let actual = vec![tiny, f32::NAN, -tiny, f32::INFINITY, -0.0];
    let positions = non_finite_mismatch_positions(&reference, &actual);
    assert_eq!(positions, vec![1, 3]);

    let plain = compare_f32(&reference, &actual, T1).unwrap();
    assert!(!plain.is_match());
    assert_eq!(plain.non_finite(), 2);
    assert_eq!(plain.exceeded(), 0);
    assert_eq!(plain.first_mismatch(), Some(1));
    let sv = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert!(!sv.is_match());
    assert_eq!(sv.non_finite_mismatch(), 2);
    assert_eq!(sv.exceeded(), 0);
    assert_eq!(sv.first_mismatch(), Some(1));
}

#[test]
fn elementwise_like_zeros_are_judged_by_absolute_bound() {
    // ReLU 相当の出力（負の入力が零になる）を直書きで作る。
    let input = [-2.0_f32, 1.5, -0.5, 3.0, -1.0];
    let reference: Vec<f32> = input.iter().map(|&x| x.max(0.0)).collect();
    let zeros: Vec<usize> = (0..reference.len())
        .filter(|&i| reference[i] == 0.0)
        .collect();
    assert_eq!(zeros, vec![0, 2, 4]);

    let bound = zero_bound();
    let mut actual = reference.clone();
    actual[0] = f32::from_bits(1);
    actual[2] = -bound.next_down();
    let ok = compare_f32(&reference, &actual, T1).unwrap();
    assert!(ok.is_match());

    // 上限以上の値を入れた位置だけが外れ。
    actual[4] = bound * 4.0;
    assert!(!inside_absolute_bound(actual[4]));
    let plain = compare_f32(&reference, &actual, T1).unwrap();
    assert!(!plain.is_match());
    assert_eq!(plain.exceeded(), 1);
    assert_eq!(plain.first_mismatch(), Some(4));
    let sv = compare_f32_with_special_values(&reference, &actual, T1).unwrap();
    assert_eq!(sv.exceeded(), 1);
    assert_eq!(sv.first_mismatch(), Some(4));
}
