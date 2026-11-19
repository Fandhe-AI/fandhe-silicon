//! CUDA の device で、表に無い識別子・偽造・解放の後・世代違い・別の device のバッファのハンドルを
//! `upload`・`download_blocking`・`download_async`・`free` に渡す試験（TASK-343 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。どの試験も本体を別のスレッドで流し、上限の時間で打ち切る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 試験 1 本の上限。
const LIMIT: Duration = Duration::from_secs(30);

/// 確保の長さ。
const LEN: u64 = 16;

/// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
fn within(body: impl FnOnce() + Send + 'static) {
    let (sender, receiver) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        body();
        let _ = sender.send(());
    });
    match receiver.recv_timeout(LIMIT) {
        Ok(()) => {
            let _ = handle.join();
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
            panic!("the test body ended without reporting");
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("the test body did not end within {LIMIT:?}")
        }
    }
}

fn new_device() -> Box<dyn Device> {
    match CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

fn usage() -> BufferUsage {
    BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

fn allocate(memory: &dyn MemoryOps) -> BufferHandle {
    match memory.allocate(LEN, usage()) {
        Ok(h) => h,
        Err(e) => panic!("allocate failed: {e:?}"),
    }
}

/// `handle` の世代・識別子・device の識別子を差し替えた写しを返す。
fn forged(handle: &BufferHandle, generation: u64, id: u64, device_id: u64) -> BufferHandle {
    BufferHandle::__backend_new(generation, id, device_id, handle.size(), handle.usage())
}

/// `handle` の写しを返す。
fn copy_of(handle: &BufferHandle) -> BufferHandle {
    forged(
        handle,
        handle.__backend_generation(),
        handle.__backend_id(),
        handle.__backend_device_id(),
    )
}

/// 自 device の識別子のまま、表に無い識別子（0 と `u64::MAX`）・解放の後の写し・世代違いにしたハンドルを返す。
fn own_device_forgeries(
    memory: &dyn MemoryOps,
    own: &BufferHandle,
) -> Vec<(&'static str, BufferHandle)> {
    let freed = allocate(memory);
    let freed_copy = copy_of(&freed);
    memory.free(freed);
    let generation = own.__backend_generation();
    let device_id = own.__backend_device_id();
    vec![
        ("id 0", forged(own, generation, 0, device_id)),
        ("id u64::MAX", forged(own, generation, u64::MAX, device_id)),
        ("freed", freed_copy),
        (
            "other generation",
            forged(own, generation + 1, own.__backend_id(), device_id),
        ),
    ]
}

/// どの device のものでもない device の識別子にしたハンドルを返す。
fn foreign_device(own: &BufferHandle) -> BufferHandle {
    forged(
        own,
        own.__backend_generation(),
        own.__backend_id(),
        own.__backend_device_id() ^ u64::MAX,
    )
}

/// `download_async` を呼び、callback が受けた値を呼ばれた順に返す。
fn download_async_results(
    memory: &dyn MemoryOps,
    handle: &BufferHandle,
) -> Vec<Result<Vec<u8>, ContractError>> {
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    memory.download_async(
        handle,
        0,
        LEN,
        Box::new(move |result| match sink.lock() {
            Ok(mut seen) => seen.push(result),
            Err(e) => panic!("callback lock poisoned: {e}"),
        }),
    );
    match received.lock() {
        Ok(mut seen) => std::mem::take(&mut *seen),
        Err(e) => panic!("callback lock poisoned: {e}"),
    }
}

fn read(memory: &dyn MemoryOps, handle: &BufferHandle) -> Vec<u8> {
    match memory.download_blocking(handle, 0, LEN) {
        Ok(bytes) => bytes,
        Err(e) => panic!("download_blocking failed: {e:?}"),
    }
}

// MEM-79
/// 表に無い識別子・解放の後・世代違いのハンドルは 3 つの口で `InvalidArgument`、どの device のものでもない
/// 識別子は `DeviceMismatch` で拒まれ、自 device のバッファの中身は変わらない。
#[test]
#[ignore = "requires a CUDA device"]
fn unknown_or_forged_handles_are_rejected_by_upload_and_both_downloads() {
    within(|| {
        let device = new_device();
        let memory = device.memory();
        let own = allocate(memory);
        assert_eq!(memory.upload(&own, 0, &[0x3C; 16]), Ok(()));

        for (label, handle) in own_device_forgeries(memory, &own) {
            let up = memory.upload(&handle, 0, &[0xEE; 16]);
            assert!(
                matches!(up, Err(ContractError::InvalidArgument(_))),
                "upload of {label}: {up:?}"
            );
            let down = memory.download_blocking(&handle, 0, LEN);
            assert!(
                matches!(down, Err(ContractError::InvalidArgument(_))),
                "download_blocking of {label}: {down:?}"
            );
            let results = download_async_results(memory, &handle);
            assert_eq!(results.len(), 1, "download_async of {label}");
            assert!(
                matches!(results[0], Err(ContractError::InvalidArgument(_))),
                "download_async of {label}: {results:?}"
            );
        }

        let foreign = foreign_device(&own);
        assert_eq!(
            memory.upload(&foreign, 0, &[0xEE; 16]),
            Err(ContractError::DeviceMismatch)
        );
        assert_eq!(
            memory.download_blocking(&foreign, 0, LEN),
            Err(ContractError::DeviceMismatch)
        );
        assert_eq!(
            download_async_results(memory, &foreign),
            vec![Err(ContractError::DeviceMismatch)]
        );

        assert_eq!(read(memory, &own), vec![0x3C; 16]);
        memory.free(own);
    });
}

/// 別の device のバッファを `download_async` に渡すと callback が 1 回だけ `DeviceMismatch` を受け、
/// 同じ識別子の自 device のバッファの中身は変わらない。
#[test]
#[ignore = "requires a CUDA device"]
fn another_devices_buffer_is_rejected_by_download_async_and_the_same_id_stays_intact() {
    within(|| {
        let first = new_device();
        let second = new_device();
        let from_first = allocate(first.memory());
        let from_second = allocate(second.memory());
        assert_ne!(
            from_first.__backend_device_id(),
            from_second.__backend_device_id()
        );
        assert_eq!(from_first.__backend_id(), from_second.__backend_id());
        assert_eq!(second.memory().upload(&from_second, 0, &[0x77; 16]), Ok(()));

        assert_eq!(
            download_async_results(second.memory(), &from_first),
            vec![Err(ContractError::DeviceMismatch)]
        );

        assert_eq!(read(second.memory(), &from_second), vec![0x77; 16]);
        first.memory().free(from_first);
        second.memory().free(from_second);
    });
}

// MEM-16
/// 表に無い識別子・解放の後・世代違い・どの device のものでもない識別子のハンドルの `free` は何もせず、
/// 自 device のバッファは中身を保ち、読み書きできる。
#[test]
#[ignore = "requires a CUDA device"]
fn freeing_unknown_or_forged_handles_is_ignored_and_keeps_own_buffers() {
    within(|| {
        let device = new_device();
        let memory = device.memory();
        let a = allocate(memory);
        let b = allocate(memory);
        assert_eq!(memory.upload(&a, 0, &[0x11; 16]), Ok(()));
        assert_eq!(memory.upload(&b, 0, &[0x22; 16]), Ok(()));

        for (_, handle) in own_device_forgeries(memory, &a) {
            memory.free(handle);
        }
        memory.free(foreign_device(&a));

        assert_eq!(read(memory, &a), vec![0x11; 16]);
        assert_eq!(read(memory, &b), vec![0x22; 16]);
        assert_eq!(memory.upload(&a, 0, &[0x33; 16]), Ok(()));
        assert_eq!(read(memory, &a), vec![0x33; 16]);
        memory.free(a);
        memory.free(b);
    });
}
