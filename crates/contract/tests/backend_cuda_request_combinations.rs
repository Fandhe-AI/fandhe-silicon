//! CUDA の背後実装の `request` の、組み合わせの検査の確認（TASK-290 のポインタ）。
//!
//! 検査で拒まれる要求を渡した `CudaDeviceProvider::request` と、差し込みの provider（feature
//! `fault-injection`）だけを呼ぶので、device には触らない。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::{
    CudaComputeCapabilityFailsProvider, CudaContextNewFailsProvider, CudaDeviceProvider,
    CudaDriverMissingProvider, CudaNameFailsProvider, CudaTotalMemFailsProvider,
};
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;

type RequestFn = fn(DeviceRequest) -> Result<Box<dyn Device>, ContractError>;

/// `ProductIndependent` の要求を作る。
fn request(backend: BackendKind, profile: Profile) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

/// `Box<dyn Device>` は `Debug` を持たないので、失敗側だけを取り出す。成功したら試験を落とす。
fn expect_err(label: &str, r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("{label}: expected an error"),
        Err(e) => e,
    }
}

/// ERR-1。CUDA 以外の背後実装に `Graphics`・`CpuOnly` を付けた要求は、本物の provider でも差し込みの provider でも `InvalidCombination`。
#[test]
fn other_backends_with_graphics_or_cpu_only_are_invalid_combinations() {
    let providers: [(&str, RequestFn); 6] = [
        ("CudaDeviceProvider", CudaDeviceProvider::request),
        (
            "CudaDriverMissingProvider",
            CudaDriverMissingProvider::request,
        ),
        (
            "CudaContextNewFailsProvider",
            CudaContextNewFailsProvider::request,
        ),
        ("CudaNameFailsProvider", CudaNameFailsProvider::request),
        (
            "CudaComputeCapabilityFailsProvider",
            CudaComputeCapabilityFailsProvider::request,
        ),
        (
            "CudaTotalMemFailsProvider",
            CudaTotalMemFailsProvider::request,
        ),
    ];
    for (label, call) in providers {
        for backend in [
            BackendKind::Metal,
            BackendKind::Vulkan,
            BackendKind::Dx12Compute,
        ] {
            for profile in [Profile::Graphics, Profile::CpuOnly] {
                let e = expect_err(label, call(request(backend, profile)));
                assert!(
                    matches!(e, ContractError::InvalidCombination(_)),
                    "{label} {backend:?} {profile:?}: {e:?}"
                );
            }
        }
    }
}

/// CUDA の背後実装のソースは、ほかの背後実装の provider と module の名を含まない。
#[test]
fn the_cuda_source_names_no_other_backend_provider() {
    const SOURCE: &str = include_str!("../src/backend_cuda.rs");
    for word in [
        "MetalDeviceProvider",
        "VulkanDeviceProvider",
        "backend_metal",
        "backend_vulkan",
    ] {
        assert!(!SOURCE.contains(word), "{word}");
    }
}
