//! exec の統合テストが `mod common;` で取り込む試験用の device。
//!
//! 能力の問合せの値を差し込み、`query_ex`・`memory`・`queue` の呼び出し回数を数える。
//! 公開 API ではなく、試験の中だけで使う。
// 各テストバイナリは全項目を使わないので、未使用の警告を抑える。
#![allow(dead_code)]

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceRequest, Profile,
};
use fandhe_silicon_core::execution::CommandQueue;
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::memory::MemoryOps;
use fandhe_silicon_core::stub_device::StubDevice;
use std::cell::Cell;

/// 能力の問合せを差し込み、呼び出しを数える試験用の device。
pub struct Scripted {
    dev: FaultInjecting<StubDevice>,
    query_ex_calls: Cell<u32>,
    memory_calls: Cell<u32>,
    queue_calls: Cell<u32>,
}

impl Scripted {
    fn with(script: FaultScript) -> Self {
        let inner = StubDevice::new(DeviceRequest {
            backend: BackendKind::Metal,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::Shared,
        });
        Self {
            dev: FaultInjecting::new(inner, script),
            query_ex_calls: Cell::new(0),
            memory_calls: Cell::new(0),
            queue_calls: Cell::new(0),
        }
    }

    /// 何も差し込まない device。
    pub fn bare() -> Self {
        Self::with(FaultScript::PassThrough)
    }

    /// `feature` だけを能力不足にする device。
    pub fn insufficient(feature: FeatureId) -> Self {
        Self::with(FaultScript::CapabilityInsufficient {
            feature,
            required: CapabilityLevel::S2,
        })
    }

    /// 能力の取得が失敗する device。
    pub fn query_fails() -> Self {
        Self::with(FaultScript::CapabilityQueryFails)
    }

    /// `query_ex` が呼ばれた回数。
    pub fn query_ex_calls(&self) -> u32 {
        self.query_ex_calls.get()
    }

    /// `memory` が呼ばれた回数。
    pub fn memory_calls(&self) -> u32 {
        self.memory_calls.get()
    }

    /// `queue` が呼ばれた回数。
    pub fn queue_calls(&self) -> u32 {
        self.queue_calls.get()
    }
}

impl Capabilities for Scripted {
    fn limits(&self) -> &LimitsTable {
        self.dev.limits()
    }

    fn query(&self, f: FeatureId) -> CapabilityLevel {
        self.dev.query(f)
    }

    // 既定の実装では理由が落ちるので、数えたうえで内側の値をそのまま返す。
    fn query_ex(&self, f: FeatureId) -> CapabilityQuery {
        self.query_ex_calls.set(self.query_ex_calls.get() + 1);
        self.dev.query_ex(f)
    }
}

impl Device for Scripted {
    fn backend(&self) -> BackendKind {
        self.dev.inner().backend()
    }

    fn ownership(&self) -> &DeviceOwnership {
        self.dev.inner().ownership()
    }

    fn generation(&self) -> u64 {
        self.dev.inner().generation()
    }

    fn memory(&self) -> &dyn MemoryOps {
        self.memory_calls.set(self.memory_calls.get() + 1);
        self.dev.inner().memory()
    }

    fn queue(&self) -> &dyn CommandQueue {
        self.queue_calls.set(self.queue_calls.get() + 1);
        self.dev.inner().queue()
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        self.dev.inner().diagnostics()
    }
}
