//! コマンド `fetch-kernel-source` の 5 引数の形（rev・パス・sha256 を実行時に受ける形。TASK-1009 のポインタ）を、
//! 合成の一時 git repo で確かめる試験。device には触れず、どの OS でも流れる。
//!
//! 期待の sha256 は試験の中で計算する。引数の検査の関数は argv で渡せない入力のために直に呼ぶ。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use fandhe_silicon_core::hash::sha256_bytes;
use measure_metal_gemm_arm::source::{
    ArgError, MAX_SOURCE_PATH_BYTES, check_rev, check_source_path, parse_sha256_hex,
};

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// 一意な一時ディレクトリ。`Drop` で消すので assert が失敗しても残らない。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("fk-pinned-{}-{n}-{t}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 利用者の設定と実 repo から切り離した git の起動。
fn git(dir: &Path) -> Command {
    let mut c = Command::new("git");
    isolate(&mut c);
    c.arg("-C").arg(dir);
    c
}

fn isolate(c: &mut Command) {
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("GIT_") {
            c.env_remove(k);
        }
    }
    c.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
}

fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

struct Fixture {
    _s: Scratch,
    clone: PathBuf,
    out_dir: PathBuf,
    rev: String,
    content: Vec<u8>,
    sha: String,
}

const REL: &str = "dir/file.txt";

fn fixture() -> Fixture {
    let s = Scratch::new();
    let clone = s.0.join("clone");
    let out_dir = s.0.join("out");
    fs::create_dir_all(clone.join("dir")).unwrap();
    fs::create_dir_all(&out_dir).unwrap();
    let content = b"synthetic content\nline 2\n".to_vec();
    fs::write(clone.join(REL), &content).unwrap();
    let ok = |o: Output| assert!(o.status.success(), "git setup failed");
    ok(git(&clone)
        .args(["-c", "init.defaultBranch=main", "init", "-q"])
        .output()
        .unwrap());
    ok(git(&clone).args(["add", REL]).output().unwrap());
    ok(git(&clone)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "t",
        ])
        .output()
        .unwrap());
    let o = git(&clone).args(["rev-parse", "HEAD"]).output().unwrap();
    assert!(o.status.success());
    let rev = String::from_utf8(o.stdout).unwrap().trim().to_owned();
    let sha = hex(sha256_bytes(&content));
    Fixture {
        _s: s,
        clone,
        out_dir,
        rev,
        content,
        sha,
    }
}

fn run(args: &[&str]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_fetch-kernel-source"));
    isolate(&mut c);
    c.args(args).output().expect("spawn fetch-kernel-source")
}

fn run5(f: &Fixture, dest: &Path, rev: &str, path: &str, sha: &str) -> Output {
    run(&[
        f.clone.to_str().unwrap(),
        dest.to_str().unwrap(),
        rev,
        path,
        sha,
    ])
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn matching_sha_writes_the_file_bytes() {
    let f = fixture();
    let dest = f.out_dir.join("got.txt");
    let o = run5(&f, &dest, &f.rev, REL, &f.sha);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(fs::read(&dest).unwrap(), f.content);
    assert_no_leak(&o, &f, &dest);
}

#[test]
fn one_digit_off_sha_fails_without_output() {
    let f = fixture();
    let dest = f.out_dir.join("got.txt");
    let mut sha = f.sha.clone();
    let last = if sha.ends_with('0') { '1' } else { '0' };
    sha.pop();
    sha.push(last);
    let o = run5(&f, &dest, &f.rev, REL, &sha);
    assert_eq!(o.status.code(), Some(1));
    assert!(!dest.exists());
    assert_no_leak(&o, &f, &dest);
}

#[test]
fn path_missing_from_tree_fails_without_output() {
    let f = fixture();
    let dest = f.out_dir.join("got.txt");
    let o = run5(&f, &dest, &f.rev, "dir/absent.txt", &f.sha);
    assert_eq!(o.status.code(), Some(1));
    assert!(!dest.exists());
    assert_no_leak(&o, &f, &dest);
}

/// 0／1 の終了では標準エラーに引数の値を出さない。
fn assert_no_leak(o: &Output, f: &Fixture, dest: &Path) {
    let err = stderr(o);
    for v in [
        f.clone.to_str().unwrap(),
        dest.to_str().unwrap(),
        f.rev.as_str(),
        f.sha.as_str(),
        REL,
    ] {
        assert!(!err.contains(v), "stderr leaked an argument: {err}");
    }
}

#[test]
fn invalid_arguments_are_usage_errors_before_git() {
    let f = fixture();
    let dest = f.out_dir.join("got.txt");
    // 存在しない clone を渡す。有効な引数なら git の失敗で 1 になるので、2 なら git の前に返っている。
    let no_clone = f.out_dir.join("no-such-clone");
    let good_rev = f.rev.clone();
    let good_sha = f.sha.clone();
    let upper_rev = good_rev.to_uppercase();
    let upper_sha = good_sha.to_uppercase();
    let non_hex_rev = format!("{}g", &good_rev[..39]);
    let cases: Vec<(&str, String, String, &str)> = vec![
        (
            "3",
            good_rev[..39].to_owned(),
            "x".into(),
            "rev must be 40 lowercase hex digits",
        ),
        (
            "3",
            format!("{good_rev}0"),
            "x".into(),
            "rev must be 40 lowercase hex digits",
        ),
        (
            "3",
            upper_rev,
            "x".into(),
            "rev must be 40 lowercase hex digits",
        ),
        (
            "3",
            non_hex_rev,
            "x".into(),
            "rev must be 40 lowercase hex digits",
        ),
    ];
    let mut all = cases;
    for p in ["-x", "/abs", "../x", "a/../b", "a//b", "./a", ""] {
        all.push((
            "4",
            good_rev.clone(),
            p.into(),
            "path must be a plain relative path",
        ));
    }
    let run_case = |n: &str, rev: &str, path: &str, sha: &str, msg: &str| {
        let o = run(&[
            no_clone.to_str().unwrap(),
            dest.to_str().unwrap(),
            rev,
            path,
            sha,
        ]);
        assert_eq!(o.status.code(), Some(2), "arg {n} rev={rev} path={path}");
        assert_eq!(stderr(&o), format!("error: arg {n}: {msg}\n"));
        assert!(!dest.exists());
    };
    for (n, rev, path, msg) in &all {
        run_case(n, rev, path, &good_sha, msg);
    }
    let sha_msg = "sha256 must be 64 lowercase hex digits";
    for bad in [good_sha[..63].to_owned(), format!("{good_sha}0"), upper_sha] {
        run_case("5", &good_rev, "x", &bad, sha_msg);
    }
}

#[test]
fn existing_output_is_kept_in_five_arg_form() {
    let f = fixture();
    let dest = f.out_dir.join("exists.txt");
    fs::write(&dest, b"keep").unwrap();
    let o = run5(&f, &dest, &f.rev, REL, &f.sha);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(fs::read(&dest).unwrap(), b"keep");
    assert_eq!(stderr(&o), "error: arg 2: output already exists\n");
}

#[test]
fn other_arg_counts_are_usage_errors() {
    for n in [3usize, 4, 6] {
        let args = vec!["a"; n];
        assert_eq!(run(&args).status.code(), Some(2), "n={n}");
    }
}

#[test]
fn check_source_path_boundaries() {
    for bad in [
        "a\0b", "a\nb", "a\rb", "a\\b", "a:b", "a/", "/", "a//b", "-", ".", "..",
    ] {
        assert_eq!(check_source_path(bad), Err(ArgError::Path), "{bad:?}");
    }
    let ok = "a".repeat(MAX_SOURCE_PATH_BYTES);
    assert_eq!(check_source_path(&ok), Ok(ok.as_str()));
    let long = "a".repeat(MAX_SOURCE_PATH_BYTES + 1);
    assert_eq!(check_source_path(&long), Err(ArgError::Path));
    assert_eq!(check_source_path("a/b.c"), Ok("a/b.c"));
    assert_eq!(check_source_path("x"), Ok("x"));
}

#[test]
fn check_rev_and_sha_boundaries() {
    let rev = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(check_rev(rev), Ok(rev));
    assert_eq!(check_rev(&rev[..39]), Err(ArgError::Rev));
    assert_eq!(check_rev(&rev.to_uppercase()), Err(ArgError::Rev));
    assert_eq!(check_rev(""), Err(ArgError::Rev));
    let sha = hex(sha256_bytes(b"abc"));
    assert_eq!(parse_sha256_hex(&sha), Ok(sha256_bytes(b"abc")));
    assert_eq!(parse_sha256_hex(&sha[..63]), Err(ArgError::Sha256));
    assert_eq!(parse_sha256_hex(&sha.to_uppercase()), Err(ArgError::Sha256));
    assert_eq!(
        parse_sha256_hex(&format!("{}g", &sha[..63])),
        Err(ArgError::Sha256)
    );
}
