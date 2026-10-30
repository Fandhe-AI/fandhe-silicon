//! 相互運用の公開の型と `ExternalInterop` の trait の形を別の crate から固定する公開テスト（TASK-717）。
//!
//! 試験用のダミーの背後実装はこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: `dyn ExternalInterop` を作れること、必須の 4 操作だけの実装でコンパイルが通ること、
//! 拡張の操作の既定の本体の値、ハンドルの値の出し入れ、`Clone`・`Copy` の有無、unix での fd の所有。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
use fandhe_silicon_core::interop::{
    ExportedImageHandle, ExternalImageDescriptor, ExternalImageFormat, ExternalImageHandle,
    ExternalImageLayout, ExternalImageTiling, ExternalInterop, ExternalMemoryHandle,
    ExternalSemaphoreHandle, PhysicalGpuId,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

/// 必須の 4 操作だけを実装するダミー。取り込みはハンドルの値を使って結果を作る。
struct MinimalBackend;

impl ExternalInterop for MinimalBackend {
    fn export_memory(&self, _: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> {
        Ok(ExternalMemoryHandle::__backend_new(7, true, None))
    }

    fn import_memory(
        &self,
        external: ExternalMemoryHandle,
        byte_len: u64,
        usage: BufferUsage,
    ) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(
            1,
            external.__backend_id(),
            1,
            byte_len,
            usage,
        ))
    }

    fn export_semaphore(&self, _: SubmitToken) -> Result<ExternalSemaphoreHandle, ContractError> {
        Ok(ExternalSemaphoreHandle::__backend_new(3, false))
    }

    fn import_semaphore(
        &self,
        external: ExternalSemaphoreHandle,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        Ok(ImportedSemaphoreHandle::__backend_new(
            external.__backend_id(),
        ))
    }
}

fn descriptor() -> ExternalImageDescriptor {
    ExternalImageDescriptor {
        format: ExternalImageFormat::Rgba8Unorm,
        extent: [4, 4, 1],
        tiling: ExternalImageTiling::Optimal,
        initial_layout: ExternalImageLayout::Undefined,
        dedicated: None,
    }
}

#[test]
fn required_ops_work_through_dyn() {
    let backend: Box<dyn ExternalInterop> = Box::new(MinimalBackend);
    let buffer = BufferHandle::__backend_new(1, 5, 1, 64, BufferUsage::EXPORTABLE);
    let memory = backend.export_memory(&buffer).expect("export");
    assert!(memory.__backend_dedicated());
    let imported = backend
        .import_memory(memory, 64, BufferUsage::STORAGE)
        .expect("import");
    assert_eq!(imported.size(), 64);
    let semaphore = backend
        .export_semaphore(SubmitToken::__backend_new(1, 1, 1))
        .expect("export semaphore");
    assert!(!semaphore.__backend_timeline());
    assert!(backend.import_semaphore(semaphore).is_ok());
}

#[test]
fn extension_ops_default_to_unsupported_extension_default() {
    let backend = MinimalBackend;
    let name = |e: ContractError| match e {
        ContractError::UnsupportedExtensionDefault { op } => op,
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(
        name(backend.export_image(descriptor()).unwrap_err()),
        "export_image"
    );
    assert_eq!(
        name(
            backend
                .import_image(ExternalImageHandle::__backend_new(1, false), descriptor())
                .unwrap_err()
        ),
        "import_image"
    );
    assert_eq!(
        name(
            backend
                .export_semaphore_timeline(SubmitToken::__backend_new(1, 1, 1))
                .unwrap_err()
        ),
        "export_semaphore_timeline"
    );
    assert_eq!(
        name(
            backend
                .import_semaphore_timeline(ExternalSemaphoreHandle::__backend_new(1, true), 1)
                .unwrap_err()
        ),
        "import_semaphore_timeline"
    );
}

#[test]
fn exported_image_handle_is_copy_and_others_are_not() {
    fn is_copy<T: Copy>() {}
    is_copy::<ExportedImageHandle>();
    is_copy::<PhysicalGpuId>();
    let handle = ExportedImageHandle::__backend_new(9);
    let copy = handle;
    assert_eq!(handle.__backend_id(), copy.__backend_id());
}

#[test]
fn memory_handle_carries_source_gpu() {
    let id = PhysicalGpuId {
        device_uuid: [1; 16],
        driver_uuid: Some([2; 16]),
    };
    let handle = ExternalMemoryHandle::__backend_new(1, false, Some(id));
    assert_eq!(handle.__backend_source_gpu(), Some(&id));
    let none = ExternalMemoryHandle::__backend_new(1, false, None);
    assert_eq!(none.__backend_source_gpu(), None);
}

#[cfg(unix)]
mod fd_ownership {
    use super::*;
    use std::os::fd::{AsRawFd, OwnedFd};

    /// `/dev/null` を開いた fd を作る（unsafe を使わない）。
    fn some_fd() -> OwnedFd {
        OwnedFd::from(std::fs::File::open("/dev/null").expect("open"))
    }

    #[test]
    fn take_fd_moves_ownership_once() {
        let fd = some_fd();
        let raw = fd.as_raw_fd();
        let mut handle = ExternalMemoryHandle::__backend_new(1, false, None).__backend_with_fd(fd);
        let taken = handle.__backend_take_fd().expect("first take");
        assert_eq!(taken.as_raw_fd(), raw);
        assert!(handle.__backend_take_fd().is_none());
    }

    #[test]
    fn semaphore_and_image_handles_take_fd_once() {
        let mut semaphore =
            ExternalSemaphoreHandle::__backend_new(1, true).__backend_with_fd(some_fd());
        assert!(semaphore.__backend_take_fd().is_some());
        assert!(semaphore.__backend_take_fd().is_none());
        let mut image = ExternalImageHandle::__backend_new(1, false).__backend_with_fd(some_fd());
        assert!(image.__backend_take_fd().is_some());
        assert!(image.__backend_take_fd().is_none());
    }

    #[test]
    fn handle_without_fd_has_none() {
        let mut handle = ExternalMemoryHandle::__backend_new(1, false, None);
        assert!(handle.__backend_take_fd().is_none());
    }
}
