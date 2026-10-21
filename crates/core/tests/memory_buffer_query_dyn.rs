//! `&dyn MemoryOps` 越しに `allocate` へ渡した大きさ・用途が `size()`・`usage()` で読めることの確認（TASK-310 の 3/6、MEM-44・MEM-45 のポインタ）。
//!
//! 試験用の実装はこのファイルの中だけにあり、公開 API ではない。feature なしで公開のパスだけを使う。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

/// 受けた引数をそのままハンドルへ渡す最小の実装。記憶域は確保しない。
struct PassThroughOps;

impl MemoryOps for PassThroughOps {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(0, 1, 1, byte_len, usage))
    }

    fn upload(&self, _: &BufferHandle, _: u64, _: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, _: BufferHandle) {}
}

/// 陽性対照用。わざと大きさと用途をずらして渡す。
struct SkewedOps;

impl MemoryOps for SkewedOps {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(
            0,
            1,
            1,
            byte_len.wrapping_add(1),
            usage | BufferUsage::UNIFORM,
        ))
    }

    fn upload(&self, _: &BufferHandle, _: u64, _: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, _: BufferHandle) {}
}

const LENS: [u64; 5] = [0, 1, 16, 4096, u64::MAX];

fn usages() -> Vec<BufferUsage> {
    vec![
        BufferUsage::empty(),
        BufferUsage::STORAGE,
        BufferUsage::UPLOAD,
        BufferUsage::EXPORTABLE,
        BufferUsage::STORAGE | BufferUsage::UPLOAD,
        BufferUsage::VERTEX | BufferUsage::INDEX | BufferUsage::DOWNLOAD,
    ]
}

/// 渡した値と読めた値が食い違う組を集める。
fn mismatches(ops: &dyn MemoryOps) -> Vec<(u64, u32)> {
    let mut bad = Vec::new();
    for len in LENS {
        for usage in usages() {
            let h = ops.allocate(len, usage).expect("allocate");
            if h.size() != len || h.usage() != usage {
                bad.push((len, usage.bits()));
            }
        }
    }
    bad
}

#[test]
fn allocate_through_dyn_returns_given_size() {
    let ops: &dyn MemoryOps = &PassThroughOps;
    for len in LENS {
        let h = ops.allocate(len, BufferUsage::STORAGE).expect("allocate");
        assert_eq!(h.size(), len);
    }
}

#[test]
fn allocate_through_dyn_returns_given_usage() {
    let ops: &dyn MemoryOps = &PassThroughOps;
    for usage in usages() {
        let h = ops.allocate(16, usage).expect("allocate");
        assert_eq!(h.usage(), usage);
    }
}

#[test]
fn allocate_through_generic_matches_dyn() {
    fn alloc<O: MemoryOps + ?Sized>(ops: &O, len: u64, usage: BufferUsage) -> (u64, BufferUsage) {
        let h = ops.allocate(len, usage).expect("allocate");
        (h.size(), h.usage())
    }
    let dyn_ops: &dyn MemoryOps = &PassThroughOps;
    for len in LENS {
        for usage in usages() {
            assert_eq!(alloc(&PassThroughOps, len, usage), (len, usage));
            assert_eq!(alloc(dyn_ops, len, usage), (len, usage));
        }
    }
}

#[test]
fn size_usage_survive_until_free() {
    let ops: &dyn MemoryOps = &PassThroughOps;
    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD;
    let h = ops.allocate(8, usage).expect("allocate");
    ops.upload(&h, 0, &[1, 2, 3]).expect("upload");
    assert_eq!(h.size(), 8);
    assert_eq!(h.usage(), usage);
    ops.free(h);
}

#[test]
fn mismatch_detects_wrong_pass_through() {
    assert!(mismatches(&PassThroughOps).is_empty());
    assert!(!mismatches(&SkewedOps).is_empty());
}
