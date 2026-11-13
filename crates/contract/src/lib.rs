//! fandhe-silicon の下-1 crate（チップごとの薄い呼び出し）。
//!
//! `fandhe-silicon-core` の共通型を入口に、Metal / Vulkan / CUDA の背後実装を
//! `backend-*` feature で切り替えて提供する。`unsafe` と FFI はこの crate の backend
//! モジュールに閉じ込め（`unsafe` は `cpu_ops/neon.rs` にも置く）、公開 API は safe なラッパーとして下-2・上へ渡す。
//! CPU の演算は feature `cpu-ops` の `cpu_ops` にある。

// backend と `cpu_ops/neon.rs` の外のモジュールに `unsafe` を書けなくする（一次の守り）。内側の `allow` で外せるのは
// `deny` だけなので `forbid` は使わない。個数の固定は tests/backend_*_unsafe_count.rs と
// tests/cpu_ops_neon_unsafe_count.rs
// （方針: docs/unsafe-policy.md、spec TASK-9 のポインタ）。
#![deny(unsafe_code)]

// feature ゲートのコンパイル失敗の試験（doctest のみ）。rustdoc は公開到達可能な項目の doctest しか集めないため
// `pub` にするが、`doc(hidden)` で文書には出さない。
#[doc(hidden)]
pub mod cfg_gate_doctest;

// 二重の `free` が型で拒否されることの試験（doctest のみ）。core の型へは `test-support` の構成で届くため、その裏に置く。
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod memory_free_doctest;

// コールバックの `Send` 要求が native で型として拒否されることの試験（doctest のみ）。wasm32 は `Send` を要求しないので native だけ。
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub mod callback_send_doctest;

// 背後実装のモジュール。
// Metal は feature のみでゲートする（spec BUILD-22）。
#[cfg(feature = "backend-metal")]
#[allow(unsafe_code)]
pub mod backend_metal;

// Vulkan・CUDA は wasm32 では有効にしない（spec BUILD-23）。
#[cfg(all(feature = "backend-vulkan", not(target_arch = "wasm32")))]
#[allow(unsafe_code)]
pub mod backend_vulkan;

#[cfg(all(feature = "backend-cuda", not(target_arch = "wasm32")))]
#[allow(unsafe_code)]
pub mod backend_cuda;

// CPU の演算（TASK-1351 のポインタ）。
#[cfg(feature = "cpu-ops")]
pub mod cpu_ops;

// GEMM と GEMM→加算→Relu の device の要る試験とその純粋な部品（TASK-485 のポインタ）。
#[cfg(all(test, feature = "backend-metal", feature = "test-support"))]
mod metal_flow_device_tests;
