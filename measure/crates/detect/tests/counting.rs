//! 公開のパスだけで、分類からセルのラベルまでを境界の入力で確かめる試験。
//!
//! 値は合成のものだけ（TASK-1082 のポインタ）。

use measure_detect::DetectError;
use measure_detect::classify::{Means, MutantClass, MutantInput, Pair, PairResult, classify};
use measure_detect::rate::{
    DETECTION_THRESHOLD, Fraction, MIN_CELL_DENOMINATOR, REASON_BETWEEN_BOUNDS,
    REASON_FEWER_THAN_MIN, REASON_NOT_CONNECTED, RateLabel, judge_cell,
};

fn f(n: u64, d: u64) -> Fraction {
    Fraction {
        numerator: n,
        denominator: d,
    }
}

fn many(c: MutantClass, n: usize) -> Vec<MutantClass> {
    vec![c; n]
}

fn nr(r: &str) -> MutantClass {
    MutantClass::NotRun {
        reason: r.to_string(),
    }
}

fn pair(means: Means, group: &str) -> Pair {
    Pair {
        means,
        group: group.to_string(),
    }
}

fn ran(results: &[(Pair, PairResult)]) -> MutantInput {
    MutantInput::Ran {
        counted: results.iter().map(|(p, _)| p.clone()).collect(),
        results: results.to_vec(),
    }
}

fn mix(detected: usize, missed: usize, indeterminate: usize) -> Vec<MutantClass> {
    let mut v = many(MutantClass::Detected, detected);
    v.extend(many(MutantClass::Missed, missed));
    v.extend(many(MutantClass::Indeterminate, indeterminate));
    v
}

#[test]
fn ten_with_nine_detected_is_ok() {
    // spec: MAINT-27（ポインタのみ）
    let r = judge_cell(&mix(9, 1, 0), true).unwrap();
    let b = r.bounds.unwrap();
    assert_eq!((b.lower, b.upper), (f(9, 10), f(9, 10)));
    assert_eq!(r.label, RateLabel::Ok);
    assert_eq!((r.detected, r.missed, r.indeterminate), (9, 1, 0));
}

#[test]
fn eight_detected_two_indeterminate_is_indeterminate() {
    // spec: MAINT-27（ポインタのみ）
    let r = judge_cell(&mix(8, 0, 2), true).unwrap();
    let b = r.bounds.unwrap();
    assert_eq!((b.lower, b.upper), (f(8, 10), f(10, 10)));
    assert_eq!(
        r.label,
        RateLabel::Indeterminate {
            reason: REASON_BETWEEN_BOUNDS
        }
    );
}

#[test]
fn two_or_more_missed_is_ng() {
    // spec: MAINT-27（ポインタのみ）
    for (missed, upper) in [(2usize, 8u64), (3, 7), (10, 0)] {
        let r = judge_cell(&mix(10 - missed, missed, 0), true).unwrap();
        let b = r.bounds.unwrap();
        assert_eq!(b.lower, f(upper, 10));
        assert_eq!(b.upper, f(upper, 10));
        assert_eq!(r.label, RateLabel::Ng, "missed={missed}");
    }
}

#[test]
fn fewer_than_ten_is_indeterminate() {
    // spec: MAINT-27（ポインタのみ）
    let r = judge_cell(&mix(9, 0, 0), true).unwrap();
    assert_eq!(
        r.label,
        RateLabel::Indeterminate {
            reason: REASON_FEWER_THAN_MIN
        }
    );
    assert_eq!(r.bounds.unwrap().lower, f(9, 9));

    let mut v = mix(9, 1, 0);
    v.push(MutantClass::TypeRejected);
    v.push(MutantClass::Equivalent);
    v.push(nr("r"));
    let r = judge_cell(&v, true).unwrap();
    assert_eq!(r.bounds.unwrap().lower, f(9, 10));
    assert_eq!(r.label, RateLabel::Ok);
}

#[test]
fn not_connected_is_indeterminate() {
    // spec: MAINT-27（ポインタのみ）
    let r = judge_cell(&mix(10, 0, 0), false).unwrap();
    assert_eq!(
        r.label,
        RateLabel::Indeterminate {
            reason: REASON_NOT_CONNECTED
        }
    );
    assert!(r.bounds.is_some());
}

#[test]
fn excluded_classes_are_counted_only() {
    // spec: MAINT-26（ポインタのみ）
    let mut v = mix(10, 0, 0);
    v.extend(many(MutantClass::TypeRejected, 2));
    v.push(MutantClass::Equivalent);
    v.extend([nr("b"), nr("a"), nr("b")]);
    let r = judge_cell(&v, true).unwrap();
    assert_eq!((r.type_rejected, r.equivalent), (2, 1));
    assert_eq!(r.not_run, vec![("a".to_string(), 1), ("b".to_string(), 2)]);
    assert_eq!(r.bounds.unwrap().lower, f(10, 10));
}

#[test]
fn classify_then_judge() {
    // spec: MAINT-26（ポインタのみ）
    use PairResult::{Negative as Ng, NotCompleted as Nc, Positive as Po};
    let (a, b, c) = (
        pair(Means::D1, "g1"),
        pair(Means::D2, "g1"),
        pair(Means::D1, "g2"),
    );
    let detected = |i: usize| match i % 3 {
        0 => ran(&[(a.clone(), Po), (b.clone(), Ng)]),
        1 => ran(&[(a.clone(), Ng), (b.clone(), Po)]),
        _ => ran(&[(a.clone(), Po), (b.clone(), Nc), (c.clone(), Ng)]),
    };
    let missed = ran(&[(a.clone(), Ng), (b.clone(), Ng)]);
    let unknown = |i: usize| {
        if i == 0 {
            ran(&[(a.clone(), Ng), (b.clone(), Nc)])
        } else {
            ran(&[(a.clone(), Nc), (b.clone(), Nc), (c.clone(), Ng)])
        }
    };

    let mut inputs: Vec<MutantInput> = (0..9).map(detected).collect();
    inputs.push(missed);
    let classes: Vec<MutantClass> = inputs.iter().map(|i| classify(i).unwrap()).collect();
    let r = judge_cell(&classes, true).unwrap();
    assert_eq!((r.detected, r.missed, r.indeterminate), (9, 1, 0));
    assert_eq!(r.label, RateLabel::Ok);

    let mut inputs: Vec<MutantInput> = (0..8).map(detected).collect();
    inputs.extend((0..2).map(unknown));
    let classes: Vec<MutantClass> = inputs.iter().map(|i| classify(i).unwrap()).collect();
    let r = judge_cell(&classes, true).unwrap();
    assert_eq!((r.detected, r.missed, r.indeterminate), (8, 0, 2));
    assert_eq!(
        r.label,
        RateLabel::Indeterminate {
            reason: REASON_BETWEEN_BOUNDS
        }
    );
}

#[test]
fn classify_rejects_inconsistent_pairs() {
    // spec: MAINT-26（ポインタのみ）
    use PairResult::Negative as Ng;
    let a = pair(Means::D1, "g1");
    let b = pair(Means::D2, "g1");
    let input = |counted: Vec<Pair>, results: Vec<(Pair, PairResult)>| MutantInput::Ran {
        counted,
        results,
    };
    assert_eq!(
        classify(&input(vec![], vec![])),
        Err(DetectError::EmptyCounted)
    );
    assert_eq!(
        classify(&input(vec![a.clone(), a.clone()], vec![(a.clone(), Ng)])),
        Err(DetectError::DuplicateCounted)
    );
    assert_eq!(
        classify(&input(vec![a.clone(), b.clone()], vec![(a.clone(), Ng)])),
        Err(DetectError::MissingResult)
    );
    assert_eq!(
        classify(&input(
            vec![a.clone()],
            vec![(a.clone(), Ng), (b.clone(), Ng)]
        )),
        Err(DetectError::UnexpectedResult)
    );
    assert_eq!(
        classify(&input(
            vec![a.clone()],
            vec![(a.clone(), Ng), (a.clone(), Ng)]
        )),
        Err(DetectError::DuplicateResult)
    );
}

#[test]
fn threshold_constants() {
    assert_eq!(DETECTION_THRESHOLD, f(9, 10));
    assert_eq!(MIN_CELL_DENOMINATOR, 10);
}
