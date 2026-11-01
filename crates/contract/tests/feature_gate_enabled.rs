//! `native-blocking-wait` を有効にした構成で core の 2 アイテムが見えること、
//! `ext-interop` を有効にした構成で門の目印が見えることの確認。
//! 無効側は `src/cfg_gate_doctest.rs` の doctest（`compile_fail`）が受け持つ。
//! spec との対応: TASK-233 の 2/4・3/4・5/7、BUILD-11〜13（ポインタ表記）。

#[cfg(feature = "native-blocking-wait")]
#[test]
fn native_blocking_wait_gate_is_visible() {
    use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
    use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
    fn probe(
        ops: &dyn MemoryOps,
        buffer: &BufferHandle,
        queue: &dyn CommandQueue,
        token: SubmitToken,
    ) {
        let _ = ops.download_blocking(buffer, 0, 4);
        let _ = queue.wait_blocking(token, None);
    }
    let _: fn(&dyn MemoryOps, &BufferHandle, &dyn CommandQueue, SubmitToken) = probe;
}

#[cfg(feature = "ext-interop")]
#[test]
fn ext_interop_gate_is_visible() {
    #[allow(unused_imports)]
    use fandhe_silicon_contract::cfg_gate_doctest::ExtInteropEnabled;
}
