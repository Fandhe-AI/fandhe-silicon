//! feature `test-support` の裏の試験用の device の、要求・能力問合せ・診断の振る舞いを確かめる統合テスト（TASK-156 の 3/5。DEV-34 のポインタ）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`capability`）だけを使う。

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
    WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT,
};
use fandhe_silicon_core::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle,
    Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::stub_device::{StubDevice, StubDeviceProvider};

fn backends() -> [BackendKind; 4] {
    [
        BackendKind::Metal,
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ]
}

fn profiles() -> [Profile; 3] {
    [Profile::CpuOnly, Profile::ComputeOnly, Profile::Graphics]
}

/// 所有形態を番号から作る。`Imported` は渡した値を包む。
fn ownership(kind: usize, tag: u32) -> DeviceOwnership {
    match kind {
        0 => DeviceOwnership::ProductIndependent,
        1 => DeviceOwnership::Shared,
        _ => DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(tag),
        },
    }
}

fn request(backend: BackendKind) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::Shared,
    }
}

#[test]
fn request_succeeds_for_every_backend_profile_ownership() {
    let mut count = 0u32;
    for backend in backends() {
        for profile in profiles() {
            for kind in 0..3 {
                let tag = 1000 + count;
                let req = DeviceRequest {
                    backend,
                    profile,
                    ownership: ownership(kind, tag),
                };
                let device = StubDeviceProvider::request(req).expect("device");
                assert_eq!(device.backend(), backend);
                match (kind, device.ownership()) {
                    (0, DeviceOwnership::ProductIndependent) => {}
                    (1, DeviceOwnership::Shared) => {}
                    (2, DeviceOwnership::Imported { native }) => {
                        assert_eq!(native.__backend_downcast_ref::<u32>(), Some(&tag));
                        assert!(native.__backend_downcast_ref::<u64>().is_none());
                    }
                    _ => panic!("ownership does not match the request"),
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 36);
}

#[test]
fn stub_device_new_matches_provider() {
    for backend in backends() {
        let device = StubDevice::new(request(backend));
        assert_eq!(device.backend(), backend);
        assert!(matches!(device.ownership(), DeviceOwnership::Shared));
    }
}

#[test]
fn generation_is_positive_and_increases() {
    let first = StubDevice::new(request(BackendKind::Metal));
    let second = StubDevice::new(request(BackendKind::Metal));
    assert!(first.generation() >= 1);
    assert!(second.generation() > first.generation());
}

#[test]
fn diagnostics_are_fixed_strings() {
    let expected = DeviceDiagnostics {
        adapter_name: "stub".to_string(),
        backend_name: "stub",
    };
    for backend in backends() {
        assert_eq!(StubDevice::new(request(backend)).diagnostics(), expected);
    }
}

fn feature_ids() -> Vec<FeatureId> {
    vec![
        FeatureId(""),
        FeatureId("any-feature"),
        FeatureId("feature:TEXTURE_COMPRESSION_BC"),
        WEBGPU_DEVICE_PORT,
        WEBGPU_HANDLE_PORT,
    ]
}

#[test]
fn query_is_unsupported_for_any_feature() {
    for backend in backends() {
        let device = StubDevice::new(request(backend));
        for id in feature_ids() {
            assert_eq!(device.query(id), CapabilityLevel::Unsupported);
        }
    }
}

#[test]
fn query_ex_uses_default_body() {
    let device = StubDevice::new(request(BackendKind::Vulkan));
    for id in feature_ids() {
        assert_eq!(
            device.query_ex(id),
            CapabilityQuery {
                level: CapabilityLevel::Unsupported,
                unsupported_reason: None,
                evidence: None,
            }
        );
    }
}

#[test]
fn limits_table_is_fixed() {
    for backend in backends() {
        let device = StubDevice::new(request(backend));
        assert_eq!(
            *device.limits(),
            LimitsTable {
                max_buffer_bytes: 1 << 28,
                max_dispatch_dims: [0, 0, 0],
                max_texture_size: 0,
            }
        );
    }
}

#[test]
fn provider_returns_boxed_device_and_stub_is_send_sync() {
    let result: Result<Box<dyn Device>, ContractError> =
        StubDeviceProvider::request(request(BackendKind::Cuda));
    assert!(result.is_ok());
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<StubDevice>();
}
