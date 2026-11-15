//! Metal の device で、列に挙げた大きさのバッファを確保し、`upload` と `download_blocking` で往復させる試験（TASK-314 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::memory::BufferUsage;

/// 確保して往復させるバイト数の列（11 個。重複なし）。
const BYTE_LENS: [u64; 11] = [
    37 * 41 * 4,
    37 * 53 * 4,
    41 * 53 * 4,
    64 * 128 * 4,
    64 * 256 * 10 * 4,
    512 * 512 * 4,
    20_000 * 64 * 4,
    20_000 * 128 * 4,
    2048 * 2048 * 4,
    100_000 * 64 * 4,
    100_000 * 128 * 4,
];

fn new_device() -> Box<dyn Device> {
    match MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

/// 長さ `len` の決まった並びのバイト列（0 以外の値を含む）。
fn pattern(len: u64) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(i % 251).unwrap_or(0).wrapping_add(1))
        .collect()
}

/// 列の各大きさで、`allocate` が `Ok` を返し、`upload` した並びが `download_blocking` でそのまま戻る（MEM-1 のポインタ）。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn each_listed_size_allocates_and_round_trips_bytes() {
    let device = new_device();
    let limit = device.limits().max_buffer_bytes;
    let memory = device.memory();
    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    for len in BYTE_LENS {
        assert!(len <= limit, "len {len} is over max_buffer_bytes {limit}");
        let handle = memory.allocate(len, usage).expect("allocate");
        assert_eq!(handle.size(), len);
        let payload = pattern(len);
        memory.upload(&handle, 0, &payload).expect("upload");
        let back = memory
            .download_blocking(&handle, 0, len)
            .expect("download_blocking");
        assert_eq!(back.len() as u64, len, "len {len}");
        assert!(back == payload, "len {len}: the bytes did not round-trip");
        memory.free(handle);
    }
}
