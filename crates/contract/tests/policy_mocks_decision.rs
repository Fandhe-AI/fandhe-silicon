//! 決定表の出力を 2 つの利用コードの模擬へ流す、device に触らない試験（TASK-219 のポインタ）。
//!
//! 能力の段階の 8 つの値すべてを `policy_mocks::decision_for` に通し、fail-fast の模擬と縮退の模擬が
//! 同じ `Decision` の値から方針どおりに分かれることを、値そのもので照らす。背後実装の feature は要らない。

mod policy_mocks;

use std::sync::atomic::{AtomicU64, Ordering};

use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::decision::{
    Decision, DecisionReason, NoRouteReason, RouteCandidate, RouteOutput,
};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use policy_mocks::{
    DegradeEvent, Degrading, FailFastError, PrimarySlot, decision_for, request_or_fail,
    route_or_fail,
};

const FEATURE: FeatureId = FeatureId("feature:SYNTHETIC");

const fn no_route(kind: NoRouteReason, reason: DecisionReason) -> Decision {
    Decision {
        output: RouteOutput::NoRoute(kind),
        reason,
    }
}

const fn route(candidate: RouteCandidate, reason: DecisionReason) -> Decision {
    Decision {
        output: RouteOutput::Route(candidate),
        reason,
    }
}

/// 段階ごとの `decision_for` の戻り値（規模 `Large`・常駐位置 `Unified`・数値型 `F32`）。
const EXPECTED: [(CapabilityLevel, Decision); 8] = [
    (
        CapabilityLevel::S0,
        no_route(NoRouteReason::Unknown, DecisionReason::CapabilityS0),
    ),
    (
        CapabilityLevel::Unsupported,
        no_route(
            NoRouteReason::Unsupported,
            DecisionReason::CapabilityUnsupported,
        ),
    ),
    (
        CapabilityLevel::S1,
        no_route(
            NoRouteReason::NotVerified,
            DecisionReason::CapabilityBelowS5,
        ),
    ),
    (
        CapabilityLevel::S2,
        no_route(
            NoRouteReason::NotVerified,
            DecisionReason::CapabilityBelowS5,
        ),
    ),
    (
        CapabilityLevel::S3,
        no_route(
            NoRouteReason::NotVerified,
            DecisionReason::CapabilityBelowS5,
        ),
    ),
    (
        CapabilityLevel::S4,
        no_route(
            NoRouteReason::NotVerified,
            DecisionReason::CapabilityBelowS5,
        ),
    ),
    (
        CapabilityLevel::S5,
        route(
            RouteCandidate::GpuGeneric,
            DecisionReason::CapabilityS5ScaleLarge,
        ),
    ),
    (
        CapabilityLevel::S6,
        route(
            RouteCandidate::GpuGeneric,
            DecisionReason::CapabilityS6ScaleLargeOtherScalar,
        ),
    ),
];

// DEV-131
/// fail-fast の模擬は、経路の候補があれば `Ok(候補)`、無ければ受けた `Decision` を載せた `Err` を返す。
#[test]
fn fail_fast_routes_or_fails_for_every_level() {
    for (level, expected) in EXPECTED {
        let decision = decision_for(level);
        assert_eq!(decision, expected, "{level:?}");
        let got = route_or_fail(decision);
        match expected.output {
            RouteOutput::Route(candidate) => assert_eq!(got, Ok(candidate), "{level:?}"),
            _ => assert_eq!(got, Err(FailFastError::NoRoute(expected)), "{level:?}"),
        }
    }
    assert_eq!(
        route_or_fail(decision_for(CapabilityLevel::S5)),
        Ok(RouteCandidate::GpuGeneric)
    );
    assert_eq!(
        route_or_fail(decision_for(CapabilityLevel::S2)),
        Err(FailFastError::NoRoute(no_route(
            NoRouteReason::NotVerified,
            DecisionReason::CapabilityBelowS5
        )))
    );
}

// DEV-132
/// 縮退の模擬は、経路の候補があれば `Route(候補)`、無ければ `Unavailable` に倒し、理由つきのイベントを 1 つ残す。
#[test]
fn degrading_falls_back_with_a_reason_for_every_level() {
    let mut degrading = Degrading::default();
    let mut expected_events = Vec::new();
    for (level, expected) in EXPECTED {
        let slot = degrading.slot(FEATURE, decision_for(level));
        match expected.output {
            RouteOutput::Route(candidate) => {
                assert_eq!(slot, PrimarySlot::Route(candidate), "{level:?}")
            }
            _ => {
                assert_eq!(slot, PrimarySlot::Unavailable, "{level:?}");
                expected_events.push(DegradeEvent {
                    feature: FEATURE,
                    decision: expected,
                });
            }
        }
    }
    assert_eq!(expected_events.len(), 6);
    assert_eq!(degrading.events(), expected_events.as_slice());
}

// DEV-133
/// 同じ `Decision` の値を 2 つの模擬に渡すと、経路の候補は同じ値になり、候補の無い値は両方で候補なしになる。
#[test]
fn both_policies_take_the_same_decision_values() {
    let mut degrading = Degrading::default();
    for (level, _) in EXPECTED {
        let decision = decision_for(level);
        assert_eq!(decision_for(level), decision, "{level:?}");
        let fail_fast = route_or_fail(decision);
        let slot = degrading.slot(FEATURE, decision);
        match (fail_fast, slot) {
            (Ok(a), PrimarySlot::Route(b)) => assert_eq!(a, b, "{level:?}"),
            (Err(FailFastError::NoRoute(d)), PrimarySlot::Unavailable) => {
                assert_eq!(d, decision, "{level:?}");
                assert_eq!(
                    degrading.events().last(),
                    Some(&DegradeEvent {
                        feature: FEATURE,
                        decision
                    }),
                    "{level:?}"
                );
            }
            other => panic!("{level:?}: {other:?}"),
        }
    }
    assert_eq!(degrading.events().len(), 6);
}

// DEV-133
/// 決定表のソースは、利用コードの側の名前を含まない。
#[test]
fn the_decision_source_names_no_product() {
    const SOURCE: &str = include_str!("../../core/src/decision.rs");
    let lower = SOURCE.to_ascii_lowercase();
    for word in [
        "fandhe-ai",
        "fandhe_ai",
        "fandhe-db",
        "fandhe_db",
        "vector-db",
        "vector_db",
        "fandhe-3d",
        "fandhe_3d",
    ] {
        assert!(!lower.contains(word), "{word}");
    }
}

/// 呼ばれるたびに数を 1 つ足し、`DeviceUnavailable` を返す試験用の provider。
struct UnavailableProvider;

static UNAVAILABLE_CALLS: AtomicU64 = AtomicU64::new(0);

impl DeviceProvider for UnavailableProvider {
    fn request(_req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        UNAVAILABLE_CALLS.fetch_add(1, Ordering::SeqCst);
        Err(ContractError::DeviceUnavailable("synthetic".into()))
    }
}

// DEV-214
/// fail-fast の模擬は、名指した provider の `request` を 1 回だけ呼び、その失敗を値のまま返す。
#[test]
fn fail_fast_returns_a_request_failure_as_is() {
    let req = DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    };
    let got = match request_or_fail::<UnavailableProvider>(req) {
        Ok(_) => panic!("request must fail"),
        Err(e) => e,
    };
    assert_eq!(
        got,
        FailFastError::Request(ContractError::DeviceUnavailable("synthetic".into()))
    );
    assert_eq!(UNAVAILABLE_CALLS.load(Ordering::SeqCst), 1);
}
