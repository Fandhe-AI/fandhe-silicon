//! `DeviceLostReason` の 3 値（`Destroyed`・`Unknown`・`ValidationError(String)`）を別の crate から確かめる公開テスト（TASK-136 の 5/6）。
//!
//! 仕組み: 3 値を組み立て、`match` の 3 つの腕と `_` の腕でラベルに写す（DEV-142。ポインタのみ）。
//! `ValidationError` は `fn(String) -> DeviceLostReason` への強制で形を固定する。
//!
//! 対象外: 型の定義と `#[non_exhaustive]` の試験は TASK-269 の持ち分（`error_non_exhaustive.rs`・
//! `error_fields.rs`）。ほかの値は `_` の腕に落ちるので、値の個数は見ない。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::error::DeviceLostReason;

/// 3 値の腕と `_` の腕でラベルを返す。別の crate からは `_` の腕が要る。
fn label(r: &DeviceLostReason) -> &'static str {
    match r {
        DeviceLostReason::Destroyed => "Destroyed",
        DeviceLostReason::Unknown => "Unknown",
        DeviceLostReason::ValidationError(_) => "ValidationError",
        _ => "Other",
    }
}

#[test]
fn device_lost_reason_has_destroyed_unknown_validation_error() {
    assert_eq!(label(&DeviceLostReason::Destroyed), "Destroyed");
    assert_eq!(label(&DeviceLostReason::Unknown), "Unknown");
    assert_eq!(
        label(&DeviceLostReason::ValidationError("bad".to_string())),
        "ValidationError"
    );
}

#[test]
fn validation_error_is_a_string_constructor() {
    let f: fn(String) -> DeviceLostReason = DeviceLostReason::ValidationError;
    let v = f("line one\nline two".to_string());
    match v {
        DeviceLostReason::ValidationError(s) => assert_eq!(s, "line one\nline two"),
        _ => panic!("expected ValidationError"),
    }
}

#[test]
fn three_values_are_distinct() {
    let a = DeviceLostReason::Destroyed;
    let b = DeviceLostReason::Unknown;
    let c = DeviceLostReason::ValidationError(String::new());
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(b, c);
}
