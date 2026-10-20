//! `native-blocking-wait`・`ext-interop` を有効にした構成で、門の目印が見えることの確認。
//! 無効側は `src/cfg_gate_doctest.rs` の doctest（`compile_fail`）が受け持つ。
//! spec との対応: TASK-233 の 2/4・3/4、BUILD-11〜13（ポインタ表記）。

#[cfg(feature = "native-blocking-wait")]
#[test]
fn native_blocking_wait_gate_is_visible() {
    #[allow(unused_imports)]
    use fandhe_silicon_contract::cfg_gate_doctest::NativeBlockingWaitEnabled;
}

#[cfg(feature = "ext-interop")]
#[test]
fn ext_interop_gate_is_visible() {
    #[allow(unused_imports)]
    use fandhe_silicon_contract::cfg_gate_doctest::ExtInteropEnabled;
}
