//! バッファの用途と不透明なハンドルの型。
//!
//! 上・下-2 が確保を申告する `BufferUsage` と、背後実装（下-1）が払い出す `BufferHandle` を置く。
//! `MemoryOps`（`ops.rs`）の引数と戻り値として受け渡される（TASK-308 のポインタ）。

use core::ops::{BitOr, BitOrAssign};

/// 確保のときに申告する用途のビットの集まり（MEM-3 のポインタ）。
///
/// 内部のビットは非公開で、定数と `|` で組み立てる。生のビットからの公開コンストラクタは無い。
/// 確保した用途は `BufferHandle::usage` で読める。
/// 用途と操作の対応の検査は、この型の責務ではない（[`validate_usage_for_role`](super::validate_usage_for_role) が持つ。
/// 上の層の 12 値との対応は [`BufferUsageKind`](super::BufferUsageKind)）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferUsage(u32);

impl BufferUsage {
    /// 計算の入出力として使う記憶領域。
    pub const STORAGE: Self = Self(1 << 0);
    /// ホストからの書き込み（`upload`）の宛先にする。
    pub const UPLOAD: Self = Self(1 << 1);
    /// ホストへの読み出し（`download`）の元にする。
    pub const DOWNLOAD: Self = Self(1 << 2);
    /// 頂点データとして使う。
    pub const VERTEX: Self = Self(1 << 3);
    /// インデックスデータとして使う。
    pub const INDEX: Self = Self(1 << 4);
    /// 定数（uniform）データとして使う。
    pub const UNIFORM: Self = Self(1 << 5);
    /// 確保のときに export の意図を申告する。
    pub const EXPORTABLE: Self = Self(1 << 6);
    /// 読みの対応付けの対象にする。
    pub const MAP_READ: Self = Self(1 << 7);
    /// 書きの対応付けの対象にする。
    pub const MAP_WRITE: Self = Self(1 << 8);
    /// 間接実行の引数として使う。
    pub const INDIRECT: Self = Self(1 << 9);
    /// 問合せの解決先として使う。
    pub const QUERY_RESOLVE: Self = Self(1 << 10);
    /// 下位の加速構造の構築の入力として使う。
    pub const BLAS_INPUT: Self = Self(1 << 11);
    /// 上位の加速構造の構築の入力として使う。
    pub const TLAS_INPUT: Self = Self(1 << 12);

    /// どの用途も含まない値。
    pub const fn empty() -> Self {
        Self(0)
    }

    /// どの用途も含まないとき `true`。
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// ビット値を返す。
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// `other` のビットをすべて含むとき `true`。
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for BufferUsage {
    type Output = Self;

    /// 2 つの用途の和集合を返す。
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for BufferUsage {
    /// 用途を足す。
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// 背後実装が払い出す不透明なバッファのハンドル（MEM-7 のポインタ）。
///
/// フィールドは非公開（世代・識別子・device の識別子・確保した大きさ・用途）で、`Clone`・`Copy` を実装しない。
/// 大きさと用途は `size`・`usage` で読める（TASK-310 のポインタ）。解放は `MemoryOps::free` が値で受け取って消費する。
/// 下の `__backend_*` は背後実装（下-1）だけが使う口で、上・下-2 は使わない。
/// `__backend_new` は safe で誰でも値を作れるため、背後実装は device・id・世代を照合して拒否すること（MEM-79・MEM-80 のポインタ）。
#[derive(Debug)]
pub struct BufferHandle {
    generation: u64,
    id: u64,
    device_id: u64,
    byte_len: u64,
    usage: BufferUsage,
}

impl BufferHandle {
    /// 確保した大きさ（バイト）を返す（MEM-44 のポインタ）。
    ///
    /// `__backend_new` に渡された値をそのまま返す。背後実装を呼ばず、失敗しない。
    /// 範囲の検査の正は背後実装が持つ記録で、この値を正にしない。
    pub fn size(&self) -> u64 {
        self.byte_len
    }

    /// 確保のときに申告した用途を返す（MEM-45 のポインタ）。
    ///
    /// `__backend_new` に渡された値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn usage(&self) -> BufferUsage {
        self.usage
    }

    /// 背後実装がハンドルを作る。`byte_len`・`usage` は `allocate` に渡された値を渡す。
    #[doc(hidden)]
    pub fn __backend_new(
        generation: u64,
        id: u64,
        device_id: u64,
        byte_len: u64,
        usage: BufferUsage,
    ) -> Self {
        Self {
            generation,
            id,
            device_id,
            byte_len,
            usage,
        }
    }

    /// 背後実装が世代を読む。
    #[doc(hidden)]
    pub fn __backend_generation(&self) -> u64 {
        self.generation
    }

    /// 背後実装が識別子を読む。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// 背後実装が払い出し元の device の識別子を読む。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }
}
