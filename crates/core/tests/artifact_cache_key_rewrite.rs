//! `artifact_cache` の入れ物・照合・台帳を、キーの版の欄だけが違うエントリの拒否と作り直しの流れで
//! つないで確かめる試験（TASK-483 のポインタ）。
//!
//! バイト列は同じなので、作り直したバイト列の SHA-256 は古いエントリの保持値と等しくなる。
//! 台帳の全記録との照合では区別が付かず、読み込みの `KeyMismatch` で区別が付くことを示す。
//! feature なしで動く。

use fandhe_silicon_core::artifact_cache::{
    CacheEntry, CacheStore, Ledger, LoadOutcome, RejectReason,
};
use fandhe_silicon_core::execution::CacheKey;
use fandhe_silicon_core::hash::sha256_bytes;

/// 基準のキー（ダミーの値）。
fn base_key() -> CacheKey {
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

/// 版の欄を 1 つだけ変えたキーの一覧（名前付き）。
fn rewritten_keys() -> Vec<(&'static str, CacheKey)> {
    let k = base_key();
    vec![
        (
            "compiler_version",
            CacheKey {
                compiler_version: 9,
                ..k
            },
        ),
        (
            "target",
            CacheKey {
                target: "target-b",
                ..k
            },
        ),
        (
            "driver_version some->some",
            CacheKey {
                driver_version: Some(6),
                ..k
            },
        ),
        (
            "driver_version some->none",
            CacheKey {
                driver_version: None,
                ..k
            },
        ),
    ]
}

/// 正しいバイト列（ダミー）。
fn good_bytes() -> Vec<u8> {
    (0u8..16).collect()
}

/// 1 バイト反転したバイト列。
fn flipped(pos: usize) -> Vec<u8> {
    let mut b = good_bytes();
    b[pos] ^= 0xff;
    b
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

/// `load` が返すバイト列（`Hit` のときだけ）。
fn hit_bytes(store: &CacheStore, k: &CacheKey) -> Vec<u8> {
    match store.load(k) {
        LoadOutcome::Hit(e) => e.bytes().to_vec(),
        _ => panic!("expected hit"),
    }
}

#[test]
fn rewritten_key_is_rejected_and_ledger_unchanged() {
    for (name, k2) in rewritten_keys() {
        assert_ne!(base_key(), k2, "{name}");
        let mut store = CacheStore::new();
        let mut ledger = Ledger::new();
        store.insert(CacheEntry::new(base_key(), good_bytes()));
        let before = ledger.len();
        assert_eq!(
            pass_to_backend(&store, &k2, &mut ledger),
            Err(RejectReason::KeyMismatch),
            "{name}"
        );
        assert_eq!(ledger.len(), before, "{name}");
    }
}

#[test]
fn rebuilt_entry_has_same_sha256_and_one_matching_record() {
    for (name, k2) in rewritten_keys() {
        let mut store = CacheStore::new();
        let mut ledger = Ledger::new();
        let old = CacheEntry::new(base_key(), good_bytes());
        let old_sha = *old.sha256();
        assert!(store.insert(old).is_none());

        // 作り直し（コンパイラは呼ばず、同じバイト列を新しいキーで作る）。
        let rebuilt = CacheEntry::new(k2, good_bytes());
        assert_eq!(*rebuilt.sha256(), old_sha, "{name}");
        assert!(store.insert(rebuilt).is_none(), "{name}");

        assert_eq!(
            pass_to_backend(&store, &k2, &mut ledger),
            Ok(old_sha),
            "{name}"
        );
        assert_eq!(ledger.len(), 1, "{name}");
        assert_eq!(ledger.records().last(), Some(&old_sha), "{name}");
        // 5/8 の「全記録との照合で 0 件」はここでは当てはまらない。
        assert_eq!(count_matches(&ledger, &old_sha), 1, "{name}");

        // どちらのキーでも同じバイト列が返る。区別はキーでだけ付く。
        assert_eq!(
            hit_bytes(&store, &base_key()),
            hit_bytes(&store, &k2),
            "{name}"
        );
    }
}

#[test]
fn hash_mismatch_differs_but_key_rewrite_does_not() {
    let good_sha = sha256_bytes(&good_bytes());

    // バイト列が壊れた流れ: 壊れた側と作り直した側の SHA-256 は違う。
    assert_ne!(sha256_bytes(&flipped(3)), good_sha);

    // キーの版の書き換えの流れ: 古い側と作り直した側の SHA-256 は等しい。
    let (_, k2) = rewritten_keys().remove(0);
    let old = CacheEntry::new(base_key(), good_bytes());
    let rebuilt = CacheEntry::new(k2, good_bytes());
    assert_eq!(old.sha256(), rebuilt.sha256());
}

#[test]
fn bypassing_load_gives_equal_records_but_load_tells_them_apart() {
    for (name, k2) in rewritten_keys() {
        let mut store = CacheStore::new();
        store.insert(CacheEntry::new(base_key(), good_bytes()));

        // 正しい流れ: 拒否 → 作り直し → 記録。
        let mut a = Ledger::new();
        assert_eq!(
            pass_to_backend(&store, &k2, &mut a),
            Err(RejectReason::KeyMismatch),
            "{name}"
        );
        store.insert(CacheEntry::new(k2, good_bytes()));
        pass_to_backend(&store, &k2, &mut a).expect("rebuilt entry must pass");

        // 迂回: 照合を通さず古いバイト列をそのまま記録。
        let mut b = Ledger::new();
        b.record(&good_bytes());

        assert_eq!(a.records(), b.records(), "{name}");

        // 区別は読み込みの KeyMismatch で付く。
        let mut only_old = CacheStore::new();
        only_old.insert(CacheEntry::new(base_key(), good_bytes()));
        assert!(matches!(
            only_old.load(&k2),
            LoadOutcome::Rejected(RejectReason::KeyMismatch)
        ));
    }
}

#[test]
fn key_rewrite_with_corrupted_bytes_is_key_mismatch_first() {
    for (name, k2) in rewritten_keys() {
        let (k, mut b, sha) = CacheEntry::new(base_key(), good_bytes()).into_parts();
        b[0] ^= 0xff;
        let mut store = CacheStore::new();
        store.insert(CacheEntry::from_parts(k, b, sha));
        let mut ledger = Ledger::new();
        assert_eq!(
            pass_to_backend(&store, &k2, &mut ledger),
            Err(RejectReason::KeyMismatch),
            "{name}"
        );
        assert!(ledger.is_empty(), "{name}");
    }
}
