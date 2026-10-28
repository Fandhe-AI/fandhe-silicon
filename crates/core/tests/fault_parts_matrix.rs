//! 故障の差し込みの部品の横断の確認（spec TASK-271 の 7/8）。デコレータの全ての筋書きと全ての差し込みの組で、
//! 内側を呼んだ回数と返す値がコードの振る舞いどおりであること、合成の失敗どうしが値で区別できることを確かめる。
//! 差し込むのは模擬であって、実際の故障ではない。

use std::cell::{Cell, RefCell};
use std::mem::discriminant;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, DeviceLostReason, UnsupportedReason,
};
use fandhe_silicon_core::fault::FaultHooks;
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");
const B: FeatureId = FeatureId("feature:B");
const BYTES: u64 = 4096;

/// 内側の呼び出し回数。差し込みごとに数える。
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
struct Calls {
    submit: u32,
    allocate: u32,
    wait: u32,
    poll_ex: u32,
    poll: u32,
    pipeline: u32,
    query: u32,
    query_ex: u32,
}

/// 呼び出しを数え、合成の値と必ず違う値を返す偽の内側。
#[derive(Default)]
struct Fake {
    query_calls: Cell<u32>,
    query_ex_calls: Cell<u32>,
    hook_log: RefCell<Vec<String>>,
}

/// `Capabilities::limits` の試験用の表。値は試験用の小さな数で、意味は持たない。
static TEST_LIMITS: fandhe_silicon_core::capability::LimitsTable =
    fandhe_silicon_core::capability::LimitsTable {
        max_buffer_bytes: 1,
        max_dispatch_dims: [1, 1, 1],
        max_texture_size: 1,
    };

impl Capabilities for Fake {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, _f: FeatureId) -> CapabilityLevel {
        self.query_calls.set(self.query_calls.get() + 1);
        CapabilityLevel::S2
    }
    fn query_ex(&self, _f: FeatureId) -> CapabilityQuery {
        self.query_ex_calls.set(self.query_ex_calls.get() + 1);
        inner_query_ex()
    }
}

impl FaultHooks for Fake {
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        self.hook_log.borrow_mut().push(format!("lost:{reason:?}"));
        Ok(())
    }
    fn arm_gate(&self) -> Result<(), ContractError> {
        self.hook_log.borrow_mut().push("arm".into());
        Err(ContractError::InvalidArgument("inner-arm".into()))
    }
    fn open_gate(&self) -> Result<(), ContractError> {
        self.hook_log.borrow_mut().push("open".into());
        Ok(())
    }
    fn simulate_runtime_error(&self, detail: String) -> Result<(), ContractError> {
        self.hook_log.borrow_mut().push(format!("rt:{detail}"));
        Ok(())
    }
}

/// 全差し込みを 1 回ずつ呼んだ結果。
#[derive(Debug, PartialEq, Eq)]
struct Observed {
    submit: Result<u32, ContractError>,
    allocate: Result<u64, ContractError>,
    wait: Result<(), ContractError>,
    poll_ex: CompletionStateEx,
    poll: CompletionState,
    pipeline: Result<u32, ContractError>,
    query_a: CapabilityLevel,
    query_b: CapabilityLevel,
    query_ex_a: CapabilityQuery,
    query_ex_b: CapabilityQuery,
    calls: Calls,
    submit_count: u64,
}

fn scripts() -> Vec<(&'static str, FaultScript)> {
    vec![
        ("PassThrough", FaultScript::PassThrough),
        ("CompletionUnknown", FaultScript::CompletionUnknown),
        (
            "CapabilityInsufficient",
            FaultScript::CapabilityInsufficient {
                feature: A,
                required: CapabilityLevel::S3,
            },
        ),
        ("CapabilityQueryFails", FaultScript::CapabilityQueryFails),
        ("AllocationFails", FaultScript::AllocationFails),
        (
            "RuntimeError",
            FaultScript::RuntimeError {
                backend: BackendKind::Vulkan,
                detail: "boom".into(),
            },
        ),
    ]
}

fn observe(script: FaultScript) -> Observed {
    let d = FaultInjecting::new(Fake::default(), script);
    let c = Cell::new(Calls::default());
    let bump = |f: fn(&mut Calls)| {
        let mut v = c.get();
        f(&mut v);
        c.set(v);
    };
    let submit = d.submit_with(|_| {
        bump(|c| c.submit += 1);
        Ok(7u32)
    });
    let allocate = d.allocate_with(BYTES, |_, n| {
        bump(|c| c.allocate += 1);
        Ok(n)
    });
    let wait = d.wait_blocking_with(|_| {
        bump(|c| c.wait += 1);
        Ok(())
    });
    let poll_ex = d.poll_ex_with(|_| {
        bump(|c| c.poll_ex += 1);
        CompletionStateEx::Complete
    });
    let poll = d.poll_with(|_| {
        bump(|c| c.poll += 1);
        CompletionState::Complete
    });
    let pipeline = d.create_pipeline_with(|_| {
        bump(|c| c.pipeline += 1);
        Ok(9u32)
    });
    let query_a = d.query(A);
    let query_b = d.query(B);
    let query_ex_a = d.query_ex(A);
    let query_ex_b = d.query_ex(B);
    let mut calls = c.get();
    calls.query = d.inner().query_calls.get();
    calls.query_ex = d.inner().query_ex_calls.get();
    Observed {
        submit,
        allocate,
        wait,
        poll_ex,
        poll,
        pipeline,
        query_a,
        query_b,
        query_ex_a,
        query_ex_b,
        calls,
        submit_count: d.submit_call_count(),
    }
}

fn inner_query_ex() -> CapabilityQuery {
    CapabilityQuery {
        level: CapabilityLevel::S2,
        unsupported_reason: None,
        evidence: Some(CapabilityEvidence::RuntimeApi),
    }
}

/// コードに書かれた振る舞いの写し。通す差し込みは内側を 1 回呼んで内側の値、合成する差し込みは内側を 0 回。
fn expected(script: &FaultScript) -> Observed {
    let mut o = Observed {
        submit: Ok(7),
        allocate: Ok(BYTES),
        wait: Ok(()),
        poll_ex: CompletionStateEx::Complete,
        poll: CompletionState::Complete,
        pipeline: Ok(9),
        query_a: CapabilityLevel::S2,
        query_b: CapabilityLevel::S2,
        query_ex_a: inner_query_ex(),
        query_ex_b: inner_query_ex(),
        calls: Calls {
            submit: 1,
            allocate: 1,
            wait: 1,
            poll_ex: 1,
            poll: 1,
            pipeline: 1,
            query: 2,
            query_ex: 2,
        },
        submit_count: 1,
    };
    match script {
        FaultScript::PassThrough => {}
        FaultScript::CompletionUnknown => {
            o.wait = Err(ContractError::Timeout);
            o.poll_ex = CompletionStateEx::Unknown;
            o.calls.wait = 0;
            o.calls.poll_ex = 0;
        }
        FaultScript::CapabilityInsufficient { feature, required } => {
            assert_eq!(*feature, A);
            o.pipeline = Err(ContractError::CapabilityInsufficient {
                feature: *feature,
                required: *required,
                actual: CapabilityLevel::Unsupported,
            });
            o.calls.pipeline = 0;
            o.query_a = CapabilityLevel::Unsupported;
            o.query_ex_a = CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: Some(UnsupportedReason::NotSupported),
                evidence: None,
            };
            o.calls.query = 1;
            o.calls.query_ex = 1;
        }
        FaultScript::CapabilityQueryFails => {
            let q = CapabilityQuery {
                level: CapabilityLevel::S0,
                unsupported_reason: None,
                evidence: None,
            };
            o.query_a = CapabilityLevel::S0;
            o.query_b = CapabilityLevel::S0;
            o.query_ex_a = q;
            o.query_ex_b = q;
            o.calls.query = 0;
            o.calls.query_ex = 0;
        }
        FaultScript::AllocationFails => {
            o.allocate = Err(ContractError::AllocationFailed {
                requested_bytes: BYTES,
            });
            o.calls.allocate = 0;
        }
        FaultScript::RuntimeError { backend, detail } => {
            o.wait = Err(ContractError::BackendFailure {
                op: "wait_blocking",
                backend: *backend,
                detail: detail.clone(),
            });
            o.poll_ex = CompletionStateEx::RuntimeError;
            o.poll = CompletionState::Lost;
            o.calls.wait = 0;
            o.calls.poll_ex = 0;
            o.calls.poll = 0;
        }
        _ => panic!("unhandled script"),
    }
    o
}

#[test]
fn every_script_times_every_interception_matches_expected() {
    for (name, script) in scripts() {
        let want = expected(&script);
        let got = observe(script);
        assert_eq!(got, want, "script {name}");
    }
}

#[test]
fn submit_is_counted_once_and_other_interceptions_do_not_count() {
    for (name, script) in scripts() {
        let got = observe(script);
        assert_eq!(got.submit_count, 1, "script {name}");
        assert_eq!(got.calls.submit, 1, "script {name}");
    }
}

#[test]
fn inner_err_is_passed_through_when_not_intercepted() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough);
    let r = d.wait_blocking_with(|_| Err(ContractError::InvalidArgument("inner".into())));
    assert_eq!(r, Err(ContractError::InvalidArgument("inner".into())));
    let r: Result<u32, _> = d.submit_with(|_| Err(ContractError::InvalidArgument("inner".into())));
    assert_eq!(r, Err(ContractError::InvalidArgument("inner".into())));
    assert_eq!(d.submit_call_count(), 1);
}

#[test]
fn limits_override_is_independent_of_script() {
    #[derive(Debug, Clone, PartialEq)]
    struct Lim(u32);
    for (name, script) in scripts() {
        let inner_calls = Cell::new(0u32);
        let call = |_: &Fake| {
            inner_calls.set(inner_calls.get() + 1);
            Lim(1)
        };
        // 置き換え無し: 内側を 1 回呼んで内側の値。
        let d = FaultInjecting::new(Fake::default(), script.clone());
        assert_eq!(d.limits_with::<Lim, _>(call), Ok(Lim(1)), "script {name}");
        assert_eq!(inner_calls.get(), 1, "script {name}");
        // 置き換え有り: 内側を呼ばず複製を返す。
        inner_calls.set(0);
        let d = FaultInjecting::new(Fake::default(), script.clone()).with_limits_override(Lim(99));
        assert_eq!(d.limits_with::<Lim, _>(call), Ok(Lim(99)), "script {name}");
        assert_eq!(d.limits_with::<Lim, _>(call), Ok(Lim(99)), "script {name}");
        assert_eq!(inner_calls.get(), 0, "script {name}");
        // 型違い: 内側を呼ばず InvalidArgument。
        let d = FaultInjecting::new(Fake::default(), script).with_limits_override(5u8);
        assert!(
            matches!(
                d.limits_with::<Lim, _>(call),
                Err(ContractError::InvalidArgument(_))
            ),
            "script {name}"
        );
        assert_eq!(inner_calls.get(), 0, "script {name}");
    }
}

#[test]
fn hooks_pass_through_to_inner_for_every_script() {
    for (name, script) in scripts() {
        let d = FaultInjecting::new(Fake::default(), script);
        assert_eq!(d.simulate_device_lost(DeviceLostReason::Destroyed), Ok(()));
        assert_eq!(
            d.arm_gate(),
            Err(ContractError::InvalidArgument("inner-arm".into())),
            "script {name}"
        );
        assert_eq!(d.open_gate(), Ok(()));
        assert_eq!(d.simulate_runtime_error("x".into()), Ok(()));
        assert_eq!(
            *d.inner().hook_log.borrow(),
            ["lost:Destroyed", "arm", "open", "rt:x"],
            "script {name}"
        );
    }
}

#[test]
fn synthesized_failures_are_distinguishable_by_value() {
    let errs = [
        ContractError::Timeout,
        ContractError::BackendFailure {
            op: "wait_blocking",
            backend: BackendKind::Metal,
            detail: "d".into(),
        },
        ContractError::AllocationFailed {
            requested_bytes: BYTES,
        },
        ContractError::CapabilityInsufficient {
            feature: A,
            required: CapabilityLevel::S2,
            actual: CapabilityLevel::Unsupported,
        },
        ContractError::InvalidArgument("limits override type mismatch".into()),
    ];
    for (i, x) in errs.iter().enumerate() {
        for (j, y) in errs.iter().enumerate() {
            if i != j {
                assert_ne!(x, y, "{i} vs {j}");
                assert_ne!(discriminant(x), discriminant(y), "{i} vs {j}");
            }
        }
    }
    let states = [
        CompletionStateEx::Unknown,
        CompletionStateEx::RuntimeError,
        CompletionStateEx::Complete,
        CompletionStateEx::Lost,
    ];
    for (i, x) in states.iter().enumerate() {
        for (j, y) in states.iter().enumerate() {
            assert_eq!(i == j, x == y, "{x:?} vs {y:?}");
        }
    }
    assert_ne!(CapabilityLevel::S0, CapabilityLevel::Unsupported);
    // B は能力不足の対象外の機能として、A と区別される。
    assert_ne!(A, B);
}
