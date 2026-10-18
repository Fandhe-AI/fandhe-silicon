//! `artifact_cache` の入れ物・照合・台帳を、壊れた成果物の拒否と作り直しの流れでつないで確かめる試験
//! （TASK-483 のポインタ）。
//!
//! 読み込みで拒否したバイト列が台帳に残らず、作り直したバイト列だけが記録されることを、
//! 件数の比較と全記録との照合の両方で示す。feature なしで動く。

use fandhe_silicon_core::artifact_cache::{
    CacheEntry, CacheStore, Ledger, LoadOutcome, RejectReason,
};
use fandhe_silicon_core::execution::CacheKey;
use fandhe_silicon_core::hash::sha256_bytes;

/// 基準のキー（ダミーの値）。`n` でキーだけを変える。
fn key(n: u64) -> CacheKey {
    CacheKey {
        source_hash: n,
        compiler_id: "compiler-a",
        compiler_version: 2,
        target: "target-a",
        options_hash: 3,
        bounds_checked: true,
        specialization_hash: 4,
        driver_version: Some(5),
    }
}

/// 正しいバイト列（ダミー）。
fn good_bytes() -> Vec<u8> {
    (0u8..16).collect()
}

/// `pos` のバイトを反転したバイト列。
fn flipped(pos: usize) -> Vec<u8> {
    let mut b = good_bytes();
    b[pos] ^= 0xff;
    b
}

/// `pos` のバイトを反転した壊れたエントリ。SHA-256 は元のまま。
fn corrupted_at(k: CacheKey, pos: usize) -> CacheEntry {
    let (k, mut b, sha) = CacheEntry::new(k, good_bytes()).into_parts();
    b[pos] ^= 0xff;
    CacheEntry::from_parts(k, b, sha)
}

/// 読み込みを通ったバイト列だけを台帳に記録し、その SHA-256 を返す（背後実装へ渡す手前の流れ）。
///
/// 拒否したときは台帳に触れずに理由を返す。
fn pass_to_backend(
    store: &CacheStore,
    k: &CacheKey,
    ledger: &mut Ledger,
) -> Result<[u8; 32], RejectReason> {
    match store.load(k) {
        LoadOutcome::Hit(e) => Ok(ledger.record(e.bytes())),
        LoadOutcome::Rejected(r) => Err(r),
        _ => panic!("unexpected outcome"),
    }
}

/// 台帳の全記録を走査して、`sha` と一致する記録の数を返す。
fn count_matches(ledger: &Ledger, sha: &[u8; 32]) -> usize {
    ledger.records().iter().filter(|r| *r == sha).count()
}

/// 別のキーの正しいエントリを 1 件通した入れ物と台帳（比較が空同士にならないように）。
fn seeded() -> (CacheStore, Ledger) {
    let mut store = CacheStore::new();
    let mut ledger = Ledger::new();
    store.insert(CacheEntry::new(key(100), vec![9, 9, 9]));
    pass_to_backend(&store, &key(100), &mut ledger).expect("seed entry must pass");
    assert_eq!(ledger.len(), 1);
    (store, ledger)
}

/// 壊れたエントリの拒否と作り直しを通して、件数と 0 件を確かめる。
fn run_flow(pos: usize) {
    let (mut store, mut ledger) = seeded();
    let k = key(1);
    let bad_sha = sha256_bytes(&flipped(pos));
    let good_sha = sha256_bytes(&good_bytes());
    assert_ne!(bad_sha, good_sha);

    store.insert(corrupted_at(k, pos));
    let before = ledger.len();
    assert_eq!(
        pass_to_backend(&store, &k, &mut ledger),
        Err(RejectReason::HashMismatch)
    );
    assert_eq!(ledger.len(), before);
    assert_eq!(count_matches(&ledger, &bad_sha), 0);

    let replaced = store.insert(CacheEntry::new(k, good_bytes()));
    assert!(replaced.is_some());
    assert_eq!(pass_to_backend(&store, &k, &mut ledger), Ok(good_sha));
    assert_eq!(ledger.len(), before + 1);
    assert_eq!(ledger.records().last(), Some(&good_sha));
    assert_eq!(count_matches(&ledger, &bad_sha), 0);
    assert!(!ledger.contains(&bad_sha));
    assert_eq!(count_matches(&ledger, &good_sha), 1);
}

#[test]
fn counts_before_reject_after_reject_and_after_rebuild() {
    run_flow(0);
}

#[test]
fn corrupted_sha256_is_absent_from_every_record() {
    run_flow(7);
}

#[test]
fn corruption_at_first_middle_last_byte() {
    let n = good_bytes().len();
    for pos in [0, n / 2, n - 1] {
        run_flow(pos);
    }
}

#[test]
fn repeated_rejections_add_no_records() {
    let (mut store, mut ledger) = seeded();
    store.insert(corrupted_at(key(1), 3));
    let bad_sha = sha256_bytes(&flipped(3));
    let before = ledger.len();
    for _ in 0..3 {
        assert_eq!(
            pass_to_backend(&store, &key(1), &mut ledger),
            Err(RejectReason::HashMismatch)
        );
        assert_eq!(ledger.len(), before);
        assert_eq!(count_matches(&ledger, &bad_sha), 0);
    }
}

#[test]
fn other_entries_in_store_are_unaffected() {
    let (mut store, mut ledger) = seeded();
    for n in 10..13u64 {
        store.insert(CacheEntry::new(key(n), vec![n as u8; 4]));
    }
    store.insert(corrupted_at(key(1), 5));
    let bad_sha = sha256_bytes(&flipped(5));

    for n in 10..13u64 {
        assert_eq!(
            pass_to_backend(&store, &key(n), &mut ledger),
            Ok(sha256_bytes(&[n as u8; 4]))
        );
    }
    assert_eq!(
        pass_to_backend(&store, &key(1), &mut ledger),
        Err(RejectReason::HashMismatch)
    );
    assert_eq!(ledger.len(), 4);
    assert_eq!(count_matches(&ledger, &bad_sha), 0);
}

#[test]
fn check_detects_ledger_that_recorded_corrupted_bytes() {
    // 陽性対照: 照合を通さず壊れたバイト列を記録した台帳では、同じ確かめが 1 件を数える。
    let bad = flipped(0);
    let mut ledger = Ledger::new();
    ledger.record(&bad);
    assert_eq!(count_matches(&ledger, &sha256_bytes(&bad)), 1);
}

#[test]
fn equal_counts_do_not_rule_out_corrupted_record() {
    let bad = flipped(0);
    let bad_sha = sha256_bytes(&bad);

    // 台帳 A: 拒否 → 作り直し → 記録。
    let mut store = CacheStore::new();
    let mut a = Ledger::new();
    store.insert(corrupted_at(key(1), 0));
    assert!(pass_to_backend(&store, &key(1), &mut a).is_err());
    store.insert(CacheEntry::new(key(1), good_bytes()));
    pass_to_backend(&store, &key(1), &mut a).expect("rebuilt entry must pass");

    // 台帳 B: 拒否したのに壊れたバイト列を記録し、作り直しは記録しない。
    let mut b = Ledger::new();
    b.record(&bad);

    assert_eq!(a.len(), b.len());
    assert_eq!(count_matches(&a, &bad_sha), 0);
    assert_eq!(count_matches(&b, &bad_sha), 1);
}
