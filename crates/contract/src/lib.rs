//! fandhe-silicon の下-1 crate（チップごとの薄い呼び出し）。
//!
//! `fandhe-silicon-core` の共通型を入口に、Metal / Vulkan / CUDA の背後実装を
//! `backend-*` feature で切り替えて提供する。`unsafe` と FFI はこの crate の backend
//! モジュールに閉じ込め、公開 API は safe なラッパーとして下-2・上へ渡す。

// 背後実装のモジュール。
// Metal は feature のみでゲートし、OS 固有の分岐は将来モジュール内部で行う（spec BUILD-22）。
#[cfg(feature = "backend-metal")]
pub mod backend_metal;

// Vulkan・CUDA は wasm32 ではビルドしない（spec BUILD-23）。
#[cfg(all(feature = "backend-vulkan", not(target_arch = "wasm32")))]
pub mod backend_vulkan;

#[cfg(all(feature = "backend-cuda", not(target_arch = "wasm32")))]
pub mod backend_cuda;
