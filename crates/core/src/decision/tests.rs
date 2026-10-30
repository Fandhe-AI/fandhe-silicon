//! 決定表 `decide` の試験（TASK-141）。同じ crate の中に置くのは、`CapabilityLevel` の全値を網羅する match で試験の値の一覧を縛るため。
//!
//! 能力段階 8 値 × データ規模 2 値 × 常駐位置 3 値 × 数値型 3 値の 144 通りを対象にする。
//! `expected` は組み合わせごとの期待を持ち、性質の試験は `expected` に依らず出力の条件だけを見る。

use super::*;
use CapabilityLevel as L;
use DecisionReason as R;
use NoRouteReason as N;
use RouteCandidate as C;
use RouteOutput as O;

const LEVELS: [CapabilityLevel; 8] = [
    L::S0,
    L::S1,
    L::S2,
    L::S3,
    L::S4,
    L::S5,
    L::S6,
    L::Unsupported,
];
const SCALES: [DataScale; 2] = [DataScale::Small, DataScale::Large];
const RESIDENCIES: [Residency; 3] = [
    Residency::HostOnly,
    Residency::DeviceResident,
    Residency::Unified,
];
const SCALARS: [ScalarKind; 3] = [ScalarKind::F32, ScalarKind::F16, ScalarKind::I8];

/// 腕を省略せず全値を並べる match。`CapabilityLevel` に値が増えるとここがコンパイルで落ち、`LEVELS` の更新を促す。
fn level_index(l: CapabilityLevel) -> usize {
    match l {
        L::S0 => 0,
        L::S1 => 1,
        L::S2 => 2,
        L::S3 => 3,
        L::S4 => 4,
        L::S5 => 5,
        L::S6 => 6,
        L::Unsupported => 7,
    }
}

fn all_inputs() -> Vec<DecisionInput> {
    let mut v = Vec::new();
    for &capability in &LEVELS {
        for &scale in &SCALES {
            for &residency in &RESIDENCIES {
                for &scalar in &SCALARS {
                    v.push(DecisionInput::new(capability, scale, residency, scalar));
                }
            }
        }
    }
    v
}

fn expected(i: DecisionInput) -> Decision {
    let (output, reason) = match (i.capability, i.scale, i.scalar) {
        (L::S0, ..) => (O::NoRoute(N::Unknown), R::CapabilityS0),
        (L::Unsupported, ..) => (O::NoRoute(N::Unsupported), R::CapabilityUnsupported),
        (L::S1 | L::S2 | L::S3 | L::S4, ..) => (O::NoRoute(N::NotVerified), R::CapabilityBelowS5),
        (L::S5, DataScale::Small, _) => (O::NoRoute(N::NotVerified), R::ScaleSmall),
        (L::S5, DataScale::Large, _) => (O::Route(C::GpuGeneric), R::CapabilityS5ScaleLarge),
        (L::S6, DataScale::Small, _) => (O::NoRoute(N::NotVerified), R::ScaleSmall),
        (L::S6, DataScale::Large, ScalarKind::F16) => {
            (O::Route(C::GpuMatrixUnit), R::CapabilityS6ScaleLargeF16)
        }
        (L::S6, DataScale::Large, _) => (
            O::Route(C::GpuGeneric),
            R::CapabilityS6ScaleLargeOtherScalar,
        ),
    };
    Decision { output, reason }
}

#[test]
fn levels_list_covers_every_variant_in_order() {
    let indexes: Vec<usize> = LEVELS.iter().map(|&l| level_index(l)).collect();
    assert_eq!(indexes, (0..8).collect::<Vec<_>>());
}

#[test]
fn input_space_has_144_distinct_combinations() {
    let inputs = all_inputs();
    assert_eq!(inputs.len(), 8 * 2 * 3 * 3);
    for (n, a) in inputs.iter().enumerate() {
        assert!(!inputs[n + 1..].contains(a), "duplicate input {a:?}");
    }
}

/// DEV-116。全組み合わせで期待と一致する。同じ入力を 2 回呼んで同じ値が返ることも見る（戻り値が入力だけで決まることの確認）。
#[test]
fn every_combination_matches_expected_and_returns_same_value_twice() {
    for i in all_inputs() {
        let first = decide(i);
        assert_eq!(first, decide(i), "differs between calls: {i:?}");
        assert_eq!(first, expected(i), "mismatch: {i:?}");
    }
}

#[test]
fn residency_does_not_change_the_decision() {
    for &capability in &LEVELS {
        for &scale in &SCALES {
            for &scalar in &SCALARS {
                let base = decide(DecisionInput::new(
                    capability,
                    scale,
                    Residency::HostOnly,
                    scalar,
                ));
                for &residency in &RESIDENCIES {
                    let d = decide(DecisionInput::new(capability, scale, residency, scalar));
                    assert_eq!(d, base);
                }
            }
        }
    }
}

/// DEV-120。
#[test]
fn s0_is_no_route_unknown_for_every_other_input() {
    for i in all_inputs().into_iter().filter(|i| i.capability == L::S0) {
        let d = decide(i);
        assert_eq!(d.output, O::NoRoute(N::Unknown));
        assert_eq!(d.reason, R::CapabilityS0);
    }
}

/// DEV-121。
#[test]
fn unsupported_is_no_route_unsupported_for_every_other_input() {
    for i in all_inputs()
        .into_iter()
        .filter(|i| i.capability == L::Unsupported)
    {
        let d = decide(i);
        assert_eq!(d.output, O::NoRoute(N::Unsupported));
        assert_eq!(d.reason, R::CapabilityUnsupported);
    }
}

/// DEV-122。未知の段階を `None` で模した内側の関数の結果を見る。
#[test]
fn unrecognized_level_is_no_route_unknown() {
    for scale in SCALES {
        for scalar in SCALARS {
            let d = decide_level(None, scale, scalar);
            assert_eq!(d.output, O::NoRoute(N::Unknown));
            assert_eq!(d.reason, R::CapabilityUnrecognized);
        }
    }
}

/// `expected` に依らない性質: S5 に届かない段階では、どの入力でも経路の候補が出ない。
#[test]
fn levels_below_s5_never_route() {
    for i in all_inputs() {
        if level_index(i.capability) < level_index(L::S5) || i.capability == L::Unsupported {
            assert!(matches!(decide(i).output, O::NoRoute(_)), "{i:?}");
        }
    }
}

/// `expected` に依らない性質: 小さいデータ規模では経路の候補が出ない。
#[test]
fn small_scale_never_routes() {
    for i in all_inputs()
        .into_iter()
        .filter(|i| i.scale == DataScale::Small)
    {
        assert!(matches!(decide(i).output, O::NoRoute(_)), "{i:?}");
    }
}

/// `expected` に依らない性質: 行列の演算器の候補が出るのは、S6・大・F16 のときだけ。
#[test]
fn matrix_unit_route_only_for_s6_large_f16() {
    for i in all_inputs() {
        let is_matrix = decide(i).output == O::Route(C::GpuMatrixUnit);
        let cond =
            i.capability == L::S6 && i.scale == DataScale::Large && i.scalar == ScalarKind::F16;
        assert_eq!(is_matrix, cond, "{i:?}");
    }
}

/// `expected` に依らない性質: 経路の候補が出るのは、S5 または S6 で大きいデータ規模のときだけ。
#[test]
fn any_route_only_for_s5_or_s6_large() {
    for i in all_inputs() {
        let routed = matches!(decide(i).output, O::Route(_));
        let cond = matches!(i.capability, L::S5 | L::S6) && i.scale == DataScale::Large;
        assert_eq!(routed, cond, "{i:?}");
    }
}
