//! `artifact_cache` のエントリの型を外の crate から使い、作り方・保持・分解・等価比較を確かめる試験（TASK-483 のポインタ）。
//!
//! feature なしで動く。

use fandhe_silicon_core::artifact_cache::CacheEntry;
use fandhe_silicon_core::execution::CacheKey;
use fandhe_silicon_core::hash::sha256_bytes;

/// 基準のキー（ダミーの値）。
fn key() -> CacheKey {
    CacheKey {
        source_hash: 1,
        compiler_id: "compiler-a",
        compiler_version: 2,
        target: "target-a",
        options_hash: 3,
        bounds_checked: true,
        specialization_hash: 4,
        driver_version: Some(5),
    }
}

#[test]
fn new_computes_sha256_of_bytes() {
    let bytes = b"artifact-bytes".to_vec();
    let e = CacheEntry::new(key(), bytes.clone());
    assert_eq!(e.sha256(), &sha256_bytes(&bytes));
}

#[test]
fn new_with_empty_bytes_has_known_digest() {
    let e = CacheEntry::new(key(), Vec::new());
    let expected: [u8; 32] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    assert_eq!(e.sha256(), &expected);
}

#[test]
fn new_keeps_key_and_bytes() {
    let e = CacheEntry::new(key(), vec![1, 2, 3]);
    assert_eq!(e.key(), &key());
    assert_eq!(e.bytes(), &[1, 2, 3]);
}

#[test]
fn from_parts_keeps_given_sha256_without_recompute() {
    let bogus = [7u8; 32];
    let e = CacheEntry::from_parts(key(), vec![1, 2, 3], bogus);
    assert_eq!(e.sha256(), &bogus);
    assert_ne!(e.sha256(), &sha256_bytes(e.bytes()));
}

#[test]
fn corrupted_bytes_keep_original_sha256() {
    let original = CacheEntry::new(key(), vec![10, 20, 30]);
    let (k, mut bytes, sha) = original.clone().into_parts();
    bytes[0] ^= 0xff;
    let broken = CacheEntry::from_parts(k, bytes, sha);
    assert_eq!(broken.sha256(), original.sha256());
    assert_ne!(broken.sha256(), &sha256_bytes(broken.bytes()));
}

#[test]
fn into_parts_returns_all_parts() {
    let e = CacheEntry::new(key(), vec![4, 5]);
    let sha = *e.sha256();
    let (k, b, s) = e.into_parts();
    assert_eq!(k, key());
    assert_eq!(b, vec![4, 5]);
    assert_eq!(s, sha);
}

#[test]
fn clone_is_equal_and_each_part_matters() {
    let e = CacheEntry::new(key(), vec![1, 2, 3]);
    assert_eq!(e.clone(), e);

    let mut k2 = key();
    k2.source_hash = 99;
    assert_ne!(CacheEntry::new(k2, vec![1, 2, 3]), e);
    assert_ne!(CacheEntry::new(key(), vec![1, 2, 4]), e);
    assert_ne!(CacheEntry::from_parts(key(), vec![1, 2, 3], [0; 32]), e);
}

#[test]
fn new_equals_from_parts_with_computed_sha256() {
    let bytes = vec![9, 8, 7];
    let sha = sha256_bytes(&bytes);
    assert_eq!(
        CacheEntry::new(key(), bytes.clone()),
        CacheEntry::from_parts(key(), bytes, sha)
    );
}
