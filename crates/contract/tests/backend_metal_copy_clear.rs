//! Metal の公開の device から、バッファのコピーとクリアを記録し、製品のビルドの投入が `Unsupported` になる確認
//! （TASK-610 のポインタ）。
//!
//! どの試験も Metal の device に触るので `#[ignore]` を付け、専用の device の回でだけ流す。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::BufferUsage;

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

fn usage() -> BufferUsage {
    BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

/// 公開の `Device` の記録先に、コピーとクリアが記録できる。別の device のバッファは `DeviceMismatch`。
/// 製品のビルドの `submit` は `Unsupported` で、`dst` の内容は投入の前のまま。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn copy_and_clear_through_the_public_device_are_recorded_and_submit_is_unsupported() {
    let device = new_device();
    let other = new_device();
    let memory = device.memory();
    let src = memory.allocate(16, usage()).expect("allocate src");
    let dst = memory.allocate(16, usage()).expect("allocate dst");
    let foreign = other
        .memory()
        .allocate(16, usage())
        .expect("allocate foreign");
    memory.upload(&src, 0, &[7; 16]).expect("upload src");
    memory.upload(&dst, 0, &[9; 16]).expect("upload dst");

    let queue = device.queue();
    let mut encoder = queue.create_encoder();
    memory
        .copy_buffer_to_buffer(&mut encoder, &src, 0, &dst, 4, 8)
        .expect("copy");
    memory
        .clear_buffer(&mut encoder, &dst, 0, 4)
        .expect("clear");

    let r = memory.copy_buffer_to_buffer(&mut encoder, &foreign, 0, &dst, 0, 4);
    assert!(matches!(r, Err(ContractError::DeviceMismatch)), "{r:?}");
    let r = memory.copy_buffer_to_buffer(&mut encoder, &src, 0, &foreign, 0, 4);
    assert!(matches!(r, Err(ContractError::DeviceMismatch)), "{r:?}");
    let r = memory.clear_buffer(&mut encoder, &foreign, 0, 4);
    assert!(matches!(r, Err(ContractError::DeviceMismatch)), "{r:?}");

    let r = queue.submit(encoder, &[]);
    assert!(
        matches!(
            r,
            Err(ContractError::Unsupported {
                op: "submit",
                backend: BackendKind::Metal
            })
        ),
        "{r:?}"
    );
    assert_eq!(
        memory.download_blocking(&dst, 0, 16).expect("download"),
        vec![9; 16]
    );
}
