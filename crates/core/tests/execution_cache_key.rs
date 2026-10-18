//! `execution` のキャッシュキーの型を外の crate から組み立て、読み出し・等価比較・ハッシュを確かめる試験（TASK-470 のポインタ）。
//!
//! feature なしで動く。

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fandhe_silicon_core::execution::CacheKey;

/// 基準のキー（ダミーの値）。
fn base() -> CacheKey {
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

fn hash_of(key: &CacheKey) -> u64 {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    h.finish()
}

#[test]
fn execution_cache_key_keeps_fields() {
    let a = base();
    let b = a; // Copy: 元も読める
    assert_eq!(a.source_hash, 1);
    assert_eq!(a.compiler_id, "compiler-a");
    assert_eq!(a.compiler_version, 2);
    assert_eq!(b.target, "target-a");
    assert_eq!(b.options_hash, 3);
    assert!(b.bounds_checked);
    assert_eq!(b.specialization_hash, 4);
    assert_eq!(b.driver_version, Some(5));
}

#[test]
fn execution_cache_key_equal_when_all_fields_equal() {
    assert_eq!(base(), base());
}

#[test]
fn execution_cache_key_differs_when_one_field_differs() {
    let b = base();
    let variants = [
        CacheKey {
            source_hash: 9,
            ..b
        },
        CacheKey {
            compiler_id: "compiler-b",
            ..b
        },
        CacheKey {
            compiler_version: 9,
            ..b
        },
        CacheKey {
            target: "target-b",
            ..b
        },
        CacheKey {
            options_hash: 9,
            ..b
        },
        CacheKey {
            bounds_checked: false,
            ..b
        },
        CacheKey {
            specialization_hash: 9,
            ..b
        },
        CacheKey {
            driver_version: None,
            ..b
        },
    ];
    for v in &variants {
        assert_ne!(*v, b);
    }
}

#[test]
fn execution_cache_key_hash_follows_equality() {
    assert_eq!(hash_of(&base()), hash_of(&base()));
    let other = CacheKey {
        options_hash: 9,
        ..base()
    };
    let set: HashSet<CacheKey> = [base(), base(), other].into_iter().collect();
    assert_eq!(set.len(), 2);
    assert!(set.contains(&base()));
    assert!(set.contains(&other));
}
