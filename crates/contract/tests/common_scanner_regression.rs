//! `common` の数え方の回帰テスト（文字リテラルのエスケープ終端・識別子境界）。
//!
//! 見本の文字列で道具そのものを確かめる網羅的なテストは `common_unsafe_count_samples.rs` にある。
//! ここには、レビューで見つかった取りこぼしの再現だけを置く。

#[allow(dead_code)]
mod common;

use common::count_unsafe;

#[test]
fn escaped_quote_char_literal_does_not_hide_unsafe() {
    let src = "fn f() { let _ = '\\''; let _ = '<'; let _ = '\"'; unsafe { g() } }";
    assert_eq!(count_unsafe(src).block, 1);
}

#[test]
fn escaped_quote_byte_literal_does_not_hide_unsafe() {
    let src = "fn f() { let _ = b'\\''; let _ = b'\"'; unsafe { g() } }";
    assert_eq!(count_unsafe(src).block, 1);
}

#[test]
fn hex_and_unicode_escapes_are_skipped_whole() {
    let src = "fn f() { let _ = '\\x41'; let _ = '\\u{1F600}'; let _ = b'\\x22'; unsafe { g() } }";
    assert_eq!(count_unsafe(src).block, 1);
}

#[test]
fn backslash_char_literal_is_skipped() {
    let src = "fn f() { let _ = '\\\\'; unsafe { g() } }";
    assert_eq!(count_unsafe(src).block, 1);
}

#[test]
fn identifier_with_combining_mark_is_not_the_keyword() {
    let src = "fn f() { let unsafe\u{0301} = 1; }";
    assert_eq!(count_unsafe(src).total(), 0);
    let src = "fn f() { let x\u{0301}unsafe = 1; }";
    assert_eq!(count_unsafe(src).total(), 0);
}
