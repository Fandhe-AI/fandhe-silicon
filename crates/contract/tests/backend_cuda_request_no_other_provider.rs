//! CUDA の `request` の失敗の後に、ほかの背後実装の provider の `request` が呼ばれないことの確認（TASK-290 のポインタ）。
//!
//! 各背後実装の `request_call_count` で数を照らす。device に触らない入口だけを使う。
//! 試験は 1 本で、Metal の数は macOS かつ feature `backend-metal` のビルドでだけ照らす。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::{self, CudaDeviceProvider, CudaDriverMissingProvider};
use fandhe_silicon_contract::backend_vulkan::{self, VulkanDeviceProvider};
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

/// CUDA・Vulkan の順に、provider の `request` の呼び出しの数を読む。
fn counts() -> [u64; 2] {
    [
        backend_cuda::request_call_count(),
        backend_vulkan::request_call_count(),
    ]
}

/// Metal の provider の `request` の呼び出しの数を読む。
#[cfg(all(target_os = "macos", feature = "backend-metal"))]
fn metal_count() -> u64 {
    fandhe_silicon_contract::backend_metal::request_call_count()
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

/// ERR-57。CUDA の `request` の失敗を 2 つの利用コードの模擬が受けても、Vulkan・Metal の provider は呼ばれない。
#[test]
fn cuda_request_failures_call_no_other_backend_provider() {
    assert_eq!(counts(), [0, 0]);
    #[cfg(all(target_os = "macos", feature = "backend-metal"))]
    assert_eq!(metal_count(), 0);

    let e = fail_fast::<CudaDriverMissingProvider>(request(BackendKind::Cuda)).err();
    assert!(
        matches!(e, Some(ContractError::DeviceUnavailable(_))),
        "{e:?}"
    );
    let e = fail_fast::<CudaDriverMissingProvider>(request(BackendKind::Vulkan)).err();
    assert!(
        matches!(e, Some(ContractError::InvalidCombination(_))),
        "{e:?}"
    );
    let e = fail_fast::<CudaDeviceProvider>(request(BackendKind::Vulkan)).err();
    assert!(
        matches!(e, Some(ContractError::InvalidCombination(_))),
        "{e:?}"
    );

    let mut events = Vec::new();
    assert!(!degrade::<CudaDriverMissingProvider>(
        request(BackendKind::Cuda),
        &mut events
    ));
    assert!(!degrade::<CudaDeviceProvider>(
        request(BackendKind::Vulkan),
        &mut events
    ));
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ContractError::DeviceUnavailable(_)));
    assert!(matches!(events[1], ContractError::InvalidCombination(_)));

    assert_eq!(counts(), [5, 0]);
    #[cfg(all(target_os = "macos", feature = "backend-metal"))]
    assert_eq!(metal_count(), 0);

    assert!(VulkanDeviceProvider::request(request(BackendKind::Cuda)).is_err());
    assert_eq!(counts(), [5, 1]);

    #[cfg(all(target_os = "macos", feature = "backend-metal"))]
    {
        use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
        assert!(MetalDeviceProvider::request(request(BackendKind::Cuda)).is_err());
        assert_eq!(metal_count(), 1);
        assert_eq!(counts(), [5, 1]);
    }
}
