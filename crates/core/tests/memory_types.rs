//! バッファ・メモリ操作の公開の型を、別の crate から作って呼べることの最小の確認（TASK-308 の 2/6）。

use std::cell::RefCell;
use std::collections::HashMap;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

#[derive(Default)]
struct Stub {
    store: RefCell<HashMap<u64, Vec<u8>>>,
}

impl Stub {
    fn read(&self, h: &BufferHandle, offset: u64, len: u64) -> Vec<u8> {
        let store = self.store.borrow();
        let buf = store.get(&h.__backend_id()).expect("known handle");
        let start = offset as usize;
        buf[start..start + len as usize].to_vec()
    }
}

impl MemoryOps for Stub {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        let id = self.store.borrow().len() as u64 + 1;
        self.store
            .borrow_mut()
            .insert(id, vec![0; byte_len as usize]);
        Ok(BufferHandle::__backend_new(0, id, 1, byte_len, usage))
    }

    fn upload(
        &self,
        dst: &BufferHandle,
        dst_offset: u64,
        bytes: &[u8],
    ) -> Result<(), ContractError> {
        let mut store = self.store.borrow_mut();
        let buf = store.get_mut(&dst.__backend_id()).expect("known handle");
        let start = dst_offset as usize;
        buf[start..start + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(self.read(src, src_offset, byte_len))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(self.read(src, src_offset, byte_len)));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        src: &BufferHandle,
        src_offset: u64,
        byte_len: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(self.read(src, src_offset, byte_len)));
    }

    fn free(&self, buffer: BufferHandle) {
        self.store.borrow_mut().remove(&buffer.__backend_id());
    }
}

#[test]
fn memory_ops_allocate_upload_download_free_through_dyn() {
    let stub = Stub::default();
    let ops: &dyn MemoryOps = &stub;
    let buf = ops
        .allocate(8, BufferUsage::STORAGE | BufferUsage::UPLOAD)
        .expect("allocate");
    ops.upload(&buf, 2, &[1, 2, 3]).expect("upload");

    #[cfg(feature = "native-blocking-wait")]
    assert_eq!(
        ops.download_blocking(&buf, 2, 3).expect("blocking"),
        vec![1, 2, 3]
    );

    let got = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = std::sync::Arc::clone(&got);
    ops.download_async(
        &buf,
        2,
        3,
        Box::new(move |r| {
            *sink.lock().expect("lock") = Some(r);
        }),
    );
    let taken = got.lock().expect("lock").take().expect("callback ran");
    assert_eq!(taken.expect("download"), vec![1, 2, 3]);

    ops.free(buf);
    assert!(stub.store.borrow().is_empty());
}

#[test]
fn buffer_usage_combines_with_bitor() {
    let mut usage = BufferUsage::empty();
    assert!(usage.is_empty());
    usage |= BufferUsage::STORAGE;
    let both = usage | BufferUsage::DOWNLOAD;
    assert!(!both.is_empty());
    assert!(both.contains(BufferUsage::STORAGE));
    assert!(both.contains(BufferUsage::DOWNLOAD));
    assert!(!both.contains(BufferUsage::UPLOAD));
    assert_ne!(both.bits(), usage.bits());
}

#[test]
fn buffer_handle_backend_accessors_round_trip() {
    let h = BufferHandle::__backend_new(3, 5, 7, 64, BufferUsage::STORAGE);
    assert_eq!(h.__backend_generation(), 3);
    assert_eq!(h.__backend_id(), 5);
    assert_eq!(h.__backend_device_id(), 7);
    assert_eq!(h.size(), 64);
    assert_eq!(h.usage(), BufferUsage::STORAGE);
}
