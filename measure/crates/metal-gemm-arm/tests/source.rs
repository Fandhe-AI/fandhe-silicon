//! `source` module の純粋な関数と定数の試験（TASK-1008 のポインタ）。値は合成のものだけ。

use std::fs;
use std::path::PathBuf;

use fandhe_silicon_core::hash::sha256_bytes;
use measure_metal_gemm_arm::source::{
    GEMM_SOURCE_PATH, GEMM_SOURCE_SHA256, MAX_SOURCE_BYTES, SOURCE_REV, SourceError, load_source,
    verify_source, verify_source_against,
};

fn tmp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("source-{name}"))
}

#[test]
fn pinned_digest_rejects_synthetic_bytes() {
    assert_eq!(verify_source(b""), Err(SourceError::DigestMismatch));
    assert_eq!(
        verify_source(b"synthetic"),
        Err(SourceError::DigestMismatch)
    );
}

#[test]
fn oversize_is_rejected_before_digest() {
    let big = vec![b'a'; MAX_SOURCE_BYTES + 1];
    assert_eq!(
        verify_source_against(&big, &sha256_bytes(&big)),
        Err(SourceError::TooLarge)
    );
}

#[test]
fn exact_limit_is_not_rejected_by_length() {
    let max = vec![b'a'; MAX_SOURCE_BYTES];
    assert_eq!(
        verify_source_against(&max, &sha256_bytes(&max)).map(str::len),
        Ok(MAX_SOURCE_BYTES)
    );
}

#[test]
fn matching_digest_is_accepted() {
    assert_eq!(
        verify_source_against(b"abc", &sha256_bytes(b"abc")),
        Ok("abc")
    );
}

#[test]
fn non_utf8_is_rejected() {
    let bytes = [0xffu8, 0xfe];
    assert_eq!(
        verify_source_against(&bytes, &sha256_bytes(&bytes)),
        Err(SourceError::NotUtf8)
    );
    assert!(verify_source(&bytes).is_err());
}

#[test]
fn pinned_constants_have_expected_shape() {
    assert_eq!(SOURCE_REV.len(), 40);
    assert!(
        SOURCE_REV
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    );
    assert_eq!(GEMM_SOURCE_SHA256.len(), 32);
    assert!(GEMM_SOURCE_SHA256.iter().any(|b| *b != 0));
    assert!(GEMM_SOURCE_PATH.ends_with(".metal"));
    assert!(!GEMM_SOURCE_PATH.starts_with('/'));
}

#[test]
fn load_source_missing_file_is_unreadable() {
    assert_eq!(
        load_source(&tmp("missing-never-created")),
        Err(SourceError::Unreadable)
    );
}

#[test]
fn load_source_directory_is_unreadable() {
    let dir = tmp("dir");
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(load_source(&dir), Err(SourceError::Unreadable));
    fs::remove_dir(&dir).unwrap();
}

#[test]
fn load_source_synthetic_content_mismatches() {
    let p = tmp("synthetic-file");
    fs::write(&p, b"synthetic content").unwrap();
    let r = load_source(&p);
    fs::remove_file(&p).unwrap();
    assert_eq!(r, Err(SourceError::DigestMismatch));
}

#[test]
fn display_has_no_hex_or_path() {
    for e in [
        SourceError::Unreadable,
        SourceError::TooLarge,
        SourceError::DigestMismatch,
        SourceError::NotUtf8,
    ] {
        let s = e.to_string();
        assert!(!s.contains('/'));
        assert!(!s.contains("7c0127"));
    }
}
