//! `capture_one`・`shader_text` の試験（spec TASK-1010 のポインタ）。
//!
//! device には触らない。合成の記録は `f` の中で `log` の macro から出す。
//! 既定の並列の実行で通ること（`capture_one` が process 全体で直列に動くこと）も同じ試験で確かめる。

use std::process::{Command, Stdio};

use baseline_wgpu_gemm_arm::capture::{
    CaptureError, MAX_SHADER_TEXT_BYTES, capture_one, shader_text,
};

const ENTRY: &str = "test_entry";
const CHILD_MARK: &str = "CAPTURE_LOGGER_TAKEN_CHILD";

fn record(body: &str) -> String {
    format!("compiled '{ENTRY}'\n// language: metal\n{body}")
}

#[test]
fn one_record_is_returned_with_the_closure_result() {
    for _ in 0..200 {
        let (r, text) = capture_one(ENTRY, || {
            log::debug!("{}", record("kernel void a() {}"));
            7
        })
        .expect("one record");
        assert_eq!(r, 7);
        assert_eq!(text, "// language: metal\nkernel void a() {}");
    }
}

#[test]
fn records_that_do_not_match_are_not_counted() {
    let err = capture_one(ENTRY, || {
        log::debug!("compiled '{ENTRY}'\nno marker");
        log::debug!("compiled 'other_entry'\n// language: metal\nx");
        log::debug!("compiled {ENTRY}\n// language: metal\nx");
        log::debug!("single line '{ENTRY}'");
        log::info!("{}", record("info level"));
    })
    .unwrap_err();
    assert_eq!(err, CaptureError::NotFound);

    let (_, text) = capture_one(ENTRY, || {
        log::debug!("compiled 'other_entry'\n// language: metal\nx");
        log::debug!("{}", record("kept"));
    })
    .expect("mixed");
    assert!(text.ends_with("kept"));
}

#[test]
fn zero_two_and_oversize_records_are_refused() {
    assert_eq!(
        capture_one(ENTRY, || ()).unwrap_err(),
        CaptureError::NotFound
    );
    let err = capture_one(ENTRY, || {
        log::debug!("{}", record("a"));
        log::debug!("{}", record("b"));
    })
    .unwrap_err();
    assert_eq!(err, CaptureError::Ambiguous);
    let big = "x".repeat(MAX_SHADER_TEXT_BYTES + 1);
    let err = capture_one(ENTRY, || log::debug!("{}", record(&big))).unwrap_err();
    assert_eq!(err, CaptureError::TooLarge);
}

#[test]
fn long_first_line_truncation_is_not_reported_as_success() {
    // 1 行目が余裕を超えて長いと、切り詰め後の 2 行目以降は上限以下になり得る。
    // それでも不完全なテキストを成功にせず TooLarge にする。
    let pad = "p".repeat(10_000);
    let body = "x".repeat(MAX_SHADER_TEXT_BYTES - 100);
    let msg = format!("compiled '{ENTRY}' {pad}\n// language: metal\n{body}");
    let err = capture_one(ENTRY, || log::debug!("{msg}")).unwrap_err();
    assert_eq!(err, CaptureError::TooLarge);
}

#[test]
fn shader_text_boundaries() {
    assert_eq!(shader_text("'e' no newline", "e"), None);
    assert_eq!(shader_text("'e'\n", "e"), None);
    assert_eq!(shader_text("e\n// language: metal\nx", "e"), None);
    assert_eq!(shader_text("''\n// language: metal\nx", ""), None);
    assert_eq!(
        shader_text("a 'e' b\n// language: metal\nx", "e"),
        Some("// language: metal\nx")
    );
}

#[test]
fn display_is_fixed_english_without_values() {
    for (e, s) in [
        (
            CaptureError::LoggerTaken,
            "another logger is already installed",
        ),
        (
            CaptureError::NotFound,
            "no matching shader record was captured",
        ),
        (
            CaptureError::Ambiguous,
            "more than one matching shader record was captured",
        ),
        (
            CaptureError::TooLarge,
            "captured shader record exceeds the size limit",
        ),
    ] {
        assert_eq!(e.to_string(), s);
    }
}

struct Other;

impl log::Log for Other {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        false
    }
    fn log(&self, _: &log::Record<'_>) {}
    fn flush(&self) {}
}

/// 子プロセスで同じ試験を `--exact` で流し、成功の終了を求める。
fn run_child(name: &str) {
    let status = Command::new(std::env::current_exe().expect("exe"))
        .args(["--exact", name, "--test-threads=1"])
        .env(CHILD_MARK, "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert!(status.success());
}

#[test]
fn logger_taken_when_another_logger_exists() {
    if std::env::var_os(CHILD_MARK).is_none() {
        run_child("logger_taken_when_another_logger_exists");
        return;
    }
    log::set_logger(&Other).expect("first logger");
    let mut called = false;
    let r = capture_one(ENTRY, || called = true);
    assert_eq!(r.unwrap_err(), CaptureError::LoggerTaken);
    assert!(!called);
}

/// 最大の level は、ほかの試験の並列の実行に左右されないよう子プロセスで確かめる。
#[test]
fn level_returns_to_off_even_when_the_closure_panics() {
    if std::env::var_os(CHILD_MARK).is_none() {
        run_child("level_returns_to_off_even_when_the_closure_panics");
        return;
    }
    let _ = capture_one(ENTRY, || ());
    assert_eq!(log::max_level(), log::LevelFilter::Off);
    let caught = std::panic::catch_unwind(|| {
        let _ = capture_one(ENTRY, || panic!("boom"));
    });
    assert!(caught.is_err());
    assert_eq!(log::max_level(), log::LevelFilter::Off);
    assert_eq!(
        capture_one(ENTRY, || ()).unwrap_err(),
        CaptureError::NotFound
    );
}
