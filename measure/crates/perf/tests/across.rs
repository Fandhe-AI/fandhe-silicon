//! across の公開テスト（run 間の中央値・変動帯・0 のときの比・不正入力）。
use measure_perf::PerfError;
use measure_perf::across::{across_run_stats, across_runs};
use measure_perf::stats::run_stats;

#[test]
fn odd_and_even_medians() {
    assert_eq!(across_runs(&[3.0, 1.0, 2.0]).unwrap().median, 2.0);
    assert_eq!(across_runs(&[4.0, 1.0, 3.0, 2.0]).unwrap().median, 2.5);
}

#[test]
fn spreads() {
    let a = across_runs(&[10.0, 40.0, 20.0]).unwrap();
    assert_eq!(a.spread_abs, 30.0);
    assert_eq!(a.spread_ratio, Some(4.0));
}

#[test]
fn zero_min_has_no_ratio() {
    let a = across_runs(&[0.0, 5.0]).unwrap();
    assert_eq!(a.spread_abs, 5.0);
    assert_eq!(a.spread_ratio, None);
    let z = across_runs(&[0.0, 0.0]).unwrap();
    assert_eq!(z.spread_ratio, None);
}

#[test]
fn rejects_invalid() {
    assert_eq!(across_runs(&[]), Err(PerfError::Empty));
    assert_eq!(across_runs(&[1.0, f64::NAN]), Err(PerfError::InvalidValue));
    assert_eq!(across_runs(&[f64::INFINITY]), Err(PerfError::InvalidValue));
    assert_eq!(across_runs(&[-1.0]), Err(PerfError::InvalidValue));
}

#[test]
fn from_run_stats() {
    let r1 = run_stats(&[1, 2, 3]).unwrap();
    let r2 = run_stats(&[10, 20, 30]).unwrap();
    let a = across_run_stats(&[r1, r2]).unwrap();
    assert_eq!(a.median, 11.0);
    assert_eq!(a.spread_abs, 18.0);
}
