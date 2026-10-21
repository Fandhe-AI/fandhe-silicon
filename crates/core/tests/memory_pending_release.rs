//! `pending_release_count` を上書きしない最小の `MemoryOps` が既定の `None` を返し、
//! `Some(0)` と区別できることを、別の crate から確かめる（TASK-308 の 5/6。MEM-82 のポインタ）。
//!
//! 試験用の実装はこのファイルの中だけにあり、公開 API ではない。
//! feature なしで、公開のパスだけを使う。

use std::cell::Cell;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

/// 解放待ちの個数の観測結果を 3 つに分ける。`None` と `Some(0)` を取り違えないための分類。
#[derive(Debug, PartialEq)]
enum Observed {
    Unobservable,
    Zero,
    Pending(u64),
}

/// `pending_release_count` の戻り値を分類する。
fn classify(v: Option<u64>) -> Observed {
    match v {
        None => Observed::Unobservable,
        Some(0) => Observed::Zero,
        Some(n) => Observed::Pending(n),
    }
}

/// 総称の文脈（サイズ不定を含む）から `pending_release_count` を呼ぶ。
fn count_of<T: MemoryOps + ?Sized>(ops: &T) -> Option<u64> {
    ops.pending_release_count()
}

/// `pending_release_count` を上書きしない、試験用の最小の `MemoryOps`。
#[derive(Default)]
struct DefaultOps {
    next_id: Cell<u64>,
}

/// `pending_release_count` を `Some(count)` に上書きする、対照用の `MemoryOps`。
struct OverrideOps {
    count: u64,
}

/// 必須のメソッドを何もしない形で実装する（2 つの試験用の実装で同じ形）。
macro_rules! stub_required_methods {
    () => {
        fn upload(
            &self,
            _dst: &BufferHandle,
            _off: u64,
            _bytes: &[u8],
        ) -> Result<(), ContractError> {
            Ok(())
        }

        #[cfg(feature = "native-blocking-wait")]
        fn download_blocking(
            &self,
            _src: &BufferHandle,
            _off: u64,
            _len: u64,
        ) -> Result<Vec<u8>, ContractError> {
            Ok(Vec::new())
        }

        #[cfg(not(target_arch = "wasm32"))]
        fn download_async(
            &self,
            _src: &BufferHandle,
            _off: u64,
            _len: u64,
            callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
        ) {
            callback(Ok(Vec::new()));
        }

        #[cfg(target_arch = "wasm32")]
        fn download_async(
            &self,
            _src: &BufferHandle,
            _off: u64,
            _len: u64,
            callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
        ) {
            callback(Ok(Vec::new()));
        }

        fn free(&self, _buffer: BufferHandle) {}
    };
}

impl MemoryOps for DefaultOps {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        let id = self.next_id.get().checked_add(1).expect("id fits u64");
        self.next_id.set(id);
        Ok(BufferHandle::__backend_new(0, id, 1, byte_len, usage))
    }

    stub_required_methods!();
}

impl MemoryOps for OverrideOps {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(0, 1, 1, byte_len, usage))
    }

    fn pending_release_count(&self) -> Option<u64> {
        Some(self.count)
    }

    stub_required_methods!();
}

/// 直接呼んでも既定の実装は `None`。
#[test]
fn default_impl_returns_none_direct() {
    assert_eq!(DefaultOps::default().pending_release_count(), None);
}

/// `&dyn MemoryOps` 越しでも `None`。
#[test]
fn default_impl_returns_none_through_dyn() {
    let ops = DefaultOps::default();
    let dynamic: &dyn MemoryOps = &ops;
    assert_eq!(dynamic.pending_release_count(), None);
}

/// 総称の関数越しでも `None`。
#[test]
fn default_impl_returns_none_through_generic() {
    let ops = DefaultOps::default();
    assert_eq!(count_of(&ops), None);
    let dynamic: &dyn MemoryOps = &ops;
    assert_eq!(count_of(dynamic), None);
}

/// `allocate`・`free` の前後でも `None` のまま（既定の実装は状態を見ない）。
#[test]
fn default_impl_stays_none_after_allocate_and_free() {
    let ops = DefaultOps::default();
    assert_eq!(ops.pending_release_count(), None);
    let handle = ops.allocate(16, BufferUsage::STORAGE).expect("allocate");
    assert_eq!(ops.pending_release_count(), None);
    ops.free(handle);
    assert_eq!(ops.pending_release_count(), None);
}

/// 既定の `None` は、上書きした `Some(0)` と値としても分類としても区別できる。
#[test]
fn default_none_is_distinct_from_some_zero() {
    let default_value = DefaultOps::default().pending_release_count();
    let zero_value = OverrideOps { count: 0 }.pending_release_count();
    assert_eq!(default_value, None);
    assert_eq!(zero_value, Some(0));
    assert_ne!(default_value, zero_value);
    assert!(default_value.is_none());
    assert!(zero_value.is_some());
    assert_eq!(classify(default_value), Observed::Unobservable);
    assert_eq!(classify(zero_value), Observed::Zero);
}

/// 0 でない個数を返す上書きは、`Zero` とも `Unobservable` とも違う。
#[test]
fn override_reports_some_values() {
    let value = OverrideOps { count: 3 }.pending_release_count();
    assert_eq!(value, Some(3));
    assert_eq!(classify(value), Observed::Pending(3));
    assert_ne!(classify(value), Observed::Zero);
    assert_ne!(classify(value), Observed::Unobservable);
}

/// 陽性対照。既定の実装が仮に `Some(0)` を返していたら、分類が「観測できない」と判定しないこと。
#[test]
fn classify_detects_wrong_expectation() {
    assert_ne!(classify(Some(0)), Observed::Unobservable);
    assert_ne!(classify(None), Observed::Zero);
}
