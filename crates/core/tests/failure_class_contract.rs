//! 失敗の 4 種類と完了不明を戻り値だけで区別する共通の契約テスト（ERR-20・ERR-21・ERR-58。TASK-272 のポインタ）。
//!
//! 値は、スタブ `FaultStub` とデコレータ `FaultInjecting` の差し込み（TASK-271 のポインタ）、
//! device に結び付かない合成の関数、`ValidationIntake` の受け口から作る。実際の故障は起こさない。
//! 種類は戻り値の `failure_class` と、`_` を持つ `match`（別 crate の利用側と同じ形）の両方で得て、
//! 2 つが一致することも見る。
//!
//! - ERR-20: 初期化失敗・実行時エラーとロスト・完了不明・能力不足の 4 組が互いに異なる値になる
//! - ERR-58: 4 組の試験が通さなかった値（`InvalidCombination` など）も 4 組と区別できる
//! - ERR-21: 入口が 2 つの型の全部の値で種類を返し、両方の型で同じ組が同じ種類になる

use std::sync::{Arc, Mutex};

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId, LimitsTable};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, DeviceLostReason, FailureClass,
    IdentityMismatchReason,
};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::fault::stub::FaultStub;
use fandhe_silicon_core::fault::{HookFault, inject};
use fandhe_silicon_core::validation::ValidationIntake;

const FEATURE: FeatureId = FeatureId("feature:contract");

/// 内側の偽物。差し込みが内側を呼ばない筋書きでは使われない。
struct Inner;

fn wrap(script: FaultScript) -> FaultInjecting<Inner> {
    FaultInjecting::new(Inner, script)
}

/// 戻り値から得られる観測。値の型の違いも含めて比べる。
#[derive(Debug, Clone, PartialEq)]
enum Observed {
    Err(ContractError),
    Ex(CompletionStateEx),
    Closed(CompletionState),
}

impl Observed {
    /// 入口 `failure_class` から得た種類。入口を持たない `Closed` と、失敗でない状態は `None`。
    fn by_entry(&self) -> Option<FailureClass> {
        match self {
            Self::Err(e) => Some(e.failure_class()),
            Self::Ex(s) => s.failure_class(),
            Self::Closed(_) => None,
        }
    }
}

/// 別の crate の利用側が書く `match` による判別（入口を使わない経路）。
fn class_by_match(e: &ContractError) -> Option<FailureClass> {
    match e {
        ContractError::InvalidCombination(_)
        | ContractError::DeviceUnavailable(_)
        | ContractError::ArtifactRejected { .. } => Some(FailureClass::Initialization),
        ContractError::InvalidArgument(_)
        | ContractError::DeviceMismatch
        | ContractError::AllocationFailed { .. }
        | ContractError::ValidationFailed(_)
        | ContractError::BackendFailure { .. }
        | ContractError::DeviceIdentityMismatch { .. }
        | ContractError::CapacityExceeded { .. } => Some(FailureClass::Runtime),
        ContractError::DeviceLost { .. } => Some(FailureClass::DeviceLost),
        ContractError::Timeout => Some(FailureClass::CompletionUnknown),
        ContractError::CapabilityInsufficient { .. }
        | ContractError::Unsupported { .. }
        | ContractError::UnsupportedExtensionDefault { .. } => Some(FailureClass::Capability),
        _ => None,
    }
}

fn class_by_match_ex(s: &CompletionStateEx) -> Option<FailureClass> {
    match s {
        CompletionStateEx::RuntimeError => Some(FailureClass::Runtime),
        CompletionStateEx::Lost => Some(FailureClass::DeviceLost),
        CompletionStateEx::Unknown => Some(FailureClass::CompletionUnknown),
        _ => None,
    }
}

// ---- 値の作り方（ERR-20 の 4 組の出どころ） ----

/// device に結び付かない合成の関数が返す初期化失敗。
fn synthetic_init_failure() -> Result<(), ContractError> {
    Err(ContractError::DeviceUnavailable(
        "no adapter available".into(),
    ))
}

/// 実行時エラー: 筋書き `RuntimeError` の `poll_ex_with`。
fn runtime_error_ex() -> CompletionStateEx {
    wrap(FaultScript::RuntimeError {
        backend: BackendKind::Vulkan,
        detail: "synthetic".into(),
    })
    .poll_ex_with(|_| CompletionStateEx::Complete)
}

/// ロスト（拡張）: スタブに模擬のロストを入れ、スタブを内側にしたデコレータが内側の状態を返す。
fn lost_ex() -> CompletionStateEx {
    let stub = FaultStub::new();
    inject(
        Some(&stub),
        HookFault::DeviceLost(DeviceLostReason::Unrecoverable("synthetic".into())),
    )
    .unwrap();
    FaultInjecting::new(stub, FaultScript::PassThrough).poll_ex_with(|s| {
        if s.is_lost() {
            CompletionStateEx::Lost
        } else {
            CompletionStateEx::Complete
        }
    })
}

/// ロスト（閉じた状態）: 筋書き `RuntimeError` の `poll_with`。
fn lost_closed() -> CompletionState {
    wrap(FaultScript::RuntimeError {
        backend: BackendKind::Cuda,
        detail: "synthetic".into(),
    })
    .poll_with(|_| CompletionState::Complete)
}

/// 完了不明: 筋書き `CompletionUnknown` の `wait_blocking_with`（内側を呼ばず `Timeout`）。
fn timeout_err() -> ContractError {
    wrap(FaultScript::CompletionUnknown)
        .wait_blocking_with(|_| Ok(()))
        .unwrap_err()
}

/// 完了不明（拡張）: 筋書き `CompletionUnknown` の `poll_ex_with`。
fn unknown_ex() -> CompletionStateEx {
    wrap(FaultScript::CompletionUnknown).poll_ex_with(|_| CompletionStateEx::Complete)
}

/// 能力不足: 筋書き `CapabilityInsufficient` の `create_pipeline_with`。
fn capability_err() -> ContractError {
    wrap(FaultScript::CapabilityInsufficient {
        feature: FEATURE,
        required: CapabilityLevel::S2,
    })
    .create_pipeline_with(|_| Ok(()))
    .unwrap_err()
}

// ---- ERR-58 の値の出どころ ----

/// 検証レイヤーの検出: スタブの受け口へ報告を差し込み、受け口に届いた値を返す。
fn validation_failed() -> ContractError {
    let stub = FaultStub::new();
    let got = Arc::new(Mutex::new(Vec::new()));
    let got2 = Arc::clone(&got);
    stub.set_validation_callback(Box::new(move |e| got2.lock().unwrap().push(e)));
    inject(Some(&stub), HookFault::ValidationReport("synthetic".into())).unwrap();
    let mut v = got.lock().unwrap();
    assert_eq!(v.len(), 1);
    v.remove(0)
}

/// 引数の誤り: 型の違う上限の置き換えを読むと内側を呼ばず `InvalidArgument`。
fn invalid_argument() -> ContractError {
    FaultInjecting::new(Inner, FaultScript::PassThrough)
        .with_limits_override(7_u8)
        .limits_with::<LimitsTable, _>(|_| unreachable!("inner must not be called"))
        .unwrap_err()
}

/// ドライバ・下位の API の失敗: 筋書き `RuntimeError` の `wait_blocking_with`。
fn backend_failure() -> ContractError {
    wrap(FaultScript::RuntimeError {
        backend: BackendKind::Metal,
        detail: "synthetic".into(),
    })
    .wait_blocking_with(|_| Ok(()))
    .unwrap_err()
}

/// 呼び出し時点の `DeviceLost`: ロスト済みのスタブを内側にした投入が、内側の状態を見て返す。
fn device_lost_at_call() -> ContractError {
    let stub = FaultStub::new();
    inject(
        Some(&stub),
        HookFault::DeviceLost(DeviceLostReason::Destroyed),
    )
    .unwrap();
    FaultInjecting::new(stub, FaultScript::PassThrough)
        .submit_with(|s| {
            if s.is_lost() {
                Err(ContractError::DeviceLost { generation: 1 })
            } else {
                Ok(())
            }
        })
        .unwrap_err()
}

fn invalid_combination() -> ContractError {
    ContractError::InvalidCombination("synthetic".into())
}

fn artifact_rejected() -> ContractError {
    ContractError::ArtifactRejected {
        kind: "synthetic",
        reason: "synthetic".into(),
    }
}

// ---- ERR-20 ----

/// ERR-20 の 4 組の代表（実行時エラーとロストの組は 2 値）と期待の種類。
fn err20_observations() -> Vec<(&'static str, Observed, FailureClass)> {
    vec![
        (
            "init",
            Observed::Err(synthetic_init_failure().unwrap_err()),
            FailureClass::Initialization,
        ),
        (
            "runtime(ex)",
            Observed::Ex(runtime_error_ex()),
            FailureClass::Runtime,
        ),
        (
            "lost(ex)",
            Observed::Ex(lost_ex()),
            FailureClass::DeviceLost,
        ),
        (
            "timeout",
            Observed::Err(timeout_err()),
            FailureClass::CompletionUnknown,
        ),
        (
            "capability",
            Observed::Err(capability_err()),
            FailureClass::Capability,
        ),
    ]
}

#[test]
fn err20_sources_return_the_expected_values() {
    assert_eq!(
        synthetic_init_failure(),
        Err(ContractError::DeviceUnavailable(
            "no adapter available".into()
        ))
    );
    assert_eq!(runtime_error_ex(), CompletionStateEx::RuntimeError);
    assert_eq!(lost_ex(), CompletionStateEx::Lost);
    assert_eq!(lost_closed(), CompletionState::Lost);
    assert_eq!(timeout_err(), ContractError::Timeout);
    assert_eq!(unknown_ex(), CompletionStateEx::Unknown);
    assert!(matches!(
        capability_err(),
        ContractError::CapabilityInsufficient { .. }
    ));
}

#[test]
fn err20_four_groups_are_pairwise_distinct_values() {
    let mut all = err20_observations();
    all.push((
        "lost(closed)",
        Observed::Closed(lost_closed()),
        FailureClass::DeviceLost,
    ));
    all.push((
        "unknown(ex)",
        Observed::Ex(unknown_ex()),
        FailureClass::CompletionUnknown,
    ));
    for (i, (na, a, _)) in all.iter().enumerate() {
        for (nb, b, _) in &all[i + 1..] {
            assert_ne!(a, b, "{na} and {nb} must be distinct values");
        }
    }
}

#[test]
fn err20_groups_get_distinct_classes() {
    let obs = err20_observations();
    for (name, o, expected) in &obs {
        assert_eq!(o.by_entry(), Some(*expected), "{name}");
    }
    for (i, (na, a, _)) in obs.iter().enumerate() {
        for (nb, b, _) in &obs[i + 1..] {
            assert_ne!(a.by_entry(), b.by_entry(), "{na} and {nb}");
        }
    }
}

#[test]
fn err20_unknown_differs_from_every_other_value() {
    let unknown = Observed::Ex(unknown_ex());
    assert_eq!(unknown.by_entry(), Some(FailureClass::CompletionUnknown));
    for (name, o, _) in err20_observations() {
        assert_ne!(unknown, o, "Unknown vs {name}");
    }
    assert_ne!(unknown, Observed::Closed(lost_closed()));
    assert_ne!(CompletionStateEx::Unknown, CompletionStateEx::RuntimeError);
    assert_ne!(CompletionStateEx::Unknown, CompletionStateEx::Lost);
}

#[test]
fn err20_timeout_is_completion_unknown_not_a_confirmed_failure() {
    assert_eq!(
        timeout_err().failure_class(),
        FailureClass::CompletionUnknown
    );
    assert_ne!(timeout_err().failure_class(), FailureClass::Runtime);
}

// ---- ERR-58 ----

/// ERR-58 の値と期待の種類。
fn err58_values() -> Vec<(&'static str, ContractError, FailureClass)> {
    vec![
        (
            "InvalidCombination",
            invalid_combination(),
            FailureClass::Initialization,
        ),
        (
            "ArtifactRejected",
            artifact_rejected(),
            FailureClass::Initialization,
        ),
        (
            "ValidationFailed",
            validation_failed(),
            FailureClass::Runtime,
        ),
        ("InvalidArgument", invalid_argument(), FailureClass::Runtime),
        ("BackendFailure", backend_failure(), FailureClass::Runtime),
        (
            "DeviceLost(call)",
            device_lost_at_call(),
            FailureClass::DeviceLost,
        ),
        ("Timeout", timeout_err(), FailureClass::CompletionUnknown),
    ]
}

#[test]
fn err58_sources_return_the_expected_variants() {
    assert!(matches!(
        validation_failed(),
        ContractError::ValidationFailed(_)
    ));
    assert!(matches!(
        invalid_argument(),
        ContractError::InvalidArgument(_)
    ));
    assert!(matches!(
        backend_failure(),
        ContractError::BackendFailure {
            op: "wait_blocking",
            ..
        }
    ));
    assert_eq!(
        device_lost_at_call(),
        ContractError::DeviceLost { generation: 1 }
    );
}

#[test]
fn err58_values_get_the_expected_class_by_entry_and_by_match() {
    for (name, e, expected) in err58_values() {
        assert_eq!(e.failure_class(), expected, "{name} (entry)");
        assert_eq!(class_by_match(&e), Some(expected), "{name} (match)");
    }
}

#[test]
fn err58_values_are_distinct_from_the_err20_groups() {
    let err20 = err20_observations();
    for (name, e, expected) in err58_values() {
        let observed = Observed::Err(e);
        for (n20, o20, c20) in &err20 {
            // 同じ組の代表とは同じ種類、ほかの組とは別の種類になる。
            assert_eq!(
                observed.by_entry() == o20.by_entry(),
                expected == *c20,
                "{name} vs {n20}"
            );
        }
    }
}

#[test]
fn err58_call_time_device_lost_is_not_a_runtime_error() {
    let lost = device_lost_at_call();
    assert_eq!(lost.failure_class(), FailureClass::DeviceLost);
    assert_ne!(lost.failure_class(), FailureClass::Runtime);
}

// ---- ERR-21 ----

fn all_contract_errors() -> Vec<ContractError> {
    vec![
        invalid_combination(),
        ContractError::DeviceUnavailable("a".into()),
        artifact_rejected(),
        invalid_argument(),
        ContractError::DeviceMismatch,
        ContractError::AllocationFailed { requested_bytes: 8 },
        validation_failed(),
        backend_failure(),
        ContractError::DeviceIdentityMismatch {
            reason: IdentityMismatchReason::Undeterminable,
        },
        ContractError::CapacityExceeded {
            op: "o",
            backend: BackendKind::Cuda,
            capacity: 4,
        },
        device_lost_at_call(),
        timeout_err(),
        capability_err(),
        ContractError::Unsupported {
            op: "o",
            backend: BackendKind::Metal,
        },
        ContractError::UnsupportedExtensionDefault { op: "o" },
    ]
}

const ALL_STATES: [CompletionStateEx; 5] = [
    CompletionStateEx::Pending,
    CompletionStateEx::Complete,
    CompletionStateEx::RuntimeError,
    CompletionStateEx::Lost,
    CompletionStateEx::Unknown,
];

#[test]
fn err21_entry_returns_a_class_for_every_contract_error_and_agrees_with_match() {
    let all = all_contract_errors();
    assert_eq!(all.len(), 15);
    for e in &all {
        assert_eq!(Some(e.failure_class()), class_by_match(e), "{e:?}");
    }
}

#[test]
fn err21_entry_on_completion_state_ex_agrees_with_match() {
    for s in ALL_STATES {
        assert_eq!(s.failure_class(), class_by_match_ex(&s), "{s:?}");
    }
    assert_eq!(CompletionStateEx::Pending.failure_class(), None);
    assert_eq!(CompletionStateEx::Complete.failure_class(), None);
}

#[test]
fn err21_both_types_give_the_same_class_for_the_same_group() {
    let pairs = [
        (
            backend_failure().failure_class(),
            CompletionStateEx::RuntimeError,
        ),
        (
            device_lost_at_call().failure_class(),
            CompletionStateEx::Lost,
        ),
        (timeout_err().failure_class(), CompletionStateEx::Unknown),
    ];
    for (from_error, state) in pairs {
        assert_eq!(Some(from_error), state.failure_class(), "{state:?}");
    }
}

#[test]
fn err21_all_five_classes_are_returned() {
    let mut seen: Vec<FailureClass> = Vec::new();
    let from_errors = all_contract_errors().into_iter().map(|e| e.failure_class());
    let from_states = ALL_STATES.into_iter().filter_map(|s| s.failure_class());
    for c in from_errors.chain(from_states) {
        if !seen.contains(&c) {
            seen.push(c);
        }
    }
    assert_eq!(seen.len(), 5);
}
