//! 3 段で受け渡すメモリまわりの共通の部品の置き場。
//!
//! 今はバッファの引数検査の純粋関数だけを置く。背後実装の `upload`・`download`・確保が
//! 呼び出す前に使う想定で、背後実装の API は呼ばず、確保もしない（TASK-309 のポインタ）。

mod buffer_check;

pub use buffer_check::{validate_allocation_size, validate_buffer_range};
