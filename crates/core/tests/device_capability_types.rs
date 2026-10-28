//! device の要求・能力問合せの型を別の crate から作って呼べることの最小の試験（TASK-136 の 2/6）。
//!
//! 構築と呼び出しだけを見る。フィールドの列挙は `device_request_fields.rs`、`#[non_exhaustive]` の腕は
//! `device_capability_non_exhaustive.rs`、既定の実装の値は `capability_query_default.rs` が見る（3/6）。

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::device::{DeviceOwnership, DeviceRequest, NativeDeviceHandle, Profile};

#[test]
fn device_request_builds_with_every_ownership_and_profile() {
    let ownerships = [
        DeviceOwnership::ProductIndependent,
        DeviceOwnership::Shared,
        DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(7u32),
        },
    ];
    let profiles = [Profile::CpuOnly, Profile::ComputeOnly, Profile::Graphics];
    for (ownership, profile) in ownerships.into_iter().zip(profiles) {
        let req = DeviceRequest {
            backend: BackendKind::Metal,
            profile,
            ownership,
        };
        assert_eq!(req.profile, profile);
    }
}

#[test]
fn capability_query_builds_and_compares() {
    for ev in [
        CapabilityEvidence::RuntimeApi,
        CapabilityEvidence::StaticVendorTable,
        CapabilityEvidence::ProbeCompile,
        CapabilityEvidence::ImplementationLedger,
        CapabilityEvidence::FieldRecord,
        CapabilityEvidence::PerformanceComparison,
    ] {
        let q = CapabilityQuery {
            level: CapabilityLevel::S1,
            unsupported_reason: None,
            evidence: Some(ev),
        };
        assert_eq!(q, q.clone());
    }
}

#[test]
fn native_handle_round_trips_without_leaking_contents() {
    let h = NativeDeviceHandle::new(7u32);
    assert_eq!(h.__backend_downcast_ref::<u32>(), Some(&7));
    assert!(h.__backend_downcast_ref::<u64>().is_none());
    assert!(!format!("{h:?}").contains('7'));
    let own = DeviceOwnership::Imported { native: h };
    assert!(!format!("{own:?}").contains('7'));
}

struct Fixed;
/// `Capabilities::limits` の試験用の表。値は試験用の小さな数で、意味は持たない。
static TEST_LIMITS: fandhe_silicon_core::capability::LimitsTable =
    fandhe_silicon_core::capability::LimitsTable {
        max_buffer_bytes: 1,
        max_dispatch_dims: [1, 1, 1],
        max_texture_size: 1,
    };

impl Capabilities for Fixed {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S2
    }
}

#[test]
fn capabilities_is_callable_through_dyn() {
    let c: &dyn Capabilities = &Fixed;
    assert_eq!(c.query(FeatureId("x")), CapabilityLevel::S2);
    let _ = c.query_ex(FeatureId("x"));
}
