//! CUDA の device が返す相互運用と故障の差し込みの口の確認（TASK-197 のポインタ）。
//!
//! 物理 GPU の識別子を `Box<dyn Device>` から呼ぶ試験もここに置く。
//!
//! どの試験も device を作るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::SubmitToken;
use fandhe_silicon_core::interop::{
    ExternalMemoryHandle, ExternalSemaphoreHandle, check_same_physical_gpu,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

fn new_device() -> Box<dyn Device> {
    CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("request")
}

fn unsupported(op: &'static str) -> ContractError {
    ContractError::Unsupported {
        op,
        backend: BackendKind::Cuda,
    }
}

/// 必須の 4 操作は、どれも呼んだ操作の名前つきの `Unsupported` を返す。
#[test]
#[ignore = "requires a CUDA device"]
fn required_interop_ops_are_unsupported() {
    let device = new_device();
    let interop = device.external_interop().expect("external_interop");
    let buffer = BufferHandle::__backend_new(1, 1, 1, 16, BufferUsage::STORAGE);
    assert_eq!(
        interop.export_memory(&buffer).unwrap_err(),
        unsupported("export_memory")
    );
    assert_eq!(
        interop
            .import_memory(
                ExternalMemoryHandle::__backend_new(1, false, None),
                16,
                BufferUsage::STORAGE
            )
            .unwrap_err(),
        unsupported("import_memory")
    );
    assert_eq!(
        interop
            .export_semaphore(SubmitToken::__backend_new(0, 0, 0))
            .unwrap_err(),
        unsupported("export_semaphore")
    );
    assert_eq!(
        interop
            .import_semaphore(ExternalSemaphoreHandle::__backend_new(1, false))
            .unwrap_err(),
        unsupported("import_semaphore")
    );
}

/// 拡張の timeline の 2 操作は上書きされておらず、trait の既定の本体の `UnsupportedExtensionDefault`。
#[test]
#[ignore = "requires a CUDA device"]
fn extension_ops_keep_the_trait_defaults() {
    let device = new_device();
    let interop = device.external_interop().expect("external_interop");
    assert_eq!(
        interop
            .export_semaphore_timeline(SubmitToken::__backend_new(0, 0, 0))
            .unwrap_err(),
        ContractError::UnsupportedExtensionDefault {
            op: "export_semaphore_timeline"
        }
    );
    assert_eq!(
        interop
            .import_semaphore_timeline(ExternalSemaphoreHandle::__backend_new(1, true), 1)
            .unwrap_err(),
        ContractError::UnsupportedExtensionDefault {
            op: "import_semaphore_timeline"
        }
    );
}

/// 故障の差し込みの口は `Some` で、上書きしていない操作は既定の `UnsupportedExtensionDefault`。
#[cfg(feature = "fault-injection")]
#[test]
#[ignore = "requires a CUDA device"]
fn fault_hooks_are_present_with_default_bodies() {
    let device = new_device();
    let hooks = device.fault_hooks().expect("fault_hooks");
    assert_eq!(
        hooks.arm_gate().unwrap_err(),
        ContractError::UnsupportedExtensionDefault { op: "arm_gate" }
    );
}

/// 物理 GPU の識別子は `Some` で、device の UUID だけを持ち driver の UUID は持たない。
// DEV-75
#[test]
#[ignore = "requires a CUDA device"]
fn physical_gpu_id_has_a_device_uuid_and_no_driver_uuid() {
    let device = new_device();
    let Some(id) = device.physical_gpu_id() else {
        panic!("physical_gpu_id is None");
    };
    assert!(id.driver_uuid.is_none());
}

/// 同じ GPU に作った 2 つの device は、等しい識別子を返し同一の物理 GPU と判定される。
#[test]
#[ignore = "requires a CUDA device"]
fn two_devices_report_the_same_physical_gpu() {
    let a = new_device();
    let b = new_device();
    let (Some(ia), Some(ib)) = (a.physical_gpu_id(), b.physical_gpu_id()) else {
        panic!("physical_gpu_id is None");
    };
    assert_eq!(ia, ib);
    assert!(check_same_physical_gpu(Some(&ia), Some(&ib)).is_ok());
}

/// `Box<dyn Device>` 越しに `external_interop()` が `Some` を返す。
// DEV-78
#[test]
#[ignore = "requires a CUDA device"]
fn external_interop_is_some_through_dyn_device() {
    let device = new_device();
    assert!(device.external_interop().is_some());
}
