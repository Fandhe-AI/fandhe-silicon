//! 実行時に環境変数で受けるソースの設定と起動の形の試験（TASK-1009 のポインタ）。どの OS でも流れ、device に触らない。

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_inputs::GemmInputDtype;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::external::{
    EXTERNAL_TILE, ExternalError, ExternalKernel, KERNEL_ENTRY_ENV, KERNEL_SHA256_ENV,
    KERNEL_SOURCE_ENV, external_launch_for,
};
use measure_metal_gemm_arm::source::{MAX_SOURCE_BYTES, SourceError};

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// 一意な一時ディレクトリ。落とすときに消す。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("ext-kernel-{}-{n}-{t}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        fs::write(&p, bytes).unwrap();
        p
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn lookup(
    path: Option<OsString>,
    sha: Option<String>,
    entry: Option<&str>,
) -> impl Fn(&str) -> Option<OsString> {
    let sha = sha.map(OsString::from);
    let entry = entry.map(OsString::from);
    move |name| match name {
        KERNEL_SOURCE_ENV => path.clone(),
        KERNEL_SHA256_ENV => sha.clone(),
        KERNEL_ENTRY_ENV => entry.clone(),
        _ => None,
    }
}

fn load(
    path: Option<OsString>,
    sha: Option<String>,
    entry: Option<&str>,
) -> Result<ExternalKernel, ExternalError> {
    ExternalKernel::from_lookup(lookup(path, sha, entry))
}

fn assert_quiet(e: &ExternalError, secrets: &[&str]) {
    let text = e.to_string();
    for s in secrets {
        assert!(!text.contains(s), "message leaks a value");
    }
}

#[test]
fn any_missing_variable_is_missing() {
    let s = Scratch::new();
    let body = b"synthetic";
    let p = s.write("k.txt", body).into_os_string();
    let sha = hex(sha256_bytes(body));
    assert_eq!(
        load(None, Some(sha.clone()), Some("entry")).err(),
        Some(ExternalError::Missing)
    );
    assert_eq!(
        load(Some(p.clone()), None, Some("entry")).err(),
        Some(ExternalError::Missing)
    );
    assert_eq!(
        load(Some(p), Some(sha), None).err(),
        Some(ExternalError::Missing)
    );
}

#[test]
fn malformed_digest_is_rejected() {
    let s = Scratch::new();
    let body = b"synthetic";
    let p = s.write("k.txt", body).into_os_string();
    let good = hex(sha256_bytes(body));
    for bad in [
        good[..63].to_string(),
        format!("{good}0"),
        good.to_uppercase(),
    ] {
        let e = load(Some(p.clone()), Some(bad.clone()), Some("entry")).expect_err("err");
        assert_eq!(e, ExternalError::InvalidDigest);
        assert_quiet(&e, &[&bad]);
    }
}

#[test]
fn malformed_entry_is_rejected_and_boundary_is_accepted() {
    let s = Scratch::new();
    let body = b"synthetic";
    let p = s.write("k.txt", body).into_os_string();
    let sha = hex(sha256_bytes(body));
    let too_long = "a".repeat(65);
    for bad in ["", "1abc", "a-b", too_long.as_str()] {
        let e = load(Some(p.clone()), Some(sha.clone()), Some(bad)).expect_err("err");
        assert_eq!(e, ExternalError::InvalidEntry);
        if !bad.is_empty() {
            assert_quiet(&e, &[bad]);
        }
    }
    let longest = "a".repeat(64);
    assert!(load(Some(p), Some(sha), Some(&longest)).is_ok());
}

#[test]
fn unreadable_paths_are_source_errors() {
    let s = Scratch::new();
    let sha = hex(sha256_bytes(b"x"));
    let missing = s.0.join("absent.txt").into_os_string();
    let dir = s.0.clone().into_os_string();
    for p in [missing, dir] {
        let shown = p.to_string_lossy().into_owned();
        let e = load(Some(p), Some(sha.clone()), Some("entry")).expect_err("err");
        assert_eq!(e, ExternalError::Source(SourceError::Unreadable));
        assert_quiet(&e, &[&shown, &sha]);
    }
}

#[test]
fn digest_mismatch_oversize_and_non_utf8_are_source_errors() {
    let s = Scratch::new();
    let body = b"synthetic";
    let p = s.write("k.txt", body).into_os_string();
    let mut off = hex(sha256_bytes(body));
    off.replace_range(0..1, if off.starts_with('0') { "1" } else { "0" });
    assert_eq!(
        load(Some(p), Some(off), Some("entry")).err(),
        Some(ExternalError::Source(SourceError::DigestMismatch))
    );

    let big = vec![b'a'; MAX_SOURCE_BYTES + 1];
    let p = s.write("big.txt", &big).into_os_string();
    assert_eq!(
        load(Some(p), Some(hex(sha256_bytes(&big))), Some("entry")).err(),
        Some(ExternalError::Source(SourceError::TooLarge))
    );

    let bad = [0xff_u8, 0xfe];
    let p = s.write("bad.bin", &bad).into_os_string();
    assert_eq!(
        load(Some(p), Some(hex(sha256_bytes(&bad))), Some("entry")).err(),
        Some(ExternalError::Source(SourceError::NotUtf8))
    );
}

#[test]
fn matching_file_loads_with_computed_digest() {
    let s = Scratch::new();
    let body = b"synthetic kernel text";
    let p = s.write("k.txt", body).into_os_string();
    let digest = sha256_bytes(body);
    let k = load(Some(p), Some(hex(digest)), Some("entry_1")).expect("ok");
    assert_eq!(k.sha256(), digest);
    assert_eq!(k.entry(), "entry_1");
    assert_eq!(k.source().as_bytes(), body);
    assert!(!format!("{k:?}").contains("synthetic"));
}

#[test]
fn only_f32_cells_of_the_external_arm_have_a_launch() {
    let mut some = 0;
    for cell in gate_cells() {
        let launch = external_launch_for(cell);
        if cell.arm() == GemmGateArm::G5 && cell.dtype() == GemmInputDtype::F32 {
            let l = launch.expect("launch");
            some += 1;
            assert_eq!((l.threads_width, l.threads_height), (16, 16));
            assert_eq!(EXTERNAL_TILE, 16);
            assert_eq!(l.groups_width, cell.row().n().div_ceil(16));
            assert_eq!(l.groups_height, cell.row().m().div_ceil(16));
        } else {
            assert!(launch.is_none());
        }
    }
    assert_eq!(some, 5);
}
