//! stats の公開テスト（添字の式・丸め・偶数の中央値・上限・空）。
use measure_perf::PerfError;
use measure_perf::stats::{MAX_EXACT_NS, run_stats};

#[test]
fn single_value() {
    let s = run_stats(&[7]).unwrap();
    assert_eq!((s.q1, s.q3, s.p95, s.min, s.max), (7, 7, 7, 7, 7));
    assert_eq!(s.median, 7.0);
}

#[test]
fn two_values_average_and_indices() {
    let s = run_stats(&[2, 1]).unwrap();
    assert_eq!(s.median, 1.5);
    assert_eq!((s.q1, s.q3, s.p95), (1, 2, 2));
    assert_eq!((s.min, s.max), (1, 2));
}

#[test]
fn three_values_round_half_away_from_zero() {
    // 添字: q1=round(0.5)=1, q3=round(1.5)=2, p95=round(1.9)=2
    let s = run_stats(&[30, 10, 20]).unwrap();
    assert_eq!(s.median, 20.0);
    assert_eq!((s.q1, s.q3, s.p95), (20, 30, 30));
}

#[test]
fn no_interpolation_on_unsorted_input() {
    // n=5: q1=round(1.0)=1, q3=round(3.0)=3, p95=round(3.8)=4
    let input = [50, 10, 40, 20, 30];
    let s = run_stats(&input).unwrap();
    assert_eq!((s.q1, s.q3, s.p95), (20, 40, 50));
    assert_eq!(s.median, 30.0);
    assert_eq!(input, [50, 10, 40, 20, 30]);
    // n=4: q1=round(0.75)=1, q3=round(2.25)=2, p95=round(2.85)=3
    let s = run_stats(&[40, 10, 30, 20]).unwrap();
    assert_eq!((s.q1, s.q3, s.p95), (20, 30, 40));
    assert_eq!(s.median, 25.0);
}

#[test]
fn twenty_values() {
    // n=20: q1=round(4.75)=5, q3=round(14.25)=14, p95=round(18.05)=18
    let input: Vec<u64> = (0..20).rev().map(|v| v * 10).collect();
    let s = run_stats(&input).unwrap();
    assert_eq!((s.q1, s.q3, s.p95), (50, 140, 180));
    assert_eq!(s.median, 95.0);
}

#[test]
fn large_pair_does_not_overflow() {
    let a = MAX_EXACT_NS;
    let s = run_stats(&[a, a - 1]).unwrap();
    assert_eq!(s.median, (a - 1) as f64 + 0.5);
}

#[test]
fn rejects_too_large_and_empty() {
    assert_eq!(
        run_stats(&[MAX_EXACT_NS + 1]),
        Err(PerfError::ValueTooLarge)
    );
    assert_eq!(run_stats(&[]), Err(PerfError::Empty));
}
