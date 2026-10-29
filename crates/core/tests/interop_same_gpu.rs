//! 同一 GPU の判定 `check_same_physical_gpu` と、取り込み前の拒否を呼び出しの計数で確かめる公開テスト（TASK-145）。
//!
//! 計数つきのスタブはこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: 判定の直接の呼び出しの結果（DEV-76）と、`import_memory_checked` 経由で
//! 一致なら `import_memory` が 1 回、拒否なら 0 回であること。

use std::cell::Cell;

use fandhe_silicon_core::error::{ContractError, IdentityMismatchReason};
use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
use fandhe_silicon_core::interop::{
    ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle, PhysicalGpuId,
    check_same_physical_gpu, import_memory_checked,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

/// `import_memory` の呼び出しを数えるスタブ。
struct CountingBackend {
    import_calls: Cell<u32>,
}

impl CountingBackend {
    fn new() -> Self {
        Self {
            import_calls: Cell::new(0),
        }
    }
}

impl ExternalInterop for CountingBackend {
    fn export_memory(&self, _: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> {
        unimplemented!()
    }

    fn import_memory(
        &self,
        external: ExternalMemoryHandle,
        byte_len: u64,
        usage: BufferUsage,
    ) -> Result<BufferHandle, ContractError> {
        self.import_calls.set(self.import_calls.get() + 1);
        Ok(BufferHandle::__backend_new(
            1,
            external.__backend_id(),
            1,
            byte_len,
            usage,
        ))
    }

    fn export_semaphore(&self, _: SubmitToken) -> Result<ExternalSemaphoreHandle, ContractError> {
        unimplemented!()
    }

    fn import_semaphore(
        &self,
        _: ExternalSemaphoreHandle,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        unimplemented!()
    }
}

fn gpu() -> PhysicalGpuId {
    PhysicalGpuId {
        device_uuid: [7; 16],
        driver_uuid: Some([9; 16]),
    }
}

fn mismatch(reason: IdentityMismatchReason) -> ContractError {
    ContractError::DeviceIdentityMismatch { reason }
}

/// DEV-76: 同じ値は `Ok`。
#[test]
fn same_gpu_is_accepted() {
    assert!(check_same_physical_gpu(Some(&gpu()), Some(&gpu())).is_ok());
}

/// DEV-76: UUID を 1 バイト変えると `Mismatch`。
#[test]
fn one_byte_changed_uuid_is_mismatch() {
    let mut other = gpu();
    other.device_uuid[0] ^= 1;
    let err = check_same_physical_gpu(Some(&gpu()), Some(&other)).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Mismatch));
}

/// DEV-76: 識別子が取れない（`None`）は、どちらの側でも `Undeterminable`。
#[test]
fn missing_identity_is_undeterminable() {
    let expected = mismatch(IdentityMismatchReason::Undeterminable);
    assert_eq!(
        check_same_physical_gpu(None, Some(&gpu())).unwrap_err(),
        expected
    );
    assert_eq!(
        check_same_physical_gpu(Some(&gpu()), None).unwrap_err(),
        expected
    );
    assert_eq!(check_same_physical_gpu(None, None).unwrap_err(), expected);
}

/// DEV-76: 一致では `import_memory` が 1 回。
#[test]
fn match_calls_import_memory_once() {
    let backend = CountingBackend::new();
    let handle = ExternalMemoryHandle::__backend_new(5, false, Some(gpu()));
    let buffer = import_memory_checked(&backend, Some(&gpu()), handle, 32, BufferUsage::STORAGE)
        .expect("import");
    assert_eq!(buffer.size(), 32);
    assert_eq!(backend.import_calls.get(), 1);
}

/// DEV-76: `Mismatch` の拒否では `import_memory` が 0 回。
#[test]
fn mismatch_rejects_before_import_memory() {
    let backend = CountingBackend::new();
    let mut target = gpu();
    target.device_uuid[15] ^= 0x80;
    let handle = ExternalMemoryHandle::__backend_new(5, false, Some(gpu()));
    let err = import_memory_checked(&backend, Some(&target), handle, 32, BufferUsage::STORAGE)
        .unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Mismatch));
    assert_eq!(backend.import_calls.get(), 0);
}

/// DEV-76: `None` の拒否（取り込み先が `None`、ハンドルの欄が `None`）では `import_memory` が 0 回。
#[test]
fn undeterminable_rejects_before_import_memory() {
    let backend = CountingBackend::new();
    let handle = ExternalMemoryHandle::__backend_new(5, false, Some(gpu()));
    let err = import_memory_checked(&backend, None, handle, 32, BufferUsage::STORAGE).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));

    let handle = ExternalMemoryHandle::__backend_new(5, false, None);
    let err = import_memory_checked(&backend, Some(&gpu()), handle, 32, BufferUsage::STORAGE)
        .unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));
    assert_eq!(backend.import_calls.get(), 0);
}

fn with_driver(driver_uuid: Option<[u8; 16]>) -> PhysicalGpuId {
    PhysicalGpuId {
        device_uuid: [7; 16],
        driver_uuid,
    }
}

/// INTEROP-8: `device_uuid` が同じで両方の `driver_uuid` が `Some` かつ違えば `Mismatch`。
#[test]
fn differing_driver_uuids_are_mismatch() {
    let (a, b) = (with_driver(Some([1; 16])), with_driver(Some([2; 16])));
    let err = check_same_physical_gpu(Some(&a), Some(&b)).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Mismatch));
}

/// INTEROP-9: `driver_uuid` が片方だけ `None` なら（左右どちらでも）比べず `Ok`。
#[test]
fn one_sided_missing_driver_uuid_is_accepted() {
    let (some, none) = (with_driver(Some([1; 16])), with_driver(None));
    assert!(check_same_physical_gpu(Some(&some), Some(&none)).is_ok());
    assert!(check_same_physical_gpu(Some(&none), Some(&some)).is_ok());
}

/// INTEROP-65: 両方の `driver_uuid` が `None` なら `Ok`。
#[test]
fn both_missing_driver_uuid_is_accepted() {
    let none = with_driver(None);
    assert!(check_same_physical_gpu(Some(&none), Some(&none)).is_ok());
}

/// 拒否されたハンドルの fd は閉じられ、pipe の読み側が EOF を得て、`import_memory` は 0 回。
#[cfg(unix)]
#[test]
fn rejected_handle_closes_its_fd() {
    use std::io::Read;
    use std::os::fd::OwnedFd;

    let (mut reader, writer) = std::io::pipe().expect("pipe");
    let handle = ExternalMemoryHandle::__backend_new(5, false, Some(gpu()))
        .__backend_with_fd(OwnedFd::from(writer));
    let backend = CountingBackend::new();
    let mut target = gpu();
    target.device_uuid[0] ^= 1;
    import_memory_checked(&backend, Some(&target), handle, 32, BufferUsage::STORAGE).unwrap_err();
    let mut buf = [0u8; 1];
    assert_eq!(reader.read(&mut buf).expect("read"), 0);
    assert_eq!(backend.import_calls.get(), 0);
}
