//! コマンド `accept-kernel` を起動して、device に届く前に返る経路の終了コードと出力を確かめる試験（TASK-485 のポインタ）。
//!
//! 一覧とソースは試験の中で作る合成のもので、sha256 もその場で計算する。device を作る経路（全部が正しい組）は置かない。
//! 標準エラーに、一覧のパス・ディレクトリ・組の値が出ないことも確かめる。

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use fandhe_silicon_core::hash::sha256_bytes;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_accept-kernel"))
        .args(args)
        .output()
        .expect("spawn accept-kernel")
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("accept-{name}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).expect("tmp dir");
    d
}

fn rev() -> String {
    hex(&sha256_bytes(b"rev"))[..40].to_owned()
}

fn manifest_for(source: &[u8], file: &str, entry: &str) -> String {
    format!(
        "{} src/{file} {} msl {entry}\n",
        rev(),
        hex(&sha256_bytes(source))
    )
}

const SOURCE: &str = "kernel void marker_entry(int x) {}\n";

fn no_leak(o: &Output, markers: &[&str]) {
    let err = String::from_utf8_lossy(&o.stderr);
    for m in markers {
        assert!(!err.contains(m), "stderr leaks a marker");
    }
}

#[test]
fn wrong_arg_count_is_usage_error() {
    for (i, args) in [
        &[][..],
        &["a", "b", "c"][..],
        &["a", "b", "c", "d", "e"][..],
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(run(args).status.code(), Some(2), "case {i}");
    }
}

#[test]
fn unreadable_manifest_is_usage_error() {
    let d = tmp("unreadable-marker");
    let missing = d.join("no-such-manifest-marker");
    let o = run(&[
        missing.to_str().unwrap(),
        "dir-marker",
        "p-marker",
        "e-marker",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
    no_leak(
        &o,
        &[
            "unreadable-marker",
            "no-such-manifest-marker",
            "dir-marker",
            "p-marker",
            "e-marker",
        ],
    );
}

#[test]
fn malformed_manifest_is_usage_error() {
    let d = tmp("malformed-marker");
    let m = d.join("manifest-marker");
    fs::write(&m, "not a manifest line\n").unwrap();
    let o = run(&[m.to_str().unwrap(), "dir-marker", "p-marker", "e-marker"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
    no_leak(
        &o,
        &[
            "malformed-marker",
            "manifest-marker",
            "dir-marker",
            "p-marker",
            "e-marker",
            "not a manifest",
        ],
    );
}

#[test]
fn pair_missing_from_the_manifest_is_usage_error() {
    let d = tmp("pair-marker");
    let m = d.join("manifest-marker");
    fs::write(
        &m,
        manifest_for(SOURCE.as_bytes(), "a.metal", "marker_entry"),
    )
    .unwrap();
    for (path, entry) in [
        ("src/a.metal", "other_marker"),
        ("src/b-marker.metal", "marker_entry"),
    ] {
        let o = run(&[m.to_str().unwrap(), d.to_str().unwrap(), path, entry]);
        assert_eq!(o.status.code(), Some(2));
        assert!(o.stdout.is_empty());
        no_leak(
            &o,
            &[
                "pair-marker",
                "manifest-marker",
                "other_marker",
                "b-marker",
                "marker_entry",
            ],
        );
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_argument_is_usage_error() {
    use std::os::unix::ffi::OsStringExt;
    let bad = std::ffi::OsString::from_vec(vec![0xff, 0xfe]);
    let o = Command::new(env!("CARGO_BIN_EXE_accept-kernel"))
        .args(["a", "b", "c"])
        .arg(bad)
        .output()
        .expect("spawn accept-kernel");
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn valid_arguments_stop_with_code_3_off_macos() {
    let d = tmp("offmac-marker");
    let m = d.join("manifest-marker");
    fs::write(
        &m,
        manifest_for(SOURCE.as_bytes(), "a.metal", "marker_entry"),
    )
    .unwrap();
    let o = run(&[
        m.to_str().unwrap(),
        d.to_str().unwrap(),
        "src/a.metal",
        "marker_entry",
    ]);
    assert_eq!(o.status.code(), Some(3));
    assert!(o.stdout.is_empty());
    no_leak(&o, &["offmac-marker", "manifest-marker", "marker_entry"]);
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    fn case(name: &str, manifest_source: &str, file_content: Option<&str>) -> Output {
        let d = tmp(name);
        let m = d.join("manifest-marker");
        fs::write(
            &m,
            manifest_for(manifest_source.as_bytes(), "a.metal", "marker_entry"),
        )
        .unwrap();
        let dir = d.join("src-dir-marker");
        fs::create_dir_all(&dir).unwrap();
        if let Some(c) = file_content {
            fs::write(dir.join("a.metal"), c).unwrap();
        }
        let o = run(&[
            m.to_str().unwrap(),
            dir.to_str().unwrap(),
            "src/a.metal",
            "marker_entry",
        ]);
        no_leak(&o, &["manifest-marker", "src-dir-marker", "marker_entry"]);
        o
    }

    #[test]
    fn missing_source_fails_before_the_device() {
        let o = case("macos-missing-marker", SOURCE, None);
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
    }

    #[test]
    fn digest_mismatch_fails_before_the_device() {
        let o = case(
            "macos-digest-marker",
            SOURCE,
            Some("kernel void marker_entry(int y) {}\n"),
        );
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
    }

    #[test]
    fn entry_list_mismatch_fails_before_the_device() {
        let other = "kernel void marker_entry(int x) {}\nkernel void extra_one(int x) {}\n";
        let o = case("macos-entries-marker", other, Some(other));
        // 一覧は 1 入口、ソースは 2 入口。sha256 は合うので並びの照合で落ちる。
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
    }
}
