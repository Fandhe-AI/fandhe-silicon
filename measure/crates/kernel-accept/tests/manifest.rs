//! 一覧のファイルの形の検査の試験（TASK-485 のポインタ）。値はすべて試験の中で計算した合成のもの。

use std::fs;
use std::path::PathBuf;

use fandhe_silicon_core::hash::sha256_bytes;
use measure_kernel_accept::manifest::{
    MAX_MANIFEST_BYTES, MAX_MANIFEST_LINES, Manifest, ManifestErrorKind as K, Mode, load, parse,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn rev(seed: &str) -> String {
    hex(&sha256_bytes(seed.as_bytes()))[..40].to_owned()
}

fn sha(seed: &str) -> String {
    hex(&sha256_bytes(seed.as_bytes()))
}

fn line(r: &str, path: &str, s: &str, mode: &str, entry: &str) -> String {
    format!("{r} {path} {s} {mode} {entry}")
}

fn good() -> String {
    let r = rev("r");
    [
        line(&r, "src/a.metal", &sha("a"), "msl", "alpha"),
        line(&r, "src/a.metal", &sha("a"), "specialized", "beta"),
        line(&r, "src/b.metal", &sha("b"), "msl", "gamma"),
    ]
    .join("\n")
}

fn err(text: &str) -> (K, Option<usize>) {
    let e = parse(text).expect_err("must be rejected");
    (e.kind(), e.line())
}

#[test]
fn accepts_two_files_three_entries() {
    let m: Manifest = parse(&good()).expect("good");
    assert_eq!(m.rev(), rev("r"));
    assert_eq!(m.entries().len(), 3);
    let files = m.files();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path(), "src/a.metal");
    assert_eq!(files[0].sha256(), &sha256_bytes(b"a"));
    let names: Vec<&str> = files[0].entries().iter().map(|e| e.entry()).collect();
    assert_eq!(names, ["alpha", "beta"]);
    assert_eq!(files[1].path(), "src/b.metal");
    assert_eq!(files[1].entries().len(), 1);
    let g = m.find("gamma").expect("gamma");
    assert_eq!(g.line(), 3);
    assert_eq!(g.path(), "src/b.metal");
    assert_eq!(g.mode(), Mode::Msl);
    assert_eq!(g.sha256(), &sha256_bytes(b"b"));
    assert!(m.find("nope").is_none());
    assert_eq!(m.count(Mode::Msl), 2);
    assert_eq!(m.count(Mode::Specialized), 1);
    let order: Vec<usize> = m.entries().iter().map(|e| e.line()).collect();
    assert_eq!(order, [1, 2, 3]);
}

#[test]
fn trailing_lf_is_optional() {
    assert!(parse(&good()).is_ok());
    assert!(parse(&format!("{}\n", good())).is_ok());
}

#[test]
fn entry_name_of_64_chars_is_accepted() {
    let name = "a".repeat(64);
    let t = line(&rev("r"), "src/a.metal", &sha("a"), "msl", &name);
    assert!(parse(&t).is_ok());
}

#[test]
fn rejects_wrong_field_counts() {
    let r = rev("r");
    let four = format!("{r} src/a.metal {} msl", sha("a"));
    let six = format!(
        "{} extra",
        line(&r, "src/a.metal", &sha("a"), "msl", "alpha")
    );
    assert_eq!(err(&four), (K::FieldCount, Some(1)));
    assert_eq!(err(&six), (K::FieldCount, Some(1)));
}

#[test]
fn rejects_empty_input_and_empty_lines() {
    assert_eq!(err(""), (K::Empty, None));
    let g = good();
    assert_eq!(err(&format!("\n{g}")), (K::EmptyLine, Some(1)));
    let mut lines: Vec<&str> = g.split('\n').collect();
    lines.insert(1, "");
    assert_eq!(err(&lines.join("\n")), (K::EmptyLine, Some(2)));
    assert_eq!(err(&format!("{g}\n\n")), (K::EmptyLine, Some(4)));
}

#[test]
fn rejects_control_characters() {
    let g = good();
    assert_eq!(
        err(&g.replace('\n', "\r\n")),
        (K::ControlCharacter, Some(1))
    );
    assert_eq!(
        err(&g.replacen("src/a", "src\t/a", 1)),
        (K::ControlCharacter, Some(1))
    );
}

#[test]
fn rejects_extra_spaces() {
    let g = good();
    assert_eq!(err(&format!(" {g}")), (K::FieldCount, Some(1)));
    assert_eq!(err(&g.replacen("msl", "msl ", 1)), (K::FieldCount, Some(1)));
    assert_eq!(
        err(&g.replacen(" src/a", "  src/a", 1)),
        (K::FieldCount, Some(1))
    );
}

#[test]
fn rejects_malformed_fields() {
    let r = rev("r");
    let s = sha("a");
    let upper = r.to_uppercase();
    assert_eq!(
        err(&line(&upper, "src/a.metal", &s, "msl", "alpha")),
        (K::Rev, Some(1))
    );
    assert_eq!(
        err(&line(&r, "src/a.metal", &s[..63], "msl", "alpha")),
        (K::Sha256, Some(1))
    );
    assert_eq!(
        err(&line(&r, "src/a.metal", &s, "other", "alpha")),
        (K::Mode, Some(1))
    );
    assert_eq!(
        err(&line(&r, "src/a.metal", &s, "msl", "1alpha")),
        (K::Entry, Some(1))
    );
    let long = "a".repeat(65);
    assert_eq!(
        err(&line(&r, "src/a.metal", &s, "msl", &long)),
        (K::Entry, Some(1))
    );
    assert_eq!(
        err(&line(&r, "src/../a.metal", &s, "msl", "alpha")),
        (K::Path, Some(1))
    );
}

#[test]
fn rejects_cross_line_violations() {
    let r = rev("r");
    let a = sha("a");
    let b = sha("b");
    let other_rev = rev("r2");
    assert_eq!(
        err(&[
            line(&r, "src/a.metal", &a, "msl", "alpha"),
            line(&other_rev, "src/a.metal", &a, "msl", "beta")
        ]
        .join("\n")),
        (K::RevMismatch, Some(2))
    );
    assert_eq!(
        err(&[
            line(&r, "src/a.metal", &a, "msl", "alpha"),
            line(&r, "src/a.metal", &b, "msl", "beta")
        ]
        .join("\n")),
        (K::Sha256Mismatch, Some(2))
    );
    assert_eq!(
        err(&[
            line(&r, "src/a.metal", &a, "msl", "alpha"),
            line(&r, "src/b.metal", &b, "msl", "beta"),
            line(&r, "src/a.metal", &a, "msl", "gamma"),
        ]
        .join("\n")),
        (K::PathNotContiguous, Some(3))
    );
    assert_eq!(
        err(&[
            line(&r, "src/a.metal", &a, "msl", "alpha"),
            line(&r, "other/a.metal", &b, "msl", "beta")
        ]
        .join("\n")),
        (K::FileNameCollision, Some(2))
    );
    assert_eq!(
        err(&[
            line(&r, "src/a.metal", &a, "msl", "alpha"),
            line(&r, "src/b.metal", &b, "msl", "alpha")
        ]
        .join("\n")),
        (K::DuplicateEntry, Some(2))
    );
}

#[test]
fn rejects_size_and_line_limits_before_line_checks() {
    // 行の形が不正でも、上限の検査が先に返る。
    let big = "x".repeat(MAX_MANIFEST_BYTES + 1);
    assert_eq!(err(&big), (K::TooLarge, None));
    let many = vec!["x"; MAX_MANIFEST_LINES + 1].join("\n");
    assert_eq!(err(&many), (K::TooManyLines, None));
}

#[test]
fn exact_line_limit_is_not_rejected_by_count() {
    // 行数がちょうど上限なら、行数では拒否されず、行ごとの検査に進む。
    let exact = vec!["x"; MAX_MANIFEST_LINES].join("\n");
    assert_eq!(err(&exact), (K::FieldCount, Some(1)));
}

fn tmp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("manifest-{name}"))
}

#[test]
fn load_reads_good_file() {
    let p = tmp("good");
    fs::write(&p, good()).expect("write");
    let m = load(&p).expect("load");
    assert_eq!(m.entries().len(), 3);
}

#[test]
fn load_rejects_missing_and_directory() {
    let p = tmp("missing");
    let _ = fs::remove_file(&p);
    assert_eq!(load(&p).expect_err("missing").kind(), K::Unreadable);
    let d = tmp("dir");
    fs::create_dir_all(&d).expect("dir");
    assert_eq!(load(&d).expect_err("dir").kind(), K::Unreadable);
}

#[test]
fn load_rejects_oversize_and_non_utf8() {
    let p = tmp("big");
    fs::write(&p, vec![b'a'; MAX_MANIFEST_BYTES + 1]).expect("write");
    assert_eq!(load(&p).expect_err("big").kind(), K::TooLarge);
    let q = tmp("bin");
    fs::write(&q, [0xff, 0xfe, 0x00]).expect("write");
    assert_eq!(load(&q).expect_err("bin").kind(), K::NotUtf8);
}

#[test]
fn display_is_ascii_and_never_echoes_input() {
    let r = rev("r");
    let a = sha("a");
    let b = sha("b");
    let secret_path = "zzdir/secretname.metal";
    let secret_entry = "secret_entry";
    let cases: Vec<String> = vec![
        line(&r.to_uppercase(), secret_path, &a, "msl", secret_entry),
        line(&r, "../secretname.metal", &a, "msl", secret_entry),
        line(&r, secret_path, &a[..63], "msl", secret_entry),
        line(&r, secret_path, &a, "weird", secret_entry),
        line(&r, secret_path, &a, "msl", "9secret"),
        [
            line(&r, secret_path, &a, "msl", secret_entry),
            line(&r, secret_path, &b, "msl", "other"),
        ]
        .join("\n"),
        [
            line(&r, secret_path, &a, "msl", secret_entry),
            line(&r, "q/secretname.metal", &b, "msl", "other"),
        ]
        .join("\n"),
        [
            line(&r, secret_path, &a, "msl", secret_entry),
            line(&r, "q/x.metal", &b, "msl", secret_entry),
        ]
        .join("\n"),
        [
            line(&r, secret_path, &a, "msl", secret_entry),
            line(&rev("r2"), secret_path, &a, "msl", "other"),
        ]
        .join("\n"),
        format!("{r} {secret_path}\t{a} msl {secret_entry}"),
        format!("{r} {secret_path}"),
        String::new(),
    ];
    for c in cases {
        let msg = parse(&c).expect_err("rejected").to_string();
        assert!(msg.is_ascii(), "{msg}");
        for needle in [
            r.as_str(),
            r.to_uppercase().as_str(),
            a.as_str(),
            b.as_str(),
            "secretname",
            "secret_entry",
            "weird",
            "9secret",
            "zzdir",
        ] {
            assert!(!msg.contains(needle), "leaked {needle} in {msg}");
        }
    }
    let big = tmp("disp");
    fs::write(&big, vec![b'a'; MAX_MANIFEST_BYTES + 1]).expect("write");
    let msg = load(&big).expect_err("big").to_string();
    assert!(!msg.contains("manifest-disp"));
}
