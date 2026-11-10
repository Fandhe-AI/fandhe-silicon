//! 受け取ったテキストの言語版の受理の判定 `accepted_language_version` の試験（TASK-1010 のポインタ）。
//!
//! 純粋関数で、どの OS でも device に触れずに流れる。

use baseline_wgpu_gemm_arm::device::ArmError;
use baseline_wgpu_gemm_arm::prime::{PrimeLanguageVersion, accepted_language_version};

#[test]
fn the_six_versions_are_accepted_and_distinct() {
    let accepted = [
        ((2, 3), PrimeLanguageVersion::V2_3),
        ((2, 4), PrimeLanguageVersion::V2_4),
        ((3, 0), PrimeLanguageVersion::V3_0),
        ((3, 1), PrimeLanguageVersion::V3_1),
        ((3, 2), PrimeLanguageVersion::V3_2),
        ((4, 0), PrimeLanguageVersion::V4_0),
    ];
    for ((major, minor), want) in accepted {
        assert_eq!(accepted_language_version(major, minor).expect("ok"), want);
    }
    for (i, (_, x)) in accepted.iter().enumerate() {
        for (_, y) in &accepted[i + 1..] {
            assert_ne!(x, y);
        }
    }
}

#[test]
fn other_versions_are_refused() {
    for (major, minor) in [(2, 2), (1, 0), (3, 3), (4, 1), (5, 0), (0, 0), (99, 99)] {
        assert!(
            matches!(
                accepted_language_version(major, minor),
                Err(ArmError::LanguageVersion)
            ),
            "{major}.{minor}"
        );
    }
}
