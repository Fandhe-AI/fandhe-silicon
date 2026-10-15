//! ratio の公開テスト（閾値・適用範囲・区分の綴り・入力の検査・中央値の受け渡し）。
use measure_perf::PerfError;
use measure_perf::across::across_runs;
use measure_perf::ratio::*;
use measure_perf::record::RefId;
use measure_perf::stats::{MAX_EXACT_NS, run_stats};

fn task(s: &str) -> RefId {
    RefId::parse(s).unwrap()
}

#[test]
fn threshold_constants() {
    assert_eq!(diff_ratio(), (13, 10));
    assert_eq!(diff_ratio_f64(), 1.3);
}

#[test]
fn scope_entries_are_valid_ids() {
    for t in RATIO_SCOPE_TASKS {
        assert!(RefId::parse(t).is_ok(), "{t}");
    }
}

#[test]
fn out_of_scope_is_pending() {
    for (n, d) in [(100.0, 1.0), (1.0, 100.0), (13.0, 10.0)] {
        let c = classify(n, d, &task("TASK-27")).unwrap();
        assert_eq!(c, RatioClass::Pending);
        assert_eq!(c.reason_name(), Some("out-of-scope"));
    }
}

#[test]
fn tokens() {
    let all = [
        RatioClass::Equivalent,
        RatioClass::Differs,
        RatioClass::Pending,
    ];
    for (i, a) in all.iter().enumerate() {
        assert!(a.name().is_ascii());
        for b in &all[i + 1..] {
            assert_ne!(a.name(), b.name());
        }
    }
    assert_eq!(RatioClass::Equivalent.reason_name(), None);
    assert_eq!(RatioClass::Differs.reason_name(), None);
}

#[test]
fn invalid_inputs_err_even_out_of_scope() {
    let t = task("TASK-27");
    assert_eq!(classify(f64::NAN, 1.0, &t), Err(PerfError::InvalidValue));
    assert_eq!(
        classify(f64::INFINITY, 1.0, &t),
        Err(PerfError::InvalidValue)
    );
    assert_eq!(classify(-1.0, 1.0, &t), Err(PerfError::InvalidValue));
    assert_eq!(classify(1.0, 0.0, &t), Err(PerfError::ZeroDenominator));
    assert_eq!(
        classify(MAX_EXACT_NS as f64 * 2.0, 1.0, &t),
        Err(PerfError::ValueTooLarge)
    );
    assert_eq!(classify(1.1, 1.0, &t), Err(PerfError::InvalidValue));
}

#[test]
fn accepts_real_medians() {
    let t = task("TASK-27");
    let r = run_stats(&[1, 2, 3, 4]).unwrap();
    assert_eq!(r.median, 2.5);
    let a = across_runs(&[2.5, 3.0]).unwrap();
    assert_eq!(a.median, 2.75);
    assert!(classify(r.median, a.median, &t).is_ok());
}
