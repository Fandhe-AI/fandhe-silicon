//! `native-blocking-wait` を有効にした構成で core の 2 アイテムが見えること、
//! `ext-interop` を有効にした構成で core の `ExternalInterop` が見えることの確認。
//! `cpu-ops` を有効にした構成で契約の crate の `cpu_ops` の型が見えることも確かめる。
//! 無効側は `src/cfg_gate_doctest.rs` の doctest（`compile_fail`）が受け持つ。
//! spec との対応: TASK-233 の 2/4・3/4・5/7・6/7、TASK-2457、BUILD-11〜13（ポインタ表記）。

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
    use fandhe_silicon_core::interop::ExternalInterop;
    fn probe(_: &dyn ExternalInterop) {}
    let _: fn(&dyn ExternalInterop) = probe;
}

#[cfg(feature = "cpu-ops")]
#[test]
fn cpu_ops_gate_is_visible() {
    use fandhe_silicon_contract::cpu_ops::{
        CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp,
    };
    fn probe(_: &dyn CpuOps) {}
    let _: fn(&dyn CpuOps) = probe;
    probe(&DefaultCpuOps);
    probe(&ReferenceCpuOps);
    let _ = UnaryElementwiseOp::Relu;
    let _ = UnaryElementwiseOp::AddScalar(1.0);
}
