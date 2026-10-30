//! `CommandEncoder` が `Send + Sync` であることを、型境界を要求する関数のコンパイルで固定する公開テスト（CMD-9。TASK-590 のポインタ）。
//!
//! 境界を満たさない型が同じ境界で拒まれることは、`CommandEncoder` の rustdoc にある `compile_fail` の例が確かめる。
//! 境界の検査は wasm32 でも型検査される。スレッドを起こす試験は native だけで走る。機材には触れない。

use fandhe_silicon_core::execution::CommandEncoder;

/// 型 `T` が `Send + Sync` であることをコンパイル時に要求する。
fn require_send_sync<T: Send + Sync>() {}

/// `Send` だけを要求する。別スレッドへ移すときの境界。
fn require_send<T: Send>(_: &T) {}

/// 共有参照を別スレッドへ渡すときの境界（`&T: Send` は `T: Sync` と同値）。
fn require_ref_send<T: Sync>(_: &T) {}

#[test]
fn encoder_satisfies_send_and_sync_bounds() {
    require_send_sync::<CommandEncoder>();
    let encoder = CommandEncoder::__backend_new(1, None);
    require_send(&encoder);
    require_ref_send(&encoder);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn encoder_moves_to_another_thread() {
    let encoder = CommandEncoder::__backend_new(5, None);
    let id = std::thread::spawn(move || encoder.__backend_id())
        .join()
        .expect("thread must not panic");
    assert_eq!(id, 5);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn encoder_is_shared_by_reference_across_threads() {
    let encoder = CommandEncoder::__backend_new(9, None);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| encoder.__backend_id()))
            .collect();
        for handle in handles {
            assert_eq!(handle.join().expect("thread must not panic"), 9);
        }
    });
}
