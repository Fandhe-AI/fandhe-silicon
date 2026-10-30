//! 決定表の公開面の試験（TASK-141）。他の crate から `DecisionInput::new` で入力を作れることと、
//! 経路の候補の名前を見る。全組み合わせの試験は `crates/core/src/decision/tests.rs`。

use fandhe_silicon_core::capability::CapabilityLevel;
use fandhe_silicon_core::decision::{
    DataScale, DecisionInput, NoRouteReason, Residency, RouteCandidate, RouteOutput, ScalarKind,
    decide,
};

#[test]
fn input_built_with_new_is_accepted_by_decide() {
    let d = decide(DecisionInput::new(
        CapabilityLevel::S6,
        DataScale::Large,
        Residency::Unified,
        ScalarKind::F16,
    ));
    assert_eq!(d.output, RouteOutput::Route(RouteCandidate::GpuMatrixUnit));
    let d = decide(DecisionInput::new(
        CapabilityLevel::S0,
        DataScale::Large,
        Residency::Unified,
        ScalarKind::F16,
    ));
    assert_eq!(d.output, RouteOutput::NoRoute(NoRouteReason::Unknown));
}

/// DEV-119。経路の候補の名前に、チップ・製品・カーネルの固有の語が入らない。
#[test]
fn route_candidate_names_carry_no_product_specific_words() {
    let banned = [
        "tensor",
        "simdgroup",
        "nvidia",
        "apple",
        "amd",
        "intel",
        "metal",
        "vulkan",
        "cuda",
        "kernel",
        "core",
    ];
    for c in [RouteCandidate::GpuGeneric, RouteCandidate::GpuMatrixUnit] {
        let name = format!("{c:?}").to_lowercase();
        for w in banned {
            assert!(!name.contains(w), "{name} contains {w}");
        }
    }
}
