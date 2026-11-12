//! `Device::artifact_hooks` の既定と上書きを別の crate から固定する公開テスト（TASK-492）。
//!
//! 試験用のダミーの device はこのファイルの中だけにあり、公開 API ではない。機材にもファイルにも触れない。
//! 確かめるもの: 上書きしない device では `None` であること。
//! ダミーの口を返すよう上書きした device で、3 つの操作がダミーの返す値をそのまま返すこと。

use fandhe_silicon_core::artifact::ArtifactHooks;
use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{Device, DeviceDiagnostics, DeviceOwnership};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, DType, ImportedSemaphoreHandle, MslMathMode, OperandSpec,
    PipelineHandle, SubmitToken,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};
use std::path::Path;

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
        _encoder: &mut CommandEncoder,
        _pipeline: &PipelineHandle,
        _bindings: &[fandhe_silicon_core::execution::Binding<'_>],
        _consts: &[fandhe_silicon_core::execution::ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[fandhe_silicon_core::execution::ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        Err(ContractError::Unsupported {
            op: "dispatch_ex",
            backend: BackendKind::Metal,
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

    fn max_buffer_bytes(&self) -> u64 {
        0
    }
}

/// 決めた値を返すダミーの口。
struct DummyHooks;

impl ArtifactHooks for DummyHooks {
    fn compile_and_write_binary_archive(
        &self,
        _: &str,
        _: &str,
        _: MslMathMode,
        _: &Path,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(vec![1, 2, 3])
    }

    fn create_pipeline_from_archive_path(
        &self,
        _: &str,
        _: &str,
        _: MslMathMode,
        _: &Path,
        fail_on_archive_miss: bool,
    ) -> Result<PipelineHandle, ContractError> {
        if fail_on_archive_miss {
            Err(ContractError::Unsupported {
                op: "create_pipeline_from_archive_path",
                backend: BackendKind::Metal,
            })
        } else {
            Ok(PipelineHandle::__backend_new(7, 1))
        }
    }

    fn create_pipeline_checked_from_metallib(
        &self,
        _: &Path,
        _: &str,
        _: &[OperandSpec<'_>],
    ) -> Result<PipelineHandle, ContractError> {
        Err(ContractError::ArtifactRejected {
            kind: "metallib",
            reason: "dummy".to_string(),
        })
    }
}

/// 口を持つかを選べるダミーの device。
struct DummyDevice {
    hooks: Option<DummyHooks>,
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

    fn artifact_hooks(&self) -> Option<&dyn ArtifactHooks> {
        self.hooks.as_ref().map(|h| h as &dyn ArtifactHooks)
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

/// 上書きしない device は口を持たない。対応する項目のポインタ: DEV-80。
#[test]
fn default_artifact_hooks_is_none_through_dyn_device() {
    let device: Box<dyn Device> = Box::new(DefaultDevice {
        ownership: DeviceOwnership::ProductIndependent,
    });
    assert!(device.artifact_hooks().is_none());
}

#[test]
fn overridden_artifact_hooks_return_dummy_values() {
    let device: Box<dyn Device> = Box::new(DummyDevice {
        hooks: Some(DummyHooks),
        ownership: DeviceOwnership::ProductIndependent,
    });
    let hooks = device.artifact_hooks().expect("hooks");
    let path = Path::new("/nonexistent/artifact.bin");

    let bytes = hooks
        .compile_and_write_binary_archive("src", "main0", MslMathMode::Safe, path)
        .expect("archive bytes");
    assert_eq!(bytes, vec![1, 2, 3]);

    let pipeline = hooks
        .create_pipeline_from_archive_path("src", "main0", MslMathMode::Safe, path, false)
        .expect("pipeline");
    assert_eq!(pipeline.__backend_id(), 7);
    assert_eq!(pipeline.__backend_device_id(), 1);

    let miss =
        hooks.create_pipeline_from_archive_path("src", "main0", MslMathMode::Safe, path, true);
    assert!(matches!(
        miss,
        Err(ContractError::Unsupported {
            op: "create_pipeline_from_archive_path",
            ..
        })
    ));

    let declared = [OperandSpec {
        slot: 0,
        dtype: DType::F32,
        shape: &[4],
        stride: &[1],
        alignment: 4,
    }];
    let rejected = hooks.create_pipeline_checked_from_metallib(path, "main0", &declared);
    assert!(matches!(
        rejected,
        Err(ContractError::ArtifactRejected {
            kind: "metallib",
            ..
        })
    ));
}

#[test]
fn overriding_device_can_still_report_none() {
    let device = DummyDevice {
        hooks: None,
        ownership: DeviceOwnership::ProductIndependent,
    };
    assert!(device.artifact_hooks().is_none());
}
