//! CUDA の背後実装の device に故障の差し込みのデコレータを被せ、能力不足の模擬が返す値と、
//! ほかの失敗との区別を公開の `Device` から照らす試験（TASK-178 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。どの試験も本体を別のスレッドで流し、上限の時間を過ぎたら失敗とする。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::{CudaDeviceProvider, CudaDriverMissingProvider};
use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{
    CompletionStateEx, ContractError, DeviceLostReason, FailureClass, UnsupportedReason,
};
use fandhe_silicon_core::execution::{BindingLayout, PipelineInput};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::fault::{HookFault, inject};
use fandhe_silicon_core::feature_key::{
    FEATURE_EXPERIMENTAL_MESH_SHADER, FEATURE_SHADER_FLOAT32_ATOMIC,
};
use fandhe_silicon_core::memory::BufferUsage;
use std::sync::mpsc;
use std::time::Duration;

/// 試験 1 本の上限。
const LIMIT: Duration = Duration::from_secs(30);

/// 各スレッドが `out[0]` に 1 を足す。`out` の先頭の 1 語だけを使う。
const COUNT: &str = "extern \"C\" __global__ void count(unsigned int* out) { atomicAdd(out, 1u); }";

/// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
fn within(body: impl FnOnce() + Send + 'static) {
    let (sender, receiver) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        body();
        let _ = sender.send(());
    });
    match receiver.recv_timeout(LIMIT) {
        Ok(()) => {
            let _ = handle.join();
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
            panic!("the test body ended without reporting");
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("the test body did not end within {LIMIT:?}")
        }
    }
}

fn compute_only() -> DeviceRequest {
    DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

fn new_device() -> Box<dyn Device> {
    match CudaDeviceProvider::request(compute_only()) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

/// 公開の `Device` を借り、能力の問合せをそのまま渡す。デコレータの内側に置く。
struct Real<'a>(&'a dyn Device);

impl Capabilities for Real<'_> {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        self.0.query(feature)
    }

    fn limits(&self) -> &LimitsTable {
        self.0.limits()
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        self.0.query_ex(feature)
    }
}

/// 公開の `Device` の `compute()` から `COUNT` のパイプラインを作り、`PipelineHandle` を外して返す。
fn create_count(device: &dyn Device) -> Result<(), ContractError> {
    device
        .compute()
        .expect("a CUDA device must expose compute()")
        .create_pipeline(
            PipelineInput::CudaC {
                source: COUNT,
                entry_point: "count",
            },
            &BindingLayout::new(vec![0]),
        )
        .map(|_| ())
}

/// `FEATURE_SHADER_FLOAT32_ATOMIC` を `S2` で要る筋書き。
fn atomic_shortage() -> FaultScript {
    FaultScript::CapabilityInsufficient {
        feature: FEATURE_SHADER_FLOAT32_ATOMIC,
        required: CapabilityLevel::S2,
    }
}

/// `atomic_shortage` の筋書きの下で `create_pipeline_with` が返す値。
fn atomic_shortage_error() -> ContractError {
    ContractError::CapabilityInsufficient {
        feature: FEATURE_SHADER_FLOAT32_ATOMIC,
        required: CapabilityLevel::S2,
        actual: CapabilityLevel::Unsupported,
    }
}

// DEV-215
/// device の問合せが `S2` を返す機能を `Unsupported` に差し替えると、その機能の問合せは `Unsupported` になり、
/// mesh shader の機能の `query_ex` は device の値のまま。
/// パイプラインの生成は内側を呼ばずに機能・要る段階・実際の段階を載せた `CapabilityInsufficient` を返す。
/// 差し込みの無い筋書きでは、同じ device の同じ生成が通る。
#[test]
#[ignore = "requires a CUDA device"]
fn a_simulated_shortage_names_the_feature_and_both_levels_over_the_cuda_device() {
    within(|| {
        let device = new_device();
        let device = device.as_ref();
        assert_eq!(
            device.query(FEATURE_SHADER_FLOAT32_ATOMIC),
            CapabilityLevel::S2
        );
        let mesh = device.query_ex(FEATURE_EXPERIMENTAL_MESH_SHADER);

        let pass = FaultInjecting::new(Real(device), FaultScript::PassThrough);
        assert_eq!(pass.create_pipeline_with(|d| create_count(d.0)), Ok(()));

        let shortage = FaultInjecting::new(Real(device), atomic_shortage());
        assert_eq!(
            shortage.query(FEATURE_SHADER_FLOAT32_ATOMIC),
            CapabilityLevel::Unsupported
        );
        assert_eq!(
            shortage.query_ex(FEATURE_SHADER_FLOAT32_ATOMIC),
            CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: Some(UnsupportedReason::NotSupported),
                evidence: None,
            }
        );
        assert_eq!(shortage.query_ex(FEATURE_EXPERIMENTAL_MESH_SHADER), mesh);
        assert_eq!(
            device.query(FEATURE_SHADER_FLOAT32_ATOMIC),
            CapabilityLevel::S2
        );

        let mut inner_calls = 0;
        let created = shortage.create_pipeline_with(|d| {
            inner_calls += 1;
            create_count(d.0)
        });
        assert_eq!(created, Err(atomic_shortage_error()));
        assert_eq!(inner_calls, 0);
        assert_eq!(
            atomic_shortage_error().failure_class(),
            FailureClass::Capability
        );

        assert_eq!(pass.create_pipeline_with(|d| create_count(d.0)), Ok(()));
    });
}

// DEV-215
/// 同じ device の上の能力不足の模擬・実行時エラー・完了不明・ロストの値と、初期化の失敗の差し込みの値が、
/// 戻り値の値と種類だけで互いに区別できる。
#[test]
#[ignore = "requires a CUDA device"]
fn the_simulated_shortage_is_distinct_from_the_other_failures_over_the_cuda_device() {
    within(|| {
        let device = new_device();
        let device = device.as_ref();
        let queue = device.queue();

        let capability = FaultInjecting::new(Real(device), atomic_shortage())
            .create_pipeline_with(|d| create_count(d.0));
        assert_eq!(capability, Err(atomic_shortage_error()));

        let initialization = match CudaDriverMissingProvider::request(compute_only()) {
            Ok(_) => panic!("the injected request must fail"),
            Err(e) => e,
        };
        assert_eq!(initialization.failure_class(), FailureClass::Initialization);

        let token = match queue.submit(queue.create_encoder(), &[]) {
            Ok(t) => t,
            Err(e) => panic!("submit failed: {e:?}"),
        };
        let runtime = FaultInjecting::new(
            queue,
            FaultScript::RuntimeError {
                backend: BackendKind::Cuda,
                detail: "injected".into(),
            },
        )
        .poll_ex_with(|q| q.poll_ex(token));
        assert_eq!(runtime, CompletionStateEx::RuntimeError);
        let unknown = FaultInjecting::new(queue, FaultScript::CompletionUnknown)
            .poll_ex_with(|q| q.poll_ex(token));
        assert_eq!(unknown, CompletionStateEx::Unknown);

        assert_eq!(
            inject(
                device.fault_hooks(),
                HookFault::DeviceLost(DeviceLostReason::Unknown)
            ),
            Ok(())
        );
        let lost = device
            .memory()
            .allocate(16, BufferUsage::STORAGE)
            .map(|_| ());
        assert_eq!(
            lost,
            Err(ContractError::DeviceLost {
                generation: device.generation()
            })
        );
        assert_eq!(queue.poll_ex(token), CompletionStateEx::Lost);

        let capability = atomic_shortage_error();
        let lost = ContractError::DeviceLost {
            generation: device.generation(),
        };
        for other in [&initialization, &lost] {
            assert_ne!(&capability, other);
        }
        assert_ne!(initialization, lost);

        let classes = [
            capability.failure_class(),
            initialization.failure_class(),
            lost.failure_class(),
            runtime
                .failure_class()
                .expect("a runtime error has a class"),
            unknown
                .failure_class()
                .expect("an unknown completion has a class"),
        ];
        assert_eq!(
            classes,
            [
                FailureClass::Capability,
                FailureClass::Initialization,
                FailureClass::DeviceLost,
                FailureClass::Runtime,
                FailureClass::CompletionUnknown,
            ]
        );
        for (i, a) in classes.iter().enumerate() {
            for b in &classes[i + 1..] {
                assert_ne!(a, b);
            }
        }
    });
}
