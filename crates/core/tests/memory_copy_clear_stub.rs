//! 試験用の device のメモリの口が、バッファのコピーとクリアの記録に `Unsupported` を返し、
//! 記録先に何も記録しないことの統合テスト（TASK-610 の 1/4）。
//!
//! 公開のパスだけを使う。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{DeviceOwnership, DeviceProvider, DeviceRequest, Profile};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::BufferUsage;
use fandhe_silicon_core::stub_device::StubDeviceProvider;

/// 2 つの操作が `Unsupported`（操作名と要求した背後実装つき）を返し、記録の件数が増えない。
#[test]
fn copy_and_clear_on_the_stub_return_unsupported_and_record_nothing() {
    let device = StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Vulkan,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("device");
    let usage = BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    let src = device.memory().allocate(16, usage).expect("src");
    let dst = device.memory().allocate(16, usage).expect("dst");
    let mut enc = device.queue().create_encoder();
    let before = format!("{enc:?}");

    let copied = device
        .memory()
        .copy_buffer_to_buffer(&mut enc, &src, 0, &dst, 0, 8);
    assert!(matches!(
        copied,
        Err(ContractError::Unsupported {
            op: "copy_buffer_to_buffer",
            backend: BackendKind::Vulkan,
        })
    ));

    let cleared = device.memory().clear_buffer(&mut enc, &dst, 0, 8);
    assert!(matches!(
        cleared,
        Err(ContractError::Unsupported {
            op: "clear_buffer",
            backend: BackendKind::Vulkan,
        })
    ));

    assert_eq!(format!("{enc:?}"), before);
    assert!(before.contains("recorded: 0"), "{before}");
}
