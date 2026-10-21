//! `allocate` に渡した用途と、返ったハンドルの `usage()` が 7 値のすべての組み合わせで一致することを、
//! 別の crate から確かめる（TASK-310 の 4/6。MEM-3・MEM-44・MEM-45 のポインタ）。
//!
//! 試験用の実装 `PassThroughOps` はこのファイルの中だけにあり、公開 API ではない。背後実装の
//! `allocate` が用途をハンドルへ渡すことの確かめは、この試験の範囲の外。
//! feature なしで、公開のパスだけを使う。

use std::cell::Cell;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

/// 名前・定数・期待するビット値の表。
const USAGES: [(&str, BufferUsage, u32); 7] = [
    ("STORAGE", BufferUsage::STORAGE, 1 << 0),
    ("UPLOAD", BufferUsage::UPLOAD, 1 << 1),
    ("DOWNLOAD", BufferUsage::DOWNLOAD, 1 << 2),
    ("VERTEX", BufferUsage::VERTEX, 1 << 3),
    ("INDEX", BufferUsage::INDEX, 1 << 4),
    ("UNIFORM", BufferUsage::UNIFORM, 1 << 5),
    ("EXPORTABLE", BufferUsage::EXPORTABLE, 1 << 6),
];

/// `mask` のビットが立つ定数を `|` で合わせる。
fn combine(mask: u32) -> BufferUsage {
    USAGES
        .iter()
        .filter(|(_, _, bit)| mask & bit != 0)
        .fold(BufferUsage::empty(), |acc, (_, u, _)| acc | *u)
}

/// mask から決める確保の大きさ（0 を避け、mask ごとに異なる値にする）。
fn len_for(mask: u32) -> u64 {
    u64::from(mask) * 8 + 1
}

/// 渡された用途に `extra` を足してハンドルを作る、試験用の `MemoryOps`。`extra` が空なら素通し。
struct PassThroughOps {
    extra: BufferUsage,
    calls: Cell<u32>,
}

impl PassThroughOps {
    fn new(extra: BufferUsage) -> Self {
        Self {
            extra,
            calls: Cell::new(0),
        }
    }
}

impl MemoryOps for PassThroughOps {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        self.calls.set(self.calls.get() + 1);
        let id = u64::from(self.calls.get());
        Ok(BufferHandle::__backend_new(
            0,
            id,
            1,
            byte_len,
            usage | self.extra,
        ))
    }

    fn upload(&self, _dst: &BufferHandle, _off: u64, _bytes: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _src: &BufferHandle,
        _off: u64,
        _len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _src: &BufferHandle,
        _off: u64,
        _len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _src: &BufferHandle,
        _off: u64,
        _len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, _buffer: BufferHandle) {}
}

/// 128 通りで `allocate` → `usage()` を照合し、渡した値と食い違った mask を返す。
fn mismatched_masks(ops: &dyn MemoryOps) -> Vec<u32> {
    let mut bad = Vec::new();
    for mask in 0u32..128 {
        let want = combine(mask);
        let handle = ops.allocate(len_for(mask), want).expect("allocate");
        if handle.usage() != want || handle.usage().bits() != mask {
            bad.push(mask);
        }
        ops.free(handle);
    }
    bad
}

/// 空を含む 128 通りすべてで、問合せが渡した用途・大きさを返す。
#[test]
fn usage_query_matches_allocate_for_every_combination() {
    let ops = PassThroughOps::new(BufferUsage::empty());
    let dyn_ops: &dyn MemoryOps = &ops;
    for mask in 0u32..128 {
        let want = combine(mask);
        let handle = dyn_ops.allocate(len_for(mask), want).expect("allocate");
        assert_eq!(handle.usage(), want, "mask {mask:#x}");
        assert_eq!(handle.usage().bits(), mask, "bits {mask:#x}");
        for (name, single, bit) in &USAGES {
            assert_eq!(
                handle.usage().contains(*single),
                mask & bit != 0,
                "{name} in {mask:#x}"
            );
        }
        assert_eq!(handle.size(), len_for(mask), "size {mask:#x}");
        dyn_ops.free(handle);
    }
    assert_eq!(ops.calls.get(), 128);
}

/// 7 値を 1 つずつ渡したとき、問合せが渡した定数と等しい。
#[test]
fn usage_query_matches_for_each_single_value() {
    let ops = PassThroughOps::new(BufferUsage::empty());
    let dyn_ops: &dyn MemoryOps = &ops;
    for (name, usage, bit) in &USAGES {
        let handle = dyn_ops.allocate(4, *usage).expect("allocate");
        assert_eq!(handle.usage(), *usage, "{name}");
        assert_eq!(handle.usage().bits(), *bit, "{name}");
        dyn_ops.free(handle);
    }
}

/// 同じハンドルの `usage()` を共有参照越しに繰り返し呼んでも同じ値。
#[test]
fn usage_query_repeatable_through_shared_reference() {
    let ops = PassThroughOps::new(BufferUsage::empty());
    let handle = ops
        .allocate(8, BufferUsage::STORAGE | BufferUsage::EXPORTABLE)
        .expect("allocate");
    let r = &handle;
    for _ in 0..3 {
        assert_eq!(r.usage().bits(), (1 << 0) | (1 << 6));
    }
    ops.free(handle);
}

/// 陽性対照: 用途を 1 ビット足す実装を、照合が検出する（正しい実装では空）。
#[test]
fn mismatch_check_detects_altered_usage() {
    assert!(mismatched_masks(&PassThroughOps::new(BufferUsage::empty())).is_empty());
    let bad = mismatched_masks(&PassThroughOps::new(BufferUsage::STORAGE));
    let expected: Vec<u32> = (0u32..128).filter(|m| m & 1 == 0).collect();
    assert_eq!(bad.len(), 64);
    assert_eq!(bad, expected);
}
