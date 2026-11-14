//! Metal の `request` の失敗の後に、ほかの背後実装の provider の `request` が呼ばれないことの確認（TASK-277 のポインタ）。
//!
//! 各背後実装の `request_call_count` で数を照らす。device に触らない入口だけを使う。
//! 試験は 1 本で、macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::{
    self, MetalDefaultDeviceNoneProvider, MetalDeviceProvider,
};
use fandhe_silicon_contract::{backend_cuda, backend_vulkan};
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;

/// `ComputeOnly`・`ProductIndependent` の要求を作る。
fn request(backend: BackendKind) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

/// Metal・Vulkan・CUDA の順に、provider の `request` の呼び出しの数を読む。
fn counts() -> [u64; 3] {
    [
        backend_metal::request_call_count(),
        backend_vulkan::request_call_count(),
        backend_cuda::request_call_count(),
    ]
}

/// fail-fast の利用コードの模擬。`request` の結果をそのまま返す。
fn fail_fast<P: DeviceProvider>(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
    P::request(req)
}

/// 縮退の利用コードの模擬。`Err` を受けたら値を記録して `false` を返す。
fn degrade<P: DeviceProvider>(req: DeviceRequest, events: &mut Vec<ContractError>) -> bool {
    match P::request(req) {
        Ok(_) => true,
        Err(e) => {
            events.push(e);
            false
        }
    }
}

/// ERR-57。Metal の `request` の失敗を 2 つの利用コードの模擬が受けても、Vulkan・CUDA の provider は呼ばれない。
#[test]
fn metal_request_failures_call_no_other_backend_provider() {
    assert_eq!(counts(), [0, 0, 0]);

    let e = fail_fast::<MetalDefaultDeviceNoneProvider>(request(BackendKind::Metal)).err();
    assert!(
        matches!(e, Some(ContractError::DeviceUnavailable(_))),
        "{e:?}"
    );
    let e = fail_fast::<MetalDefaultDeviceNoneProvider>(request(BackendKind::Vulkan)).err();
    assert!(
        matches!(e, Some(ContractError::InvalidCombination(_))),
        "{e:?}"
    );
    let e = fail_fast::<MetalDeviceProvider>(request(BackendKind::Vulkan)).err();
    assert!(
        matches!(e, Some(ContractError::InvalidCombination(_))),
        "{e:?}"
    );

    let mut events = Vec::new();
    assert!(!degrade::<MetalDefaultDeviceNoneProvider>(
        request(BackendKind::Metal),
        &mut events
    ));
    assert!(!degrade::<MetalDeviceProvider>(
        request(BackendKind::Vulkan),
        &mut events
    ));
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ContractError::DeviceUnavailable(_)));
    assert!(matches!(events[1], ContractError::InvalidCombination(_)));

    assert_eq!(counts(), [5, 0, 0]);

    assert!(backend_vulkan::VulkanDeviceProvider::request(request(BackendKind::Metal)).is_err());
    assert!(backend_cuda::CudaDeviceProvider::request(request(BackendKind::Metal)).is_err());

    assert_eq!(counts(), [5, 1, 1]);
}
