//! 故障の差し込みの部品の組み上がりの確認（spec TASK-271 の 7/8）。デコレータとスタブが、包む内側の型や
//! 背後実装の種類に依らず同じに働くこと、(B) の口を呼ぶ側の部品 `inject` がデコレータ越し・スタブ越し・
//! 口を返す側越しに届くこと、2 系統（筋書きと口）が独立に働くことを確かめる。
//! 差し込むのは模擬であって、実際の故障ではない。

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::error::{CompletionStateEx, ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::fault::{FaultHooks, HookFault, inject};

const A: FeatureId = FeatureId("feature:A");
const B: FeatureId = FeatureId("feature:B");

// ---- 内側の型 3 種（いずれも A・B とも S2 を返す） ----

/// `Cell` で数える `!Sync` の内側。口は既定の実装のまま。
#[derive(Default)]
struct CellInner {
    queries: Cell<u32>,
}

/// `Capabilities::limits` の試験用の表。値は試験用の小さな数で、意味は持たない。
static TEST_LIMITS: fandhe_silicon_core::capability::LimitsTable =
    fandhe_silicon_core::capability::LimitsTable {
        max_buffer_bytes: 1,
        max_dispatch_dims: [1, 1, 1],
        max_texture_size: 1,
    };

impl Capabilities for CellInner {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, _f: FeatureId) -> CapabilityLevel {
        self.queries.set(self.queries.get() + 1);
        CapabilityLevel::S2
    }
}
impl FaultHooks for CellInner {}

/// `AtomicU32` で数え、スタブへ口を委譲する `Send + Sync` の内側。
#[derive(Default)]
struct StubDevice {
    queries: AtomicU32,
    stub: FaultStub,
}
impl fmt::Debug for StubDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StubDevice")
    }
}
impl Capabilities for StubDevice {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, _f: FeatureId) -> CapabilityLevel {
        self.queries.fetch_add(1, Ordering::SeqCst);
        CapabilityLevel::S2
    }
}
impl FaultHooks for StubDevice {
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        self.stub.simulate_device_lost(reason)
    }
    fn arm_gate(&self) -> Result<(), ContractError> {
        self.stub.arm_gate()
    }
    fn open_gate(&self) -> Result<(), ContractError> {
        self.stub.open_gate()
    }
    fn simulate_runtime_error(&self, detail: String) -> Result<(), ContractError> {
        self.stub.simulate_runtime_error(detail)
    }
}

/// 試験の中だけの trait。型を消した内側として使う。
/// wasm32 では `FaultStub` の通知の関数が `Send` でないので、`Send + Sync` は求めない。
#[cfg(not(target_arch = "wasm32"))]
trait Dev: Capabilities + FaultHooks + Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Capabilities + FaultHooks + Send + Sync> Dev for T {}
#[cfg(target_arch = "wasm32")]
trait Dev: Capabilities + FaultHooks {}
#[cfg(target_arch = "wasm32")]
impl<T: Capabilities + FaultHooks> Dev for T {}

/// 型を消した内側。`Capabilities`・`FaultHooks` を委譲する。
struct BoxInner(Box<dyn Dev>);
impl Capabilities for BoxInner {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        self.0.limits()
    }

    fn query(&self, f: FeatureId) -> CapabilityLevel {
        self.0.query(f)
    }
    fn query_ex(&self, f: FeatureId) -> CapabilityQuery {
        self.0.query_ex(f)
    }
}
impl FaultHooks for BoxInner {
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        self.0.simulate_device_lost(reason)
    }
    fn arm_gate(&self) -> Result<(), ContractError> {
        self.0.arm_gate()
    }
    fn open_gate(&self) -> Result<(), ContractError> {
        self.0.open_gate()
    }
    fn simulate_runtime_error(&self, detail: String) -> Result<(), ContractError> {
        self.0.simulate_runtime_error(detail)
    }
}

// ---- 内側の型に依らないこと ----

/// 内側の型 `D` に依らず、筋書きごとの結果が同じであること。
fn check_scripts<D: Capabilities + FaultHooks>(make: impl Fn() -> D) {
    let wait_calls = Cell::new(0u32);
    let wait = |_: &D| {
        wait_calls.set(wait_calls.get() + 1);
        Ok(())
    };

    // 通す
    let d = FaultInjecting::new(make(), FaultScript::PassThrough);
    assert_eq!(d.wait_blocking_with(wait), Ok(()));
    assert_eq!(wait_calls.replace(0), 1);
    assert_eq!(d.query(A), CapabilityLevel::S2);
    assert_eq!(d.query(B), CapabilityLevel::S2);

    // 完了不明
    let d = FaultInjecting::new(make(), FaultScript::CompletionUnknown);
    assert_eq!(d.wait_blocking_with(wait), Err(ContractError::Timeout));
    assert_eq!(
        d.poll_ex_with(|_| CompletionStateEx::Complete),
        CompletionStateEx::Unknown
    );
    assert_eq!(wait_calls.replace(0), 0);

    // 能力不足（A だけ）
    let d = FaultInjecting::new(
        make(),
        FaultScript::CapabilityInsufficient {
            feature: A,
            required: CapabilityLevel::S2,
        },
    );
    assert_eq!(d.query(A), CapabilityLevel::Unsupported);
    assert_eq!(d.query(B), CapabilityLevel::S2);
    assert!(matches!(
        d.create_pipeline_with(|_| Ok(1u32)),
        Err(ContractError::CapabilityInsufficient { .. })
    ));

    // 能力の取得の失敗
    let d = FaultInjecting::new(make(), FaultScript::CapabilityQueryFails);
    assert_eq!(d.query(A), CapabilityLevel::S0);
    assert_eq!(d.query(B), CapabilityLevel::S0);

    // 確保の失敗
    let d = FaultInjecting::new(make(), FaultScript::AllocationFails);
    assert_eq!(
        d.allocate_with(64, |_, n| Ok(n)),
        Err(ContractError::AllocationFailed {
            requested_bytes: 64
        })
    );

    // 実行時エラー
    let d = FaultInjecting::new(
        make(),
        FaultScript::RuntimeError {
            backend: BackendKind::Cuda,
            detail: "boom".into(),
        },
    );
    assert_eq!(
        d.wait_blocking_with(wait),
        Err(ContractError::BackendFailure {
            op: "wait_blocking",
            backend: BackendKind::Cuda,
            detail: "boom".into()
        })
    );
    assert_eq!(wait_calls.get(), 0);

    // 口は内側の戻りそのまま
    let d = FaultInjecting::new(make(), FaultScript::PassThrough);
    assert_eq!(d.arm_gate(), d.inner().arm_gate());
    assert_eq!(
        d.simulate_runtime_error("x".into()),
        d.inner().simulate_runtime_error("x".into())
    );
}

#[test]
fn works_over_cell_inner() {
    check_scripts(CellInner::default);
}

#[test]
fn works_over_sync_stub_inner() {
    check_scripts(StubDevice::default);
}

#[test]
fn works_over_type_erased_inner() {
    check_scripts(|| BoxInner(Box::new(StubDevice::default())));
    check_scripts(|| BoxInner(Box::new(CellInnerSync)));
}

/// `Sync` な、`CellInner` と同じ振る舞いの内側（型を消す内側用）。
struct CellInnerSync;
impl Capabilities for CellInnerSync {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, _f: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S2
    }
}
impl FaultHooks for CellInnerSync {}

#[test]
fn type_erased_and_direct_inner_agree_on_inner_calls() {
    let direct = FaultInjecting::new(StubDevice::default(), FaultScript::PassThrough);
    let erased = FaultInjecting::new(
        BoxInner(Box::new(StubDevice::default())),
        FaultScript::PassThrough,
    );
    assert_eq!(direct.query(A), erased.query(A));
    assert_eq!(direct.query_ex(B), erased.query_ex(B));
    assert_eq!(direct.inner().queries.load(Ordering::SeqCst), 2);
}

// ---- 背後実装の種類に依らないこと ----

#[test]
fn runtime_error_keeps_backend_kind_from_script() {
    for backend in [
        BackendKind::Metal,
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        let d = FaultInjecting::new(
            CellInner::default(),
            FaultScript::RuntimeError {
                backend,
                detail: "d".into(),
            },
        );
        let inner_calls = Cell::new(0u32);
        assert_eq!(
            d.wait_blocking_with(|_| {
                inner_calls.set(1);
                Ok(())
            }),
            Err(ContractError::BackendFailure {
                op: "wait_blocking",
                backend,
                detail: "d".into()
            }),
            "{backend:?}"
        );
        assert_eq!(inner_calls.get(), 0, "{backend:?}");
        // 種類で変わらない差し込み。
        assert_eq!(
            d.poll_ex_with(|_| CompletionStateEx::Complete),
            CompletionStateEx::RuntimeError
        );
        assert_eq!(d.submit_with(|_| Ok(1u32)), Ok(1), "{backend:?}");
        assert_eq!(d.query(A), CapabilityLevel::S2, "{backend:?}");
    }
}

// ---- 型の組み上がり ----

#[cfg(not(target_arch = "wasm32"))]
fn assert_send_sync<T: Send + Sync>() {}
fn assert_debug<T: fmt::Debug>() {}

#[test]
fn decorator_over_sync_inner_is_send_sync_debug_and_coerces() {
    // wasm32 では `FaultStub` が `Send` でないので、この確かめは外す。
    #[cfg(not(target_arch = "wasm32"))]
    {
        assert_send_sync::<FaultInjecting<StubDevice>>();
        assert_send_sync::<FaultInjecting<BoxInner>>();
    }
    assert_debug::<FaultInjecting<StubDevice>>();
    let d = FaultInjecting::new(StubDevice::default(), FaultScript::PassThrough);
    let hooks: &dyn FaultHooks = &d;
    let caps: &dyn Capabilities = &d;
    assert_eq!(hooks.arm_gate(), Ok(()));
    assert_eq!(caps.query(A), CapabilityLevel::S2);
}

// ---- (B) の口を呼ぶ側の組み上がり ----

/// 試験の中だけの口のアクセサ（本体の crate には無い）。
trait HasFaultHooks {
    fn fault_hooks(&self) -> Option<&dyn FaultHooks>;
}
impl HasFaultHooks for StubDevice {
    fn fault_hooks(&self) -> Option<&dyn FaultHooks> {
        Some(&self.stub)
    }
}
impl HasFaultHooks for FaultInjecting<StubDevice> {
    fn fault_hooks(&self) -> Option<&dyn FaultHooks> {
        Some(self)
    }
}
struct NoHooksDevice;
impl HasFaultHooks for NoHooksDevice {
    fn fault_hooks(&self) -> Option<&dyn FaultHooks> {
        None
    }
}

fn exercise_stub_through(dev: &dyn HasFaultHooks, stub: &FaultStub) {
    let count = Arc::new(AtomicU32::new(0));
    let c = Arc::clone(&count);
    stub.set_lost_callback(Box::new(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    }));
    assert_eq!(
        inject(
            dev.fault_hooks(),
            HookFault::DeviceLost(DeviceLostReason::Destroyed)
        ),
        Ok(())
    );
    assert_eq!(
        inject(
            dev.fault_hooks(),
            HookFault::DeviceLost(DeviceLostReason::Unknown)
        ),
        Ok(())
    );
    assert_eq!(stub.lost_reason(), Some(DeviceLostReason::Destroyed));
    assert_eq!(stub.lost_callback_count(), 1);
    assert_eq!(count.load(Ordering::SeqCst), 1);

    assert_eq!(inject(dev.fault_hooks(), HookFault::ArmGate), Ok(()));
    assert!(stub.gate_armed());
    assert_eq!(inject(dev.fault_hooks(), HookFault::OpenGate), Ok(()));
    assert!(!stub.gate_armed());
    assert_eq!(inject(dev.fault_hooks(), HookFault::OpenGate), Ok(()));
    assert!(!stub.gate_armed());
}

#[test]
fn inject_reaches_stub_through_accessor() {
    let dev = StubDevice::default();
    exercise_stub_through(&dev, &dev.stub);
}

#[test]
fn inject_reaches_stub_through_decorator_accessor() {
    for script in [FaultScript::PassThrough, FaultScript::CompletionUnknown] {
        let dev = FaultInjecting::new(StubDevice::default(), script);
        exercise_stub_through(&dev, &dev.inner().stub);
    }
}

#[test]
fn inject_without_hooks_is_unsupported_default() {
    assert_eq!(
        inject(NoHooksDevice.fault_hooks(), HookFault::ArmGate),
        Err(ContractError::UnsupportedExtensionDefault { op: "fault_hooks" })
    );
}

#[test]
fn inject_through_default_inner_hooks_is_unsupported_default() {
    let d = FaultInjecting::new(CellInner::default(), FaultScript::AllocationFails);
    assert_eq!(
        inject(Some(&d), HookFault::OpenGate),
        Err(ContractError::UnsupportedExtensionDefault { op: "open_gate" })
    );
}

// ---- 2 系統が独立に働く ----

#[test]
fn script_and_hooks_work_independently() {
    let d = FaultInjecting::new(StubDevice::default(), FaultScript::CompletionUnknown);
    let submits = Cell::new(0u32);
    let waits = Cell::new(0u32);
    let submit = || {
        d.submit_with(|_| {
            submits.set(submits.get() + 1);
            Ok(1u32)
        })
    };
    assert_eq!(submit(), Ok(1));
    assert_eq!(
        inject(
            Some(&d),
            HookFault::DeviceLost(DeviceLostReason::Unrecoverable("x".into()))
        ),
        Ok(())
    );
    assert!(d.inner().stub.is_lost());
    // 筋書きはロストに影響されない。
    assert_eq!(
        d.wait_blocking_with(|_| {
            waits.set(waits.get() + 1);
            Ok(())
        }),
        Err(ContractError::Timeout)
    );
    assert_eq!(waits.get(), 0);
    assert_eq!(submit(), Ok(1));
    assert_eq!(submits.get(), 2);
    assert_eq!(d.submit_call_count(), 2);
}
