//! fandhe-silicon の下-1 crate（チップごとの薄い呼び出し）。
//!
//! `fandhe-silicon-core` の共通型を入口に、Metal / Vulkan / CUDA の背後実装を
//! `backend-*` feature で切り替えて提供する。`unsafe` と FFI はこの crate の backend
//! モジュールに閉じ込め、公開 API は safe なラッパーとして下-2・上へ渡す。

// backend 以外のモジュールに `unsafe` を書けなくする（一次の守り）。内側の `allow` で外せるのは
// `deny` だけなので `forbid` は使わない。個数の固定は tests/backend_*_unsafe_count.rs
// （方針: docs/unsafe-policy.md、spec TASK-9 のポインタ）。
#![deny(unsafe_code)]

// feature ゲートのコンパイル失敗の試験（doctest のみ）。rustdoc は公開到達可能な項目の doctest しか集めないため
// `pub` にするが、`doc(hidden)` で文書には出さない。
#[doc(hidden)]
pub mod cfg_gate_doctest;

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
