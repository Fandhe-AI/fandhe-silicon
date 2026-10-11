//! 有限でない値と符号付きの零を扱う比べ方の振る舞いの確認（spec TASK-25 の 2/4・3/4）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。入力はこの場で作る。
//! 3/4 では、同じビット列・種類の組み合わせ・非正規化数・NaN のビット列の幅・閾値の getter・
//! 既存の `compare_f32` との関係・CPU 参照の出力との接続を固定した（期待値は公開コードから書く）。

use fandhe_silicon_core::parity::cpu_reference::reference_gemm_f32;
use fandhe_silicon_core::parity::special_values::{
    SpecialVerdict, compare_f32_with_special_values, judge_element_with_special_values,
};
use fandhe_silicon_core::parity::tolerance::{
    CompareError, ElementVerdict, T1, compare_f32, judge_element,
};

use SpecialVerdict::{BothNan, Exceeded, NonFiniteMismatch, SameInfinity, Within};

fn j(r: f32, a: f32) -> SpecialVerdict {
    judge_element_with_special_values(r, a, T1)
}

fn nans() -> [f32; 3] {
    [f32::NAN, -f32::NAN, f32::from_bits(f32::NAN.to_bits() | 1)]
}

#[test]
fn nan_pairs_match_regardless_of_payload_and_sign() {
    for r in nans() {
        for a in nans() {
            assert_eq!(j(r, a), BothNan);
        }
    }
}

#[test]
fn nan_reference_against_non_nan_is_mismatch() {
    for a in [0.0, 1.0, f32::MAX, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(j(f32::NAN, a), NonFiniteMismatch);
    }
}

#[test]
fn infinities_match_only_with_same_sign() {
    assert_eq!(j(f32::INFINITY, f32::INFINITY), SameInfinity);
    assert_eq!(j(f32::NEG_INFINITY, f32::NEG_INFINITY), SameInfinity);
    assert_eq!(j(f32::INFINITY, f32::NEG_INFINITY), NonFiniteMismatch);
    assert_eq!(j(f32::NEG_INFINITY, f32::INFINITY), NonFiniteMismatch);
    assert_eq!(j(f32::INFINITY, f32::NAN), NonFiniteMismatch);
    assert_eq!(j(f32::INFINITY, f32::MAX), NonFiniteMismatch);
    assert_eq!(j(f32::NEG_INFINITY, f32::MIN), NonFiniteMismatch);
}

#[test]
fn finite_reference_against_non_finite_actual_is_mismatch() {
    for r in [0.0, -0.0, 1.0, -1.0, f32::MAX] {
        for a in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(j(r, a), NonFiniteMismatch);
        }
    }
}

#[test]
fn signed_zeros_match_each_other() {
    assert_eq!(j(0.0, -0.0), Within);
    assert_eq!(j(-0.0, 0.0), Within);
    assert_eq!(j(0.0, 0.0), Within);
    assert_eq!(j(-0.0, -0.0), Within);
}

#[test]
fn zero_reference_switches_at_absolute_boundary() {
    let b = T1.absolute() as f32;
    assert_eq!(j(0.0, b * 0.5), Within);
    assert_eq!(j(-0.0, -b * 0.5), Within);
    assert_eq!(j(0.0, b * 2.0), Exceeded);
    assert_eq!(j(-0.0, -b * 2.0), Exceeded);
}

#[test]
fn finite_pairs_follow_judge_element() {
    // 相対誤差がちょうど上限になる組は外（厳密な `<`）。
    assert_eq!(j(1000.0, 1001.0), Exceeded);
    assert_eq!(j(1000.0, 1000.5), Within);
    let values = [
        0.0,
        -0.0,
        1e-7,
        -1e-7,
        1e-4,
        1.0,
        -1.0,
        1.0005,
        1.002,
        1000.0,
        1000.5,
        1001.0,
        f32::MAX,
        f32::MIN_POSITIVE,
    ];
    for &r in &values {
        for &a in &values {
            let expected = match judge_element(r, a, T1) {
                ElementVerdict::Within => Within,
                ElementVerdict::Exceeded => Exceeded,
                ElementVerdict::NonFinite => NonFiniteMismatch,
            };
            assert_ne!(expected, NonFiniteMismatch, "both finite: r={r} a={a}");
            assert_eq!(j(r, a), expected, "r={r} a={a}");
        }
    }
}

#[test]
fn is_match_covers_only_match_variants() {
    assert!(BothNan.is_match());
    assert!(SameInfinity.is_match());
    assert!(Within.is_match());
    assert!(!Exceeded.is_match());
    assert!(!NonFiniteMismatch.is_match());
}

#[test]
fn existing_judge_element_is_unchanged() {
    assert_eq!(
        judge_element(f32::NAN, f32::NAN, T1),
        ElementVerdict::NonFinite
    );
    assert_eq!(
        judge_element(f32::INFINITY, f32::INFINITY, T1),
        ElementVerdict::NonFinite
    );
}

#[test]
fn sequence_summary_counts_each_class() {
    let r = [1.0, f32::NAN, f32::INFINITY, 1.0, 0.0, f32::INFINITY, 0.0];
    let a = [
        1.0,
        -f32::NAN,
        f32::INFINITY,
        2.0,
        -0.0,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    let c = compare_f32_with_special_values(&r, &a, T1).expect("same length, non-empty");
    assert_eq!(c.compared(), 7);
    assert_eq!(c.exceeded(), 1);
    assert_eq!(c.non_finite_mismatch(), 2);
    assert_eq!(c.first_mismatch(), Some(3));
    assert!(!c.is_match());
}

#[test]
fn sequence_with_only_matching_special_values_is_match() {
    let r = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0];
    let a = [-f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.0];
    let c = compare_f32_with_special_values(&r, &a, T1).expect("same length, non-empty");
    assert!(c.is_match());
    assert_eq!(c.first_mismatch(), None);
}

#[test]
fn sequence_errors() {
    assert_eq!(
        compare_f32_with_special_values(&[1.0], &[1.0, 2.0], T1),
        Err(CompareError::LengthMismatch {
            reference: 1,
            actual: 2
        })
    );
    assert_eq!(
        compare_f32_with_special_values(&[], &[1.0], T1),
        Err(CompareError::LengthMismatch {
            reference: 0,
            actual: 1
        })
    );
    assert_eq!(
        compare_f32_with_special_values(&[], &[], T1),
        Err(CompareError::EmptyInput)
    );
}
// ---- 3/4 で追加 ----

#[test]
fn same_bit_pattern_always_matches() {
    let expected = |x: f32| {
        if x.is_nan() {
            BothNan
        } else if x.is_infinite() {
            SameInfinity
        } else {
            Within
        }
    };
    // u32 の全域を粗い刻みで歩く（NaN・無限大・非正規化数・通常の数がすべて混ざる）。
    let mut bits: u32 = 0;
    loop {
        let x = f32::from_bits(bits);
        assert_eq!(j(x, x), expected(x), "bits={bits:#010x}");
        match bits.checked_add(65_537) {
            Some(next) => bits = next,
            None => break,
        }
    }
    let edges = [
        0x0000_0000,
        0x8000_0000,
        0x0000_0001,
        0x8000_0001,
        f32::MIN_POSITIVE.to_bits(),
        f32::MAX.to_bits(),
        f32::MIN.to_bits(),
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        0x7fc0_0000,
        0x7f80_0001,
        0xffff_ffff,
    ];
    for bits in edges {
        let x = f32::from_bits(bits);
        assert_eq!(j(x, x), expected(x), "bits={bits:#010x}");
    }
}

#[test]
fn verdict_matrix_over_value_kinds_is_fixed() {
    // 行は参照、列は実測。順序は NaN・+inf・-inf・+0・-0・1.0。
    let kinds = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -0.0, 1.0];
    let m = NonFiniteMismatch;
    let table = [
        [BothNan, m, m, m, m, m],
        [m, SameInfinity, m, m, m, m],
        [m, m, SameInfinity, m, m, m],
        [m, m, m, Within, Within, Exceeded],
        [m, m, m, Within, Within, Exceeded],
        [m, m, m, Exceeded, Exceeded, Within],
    ];
    for (ri, &r) in kinds.iter().enumerate() {
        for (ai, &a) in kinds.iter().enumerate() {
            assert_eq!(j(r, a), table[ri][ai], "ref kind {ri}, actual kind {ai}");
        }
    }
}

#[test]
fn subnormals_are_treated_as_finite() {
    let sub = f32::from_bits(1);
    assert!(sub.is_subnormal());
    for (r, a) in [
        (sub, -sub),
        (sub, 0.0),
        (0.0, sub),
        (-0.0, -sub),
        (sub, sub),
    ] {
        assert_eq!(j(r, a), Within, "r={r:e} a={a:e}");
    }
    for special in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(j(sub, special), NonFiniteMismatch);
        assert_eq!(j(special, sub), NonFiniteMismatch);
    }
}

#[test]
fn every_nan_bit_pattern_counts_as_nan() {
    // quiet・signaling の形・符号つき・payload の最大。結果は `is_nan()` だけに依る。
    let patterns = [
        0x7fc0_0000_u32,
        0x7f80_0001,
        0xffc0_0000,
        0xff80_0001,
        0x7fff_ffff,
        0xffff_ffff,
    ];
    for p in patterns {
        let n = f32::from_bits(p);
        assert!(n.is_nan(), "bits={p:#010x}");
        for q in patterns {
            assert_eq!(j(n, f32::from_bits(q)), BothNan);
        }
        for other in [0.0, 1.0, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(j(n, other), NonFiniteMismatch);
            assert_eq!(j(other, n), NonFiniteMismatch);
        }
    }
}

#[test]
fn zero_actual_sign_does_not_change_the_verdict() {
    // 参照が零でない有限のとき、実測の零の符号は結果を変えない。
    for (r, expected) in [
        (1e-6, Within),
        (-1e-6, Within),
        (1.0, Exceeded),
        (-1.0, Exceeded),
    ] {
        assert_eq!(j(r, 0.0), expected, "r={r}");
        assert_eq!(j(r, -0.0), expected, "r={r}");
    }
}

#[test]
fn readable_absolute_bound_is_applied_for_zero_reference() {
    let abs = T1.absolute() as f32;
    let mut seen = (false, false);
    for reference in [0.0_f32, -0.0] {
        for sign in [1.0_f32, -1.0] {
            for x in [abs.next_down(), abs, abs.next_up()] {
                let x = x * sign;
                let expected = if f64::from(x.abs()) < T1.absolute() {
                    Within
                } else {
                    Exceeded
                };
                assert_eq!(j(reference, x), expected, "ref={reference} x={x:e}");
                seen.0 |= expected == Within;
                seen.1 |= expected == Exceeded;
            }
        }
    }
    assert!(seen.0 && seen.1);
}

#[test]
fn relative_error_is_measured_against_the_reference() {
    // 引数を入れ替えると区分が変わる組。向きはこの関数を通しても同じ。
    assert_eq!(j(1000.0, 1001.0), Exceeded);
    assert_eq!(j(1001.0, 1000.0), Within);
    assert_eq!(j(-1000.0, -1001.0), Exceeded);
    assert_eq!(j(-1001.0, -1000.0), Within);
}

#[test]
fn agrees_with_compare_f32_on_all_finite_sequences() {
    let r = [1.0, 0.0, 1000.0, 1000.0, -5.0, 0.0];
    let a = [1.0, -0.0, 1001.0, 1000.5, -5.5, 1.0];
    let old = compare_f32(&r, &a, T1).unwrap();
    let new = compare_f32_with_special_values(&r, &a, T1).unwrap();
    assert_eq!(old.compared(), new.compared());
    assert_eq!(old.exceeded(), new.exceeded());
    assert_eq!(old.first_mismatch(), new.first_mismatch());
    assert_eq!(old.is_match(), new.is_match());
    assert_eq!(old.non_finite(), 0);
    assert_eq!(new.non_finite_mismatch(), 0);
    assert!(new.exceeded() > 0);
}

#[test]
fn differs_from_compare_f32_only_on_matching_special_pairs() {
    let r = [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        1.0,
        f32::NAN,
        0.0,
    ];
    let a = [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        1.0,
        f32::INFINITY,
    ];
    let old = compare_f32(&r, &a, T1).unwrap();
    assert_eq!(
        (old.non_finite(), old.exceeded(), old.first_mismatch()),
        (6, 0, Some(0))
    );
    let new = compare_f32_with_special_values(&r, &a, T1).unwrap();
    assert_eq!(
        (
            new.non_finite_mismatch(),
            new.exceeded(),
            new.first_mismatch()
        ),
        (3, 0, Some(3))
    );
    assert!(!new.is_match());
}

#[test]
fn summary_agrees_with_per_element_verdicts() {
    let r = [f32::NAN, f32::INFINITY, 1.0, 1.0, 2.0, 0.0];
    let a = [f32::NAN, f32::INFINITY, 1.0, 5.0, f32::NAN, -0.0];
    let verdicts: Vec<SpecialVerdict> = r.iter().zip(&a).map(|(&x, &y)| j(x, y)).collect();
    for v in [BothNan, SameInfinity, Within, Exceeded, NonFiniteMismatch] {
        assert!(verdicts.contains(&v), "missing {v:?}");
    }
    let c = compare_f32_with_special_values(&r, &a, T1).unwrap();
    let count = |v: SpecialVerdict| verdicts.iter().filter(|&&x| x == v).count();
    assert_eq!(c.compared(), verdicts.len());
    assert_eq!(c.exceeded(), count(Exceeded));
    assert_eq!(c.non_finite_mismatch(), count(NonFiniteMismatch));
    assert_eq!(
        c.exceeded() + c.non_finite_mismatch(),
        verdicts.iter().filter(|v| !v.is_match()).count()
    );
    assert_eq!(
        c.first_mismatch(),
        verdicts.iter().position(|v| !v.is_match())
    );
    assert_eq!(c.first_mismatch(), Some(3));
}

#[test]
fn first_mismatch_can_be_a_non_finite_mismatch() {
    let r = [f32::NAN, 1.0, 1.0];
    let a = [f32::NAN, f32::INFINITY, 5.0];
    let c = compare_f32_with_special_values(&r, &a, T1).unwrap();
    assert_eq!(c.first_mismatch(), Some(1));
    assert_eq!((c.non_finite_mismatch(), c.exceeded()), (1, 1));
}

#[test]
fn single_element_sequences() {
    let kinds = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -0.0, 1.0];
    for &r in &kinds {
        for &a in &kinds {
            let v = j(r, a);
            let c = compare_f32_with_special_values(&[r], &[a], T1).unwrap();
            assert_eq!(c.compared(), 1);
            assert_eq!(c.exceeded(), usize::from(v == Exceeded));
            assert_eq!(c.non_finite_mismatch(), usize::from(v == NonFiniteMismatch));
            assert_eq!(
                c.first_mismatch(),
                if v.is_match() { None } else { Some(0) }
            );
            assert_eq!(c.is_match(), v.is_match());
        }
    }
}

#[test]
fn cpu_reference_output_feeds_the_comparison() {
    // 有限だけの入力。
    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let c = reference_gemm_f32(&a, &b, 2, 3, 2).unwrap();
    assert!(
        compare_f32_with_special_values(&c, &c, T1)
            .unwrap()
            .is_match()
    );
    let mut shifted = c.clone();
    shifted[3] += 10.0;
    assert!(
        !compare_f32_with_special_values(&c, &shifted, T1)
            .unwrap()
            .is_match()
    );

    // 有限でない値を含む入力は源としてだけ使い、出力の要素ごとの種類は固定しない。
    let a = [f32::NAN, 1.0, f32::INFINITY, 2.0, 3.0, 4.0];
    let b = [1.0, 1.0, 1.0, 1.0];
    let out = reference_gemm_f32(&a, &b, 3, 2, 2).unwrap();
    let nans = out.iter().filter(|x| x.is_nan()).count();
    let infs = out.iter().filter(|x| x.is_infinite()).count();
    let finites = out.len() - nans - infs;
    assert!(nans >= 1 && infs >= 1);

    let same = compare_f32_with_special_values(&out, &out, T1).unwrap();
    assert!(same.is_match());
    assert!(compare_f32(&out, &out, T1).unwrap().non_finite() > 0);

    // NaN は有限へ、無限大は符号反転、有限は大きくずらす。
    let broken: Vec<f32> = out
        .iter()
        .map(|&x| {
            if x.is_nan() {
                1.0
            } else if x.is_infinite() {
                -x
            } else {
                x + 10.0
            }
        })
        .collect();
    let c = compare_f32_with_special_values(&out, &broken, T1).unwrap();
    assert_eq!(c.non_finite_mismatch(), nans + infs);
    assert_eq!(c.exceeded(), finites);
    assert!(!c.is_match());
}
