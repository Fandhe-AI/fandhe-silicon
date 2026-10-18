//! `artifact_cache` の読み込み時の照合と入れ物を外の crate から確かめる試験（TASK-483 のポインタ）。
//!
//! feature なしで動く。

use fandhe_silicon_core::artifact_cache::{
    CacheEntry, CacheStore, LoadOutcome, RejectReason, verify_entry,
};
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
fn verify_ok_for_same_key() {
    let e = CacheEntry::new(key(), vec![1, 2, 3]);
    assert_eq!(verify_entry(&e, &key()), Ok(()));
}

#[test]
fn verify_ok_for_empty_bytes() {
    let e = CacheEntry::new(key(), Vec::new());
    assert_eq!(verify_entry(&e, &key()), Ok(()));
}

#[test]
fn verify_key_mismatch_for_each_field() {
    let e = CacheEntry::new(key(), vec![1, 2, 3]);
    let variants = [
        CacheKey {
            source_hash: 9,
            ..key()
        },
        CacheKey {
            compiler_id: "compiler-b",
            ..key()
        },
        CacheKey {
            compiler_version: 9,
            ..key()
        },
        CacheKey {
            target: "target-b",
            ..key()
        },
        CacheKey {
            options_hash: 9,
            ..key()
        },
        CacheKey {
            bounds_checked: false,
            ..key()
        },
        CacheKey {
            specialization_hash: 9,
            ..key()
        },
        CacheKey {
            driver_version: None,
            ..key()
        },
        CacheKey {
            driver_version: Some(6),
            ..key()
        },
    ];
    for v in variants {
        assert_eq!(verify_entry(&e, &v), Err(RejectReason::KeyMismatch));
    }
}

#[test]
fn verify_hash_mismatch_for_flipped_byte() {
    let e = corrupted(key(), vec![1, 2, 3]);
    assert_eq!(verify_entry(&e, &key()), Err(RejectReason::HashMismatch));
}

#[test]
fn verify_hash_mismatch_for_bogus_digest() {
    let e = CacheEntry::from_parts(key(), vec![1, 2, 3], [7u8; 32]);
    assert_eq!(verify_entry(&e, &key()), Err(RejectReason::HashMismatch));
}

#[test]
fn verify_checks_key_before_hash() {
    let e = corrupted(key(), vec![1, 2, 3]);
    let other = CacheKey {
        source_hash: 9,
        ..key()
    };
    assert_eq!(verify_entry(&e, &other), Err(RejectReason::KeyMismatch));
}

#[test]
fn store_empty_rejects_with_key_mismatch() {
    let s = CacheStore::new();
    assert_eq!(
        s.load(&key()),
        LoadOutcome::Rejected(RejectReason::KeyMismatch)
    );
}

#[test]
fn store_hit_returns_inserted_entry() {
    let mut s = CacheStore::new();
    assert!(s.insert(CacheEntry::new(key(), vec![4, 5, 6])).is_none());
    match s.load(&key()) {
        LoadOutcome::Hit(e) => {
            assert_eq!(e.bytes(), &[4, 5, 6]);
            assert_eq!(e.sha256(), &sha256_bytes(&[4, 5, 6]));
        }
        other => panic!("expected Hit, got {other:?}"),
    }
}

#[test]
fn store_keeps_entries_with_different_keys_apart() {
    let k2 = CacheKey {
        options_hash: 99,
        ..key()
    };
    let mut s = CacheStore::new();
    s.insert(CacheEntry::new(key(), vec![1]));
    assert_eq!(
        s.load(&k2),
        LoadOutcome::Rejected(RejectReason::KeyMismatch)
    );
    s.insert(CacheEntry::new(k2, vec![2]));
    match (s.load(&key()), s.load(&k2)) {
        (LoadOutcome::Hit(a), LoadOutcome::Hit(b)) => {
            assert_eq!(a.bytes(), &[1]);
            assert_eq!(b.bytes(), &[2]);
            assert_ne!(a.sha256(), b.sha256());
        }
        other => panic!("expected two Hits, got {other:?}"),
    }
}

#[test]
fn store_rejects_corrupted_then_recovers_after_replace() {
    let mut s = CacheStore::new();
    s.insert(corrupted(key(), vec![1, 2, 3]));
    assert_eq!(
        s.load(&key()),
        LoadOutcome::Rejected(RejectReason::HashMismatch)
    );
    let prev = s.insert(CacheEntry::new(key(), vec![1, 2, 3]));
    assert!(prev.is_some());
    assert!(matches!(s.load(&key()), LoadOutcome::Hit(_)));
}

#[test]
fn store_entry_with_rewritten_key_is_not_found_by_original_key() {
    let (_, bytes, sha) = CacheEntry::new(key(), vec![1, 2, 3]).into_parts();
    let other = CacheKey {
        source_hash: 9,
        ..key()
    };
    let mut s = CacheStore::new();
    s.insert(CacheEntry::from_parts(other, bytes, sha));
    assert_eq!(
        s.load(&key()),
        LoadOutcome::Rejected(RejectReason::KeyMismatch)
    );
}

#[test]
fn store_load_does_not_change_contents() {
    let mut s = CacheStore::new();
    s.insert(corrupted(key(), vec![1, 2, 3]));
    let first = s.load(&key());
    let second = s.load(&key());
    assert_eq!(first, second);
    assert_eq!(first, LoadOutcome::Rejected(RejectReason::HashMismatch));
}

/// `#[non_exhaustive]` なので、外の crate の `match` には `_` の腕が要る。
#[test]
fn enums_need_wildcard_arm_outside_crate() {
    let r = RejectReason::KeyMismatch;
    let name = match r {
        RejectReason::KeyMismatch => "key",
        RejectReason::HashMismatch => "hash",
        _ => "other",
    };
    assert_eq!(name, "key");
    let o = LoadOutcome::Rejected(r);
    let rejected = match o {
        LoadOutcome::Hit(_) => false,
        LoadOutcome::Rejected(_) => true,
        _ => false,
    };
    assert!(rejected);
}
