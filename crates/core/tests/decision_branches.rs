//! 決定表 `decide` の分岐ごとの試験（TASK-143）。公開 API だけで、分岐ごとに出力と理由を直接断言する。
//!
//! 期待値は各試験の中に直接書く。全組み合わせの試験は `crates/core/src/decision/tests.rs`。
//! どの分岐も、常駐位置の 3 値（と、分岐が数値型・規模を問わない場合はその値）すべてで同じ判断になることを確かめる。

use fandhe_silicon_core::capability::CapabilityLevel;
use fandhe_silicon_core::decision::{
    DataScale, Decision, DecisionInput, DecisionReason, NoRouteReason, Residency, RouteCandidate,
    RouteOutput, ScalarKind, decide,
};

const RESIDENCIES: [Residency; 3] = [
    Residency::HostOnly,
    Residency::DeviceResident,
    Residency::Unified,
];
const SCALARS: [ScalarKind; 3] = [ScalarKind::F32, ScalarKind::F16, ScalarKind::I8];
const SCALES: [DataScale; 2] = [DataScale::Small, DataScale::Large];

/// 常駐位置の 3 値すべてで `decide` を呼び、どれも `expected` と等しいことを確かめる。
fn assert_for_all_residencies(
    level: CapabilityLevel,
    scale: DataScale,
    scalar: ScalarKind,
    expected: Decision,
) {
    for residency in RESIDENCIES {
        let got = decide(DecisionInput::new(level, scale, residency, scalar));
        assert_eq!(
            got, expected,
            "level={level:?} scale={scale:?} residency={residency:?} scalar={scalar:?}"
        );
    }
}

/// DEV-123。`S1`〜`S4` は規模・数値型・常駐位置を問わず `NoRoute(NotVerified)` で、理由は `S5` に届かないこと。
#[test]
fn s1_to_s4_is_no_route_not_verified_because_below_s5() {
    let expected = Decision {
        output: RouteOutput::NoRoute(NoRouteReason::NotVerified),
        reason: DecisionReason::CapabilityBelowS5,
    };
    for level in [
        CapabilityLevel::S1,
        CapabilityLevel::S2,
        CapabilityLevel::S3,
        CapabilityLevel::S4,
    ] {
        for scale in SCALES {
            for scalar in SCALARS {
                assert_for_all_residencies(level, scale, scalar, expected);
            }
        }
    }
}

/// DEV-124。`S5` で規模が `Small` なら、数値型・常駐位置を問わず `NoRoute(NotVerified)`、理由は規模が小さいこと。
#[test]
fn s5_small_is_no_route_not_verified_because_scale_small() {
    let expected = Decision {
        output: RouteOutput::NoRoute(NoRouteReason::NotVerified),
        reason: DecisionReason::ScaleSmall,
    };
    for scalar in SCALARS {
        assert_for_all_residencies(CapabilityLevel::S5, DataScale::Small, scalar, expected);
    }
}

/// DEV-125。`S5` で規模が `Large` なら、数値型・常駐位置を問わず `Route(GpuGeneric)`、理由は `S5` かつ `Large`。
#[test]
fn s5_large_routes_to_gpu_generic_for_every_scalar() {
    let expected = Decision {
        output: RouteOutput::Route(RouteCandidate::GpuGeneric),
        reason: DecisionReason::CapabilityS5ScaleLarge,
    };
    for scalar in SCALARS {
        assert_for_all_residencies(CapabilityLevel::S5, DataScale::Large, scalar, expected);
    }
}

/// DEV-126。`S6` で規模が `Small` なら、数値型・常駐位置を問わず `NoRoute(NotVerified)`、理由は規模が小さいこと。
#[test]
fn s6_small_is_no_route_not_verified_because_scale_small() {
    let expected = Decision {
        output: RouteOutput::NoRoute(NoRouteReason::NotVerified),
        reason: DecisionReason::ScaleSmall,
    };
    for scalar in SCALARS {
        assert_for_all_residencies(CapabilityLevel::S6, DataScale::Small, scalar, expected);
    }
}

/// DEV-127。`S6` かつ `Large` かつ `F16` は、常駐位置を問わず `Route(GpuMatrixUnit)`。
#[test]
fn s6_large_f16_routes_to_gpu_matrix_unit() {
    let expected = Decision {
        output: RouteOutput::Route(RouteCandidate::GpuMatrixUnit),
        reason: DecisionReason::CapabilityS6ScaleLargeF16,
    };
    assert_for_all_residencies(
        CapabilityLevel::S6,
        DataScale::Large,
        ScalarKind::F16,
        expected,
    );
}

/// DEV-128。`S6` かつ `Large` で数値型が `F32` または `I8` なら、常駐位置を問わず `Route(GpuGeneric)`。
#[test]
fn s6_large_f32_or_i8_routes_to_gpu_generic() {
    let expected = Decision {
        output: RouteOutput::Route(RouteCandidate::GpuGeneric),
        reason: DecisionReason::CapabilityS6ScaleLargeOtherScalar,
    };
    for scalar in [ScalarKind::F32, ScalarKind::I8] {
        assert_for_all_residencies(CapabilityLevel::S6, DataScale::Large, scalar, expected);
    }
}
