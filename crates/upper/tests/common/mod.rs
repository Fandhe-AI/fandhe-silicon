//! upper の統合テストが `mod common;` で取り込む、呼び出しを記録する試験用の device。
//!
//! core の `StubDevice` を包み、契約の queue の操作の呼び出しを順に記録する。
//! 一部の戻り値は `Overrides` で差し替えられる。公開 API ではなく、試験の中だけで使う。

use std::cell::RefCell;
#[cfg(feature = "native-blocking-wait")]
use std::time::Duration;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    Binding, CommandEncoder, CommandQueue, ConstantBinding, GpuTiming, ImportedSemaphoreHandle,
    PipelineHandle, SubmitToken, ThreadgroupMemory,
};
use fandhe_silicon_core::memory::MemoryOps;
use fandhe_silicon_core::stub_device::StubDevice;

/// 記録した契約の queue の操作。
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    /// `create_encoder`。
    CreateEncoder,
    /// `submit`。待ちの個数と、`Ok` で返した印（差し替えたときは `None`）。
    Submit {
        wait_len: usize,
        token: Option<SubmitToken>,
    },
    /// `submit_many`。
    SubmitMany,
    /// `poll`。
    Poll,
    /// `on_complete`。
    OnComplete,
    /// `wait_blocking`。受けた印と待ちの上限。
    #[cfg(feature = "native-blocking-wait")]
    WaitBlocking {
        token: SubmitToken,
        timeout: Option<Duration>,
    },
    /// `dispatch_ex`。
    DispatchEx,
    /// `gpu_timing`。受けた印。
    GpuTiming { token: SubmitToken },
    /// `query_ex`。受けた鍵。
    QueryEx { feature: FeatureId },
}

/// 戻り値の差し替え。何も入れなければ `StubDevice` の値がそのまま返る。
#[derive(Default)]
pub struct Overrides {
    /// `submit` を `Err` にする。
    pub submit: Option<ContractError>,
    /// `gpu_timing` の値。
    pub gpu_timing: Option<GpuTiming>,
    /// `wait_blocking` を `Err` にする。
    #[cfg(feature = "native-blocking-wait")]
    pub wait_blocking: Option<ContractError>,
}

/// 呼び出しを記録する試験用の device。
pub struct Recorder {
    inner: StubDevice,
    overrides: Overrides,
    calls: RefCell<Vec<Call>>,
}

impl Recorder {
    /// 差し替えを指定して作る。
    pub fn new(overrides: Overrides) -> Self {
        Self {
            inner: StubDevice::new(DeviceRequest {
                backend: BackendKind::Metal,
                profile: Profile::ComputeOnly,
                ownership: DeviceOwnership::Shared,
            }),
            overrides,
            calls: RefCell::new(Vec::new()),
        }
    }

    /// これまでの記録の写し。
    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }

    fn record(&self, call: Call) {
        self.calls.borrow_mut().push(call);
    }
}

impl Capabilities for Recorder {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        self.inner.query(feature)
    }

    fn limits(&self) -> &LimitsTable {
        self.inner.limits()
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        self.record(Call::QueryEx { feature });
        self.inner.query_ex(feature)
    }
}

impl Device for Recorder {
    fn backend(&self) -> BackendKind {
        self.inner.backend()
    }

    fn ownership(&self) -> &DeviceOwnership {
        self.inner.ownership()
    }

    fn generation(&self) -> u64 {
        self.inner.generation()
    }

    fn memory(&self) -> &dyn MemoryOps {
        self.inner.memory()
    }

    fn queue(&self) -> &dyn CommandQueue {
        self
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        self.inner.diagnostics()
    }
}

impl CommandQueue for Recorder {
    fn create_encoder(&self) -> CommandEncoder {
        self.record(Call::CreateEncoder);
        self.inner.queue().create_encoder()
    }

    fn submit(
        &self,
        commands: CommandEncoder,
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        // 先に stub へ渡して、stub の内部の状態を崩さない。
        let result = self.inner.queue().submit(commands, wait);
        let (result, token) = match (&self.overrides.submit, result) {
            (Some(e), _) => (Err(e.clone()), None),
            (None, Ok(t)) => (Ok(t), Some(t)),
            (None, Err(e)) => (Err(e), None),
        };
        self.record(Call::Submit {
            wait_len: wait.len(),
            token,
        });
        result
    }

    fn submit_many(
        &self,
        commands: Vec<CommandEncoder>,
        after: &[SubmitToken],
        wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        self.record(Call::SubmitMany);
        self.inner.queue().submit_many(commands, after, wait)
    }

    fn poll(&self, token: SubmitToken) -> CompletionState {
        self.record(Call::Poll);
        self.inner.queue().poll(token)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        self.record(Call::OnComplete);
        self.inner.queue().on_complete(token, callback);
    }

    #[cfg(target_arch = "wasm32")]
    fn on_complete(&self, token: SubmitToken, callback: Box<dyn FnOnce(CompletionState)>) {
        self.record(Call::OnComplete);
        self.inner.queue().on_complete(token, callback);
    }

    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        token: SubmitToken,
        timeout: Option<Duration>,
    ) -> Result<(), ContractError> {
        self.record(Call::WaitBlocking { token, timeout });
        let result = self.inner.queue().wait_blocking(token, timeout);
        match &self.overrides.wait_blocking {
            Some(e) => Err(e.clone()),
            None => result,
        }
    }

    fn gpu_timing(&self, token: SubmitToken) -> GpuTiming {
        self.record(Call::GpuTiming { token });
        match self.overrides.gpu_timing {
            Some(t) => t,
            None => self.inner.queue().gpu_timing(token),
        }
    }

    // 引数は契約の宣言の並びそのまま。
    fn dispatch_ex(
        &self,
        encoder: &mut CommandEncoder,
        pipeline: &PipelineHandle,
        bindings: &[Binding<'_>],
        consts: &[ConstantBinding<'_>],
        dispatch_dims: [u32; 3],
        threads_per_threadgroup: [u32; 3],
        threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        self.record(Call::DispatchEx);
        self.inner.queue().dispatch_ex(
            encoder,
            pipeline,
            bindings,
            consts,
            dispatch_dims,
            threads_per_threadgroup,
            threadgroup_memory,
        )
    }
}
