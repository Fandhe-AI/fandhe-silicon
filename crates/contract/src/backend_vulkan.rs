//! Vulkan の背後実装（下-1）。`backend-vulkan` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! 現状は骨格のみで、チップのバインディング crate への依存は未導入（依存の追加はユーザー承認制）。
//! wasm32 ではコンパイル対象から外す（`lib.rs` の `cfg`）。
//! 子モジュール `spirv_operands` は検証用の道具で、feature `test-support` の裏にだけ公開される
//! （TASK-98。説明は `docs/vulkan-operand-extraction.md`）。
//!
//! spec との対応: TASK-5（ポインタ表記）。

#[cfg(feature = "test-support")]
pub mod spirv_operands;
