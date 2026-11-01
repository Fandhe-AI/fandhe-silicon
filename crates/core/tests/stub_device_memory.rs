//! feature `test-support` の裏の試験用の device の、メモリの口の振る舞いを確かめる統合テスト（TASK-156 の 3/5。DEV-34 のポインタ）。
//!
//! 確保・書き込み・読み出し・解放と、範囲・ハンドルの拒否を、公開のパスだけで確かめる。

use std::sync::{Arc, Mutex};

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};
use fandhe_silicon_core::stub_device::StubDeviceProvider;

const LIMIT: u64 = 1 << 28;

fn device() -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::Shared,
    })
    .expect("device")
}

fn usage() -> BufferUsage {
    BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

type Downloaded = Vec<Result<Vec<u8>, ContractError>>;

/// `download_async` を流し、callback が呼ばれた回数と最初の結果を返す。
fn download(
    mem: &dyn MemoryOps,
    h: &BufferHandle,
    offset: u64,
    len: u64,
) -> (usize, Result<Vec<u8>, ContractError>) {
    let slot: Arc<Mutex<Downloaded>> = Arc::default();
    let sink = Arc::clone(&slot);
    mem.download_async(
        h,
        offset,
        len,
        Box::new(move |r| sink.lock().expect("lock").push(r)),
    );
    let mut got = slot.lock().expect("lock");
    let count = got.len();
    (count, got.remove(0))
}

/// ハンドルの複製。`BufferHandle` は `Clone` でないので、背後実装向けの口で作り直す。
fn dup(h: &BufferHandle) -> BufferHandle {
    BufferHandle::__backend_new(
        h.__backend_generation(),
        h.__backend_id(),
        h.__backend_device_id(),
        h.size(),
        h.usage(),
    )
}

fn forge(h: &BufferHandle, generation: u64, id: u64, device_id: u64, size: u64) -> BufferHandle {
    BufferHandle::__backend_new(generation, id, device_id, size, h.usage())
}

fn is_invalid(r: Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

#[test]
fn allocate_returns_zeroed_buffer_with_requested_size_and_usage() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(16, usage()).expect("alloc");
    assert_eq!(h.size(), 16);
    assert_eq!(h.usage(), usage());
    let (_, r) = download(mem, &h, 0, 16);
    assert_eq!(r.expect("ok"), vec![0u8; 16]);
}

#[test]
fn upload_then_download_round_trips() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(8, usage()).expect("alloc");
    mem.upload(&h, 3, &[7, 8, 9]).expect("upload");
    let (_, r) = download(mem, &h, 0, 8);
    assert_eq!(r.expect("ok"), vec![0, 0, 0, 7, 8, 9, 0, 0]);
    let (_, part) = download(mem, &h, 3, 3);
    assert_eq!(part.expect("ok"), vec![7, 8, 9]);
}

#[test]
fn download_async_calls_callback_once() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(4, usage()).expect("alloc");
    let (n_ok, ok) = download(mem, &h, 0, 4);
    assert_eq!(n_ok, 1);
    assert!(ok.is_ok());
    let (n_err, err) = download(mem, &h, 0, 5);
    assert_eq!(n_err, 1);
    assert!(err.is_err());
}

#[test]
fn allocation_over_limit_is_rejected() {
    let d = device();
    let mem = d.memory();
    assert!(matches!(
        mem.allocate(LIMIT + 1, usage()),
        Err(ContractError::InvalidArgument(_))
    ));
    let h = mem.allocate(LIMIT, usage()).expect("alloc at limit");
    assert_eq!(h.size(), LIMIT);
    mem.free(h);
}

#[test]
fn range_end_equal_to_length_is_accepted() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(4, usage()).expect("alloc");
    mem.upload(&h, 0, &[1, 2, 3, 4]).expect("whole upload");
    mem.upload(&h, 2, &[9, 9]).expect("tail upload");
    mem.upload(&h, 4, &[]).expect("empty upload at end");
    let (_, r) = download(mem, &h, 0, 4);
    assert_eq!(r.expect("whole download"), vec![1, 2, 9, 9]);
    let (_, tail) = download(mem, &h, 2, 2);
    assert_eq!(tail.expect("tail download"), vec![9, 9]);
    let (_, empty) = download(mem, &h, 4, 0);
    assert_eq!(empty.expect("empty download"), Vec::<u8>::new());
}

#[test]
fn range_past_end_or_overflow_is_rejected_without_change() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(4, usage()).expect("alloc");
    mem.upload(&h, 0, &[1, 2, 3, 4]).expect("pattern");
    assert!(is_invalid(mem.upload(&h, 1, &[0, 0, 0, 0])));
    assert!(is_invalid(mem.upload(&h, 5, &[])));
    assert!(is_invalid(mem.upload(&h, u64::MAX - 1, &[0, 0, 0])));
    assert!(is_invalid(mem.upload(&h, u64::MAX, &[0])));
    for (offset, len) in [(0, 5), (1, 4), (5, 0), (u64::MAX - 1, 3), (u64::MAX, 1)] {
        let (n, r) = download(mem, &h, offset, len);
        assert_eq!(n, 1);
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "{offset} {len}"
        );
    }
    let (_, r) = download(mem, &h, 0, 4);
    assert_eq!(r.expect("unchanged"), vec![1, 2, 3, 4]);
}

#[test]
fn handle_from_other_device_is_device_mismatch() {
    let owner = device();
    let other = device();
    let h = owner.memory().allocate(4, usage()).expect("alloc");
    assert!(matches!(
        other.memory().upload(&h, 0, &[1]),
        Err(ContractError::DeviceMismatch)
    ));
    let (n, r) = download(other.memory(), &h, 0, 1);
    assert_eq!(n, 1);
    assert!(matches!(r, Err(ContractError::DeviceMismatch)));
}

#[test]
fn forged_handles_are_invalid_argument() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(4, usage()).expect("alloc");
    let own = h.__backend_device_id();
    let forged = [
        // 表に無い id
        forge(&h, h.__backend_generation(), h.__backend_id() + 100, own, 4),
        // 生きている id で世代違い
        forge(&h, h.__backend_generation() + 1, h.__backend_id(), own, 4),
        // 生きている id で長さ違い
        forge(&h, h.__backend_generation(), h.__backend_id(), own, 5),
    ];
    for f in &forged {
        assert!(is_invalid(mem.upload(f, 0, &[1])));
        let (n, r) = download(mem, f, 0, 1);
        assert_eq!(n, 1);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))));
    }
}

#[test]
fn freed_handle_is_invalid_argument() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(4, usage()).expect("alloc");
    let copy = dup(&h);
    mem.free(h);
    assert!(is_invalid(mem.upload(&copy, 0, &[1])));
    let (n, r) = download(mem, &copy, 0, 1);
    assert_eq!(n, 1);
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))));
}

#[test]
fn free_ignores_foreign_and_unknown_handles() {
    let owner = device();
    let other = device();
    let mem = owner.memory();
    let h = mem.allocate(4, usage()).expect("alloc");
    mem.upload(&h, 0, &[1, 2, 3, 4]).expect("pattern");
    // 別 device の free に持ち主のハンドルを渡す。
    other.memory().free(dup(&h));
    // 自分の device の表に無い id のハンドルを free に渡す。
    mem.free(forge(
        &h,
        h.__backend_generation(),
        h.__backend_id() + 100,
        h.__backend_device_id(),
        4,
    ));
    let (_, r) = download(mem, &h, 0, 4);
    assert_eq!(r.expect("still readable"), vec![1, 2, 3, 4]);
    mem.upload(&h, 0, &[5]).expect("still writable");
}

#[test]
fn pending_release_count_is_none() {
    let d = device();
    assert_eq!(d.memory().pending_release_count(), None);
}

#[cfg(feature = "native-blocking-wait")]
#[test]
fn download_blocking_matches_download_async() {
    let d = device();
    let mem = d.memory();
    let h = mem.allocate(4, usage()).expect("alloc");
    mem.upload(&h, 0, &[1, 2, 3, 4]).expect("pattern");
    let (_, async_ok) = download(mem, &h, 1, 2);
    assert_eq!(mem.download_blocking(&h, 1, 2), async_ok);
    let (_, async_err) = download(mem, &h, 3, 2);
    assert_eq!(mem.download_blocking(&h, 3, 2), async_err);
    assert!(matches!(
        mem.download_blocking(&h, 3, 2),
        Err(ContractError::InvalidArgument(_))
    ));
}
