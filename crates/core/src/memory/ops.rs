//! バッファ・メモリ操作の共通のトレイト。
//!
//! 背後実装（下-1）が実装し、下-2・上が `&dyn MemoryOps` 越しに呼ぶ（TASK-308 のポインタ）。
//! 引数の範囲の検査には `validate_buffer_range`・`validate_allocation_size`（TASK-309）を使える。

use super::{BufferHandle, BufferUsage};
use crate::error::ContractError;

/// バッファの確保・書き込み・読み出し・解放の口。
pub trait MemoryOps {
    /// `byte_len` バイトのバッファを `usage` の用途で確保する。
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError>;

    /// `bytes` を `dst` の `dst_offset` から書き込む。
    fn upload(
        &self,
        dst: &BufferHandle,
        dst_offset: u64,
        bytes: &[u8],
    ) -> Result<(), ContractError>;

    /// `src` の `src_offset` から `byte_len` バイトを、完了まで待って読み出す。native 向けで feature `native-blocking-wait` の裏にある。
    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
    ) -> Result<Vec<u8>, ContractError>;

    /// `src` の `src_offset` から `byte_len` バイトを読み出し、完了したら `callback` に渡す。
    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    );

    /// `download_async` の wasm32 版。単一スレッドなので `Send` を要求しない。
    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    );

    /// ハンドルを値で受け取って解放する。呼んだ後に同じハンドルは使えない。
    fn free(&self, buffer: BufferHandle);

    /// 解放待ちの個数。観測できない背後実装は既定の `None` を返す（MEM-82 のポインタ）。
    fn pending_release_count(&self) -> Option<u64> {
        None
    }
}
