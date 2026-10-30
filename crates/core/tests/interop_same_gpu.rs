//! 同一 GPU の判定 `check_same_physical_gpu` と、取り込み前の拒否を呼び出しの計数で確かめる公開テスト（TASK-145）。
//!
//! 計数つきのスタブはこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: 判定の直接の呼び出しの結果（DEV-76）と、`import_memory_checked` 経由で
//! 一致なら `import_memory` が 1 回、拒否なら 0 回であること。

use std::cell::Cell;

use fandhe_silicon_core::error::{ContractError, IdentityMismatchReason};
use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
use fandhe_silicon_core::interop::{
    ExternalImageDescriptor, ExternalImageFormat, ExternalImageHandle, ExternalImageLayout,
    ExternalImageTiling, ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle,
    ImportedImageHandle, PhysicalGpuId, check_same_physical_gpu, import_image_checked,
    import_memory_checked, import_semaphore_checked, import_semaphore_timeline_checked,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

/// 取り込み 4 操作（`import_memory`・`import_image`・`import_semaphore`・`import_semaphore_timeline`）の呼び出しを数えるスタブ。
struct CountingBackend {
    import_calls: Cell<u32>,
    image_calls: Cell<u32>,
    semaphore_calls: Cell<u32>,
    timeline_calls: Cell<u32>,
}

impl CountingBackend {
    fn new() -> Self {
        Self {
            import_calls: Cell::new(0),
            image_calls: Cell::new(0),
            semaphore_calls: Cell::new(0),
            timeline_calls: Cell::new(0),
        }
    }

    /// 4 つの取り込みの呼び出し回数を `[memory, image, semaphore, timeline]` の順で返す。
    fn counts(&self) -> [u32; 4] {
        [
            self.import_calls.get(),
            self.image_calls.get(),
            self.semaphore_calls.get(),
            self.timeline_calls.get(),
        ]
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
        external: ExternalSemaphoreHandle,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        self.semaphore_calls.set(self.semaphore_calls.get() + 1);
        Ok(ImportedSemaphoreHandle::__backend_new(
            external.__backend_id(),
        ))
    }

    fn import_image(
        &self,
        external: ExternalImageHandle,
        _: ExternalImageDescriptor,
    ) -> Result<ImportedImageHandle, ContractError> {
        self.image_calls.set(self.image_calls.get() + 1);
        Ok(ImportedImageHandle::__backend_new(external.__backend_id()))
    }

    fn import_semaphore_timeline(
        &self,
        external: ExternalSemaphoreHandle,
        _: u64,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        self.timeline_calls.set(self.timeline_calls.get() + 1);
        Ok(ImportedSemaphoreHandle::__backend_new(
            external.__backend_id(),
        ))
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

/// INTEROP-7: `device_uuid` の 16 バイトすべてが違う組は、引数の順によらず `Mismatch`。
#[test]
fn all_bytes_different_device_uuid_is_mismatch() {
    let a = PhysicalGpuId {
        device_uuid: [1; 16],
        driver_uuid: None,
    };
    let b = PhysicalGpuId {
        device_uuid: [2; 16],
        driver_uuid: None,
    };
    let expected = mismatch(IdentityMismatchReason::Mismatch);
    assert_eq!(
        check_same_physical_gpu(Some(&a), Some(&b)).unwrap_err(),
        expected
    );
    assert_eq!(
        check_same_physical_gpu(Some(&b), Some(&a)).unwrap_err(),
        expected
    );
}

/// INTEROP-10: 第 1 引数だけが `None` なら `Undeterminable`。
#[test]
fn first_argument_none_is_undeterminable() {
    let right = with_driver(None);
    let err = check_same_physical_gpu(None, Some(&right)).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));
}

/// INTEROP-10: 第 2 引数だけが `None` なら `Undeterminable`。
#[test]
fn second_argument_none_is_undeterminable() {
    let left = with_driver(None);
    let err = check_same_physical_gpu(Some(&left), None).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));
}

/// INTEROP-10: 両方の引数が `None` でも `Undeterminable`。
#[test]
fn both_arguments_none_is_undeterminable() {
    let err = check_same_physical_gpu(None, None).unwrap_err();
    assert_eq!(err, mismatch(IdentityMismatchReason::Undeterminable));
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

fn desc() -> ExternalImageDescriptor {
    ExternalImageDescriptor {
        format: ExternalImageFormat::Rgba8Unorm,
        extent: [4, 4, 1],
        tiling: ExternalImageTiling::Optimal,
        initial_layout: ExternalImageLayout::Undefined,
        dedicated: None,
    }
}

/// 4 つの取り込みの入口を、同じ識別子の組で順に呼び、戻りの `Result` の `Err` の値を 4 つ集める。
/// 戻りは `[memory, image, semaphore, timeline]` の順。
fn call_all_entries(
    backend: &CountingBackend,
    source: Option<&PhysicalGpuId>,
    target: Option<&PhysicalGpuId>,
) -> [Result<(), ContractError>; 4] {
    let memory = ExternalMemoryHandle::__backend_new(1, false, source.copied());
    [
        import_memory_checked(backend, target, memory, 32, BufferUsage::STORAGE).map(|_| ()),
        import_image_checked(
            backend,
            source,
            target,
            ExternalImageHandle::__backend_new(2, false),
            desc(),
        )
        .map(|_| ()),
        import_semaphore_checked(
            backend,
            source,
            target,
            ExternalSemaphoreHandle::__backend_new(3, false),
        )
        .map(|_| ()),
        import_semaphore_timeline_checked(
            backend,
            source,
            target,
            ExternalSemaphoreHandle::__backend_new(4, true),
            7,
        )
        .map(|_| ()),
    ]
}

/// ERR-59: `Mismatch` の拒否では、4 つの取り込みの入口がすべて同じ `Err` を返し、4 つの操作とも 0 回。
#[test]
fn mismatch_rejects_before_all_four_imports() {
    let backend = CountingBackend::new();
    let source = gpu();
    let mut target = gpu();
    target.device_uuid[0] ^= 1;
    let results = call_all_entries(&backend, Some(&source), Some(&target));
    for result in results {
        assert_eq!(
            result.unwrap_err(),
            mismatch(IdentityMismatchReason::Mismatch)
        );
    }
    assert_eq!(backend.counts(), [0, 0, 0, 0]);
}

/// ERR-59: 取り込み先が `None` の `Undeterminable` の拒否でも、4 つの操作とも 0 回。
#[test]
fn undeterminable_target_rejects_before_all_four_imports() {
    let backend = CountingBackend::new();
    let source = gpu();
    let results = call_all_entries(&backend, Some(&source), None);
    for result in results {
        assert_eq!(
            result.unwrap_err(),
            mismatch(IdentityMismatchReason::Undeterminable)
        );
    }
    assert_eq!(backend.counts(), [0, 0, 0, 0]);
}

/// ERR-59: export 元が `None` の `Undeterminable` の拒否でも、4 つの操作とも 0 回。
#[test]
fn undeterminable_source_rejects_before_all_four_imports() {
    let backend = CountingBackend::new();
    let target = gpu();
    let results = call_all_entries(&backend, None, Some(&target));
    for result in results {
        assert_eq!(
            result.unwrap_err(),
            mismatch(IdentityMismatchReason::Undeterminable)
        );
    }
    assert_eq!(backend.counts(), [0, 0, 0, 0]);
}

/// ERR-59: 一致なら、3 つの入口がそれぞれ自分の取り込みを 1 回だけ呼び、渡したハンドルがそのまま取り込みに渡る（戻りの id で確かめる）。
#[test]
fn match_calls_each_entry_once() {
    let backend = CountingBackend::new();
    let g = gpu();
    let image = import_image_checked(
        &backend,
        Some(&g),
        Some(&g),
        ExternalImageHandle::__backend_new(2, false),
        desc(),
    )
    .expect("image");
    assert_eq!(image.__backend_id(), 2);
    assert_eq!(backend.counts(), [0, 1, 0, 0]);
    let semaphore = import_semaphore_checked(
        &backend,
        Some(&g),
        Some(&g),
        ExternalSemaphoreHandle::__backend_new(3, false),
    )
    .expect("semaphore");
    assert_eq!(semaphore.__backend_id(), 3);
    assert_eq!(backend.counts(), [0, 1, 1, 0]);
    let timeline = import_semaphore_timeline_checked(
        &backend,
        Some(&g),
        Some(&g),
        ExternalSemaphoreHandle::__backend_new(4, true),
        7,
    )
    .expect("timeline");
    assert_eq!(timeline.__backend_id(), 4);
    assert_eq!(backend.counts(), [0, 1, 1, 1]);
}

/// ERR-59: 拒否された image・semaphore・timeline semaphore のハンドルの fd が閉じる（pipe の読み側が EOF）。
#[cfg(unix)]
#[test]
fn rejected_image_and_semaphore_handles_close_their_fds() {
    use std::io::Read;
    use std::os::fd::OwnedFd;

    fn assert_closed(mut reader: std::io::PipeReader) {
        let mut buf = [0u8; 1];
        assert_eq!(reader.read(&mut buf).expect("read"), 0);
    }

    let backend = CountingBackend::new();
    let source = gpu();
    let mut target = gpu();
    target.device_uuid[0] ^= 1;

    let (reader, writer) = std::io::pipe().expect("pipe");
    let image =
        ExternalImageHandle::__backend_new(2, false).__backend_with_fd(OwnedFd::from(writer));
    import_image_checked(&backend, Some(&source), Some(&target), image, desc()).unwrap_err();
    assert_closed(reader);

    let (reader, writer) = std::io::pipe().expect("pipe");
    let sem =
        ExternalSemaphoreHandle::__backend_new(3, false).__backend_with_fd(OwnedFd::from(writer));
    import_semaphore_checked(&backend, Some(&source), Some(&target), sem).unwrap_err();
    assert_closed(reader);

    let (reader, writer) = std::io::pipe().expect("pipe");
    let timeline =
        ExternalSemaphoreHandle::__backend_new(4, true).__backend_with_fd(OwnedFd::from(writer));
    import_semaphore_timeline_checked(&backend, Some(&source), Some(&target), timeline, 7)
        .unwrap_err();
    assert_closed(reader);

    assert_eq!(backend.counts(), [0, 0, 0, 0]);
}
