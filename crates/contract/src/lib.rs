//! fandhe-silicon の下-1 crate（チップごとの薄い呼び出し）。
//!
//! `fandhe-silicon-core` の共通型を入口に、Metal / Vulkan / CUDA の背後実装を
//! `backend-*` feature で切り替えて提供する。`unsafe` と FFI はこの crate の backend
//! モジュールに閉じ込め、公開 API は safe なラッパーとして下-2・上へ渡す。
