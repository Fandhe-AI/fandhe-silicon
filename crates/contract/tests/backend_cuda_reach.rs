//! CUDA の device から compute の口へ届くことを確かめる試験（TASK-166 のポインタ）。
//!
//! メモリ操作の確保と投入の窓口の記録先の作成へ届くことも、同じファイルで確かめる。
//!
//! どの試験も device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    BindingLayout, CommandEncoder, PipelineHandle, PipelineInput,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

/// 要素に 1 を足す小さなカーネル。
const ADD_ONE: &str = "extern \"C\" __global__ void add_one(float* out, const float* inp, unsigned int n) { unsigned int i = blockIdx.x * blockDim.x + threadIdx.x; if (i < n) { out[i] = inp[i] + 1.0f; } }";

fn open_compute_only() -> Box<dyn Device> {
    match CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

// DEV-60
#[test]
#[ignore = "requires a CUDA device"]
fn compute_only_device_has_compute_and_no_graphics() {
    let device = open_compute_only();
    assert_eq!(device.backend(), BackendKind::Cuda);
    assert!(device.compute().is_some());
    #[cfg(feature = "graphics")]
    assert!(device.graphics().is_none());
}

// DEV-164
#[test]
#[ignore = "requires a CUDA device"]
fn create_pipeline_through_compute_returns_a_handle() {
    let device = open_compute_only();
    let Some(compute) = device.compute() else {
        panic!("compute() is None");
    };
    let r: Result<PipelineHandle, ContractError> = compute.create_pipeline(
        PipelineInput::CudaC {
            source: ADD_ONE,
            entry_point: "add_one",
        },
        &BindingLayout::new(vec![0, 1, 2]),
    );
    if let Err(e) = r {
        panic!("create_pipeline failed: {e:?}");
    }
}

fn usage() -> BufferUsage {
    BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

// DEV-152
#[test]
#[ignore = "requires a CUDA device"]
fn allocate_through_memory_returns_a_buffer_handle() {
    let device = open_compute_only();
    let memory = device.memory();
    let r: Result<BufferHandle, ContractError> = memory.allocate(16, usage());
    let handle = match r {
        Ok(h) => h,
        Err(e) => panic!("allocate failed: {e:?}"),
    };
    assert_eq!(handle.size(), 16);
    assert_eq!(handle.usage(), usage());
    memory.free(handle);
}

// DEV-157
#[test]
#[ignore = "requires a CUDA device"]
fn create_encoder_through_queue_returns_an_encoder() {
    let device = open_compute_only();
    let _encoder: CommandEncoder = device.queue().create_encoder();
}
