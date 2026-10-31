//! `Device::fault_hooks` の既定と上書きを別の crate から固定する公開テスト（TASK-271）。
//!
//! 試験用のダミーの device はこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: 上書きしない device では `None` で、`inject` が口の無いときのエラーを返すこと。
//! `FaultStub` を返すよう上書きした device では `inject` 経由のロストが届くこと。

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{Device, DeviceDiagnostics, DeviceOwnership};
use fandhe_silicon_core::error::{CompletionState, ContractError, DeviceLostReason};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, ImportedSemaphoreHandle, SubmitToken,
};
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::fault::{FaultHooks, HookFault, inject};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

const LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1 << 20,
    max_dispatch_dims: [64, 64, 64],
    max_texture_size: 2048,
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

/// 何も投入しない最小の投入の窓口。
struct NoQueue;

impl CommandQueue for NoQueue {
    fn dispatch_ex(
        &self,
        _encoder: &mut fandhe_silicon_core::execution::CommandEncoder,
        _pipeline: &fandhe_silicon_core::execution::PipelineHandle,
        _bindings: &[fandhe_silicon_core::execution::Binding<'_>],
        _consts: &[fandhe_silicon_core::execution::ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[fandhe_silicon_core::execution::ThreadgroupMemory],
    ) -> Result<(), fandhe_silicon_core::error::ContractError> {
        Err(fandhe_silicon_core::error::ContractError::Unsupported {
            op: "dispatch_ex",
            backend: fandhe_silicon_core::capability::BackendKind::Metal,
        })
    }

    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(1, None)
    }

    fn submit(
        &self,
        _: CommandEncoder,
        _: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(1, 1, 1))
    }

    fn submit_many(
        &self,
        _: Vec<CommandEncoder>,
        _: &[SubmitToken],
        _: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(1, 1, 1))
    }

    fn poll(&self, _: SubmitToken) -> CompletionState {
        CompletionState::Complete
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        callback(self.poll(token));
    }

    #[cfg(target_arch = "wasm32")]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState)>) {
        callback(self.poll(token));
    }

    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        _: SubmitToken,
        _: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        Ok(())
    }
}

/// 故障の口を持つかを選べるダミーの device。
struct DummyDevice {
    stub: Option<FaultStub>,
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

    fn queue(&self) -> &dyn CommandQueue {
        &NoQueue
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: "dummy".to_string(),
            backend_name: "dummy",
        }
    }

    fn fault_hooks(&self) -> Option<&dyn FaultHooks> {
        self.stub.as_ref().map(|s| s as &dyn FaultHooks)
    }
}

/// 上書きしない device。既定の本体が使われる。
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

    fn queue(&self) -> &dyn CommandQueue {
        &NoQueue
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: "default".to_string(),
            backend_name: "default",
        }
    }
}

#[test]
fn default_fault_hooks_is_none_through_dyn_device() {
    let device: Box<dyn Device> = Box::new(DefaultDevice {
        ownership: DeviceOwnership::ProductIndependent,
    });
    assert!(device.fault_hooks().is_none());
}

#[test]
fn inject_without_hooks_returns_fault_hooks_error() {
    let device = DefaultDevice {
        ownership: DeviceOwnership::ProductIndependent,
    };
    let err = inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Unknown),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ContractError::UnsupportedExtensionDefault { op: "fault_hooks" }
    ));
}

#[test]
fn overridden_fault_hooks_deliver_device_lost() {
    let device: Box<dyn Device> = Box::new(DummyDevice {
        stub: Some(FaultStub::new()),
        ownership: DeviceOwnership::ProductIndependent,
    });
    assert!(device.fault_hooks().is_some());
    inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Destroyed),
    )
    .expect("inject");
}

#[test]
fn stub_records_the_lost_reason_after_inject() {
    let device = DummyDevice {
        stub: Some(FaultStub::new()),
        ownership: DeviceOwnership::ProductIndependent,
    };
    let stub = device.stub.as_ref().expect("stub");
    assert!(!stub.is_lost());
    inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Destroyed),
    )
    .expect("inject");
    assert!(stub.is_lost());
    assert_eq!(stub.lost_reason(), Some(DeviceLostReason::Destroyed));
}

#[test]
fn overriding_device_can_still_report_none() {
    let device = DummyDevice {
        stub: None,
        ownership: DeviceOwnership::ProductIndependent,
    };
    assert!(device.fault_hooks().is_none());
}
