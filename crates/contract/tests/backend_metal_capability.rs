//! Metal の背後実装の能力の問合せの確認（TASK-173 のポインタ）。
//!
//! device に触る試験は `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::{
    BackendKind, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::capability_rule::check_query_ex;
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::feature_key::{
    FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX, FEATURE_EXPERIMENTAL_MESH_SHADER,
    FEATURE_MAPPABLE_PRIMARY_BUFFERS, FEATURE_MULTI_DRAW_INDIRECT_COUNT, FEATURE_QUERY_KEYS,
    FEATURE_SHADER_F16, FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_SUBGROUP, FEATURE_SUBGROUP_BARRIER,
    FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES, FEATURE_TEXTURE_COMPRESSION_ASTC_HDR,
    FEATURE_VULKAN_EXTERNAL_MEMORY_FD,
};

fn new_device() -> Box<dyn Device> {
    MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("request")
}

const S2: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::S2,
    unsupported_reason: None,
    evidence: Some(CapabilityEvidence::RuntimeApi),
};
const NO_API: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::Unsupported,
    unsupported_reason: Some(UnsupportedReason::NoApi),
    evidence: Some(CapabilityEvidence::RuntimeApi),
};

const EXPECTED: [(FeatureId, CapabilityQuery); 11] = [
    (FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX, S2),
    (FEATURE_SHADER_F16, S2),
    (FEATURE_SUBGROUP, S2),
    (FEATURE_SUBGROUP_BARRIER, S2),
    (FEATURE_SHADER_FLOAT32_ATOMIC, S2),
    (
        FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        UNLISTED_FEATURE_QUERY,
    ),
    (FEATURE_MAPPABLE_PRIMARY_BUFFERS, S2),
    (FEATURE_VULKAN_EXTERNAL_MEMORY_FD, NO_API),
    (FEATURE_MULTI_DRAW_INDIRECT_COUNT, UNLISTED_FEATURE_QUERY),
    (FEATURE_TEXTURE_COMPRESSION_ASTC_HDR, S2),
    (FEATURE_EXPERIMENTAL_MESH_SHADER, UNLISTED_FEATURE_QUERY),
];

// DEV-95
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn selected_features_return_the_expected_values() {
    let device = new_device();
    for (feature, expected) in EXPECTED {
        let q = device.query_ex(feature);
        println!(
            "query: {} level={:?} reason={:?} evidence={:?}",
            feature.0, q.level, q.unsupported_reason, q.evidence
        );
        assert_eq!(q, expected, "{feature:?}");
        assert_eq!(device.query(feature), expected.level, "{feature:?}");
    }
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn every_key_passes_the_common_rules_without_s0() {
    let device = new_device();
    for &key in FEATURE_QUERY_KEYS {
        let q = check_query_ex(device.as_ref(), key).expect("common rules");
        assert_ne!(q.level, CapabilityLevel::S0, "{key:?}");
        assert_eq!(device.query(key), q.level, "{key:?}");
    }
}
