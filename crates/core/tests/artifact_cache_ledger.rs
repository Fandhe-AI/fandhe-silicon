//! `artifact_cache` の台帳 `Ledger` を外の crate から確かめる試験（TASK-483 のポインタ）。
//!
//! feature なしで動く。

use fandhe_silicon_core::artifact_cache::{CacheEntry, CacheStore, Ledger, LoadOutcome};
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

/// 壊れた（1 バイト反転した）エントリ。SHA-256 は元のまま。
fn corrupted(k: CacheKey, bytes: Vec<u8>) -> CacheEntry {
    let (k, mut b, sha) = CacheEntry::new(k, bytes).into_parts();
    b[0] ^= 0xff;
    CacheEntry::from_parts(k, b, sha)
}

#[test]
fn new_and_default_are_empty_and_equal() {
    let l = Ledger::new();
    assert_eq!(l.len(), 0);
    assert!(l.is_empty());
    assert!(l.records().is_empty());
    assert_eq!(l, Ledger::default());
}

#[test]
fn record_returns_sha256_and_appends() {
    let mut l = Ledger::new();
    let got = l.record(&[1, 2, 3]);
    assert_eq!(got, sha256_bytes(&[1, 2, 3]));
    assert_eq!(l.len(), 1);
    assert!(!l.is_empty());
    assert_eq!(l.records().last(), Some(&got));
}

#[test]
fn records_keep_order() {
    let mut l = Ledger::new();
    let a = l.record(&[1]);
    let b = l.record(&[2]);
    let c = l.record(&[3]);
    assert_eq!(l.records(), &[a, b, c]);
}

#[test]
fn duplicates_are_kept() {
    let mut l = Ledger::new();
    let a = l.record(&[7, 7]);
    let b = l.record(&[7, 7]);
    assert_eq!(a, b);
    assert_eq!(l.len(), 2);
}

#[test]
fn empty_bytes_are_recorded() {
    let mut l = Ledger::new();
    let got = l.record(&[]);
    assert_eq!(got, sha256_bytes(&[]));
    assert_eq!(l.len(), 1);
}

#[test]
fn contains_reports_recorded_only() {
    let mut l = Ledger::new();
    let a = l.record(&[1]);
    assert!(l.contains(&a));
    assert!(!l.contains(&sha256_bytes(&[2])));
}

#[test]
fn records_hash_of_given_bytes_not_held_value() {
    let bad = corrupted(key(), vec![1, 2, 3]);
    let mut l = Ledger::new();
    let got = l.record(bad.bytes());
    assert_ne!(got, *bad.sha256());
    assert_eq!(got, sha256_bytes(bad.bytes()));
    assert!(!l.contains(bad.sha256()));
}

#[test]
fn hit_entry_bytes_record_its_sha256() {
    let mut store = CacheStore::new();
    store.insert(CacheEntry::new(key(), vec![4, 5, 6]));
    let mut l = Ledger::new();
    match store.load(&key()) {
        LoadOutcome::Hit(e) => {
            assert_eq!(l.record(e.bytes()), *e.sha256());
        }
        _ => panic!("expected hit"),
    }
    assert_eq!(l.len(), 1);
}

#[test]
fn clone_is_equal_until_changed() {
    let mut l = Ledger::new();
    l.record(&[1]);
    let mut c = l.clone();
    assert_eq!(l, c);
    c.record(&[2]);
    assert_ne!(l, c);
}
