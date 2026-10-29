//! 3 段で受け渡すメモリまわりの共通の部品の置き場。
//!
//! バッファの用途・ハンドル・メモリ操作のトレイト（TASK-308 のポインタ）と、
//! バッファの引数検査の純粋関数を置く。引数検査は背後実装の `upload`・`download`・確保が
//! 呼び出す前に使う想定で、背後実装の API は呼ばず、確保もしない（TASK-309 のポインタ）。
//! 上の層の用途の 12 値と契約の `BufferUsage` の対応表（`BufferUsageKind`）と、用途と操作の対応の検査の純粋関数
//! （`validate_usage_for_role`）も置く（TASK-1796 のポインタ）。
//! ハンドルからは確保した大きさと用途を読める（TASK-310 のポインタ）。

mod buffer;
mod buffer_check;
mod ops;
mod usage_check;
mod usage_mapping;

pub use buffer::{BufferHandle, BufferUsage};
pub use buffer_check::{validate_allocation_size, validate_buffer_range};
pub use ops::MemoryOps;
pub use usage_check::{UsageRole, validate_usage_for_role};
pub use usage_mapping::BufferUsageKind;
