//! 時刻の分解能の純粋な関数と、CPU の時計の測定・出力の形の試験（TASK-2950 のポインタ）。device に触れない。

use measure_clock_resolution::{
    CPU_READINGS, Config, ResolutionError, adjacent_differences, cpu_resolution,
    distinct_differences, histogram, min_distinct_difference, min_nonzero, parse_args,
    read_cpu_clock, run, seconds_span_to_ns, summary_json,
};
use measure_perf::record::{Id, SessionRef, UtcTime};

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn adjacent_differences_are_the_gaps_between_readings() {
    assert_eq!(
        adjacent_differences(&[0, 0, 41, 83, 83]),
        Ok(vec![0, 41, 42, 0])
    );
    assert_eq!(adjacent_differences(&[5]), Err(ResolutionError::Empty));
    assert_eq!(adjacent_differences(&[]), Err(ResolutionError::Empty));
    assert_eq!(
        adjacent_differences(&[10, 9]),
        Err(ResolutionError::InvalidTime)
    );
}

#[test]
fn min_nonzero_skips_zero_and_rejects_all_zero() {
    assert_eq!(min_nonzero(&[0, 42, 41, 0, 83]), Ok(41));
    assert_eq!(
        min_nonzero(&[0, 0]),
        Err(ResolutionError::NoNonzeroDifference)
    );
    assert_eq!(min_nonzero(&[]), Err(ResolutionError::Empty));
}

#[test]
fn histogram_counts_each_value_in_ascending_order() {
    let h = histogram(&[41, 0, 41, 42, 0, 0]);
    assert_eq!(
        h.into_iter().collect::<Vec<_>>(),
        vec![(0, 3), (41, 2), (42, 1)]
    );
}

#[test]
fn distinct_differences_use_sorted_distinct_values() {
    assert_eq!(
        distinct_differences(&[125, 42, 125, 0, 83]),
        vec![42, 41, 42]
    );
    assert_eq!(min_distinct_difference(&[125, 42, 125, 0, 83]), Ok(41));
    assert_eq!(
        min_distinct_difference(&[7, 7, 7]),
        Err(ResolutionError::NoDistinctPair)
    );
    assert_eq!(min_distinct_difference(&[]), Err(ResolutionError::Empty));
}

#[test]
fn seconds_span_rounds_half_away_from_zero() {
    assert_eq!(seconds_span_to_ns(1.0, 1.0), Ok(0));
    assert_eq!(seconds_span_to_ns(2.0, 2.0 + 0.5e-9), Ok(1));
    assert_eq!(seconds_span_to_ns(1.0, 1.000_001), Ok(1000));
    assert_eq!(seconds_span_to_ns(2.5e-9, 5e-9), Ok(3));
    assert_eq!(
        seconds_span_to_ns(0.0, 1.0),
        Err(ResolutionError::InvalidTime)
    );
    assert_eq!(
        seconds_span_to_ns(2.0, 1.0),
        Err(ResolutionError::InvalidTime)
    );
    assert_eq!(
        seconds_span_to_ns(f64::NAN, 1.0),
        Err(ResolutionError::InvalidTime)
    );
    assert_eq!(
        seconds_span_to_ns(1.0, f64::INFINITY),
        Err(ResolutionError::InvalidTime)
    );
    assert_eq!(seconds_span_to_ns(1.0, 3.0), Ok(2_000_000_000));
    assert_eq!(
        seconds_span_to_ns(1.0, 1.0e11),
        Err(ResolutionError::Overflow)
    );
}

#[test]
fn cpu_resolution_reports_min_nonzero_and_distribution() {
    let r = cpu_resolution(&[0, 0, 41, 83, 83, 125]).unwrap();
    assert_eq!(r.readings, 6);
    assert_eq!(r.resolution_ns, 41);
    assert_eq!(
        r.differences.into_iter().collect::<Vec<_>>(),
        vec![(0, 2), (41, 1), (42, 2)]
    );
    assert_eq!(
        cpu_resolution(&[3, 3, 3]),
        Err(ResolutionError::NoNonzeroDifference)
    );
}

#[test]
fn reading_the_cpu_clock_is_monotonic_and_counted() {
    let v = read_cpu_clock(1000).unwrap();
    assert_eq!(v.len(), 1000);
    assert_eq!(v[0], 0);
    assert!(v.windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(read_cpu_clock(1), Err(ResolutionError::Empty));
    assert_eq!(CPU_READINGS, 1_000_000);
}

#[test]
fn parse_args_requires_a_run_id_and_rejects_unknown_or_duplicate() {
    let c = parse_args(&args(&["--run-id", "r1", "--session-ref", "s-1"])).unwrap();
    assert_eq!(c.run_id.as_str(), "r1");
    assert_eq!(c.session_ref.as_str(), "s-1");
    assert_eq!(c.repo_commit, None);
    assert_eq!(
        parse_args(&args(&[])),
        Err(ResolutionError::Usage("--run-id is required"))
    );
    assert_eq!(
        parse_args(&args(&["--x", "1"])),
        Err(ResolutionError::Usage("unknown argument"))
    );
    assert_eq!(
        parse_args(&args(&["--run-id", "a", "--run-id", "b"])),
        Err(ResolutionError::Usage("duplicate argument"))
    );
    assert_eq!(
        parse_args(&args(&["--run-id"])),
        Err(ResolutionError::Usage("missing value"))
    );
    assert_eq!(
        parse_args(&args(&["--run-id", "A!"])),
        Err(ResolutionError::Usage("invalid value"))
    );
}

#[test]
fn summary_json_has_fixed_fields_and_order() {
    let config = Config {
        run_id: Id::parse("r1").unwrap(),
        session_ref: SessionRef::Session(Id::parse("s-1").unwrap()),
        repo_commit: None,
    };
    let t = UtcTime::parse("2026-11-20T00:00:00Z").unwrap();
    let r = cpu_resolution(&[0, 41, 41]).unwrap();
    assert_eq!(
        summary_json(&config, &t, &t, "cpu-instant", &r),
        "{\"task_id\":\"TASK-2950\",\"run_id\":\"r1\",\"session_ref\":\"s-1\",\"started_utc\":\"2026-11-20T00:00:00Z\",\"ended_utc\":\"2026-11-20T00:00:00Z\",\"repo_commit\":null,\"clock\":\"cpu-instant\",\"readings\":3,\"resolution_ns\":41,\"differences\":[[0,1],[41,1]]}"
    );
}

#[test]
fn run_writes_one_line_with_the_full_reading_count() {
    let config = parse_args(&args(&["--run-id", "r1"])).unwrap();
    let mut out = Vec::new();
    run(&config, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.ends_with('\n'));
    assert!(text.contains("\"readings\":1000000,"));
    assert!(text.contains("\"session_ref\":\"_none\""));
}

#[test]
fn reading_an_unallocatable_count_returns_an_error_instead_of_panicking() {
    assert!(read_cpu_clock(usize::MAX).is_err());
}

#[test]
fn wall_clock_going_backwards_is_invalid_time() {
    use measure_clock_resolution::check_time_order;
    assert_eq!(check_time_order(10, 10), Ok(()));
    assert_eq!(check_time_order(10, 11), Ok(()));
    assert_eq!(check_time_order(11, 10), Err(ResolutionError::InvalidTime));
}

#[cfg(unix)]
#[test]
fn bin_rejects_a_non_utf8_argument_with_exit_code_2() {
    use std::os::unix::ffi::OsStrExt;
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_clock-resolution"))
        .arg(std::ffi::OsStr::from_bytes(&[0x66, 0xff, 0x6f]))
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}
