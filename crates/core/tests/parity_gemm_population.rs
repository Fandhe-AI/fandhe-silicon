//! GEMM の門の run の母数の表と数え方の確認（spec TASK-1005 の 3/6）。

use std::collections::HashSet;

use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{
    GEMM_GATE_POPULATION, GemmGateArm, gate_cells, population_cells,
};
use fandhe_silicon_core::test_frame::TestId;

fn arms_for(dtype: GemmInputDtype) -> HashSet<GemmGateArm> {
    population_cells()
        .into_iter()
        .filter(|c| c.dtype() == dtype)
        .map(|c| c.arm())
        .collect()
}

#[test]
fn population_is_sixty() {
    assert_eq!(population_cells().len(), 60);
    assert_eq!(GEMM_GATE_POPULATION, population_cells().len());
}

#[test]
fn population_split_by_dtype() {
    let cells = population_cells();
    let f32n = cells
        .iter()
        .filter(|c| c.dtype() == GemmInputDtype::F32)
        .count();
    let f16n = cells
        .iter()
        .filter(|c| c.dtype() == GemmInputDtype::F16Acc)
        .count();
    assert_eq!((f32n, f16n), (40, 20));
}

#[test]
fn arms_per_dtype() {
    use GemmGateArm::*;
    let f32_arms: HashSet<_> = [G1, G2, G3, G3Prime, G5, G6, G7, G9].into_iter().collect();
    let f16_arms: HashSet<_> = [G3, G3Prime, G6, G9].into_iter().collect();
    assert_eq!(arms_for(GemmInputDtype::F32), f32_arms);
    assert_eq!(arms_for(GemmInputDtype::F16Acc), f16_arms);
}

#[test]
fn each_arm_covers_all_rows() {
    let cells = gate_cells();
    for arm in GemmGateArm::ALL {
        for &dtype in arm.dtypes() {
            for row in GemmGateRow::ALL {
                let n = cells
                    .iter()
                    .filter(|c| c.arm() == arm && c.dtype() == dtype && c.row() == row)
                    .count();
                assert_eq!(n, 1, "{} {} {}", arm.name(), row.name(), dtype.name());
            }
        }
    }
}

#[test]
fn mps_is_outside_population() {
    assert!(!GemmGateArm::Mps.in_population());
    let mps = gate_cells()
        .iter()
        .filter(|c| c.arm() == GemmGateArm::Mps)
        .count();
    assert_eq!(mps, 10);
    assert!(
        population_cells()
            .iter()
            .all(|c| c.arm() != GemmGateArm::Mps)
    );
    assert_eq!(gate_cells().len(), 70);
}

#[test]
fn cells_are_unique() {
    let cells = gate_cells();
    let set: HashSet<_> = cells.iter().copied().collect();
    assert_eq!(set.len(), cells.len());
    let ids: HashSet<_> = cells.iter().map(|c| c.id()).collect();
    assert_eq!(ids.len(), cells.len());
}

#[test]
fn ids_are_test_ids() {
    for c in gate_cells() {
        assert!(TestId::new(&c.id()).is_ok(), "{}", c.id());
    }
    for arm in GemmGateArm::ALL {
        assert!(
            arm.name()
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
    }
}

#[test]
fn order_is_fixed() {
    let ids: Vec<String> = gate_cells().iter().map(|c| c.id()).collect();
    assert_eq!(ids[0], "g1-sq512-f32");
    assert_eq!(ids[1], "g1-sq2048-f32");
    assert_eq!(ids[ids.len() - 1], "mps-odd37x41x53-f16acc");
    assert_eq!(ids[ids.len() - 2], "mps-odd37x41x53-f32");
}

#[test]
fn runs_matches_dtypes() {
    for arm in GemmGateArm::ALL {
        for dtype in [GemmInputDtype::F32, GemmInputDtype::F16Acc] {
            assert_eq!(arm.runs(dtype), arm.dtypes().contains(&dtype));
        }
    }
}
