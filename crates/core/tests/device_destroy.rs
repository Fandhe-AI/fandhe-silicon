//! `Device::destroy` の既定の本体と上書きの到達を別の crate から確かめる公開テスト（TASK-191 のポインタのみ）。
//!
//! ダミーの device はこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: 既定の本体が `Drop` を 1 回だけ走らせること、上書きした `destroy` が `Box<dyn Device>` 越しに届くこと。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{Device, DeviceDiagnostics, DeviceOwnership};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, ImportedSemaphoreHandle, SubmitToken,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

const LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1 << 20,
    max_dispatch_dims: [64, 64, 64],
    max_texture_size: 2048,
};

/// 受けた引数をそのままハンドルへ渡す最小のメモリ操作。記憶域は確保しない。
struct DummyMemory;

impl MemoryOps for DummyMemory {
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
struct DummyQueue;

impl CommandQueue for DummyQueue {
    fn dispatch_ex(
        &self,
        _encoder: &mut CommandEncoder,
        _pipeline: &fandhe_silicon_core::execution::PipelineHandle,
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

/// `destroy` を上書きしない device。`Drop` の回数だけを数える。
struct DefaultDevice {
    ownership: DeviceOwnership,
    memory: DummyMemory,
    queue: DummyQueue,
    drops: Arc<AtomicU64>,
}

/// `destroy` を上書きし、呼ばれた印を立てる device。
struct OverridingDevice {
    ownership: DeviceOwnership,
    memory: DummyMemory,
    queue: DummyQueue,
    drops: Arc<AtomicU64>,
    destroyed: Arc<AtomicBool>,
}

impl Drop for DefaultDevice {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for OverridingDevice {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/// 2 つのダミーに共通の `Capabilities` の実装。
macro_rules! common_capabilities_impl {
    ($ty:ty) => {
        impl Capabilities for $ty {
            fn query(&self, _: FeatureId) -> CapabilityLevel {
                CapabilityLevel::S1
            }

            fn limits(&self) -> &LimitsTable {
                &LIMITS
            }
        }
    };
}

/// 2 つのダミーに共通の `Device` の必須の method。
macro_rules! common_device_methods {
    () => {
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
            &self.memory
        }

        fn queue(&self) -> &dyn CommandQueue {
            &self.queue
        }

        fn diagnostics(&self) -> DeviceDiagnostics {
            DeviceDiagnostics {
                adapter_name: "dummy-adapter".to_string(),
                backend_name: "dummy-backend",
            }
        }
    };
}

common_capabilities_impl!(DefaultDevice);
common_capabilities_impl!(OverridingDevice);

impl Device for DefaultDevice {
    common_device_methods!();
}

impl Device for OverridingDevice {
    common_device_methods!();

    fn destroy(self: Box<Self>) {
        // 上書きの本体: 印を立ててから値を落とす。
        self.destroyed.store(true, Ordering::SeqCst);
        drop(self);
    }
}

#[test]
fn the_default_destroy_drops_the_device_once() {
    let drops = Arc::new(AtomicU64::new(0));
    let device: Box<dyn Device> = Box::new(DefaultDevice {
        ownership: DeviceOwnership::ProductIndependent,
        memory: DummyMemory,
        queue: DummyQueue,
        drops: Arc::clone(&drops),
    });
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    device.destroy();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn an_overriding_destroy_is_reached_through_box_dyn_device() {
    let drops = Arc::new(AtomicU64::new(0));
    let destroyed = Arc::new(AtomicBool::new(false));
    let device: Box<dyn Device> = Box::new(OverridingDevice {
        ownership: DeviceOwnership::ProductIndependent,
        memory: DummyMemory,
        queue: DummyQueue,
        drops: Arc::clone(&drops),
        destroyed: Arc::clone(&destroyed),
    });
    device.destroy();
    assert!(destroyed.load(Ordering::SeqCst));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
