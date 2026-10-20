//! バッファの用途と不透明なハンドルの型。
//!
//! 上・下-2 が確保を申告する `BufferUsage` と、背後実装（下-1）が払い出す `BufferHandle` を置く。
//! `MemoryOps`（`ops.rs`）の引数と戻り値として受け渡される（TASK-308 のポインタ）。

use core::ops::{BitOr, BitOrAssign};

/// 確保のときに申告する用途のビットの集まり（MEM-3 のポインタ）。
///
/// 内部のビットは非公開で、7 つの定数と `|` で組み立てる。
/// 用途と操作の対応の検査は、この型の責務ではない。
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
/// フィールドは非公開で、`Clone`・`Copy` を実装しない。解放は `MemoryOps::free` が値で受け取って消費する。
/// 下の `__backend_*` は背後実装（下-1）だけが使う口で、上・下-2 は使わない。
/// `__backend_new` は safe で誰でも値を作れるため、背後実装は device・id・世代を照合して拒否すること（MEM-79・MEM-80 のポインタ）。
#[derive(Debug)]
pub struct BufferHandle {
    generation: u64,
    id: u64,
    device_id: u64,
}

impl BufferHandle {
    /// 背後実装がハンドルを作る。
    #[doc(hidden)]
    pub fn __backend_new(generation: u64, id: u64, device_id: u64) -> Self {
        Self {
            generation,
            id,
            device_id,
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
