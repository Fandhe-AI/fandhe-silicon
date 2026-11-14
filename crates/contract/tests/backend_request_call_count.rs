//! 背後実装ごとの provider の `request` の呼び出しの数（`request_call_count`）が、その背後実装の
//! provider の呼び出しだけで動くことの確認（TASK-277 のポインタ）。
//!
//! 検査で拒まれる要求と差し込みの provider だけを呼ぶので、device には触らない。
//! 数はプロセスに 1 つなので、このファイルの試験は 1 本だけにする。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::{backend_cuda, backend_vulkan};
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;

type RequestFn = fn(DeviceRequest) -> Result<Box<dyn Device>, ContractError>;

fn request(backend: BackendKind) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

/// 3 つの背後実装の数（macOS 以外では Metal の欄は 0 のまま）。
fn counts() -> [u64; 3] {
    #[cfg(all(target_os = "macos", feature = "backend-metal"))]
    let metal = fandhe_silicon_contract::backend_metal::request_call_count();
    #[cfg(not(all(target_os = "macos", feature = "backend-metal")))]
    let metal = 0;
    [
        metal,
        backend_vulkan::request_call_count(),
        backend_cuda::request_call_count(),
    ]
}

/// 入口を 1 回呼び、`slot` の欄だけがちょうど 1 増え、ほかの欄が変わらないことを確かめる。
fn expect_one_more(slot: usize, label: &str, call: RequestFn, req: DeviceRequest) {
    let before = counts();
    let result = call(req);
    assert!(result.is_err(), "{label}: request must fail");
    let after = counts();
    let mut expected = before;
    expected[slot] += 1;
    assert_eq!(after, expected, "{label}");
}

#[test]
fn each_backend_counts_only_its_own_provider_requests() {
    assert_eq!(counts(), [0, 0, 0]);

    let vulkan: [(&str, RequestFn); 8] = [
        (
            "VulkanDeviceProvider",
            backend_vulkan::VulkanDeviceProvider::request,
        ),
        (
            "VulkanLoaderMissingProvider",
            backend_vulkan::VulkanLoaderMissingProvider::request,
        ),
        (
            "VulkanCreateInstanceFailsProvider",
            backend_vulkan::VulkanCreateInstanceFailsProvider::request,
        ),
        (
            "VulkanEnumeratePhysicalDevicesFailsProvider",
            backend_vulkan::VulkanEnumeratePhysicalDevicesFailsProvider::request,
        ),
        (
            "VulkanNoPhysicalDeviceProvider",
            backend_vulkan::VulkanNoPhysicalDeviceProvider::request,
        ),
        (
            "VulkanNoQueueFamilyProvider",
            backend_vulkan::VulkanNoQueueFamilyProvider::request,
        ),
        (
            "VulkanCreateDeviceFailsProvider",
            backend_vulkan::VulkanCreateDeviceFailsProvider::request,
        ),
        (
            "VulkanCreateCommandPoolFailsProvider",
            backend_vulkan::VulkanCreateCommandPoolFailsProvider::request,
        ),
    ];
    for (label, call) in vulkan {
        expect_one_more(1, label, call, request(BackendKind::Cuda));
    }

    let cuda: [(&str, RequestFn); 6] = [
        (
            "CudaDeviceProvider",
            backend_cuda::CudaDeviceProvider::request,
        ),
        (
            "CudaDriverMissingProvider",
            backend_cuda::CudaDriverMissingProvider::request,
        ),
        (
            "CudaContextNewFailsProvider",
            backend_cuda::CudaContextNewFailsProvider::request,
        ),
        (
            "CudaNameFailsProvider",
            backend_cuda::CudaNameFailsProvider::request,
        ),
        (
            "CudaComputeCapabilityFailsProvider",
            backend_cuda::CudaComputeCapabilityFailsProvider::request,
        ),
        (
            "CudaTotalMemFailsProvider",
            backend_cuda::CudaTotalMemFailsProvider::request,
        ),
    ];
    for (label, call) in cuda {
        expect_one_more(2, label, call, request(BackendKind::Vulkan));
    }

    #[cfg(all(target_os = "macos", feature = "backend-metal"))]
    {
        use fandhe_silicon_contract::backend_metal;
        let metal: [(&str, RequestFn); 3] = [
            (
                "MetalDeviceProvider",
                backend_metal::MetalDeviceProvider::request,
            ),
            (
                "MetalDefaultDeviceNoneProvider",
                backend_metal::MetalDefaultDeviceNoneProvider::request,
            ),
            (
                "MetalCommandQueueNoneProvider",
                backend_metal::MetalCommandQueueNoneProvider::request,
            ),
        ];
        for (label, call) in metal {
            expect_one_more(0, label, call, request(BackendKind::Vulkan));
        }
    }
}
