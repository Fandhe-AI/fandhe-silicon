//! コマンド `detect-bounds` を実際に起動して、出力の 1 行・終了コード・標準エラーを確かめる試験。
//!
//! 値は合成のものだけ。ファイル・環境変数は触らない。

use std::process::{Command, Output};

fn cmd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_detect-bounds"))
        .args(args)
        .output()
        .expect("spawn detect-bounds")
}

fn ok_line(args: &[&str]) -> String {
    let o = cmd(args);
    assert_eq!(o.status.code(), Some(0), "args={args:?}");
    assert!(o.stderr.is_empty());
    let s = String::from_utf8(o.stdout).unwrap();
    assert!(s.ends_with('\n'));
    assert_eq!(s.matches('\n').count(), 1);
    s
}

#[test]
fn nine_one_zero_is_ok() {
    let s = ok_line(&["9", "1", "0"]);
    assert_eq!(
        s,
        "{\"detected\":9,\"missed\":1,\"indeterminate\":0,\"denominator\":10,\"lower\":{\"numerator\":9,\"denominator\":10},\"upper\":{\"numerator\":9,\"denominator\":10},\"threshold\":{\"numerator\":9,\"denominator\":10},\"label\":\"ok\",\"reason\":null}\n"
    );
}

#[test]
fn eight_zero_two_is_indeterminate() {
    let s = ok_line(&["8", "0", "2"]);
    assert!(s.contains("\"label\":\"indeterminate\""));
    assert!(s.contains("\"reason\":\"bounds straddle the threshold\""));
    assert!(s.contains("\"threshold\":{\"numerator\":9,\"denominator\":10}"));
    assert!(s.contains("\"upper\":{\"numerator\":10,\"denominator\":10}"));
    assert!(s.contains("\"lower\":{\"numerator\":8,\"denominator\":10}"));
}

#[test]
fn eight_two_zero_is_ng() {
    let s = ok_line(&["8", "2", "0"]);
    assert!(s.contains("\"label\":\"ng\""));
    assert!(s.contains("\"reason\":null"));
}

#[test]
fn leading_zeros_are_accepted() {
    let s = ok_line(&["009", "01", "0"]);
    assert!(s.contains("\"label\":\"ok\""));
    assert!(s.contains("\"detected\":9"));
}

#[test]
fn input_violations_exit_one_without_stdout() {
    for args in [
        ["0", "0", "0"],
        ["18446744073709551615", "1", "0"],
        ["18446744073709551615", "0", "0"],
    ] {
        let o = cmd(&args);
        assert_eq!(o.status.code(), Some(1), "args={args:?}");
        assert!(o.stdout.is_empty());
        assert!(!o.stderr.is_empty());
    }
}

#[test]
fn misuse_exits_two_without_stdout() {
    let cases: Vec<Vec<&str>> = vec![
        vec![],
        vec!["1", "2"],
        vec!["1", "2", "3", "4"],
        vec!["-1", "0", "0"],
        vec!["1.5", "0", "0"],
        vec!["abc", "0", "0"],
        vec!["+1", "0", "0"],
        vec!["18446744073709551616", "0", "0"],
        vec!["", "0", "0"],
        vec!["--bogus"],
    ];
    for args in cases {
        let o = cmd(&args);
        assert_eq!(o.status.code(), Some(2), "args={args:?}");
        assert!(o.stdout.is_empty(), "args={args:?}");
    }
}

#[test]
fn help_exits_zero_with_usage() {
    let o = cmd(&["--help"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("usage:"));
}

#[test]
fn stderr_does_not_echo_input_values() {
    for args in [
        ["abc", "0", "0"],
        ["1.5", "0", "0"],
        ["-1", "0", "0"],
        ["18446744073709551616", "0", "0"],
        ["18446744073709551615", "1", "0"],
    ] {
        let o = cmd(&args);
        let e = String::from_utf8_lossy(&o.stderr);
        for a in args {
            if a != "0" && a != "1" {
                assert!(!e.contains(a), "stderr echoed {a}");
            }
        }
    }
}

/// 非 Unicode の引数でも panic せず、終了コード 2・標準出力なし・入力のバイト列を出さない。
#[cfg(unix)]
#[test]
fn non_unicode_arg_exits_two_without_echo() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let bad = OsString::from_vec(vec![0xff, 0xfe, 0x80]);
    for pos in 0..3 {
        let mut args: Vec<OsString> = vec!["1".into(), "1".into(), "1".into()];
        args[pos] = bad.clone();
        let o = Command::new(env!("CARGO_BIN_EXE_detect-bounds"))
            .args(&args)
            .output()
            .expect("spawn detect-bounds");
        assert_eq!(o.status.code(), Some(2), "pos={pos}");
        assert!(o.stdout.is_empty());
        assert!(!o.stderr.windows(3).any(|w| w == [0xff, 0xfe, 0x80]));
        assert!(!String::from_utf8_lossy(&o.stderr).contains("panicked"));
    }
}
