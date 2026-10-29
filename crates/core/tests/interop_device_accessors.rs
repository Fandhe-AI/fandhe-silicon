//! `Device::external_interop`・`Device::physical_gpu_id` の既定と上書きを別の crate から固定する公開テスト（TASK-717）。
//!
//! 試験用のダミーの device はこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: 上書きしない device では `Box<dyn Device>` 越しに両方とも `None`（取得できない扱い）、
//! 上書きした device では `Some` が届くこと。

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{Device, DeviceDiagnostics, DeviceOwnership};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
use fandhe_silicon_core::interop::{
    ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle, PhysicalGpuId,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

const LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1 << 20,
    max_dispatch_dims: [64, 64, 64],
    max_texture_size: 2048,
};

const GPU: PhysicalGpuId = PhysicalGpuId {
    device_uuid: [3; 16],
    driver_uuid: None,
};

struct NoMemory;

impl MemoryOps for NoMemory {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(0, 1, 1, byte_len, usage))
    }

    fn upload(&self, _: &BufferHandle, _: u64, _: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, _: BufferHandle) {}
}

struct Interop;

impl ExternalInterop for Interop {
    fn export_memory(&self, _: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> {
        Ok(ExternalMemoryHandle::__backend_new(1, false, Some(GPU)))
    }

    fn import_memory(
        &self,
        _: ExternalMemoryHandle,
        byte_len: u64,
        usage: BufferUsage,
    ) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(0, 1, 1, byte_len, usage))
    }

    fn export_semaphore(&self, _: SubmitToken) -> Result<ExternalSemaphoreHandle, ContractError> {
        Ok(ExternalSemaphoreHandle::__backend_new(1, false))
    }

    fn import_semaphore(
        &self,
        _: ExternalSemaphoreHandle,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        Ok(ImportedSemaphoreHandle::__backend_new(1))
    }
}

/// 相互運用の口を上書きするかを選べるダミーの device。
struct DummyDevice {
    overrides: bool,
    interop: Interop,
    ownership: DeviceOwnership,
}

impl Capabilities for DummyDevice {
    fn query(&self, _: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S1
    }

    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }
}

impl Device for DummyDevice {
    fn backend(&self) -> BackendKind {
        BackendKind::Metal
    }

    fn ownership(&self) -> &DeviceOwnership {
        &self.ownership
    }

    fn generation(&self) -> u64 {
        1
    }

    fn memory(&self) -> &dyn MemoryOps {
        &NoMemory
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: "dummy".to_string(),
            backend_name: "dummy",
        }
    }

    fn external_interop(&self) -> Option<&dyn ExternalInterop> {
        self.overrides
            .then_some(&self.interop as &dyn ExternalInterop)
    }

    fn physical_gpu_id(&self) -> Option<PhysicalGpuId> {
        self.overrides.then_some(GPU)
    }
}

/// 両方の口を上書きしない device。既定の本体が使われる。
struct DefaultDevice {
    ownership: DeviceOwnership,
}

impl Capabilities for DefaultDevice {
    fn query(&self, _: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S1
    }

    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }
}

impl Device for DefaultDevice {
    fn backend(&self) -> BackendKind {
        BackendKind::Metal
    }

    fn ownership(&self) -> &DeviceOwnership {
        &self.ownership
    }

    fn generation(&self) -> u64 {
        1
    }

    fn memory(&self) -> &dyn MemoryOps {
        &NoMemory
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: "default".to_string(),
            backend_name: "default",
        }
    }
}

#[test]
fn defaults_are_none_through_dyn_device() {
    let device: Box<dyn Device> = Box::new(DefaultDevice {
        ownership: DeviceOwnership::ProductIndependent,
    });
    assert!(device.external_interop().is_none());
    assert!(device.physical_gpu_id().is_none());
}

#[test]
fn overridden_accessors_reach_through_dyn_device() {
    let device: Box<dyn Device> = Box::new(DummyDevice {
        overrides: true,
        interop: Interop,
        ownership: DeviceOwnership::ProductIndependent,
    });
    assert_eq!(device.physical_gpu_id(), Some(GPU));
    let interop = device.external_interop().expect("interop");
    let buffer = BufferHandle::__backend_new(0, 1, 1, 8, BufferUsage::EXPORTABLE);
    let handle = interop.export_memory(&buffer).expect("export");
    assert_eq!(handle.__backend_source_gpu(), Some(&GPU));
}

#[test]
fn overriding_device_can_still_report_none() {
    let device = DummyDevice {
        overrides: false,
        interop: Interop,
        ownership: DeviceOwnership::ProductIndependent,
    };
    assert!(device.external_interop().is_none());
    assert!(device.physical_gpu_id().is_none());
}
