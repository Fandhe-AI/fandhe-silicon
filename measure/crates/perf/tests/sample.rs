//! sample の公開テスト（呼ぶ回数・GPU 時間の取り扱い・失敗の伝え方）。
use measure_perf::PerfError;
use measure_perf::sample::{CollectError, SampleCounts, collect, duration_to_ns};
use std::time::Duration;

#[test]
fn calls_closure_per_phase() {
    let mut calls = 0usize;
    let counts = SampleCounts {
        cold: 2,
        warmup: 3,
        measured: 5,
    };
    let s = collect::<()>(counts, || {
        calls += 1;
        Ok(Some(calls as u64))
    })
    .unwrap();
    assert_eq!(calls, 10);
    assert_eq!((s.cold.len(), s.warmup.len(), s.measured.len()), (2, 3, 5));
    assert_eq!(s.cold[0].gpu_ns, Some(1));
    assert_eq!(s.measured[4].gpu_ns, Some(10));
    assert_eq!(s.measured_cpu_ns().len(), 5);
}

#[test]
fn gpu_none_is_not_zero() {
    let counts = SampleCounts {
        cold: 0,
        warmup: 0,
        measured: 2,
    };
    let s = collect::<()>(counts, || Ok(None)).unwrap();
    assert!(s.measured.iter().all(|x| x.gpu_ns.is_none()));
    assert_eq!(s.measured_gpu_ns(), None);
}

#[test]
fn zero_measured_is_error() {
    let counts = SampleCounts {
        cold: 1,
        warmup: 1,
        measured: 0,
    };
    let r = collect::<()>(counts, || Ok(None));
    assert_eq!(r, Err(CollectError::Perf(PerfError::InvalidCount)));
}

#[test]
fn submit_error_is_returned() {
    let counts = SampleCounts {
        cold: 0,
        warmup: 0,
        measured: 3,
    };
    let mut n = 0;
    let r = collect(counts, || {
        n += 1;
        if n == 2 { Err("boom") } else { Ok(None) }
    });
    assert_eq!(r, Err(CollectError::Submit("boom")));
}

#[test]
fn duration_conversion() {
    assert_eq!(duration_to_ns(Duration::from_nanos(1500)), Ok(1500));
    assert_eq!(
        duration_to_ns(Duration::from_secs(u64::MAX)),
        Err(PerfError::DurationOverflow)
    );
}
