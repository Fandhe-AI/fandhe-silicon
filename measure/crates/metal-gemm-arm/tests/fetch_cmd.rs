//! コマンド `fetch-kernel-source` を起動して、終了コード・書き出し先・標準エラーを確かめる試験。
//!
//! 外部のソースは使わず、clone が無い・書き出し先が既にある場合の経路だけを通す。

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fetch-kernel-source"))
        .args(args)
        .output()
        .expect("spawn fetch-kernel-source")
}

fn tmp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("fetch-{name}"))
}

#[test]
fn wrong_arg_count_is_usage_error() {
    for args in [&[][..], &["a"][..], &["a", "b", "c"][..]] {
        assert_eq!(run(args).status.code(), Some(2), "args={args:?}");
    }
}

#[test]
fn missing_clone_fails_without_creating_output() {
    let clone = tmp("no-such-clone-marker");
    let dest = tmp("not-yet-output-marker");
    let _ = fs::remove_file(&dest);
    let o = run(&[clone.to_str().unwrap(), dest.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!dest.exists());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!err.contains("no-such-clone-marker"));
    assert!(!err.contains("not-yet-output-marker"));
}

#[test]
fn existing_output_is_not_overwritten() {
    let clone = tmp("no-such-clone-marker2");
    let dest = tmp("existing-output-marker");
    fs::write(&dest, b"keep").unwrap();
    let o = run(&[clone.to_str().unwrap(), dest.to_str().unwrap()]);
    let kept = fs::read(&dest).unwrap();
    fs::remove_file(&dest).unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(kept, b"keep");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!err.contains("existing-output-marker"));
    assert!(!err.contains("no-such-clone-marker2"));
}
