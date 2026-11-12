//! `scan_entries` の試験（TASK-485 のポインタ）。ソースは試験の中の合成の文字列だけ。

use measure_kernel_accept::scan::scan_entries;

#[test]
fn entries_in_order_of_appearance() {
    let src =
        "kernel void alpha(int a) {}\nkernel void beta(int b) {}\nkernel void gamma(int c) {}";
    assert_eq!(scan_entries(src), ["alpha", "beta", "gamma"]);
}

#[test]
fn whitespace_and_newlines_between_tokens() {
    let src = "kernel\n  void\t\n name_1 \n (int a) {}";
    assert_eq!(scan_entries(src), ["name_1"]);
}

#[test]
fn space_before_paren_is_accepted() {
    assert_eq!(scan_entries("kernel void _x  ("), ["_x"]);
}

#[test]
fn no_kernel_void_yields_empty() {
    assert!(scan_entries("").is_empty());
    assert!(scan_entries("void helper(int a) {}").is_empty());
}

#[test]
fn name_not_followed_by_paren_is_skipped_and_scan_continues() {
    let src = "kernel void name; kernel void next(int a) {}";
    assert_eq!(scan_entries(src), ["next"]);
}

#[test]
fn name_starting_with_digit_is_not_taken() {
    assert!(scan_entries("kernel void 1abc(").is_empty());
}

#[test]
fn non_ascii_text_does_not_break_scanning() {
    let src = "// 日本語のコメント\nkernel void ok(";
    assert_eq!(scan_entries(src), ["ok"]);
}
