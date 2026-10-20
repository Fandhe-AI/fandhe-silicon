//! `device`・`capability` モジュールの `#[non_exhaustive]` な 5 型を別の crate から固定する公開テスト（TASK-136 の 3/6）。
//!
//! 仕組み: `#![deny(unreachable_patterns)]` のもとで全値を並べた後に `_` の腕を置く。
//! 別の crate からは `_` の腕が必須になり、属性が外れると `_` が到達不能になってコンパイルが落ちる（DEV-65 の 5 型の分。ポインタのみ）。
//! `CapabilityLevel` の 8 値が互いに別の値であることも見る（DEV-81。ポインタのみ）。

#![deny(unreachable_patterns)]

use fandhe_silicon_core::capability::{BackendKind, CapabilityEvidence, CapabilityLevel};
use fandhe_silicon_core::device::{DeviceOwnership, NativeDeviceHandle, Profile};

#[test]
fn backend_kind_stays_non_exhaustive() {
    fn label(v: BackendKind) -> &'static str {
        match v {
            BackendKind::Metal => "Metal",
            BackendKind::Vulkan => "Vulkan",
            BackendKind::Cuda => "Cuda",
            BackendKind::Dx12Compute => "Dx12Compute",
            _ => "other",
        }
    }
    assert_eq!(label(BackendKind::Metal), "Metal");
    assert_eq!(label(BackendKind::Vulkan), "Vulkan");
    assert_eq!(label(BackendKind::Cuda), "Cuda");
    assert_eq!(label(BackendKind::Dx12Compute), "Dx12Compute");
}

#[test]
fn profile_stays_non_exhaustive() {
    fn label(v: Profile) -> &'static str {
        match v {
            Profile::CpuOnly => "CpuOnly",
            Profile::ComputeOnly => "ComputeOnly",
            Profile::Graphics => "Graphics",
            _ => "other",
        }
    }
    assert_eq!(label(Profile::CpuOnly), "CpuOnly");
    assert_eq!(label(Profile::ComputeOnly), "ComputeOnly");
    assert_eq!(label(Profile::Graphics), "Graphics");
}

#[test]
fn device_ownership_stays_non_exhaustive() {
    fn label(v: &DeviceOwnership) -> &'static str {
        match v {
            DeviceOwnership::ProductIndependent => "ProductIndependent",
            DeviceOwnership::Shared => "Shared",
            DeviceOwnership::Imported { .. } => "Imported",
            _ => "other",
        }
    }
    assert_eq!(
        label(&DeviceOwnership::ProductIndependent),
        "ProductIndependent"
    );
    assert_eq!(label(&DeviceOwnership::Shared), "Shared");
    let imported = DeviceOwnership::Imported {
        native: NativeDeviceHandle::new(0u8),
    };
    assert_eq!(label(&imported), "Imported");
}

fn level_label(v: CapabilityLevel) -> &'static str {
    match v {
        CapabilityLevel::S0 => "S0",
        CapabilityLevel::S1 => "S1",
        CapabilityLevel::S2 => "S2",
        CapabilityLevel::S3 => "S3",
        CapabilityLevel::S4 => "S4",
        CapabilityLevel::S5 => "S5",
        CapabilityLevel::S6 => "S6",
        CapabilityLevel::Unsupported => "Unsupported",
        _ => "other",
    }
}

const ALL_LEVELS: [CapabilityLevel; 8] = [
    CapabilityLevel::S0,
    CapabilityLevel::S1,
    CapabilityLevel::S2,
    CapabilityLevel::S3,
    CapabilityLevel::S4,
    CapabilityLevel::S5,
    CapabilityLevel::S6,
    CapabilityLevel::Unsupported,
];

#[test]
fn capability_level_stays_non_exhaustive() {
    let labels: Vec<_> = ALL_LEVELS.into_iter().map(level_label).collect();
    assert_eq!(
        labels,
        ["S0", "S1", "S2", "S3", "S4", "S5", "S6", "Unsupported"]
    );
}

#[test]
fn capability_evidence_stays_non_exhaustive() {
    fn label(v: CapabilityEvidence) -> &'static str {
        match v {
            CapabilityEvidence::RuntimeApi => "RuntimeApi",
            CapabilityEvidence::StaticVendorTable => "StaticVendorTable",
            CapabilityEvidence::ProbeCompile => "ProbeCompile",
            CapabilityEvidence::ImplementationLedger => "ImplementationLedger",
            CapabilityEvidence::FieldRecord => "FieldRecord",
            CapabilityEvidence::PerformanceComparison => "PerformanceComparison",
            _ => "other",
        }
    }
    let labels: Vec<_> = [
        CapabilityEvidence::RuntimeApi,
        CapabilityEvidence::StaticVendorTable,
        CapabilityEvidence::ProbeCompile,
        CapabilityEvidence::ImplementationLedger,
        CapabilityEvidence::FieldRecord,
        CapabilityEvidence::PerformanceComparison,
    ]
    .into_iter()
    .map(label)
    .collect();
    assert_eq!(
        labels,
        [
            "RuntimeApi",
            "StaticVendorTable",
            "ProbeCompile",
            "ImplementationLedger",
            "FieldRecord",
            "PerformanceComparison"
        ]
    );
}

#[test]
fn capability_level_has_eight_distinct_values() {
    for (i, a) in ALL_LEVELS.iter().enumerate() {
        for (j, b) in ALL_LEVELS.iter().enumerate() {
            assert_eq!(a == b, i == j, "levels {i} and {j}");
        }
    }
    assert_ne!(CapabilityLevel::S0, CapabilityLevel::Unsupported);
}
