//! CUDA の背後実装の能力の問合せを、実の device で確かめる試験（TASK-175 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::{
    BackendKind, CapabilityEvidence, CapabilityLevel, CapabilityQuery,
};
use fandhe_silicon_core::capability_rule::check_query_ex;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::feature_key::{
    FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX, FEATURE_MAPPABLE_PRIMARY_BUFFERS, FEATURE_QUERY_KEYS,
    FEATURE_SHADER_F16, FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_SUBGROUP, FEATURE_SUBGROUP_BARRIER,
};

fn new_device() -> Box<dyn Device> {
    CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("request")
}

/// `adapter_name` の末尾の ` (cuda compute_capability=<major>.<minor>)` から値を読む。
fn compute_capability(device: &dyn Device) -> (i32, i32) {
    let name = device.diagnostics().adapter_name;
    let tail = name
        .rsplit_once("compute_capability=")
        .map(|(_, t)| t.trim_end_matches(')'))
        .expect("compute capability in adapter_name");
    let (major, minor) = tail.split_once('.').expect("major.minor");
    (major.parse().expect("major"), minor.parse().expect("minor"))
}

// DEV-97
#[test]
#[ignore = "requires a CUDA device"]
fn rows_follow_the_compute_capability_and_the_attribute() {
    let device = new_device();
    let cc = compute_capability(device.as_ref());
    let expected_level = if cc >= (7, 5) {
        CapabilityLevel::S2
    } else {
        CapabilityLevel::Unsupported
    };
    for key in [
        FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX,
        FEATURE_SHADER_F16,
        FEATURE_SUBGROUP,
        FEATURE_SUBGROUP_BARRIER,
        FEATURE_SHADER_FLOAT32_ATOMIC,
    ] {
        let q = device.query_ex(key);
        assert_eq!(q.level, expected_level, "{key:?}");
        assert_eq!(q.evidence, Some(CapabilityEvidence::RuntimeApi), "{key:?}");
    }
    let m = device.query_ex(FEATURE_MAPPABLE_PRIMARY_BUFFERS);
    assert!(
        m == CapabilityQuery {
            level: CapabilityLevel::S2,
            unsupported_reason: None,
            evidence: Some(CapabilityEvidence::RuntimeApi),
        } || m
            == CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: Some(UnsupportedReason::NotSupported),
                evidence: Some(CapabilityEvidence::RuntimeApi),
            },
        "{m:?}"
    );
}

// DEV-88
#[test]
#[ignore = "requires a CUDA device"]
fn every_key_passes_the_common_rules_without_s0() {
    let device = new_device();
    for &key in FEATURE_QUERY_KEYS {
        let q = check_query_ex(device.as_ref(), key).expect("common rules");
        assert_eq!(device.query(key), q.level, "{key:?}");
        assert_ne!(q.level, CapabilityLevel::S0, "{key:?}");
        println!(
            "query: feature={} level={:?} reason={:?} evidence={:?}",
            key.0, q.level, q.unsupported_reason, q.evidence
        );
    }
}
