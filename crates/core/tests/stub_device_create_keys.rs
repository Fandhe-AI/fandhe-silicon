//! feature `test-support` の裏の試験用の device が、テクスチャとサンプラーを作る操作の行の 2 つの鍵に、プロファイル・所有形態によらず段階 `S2` を返すことを確かめる統合テスト（TASK-150 のポインタ）。
//!
//! 公開のパスだけを使う。`Imported` は `NativeDeviceHandle::new(7u32)` を包む。

use fandhe_silicon_core::capability::{
    API_DEVICE_CREATE_SAMPLER, API_DEVICE_CREATE_TEXTURE, BackendKind, CapabilityLevel,
    CapabilityQuery, FeatureId, WEBGPU_DEVICE_PORT, WEBGPU_HANDLE_PORT,
};
use fandhe_silicon_core::capability_rule::{check_query_ex, check_webgpu_ports};
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::downlevel_key::DOWNLEVEL_QUERY_KEYS;
use fandhe_silicon_core::feature_key::FEATURE_QUERY_KEYS;
use fandhe_silicon_core::limit_key::LIMIT_QUERY_KEYS;
use fandhe_silicon_core::sampler::SamplerDescriptor;
use fandhe_silicon_core::stub_device::StubDeviceProvider;
use fandhe_silicon_core::texture::{
    TextureDescriptor, TextureDimension, TextureExtent, TextureUsage,
};
use fandhe_silicon_core::texture_format::TextureFormat;
use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;

const S2: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::S2,
    unsupported_reason: None,
    evidence: None,
};

const KEYS: [FeatureId; 2] = [API_DEVICE_CREATE_TEXTURE, API_DEVICE_CREATE_SAMPLER];

fn device_on(
    backend: BackendKind,
    profile: Profile,
    ownership: DeviceOwnership,
) -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend,
        profile,
        ownership,
    })
    .expect("device")
}

fn device(profile: Profile, ownership: DeviceOwnership) -> Box<dyn Device> {
    device_on(BackendKind::Vulkan, profile, ownership)
}

fn imported() -> DeviceOwnership {
    DeviceOwnership::Imported {
        native: NativeDeviceHandle::new(7u32),
    }
}

fn assert_answers_s2(d: &dyn Device) {
    for key in KEYS {
        assert_eq!(d.query(key), CapabilityLevel::S2);
        assert_eq!(d.query_ex(key), S2);
        assert_eq!(check_query_ex(d, key), Ok(S2));
    }
}

fn assert_cpu_only(d: &dyn Device) {
    assert_answers_s2(d);
    assert!(d.textures().is_none());
}

fn assert_creates(d: &dyn Device) {
    assert_answers_s2(d);
    let ops = d.textures().expect("textures");
    let desc = TextureDescriptor::new(
        TextureDimension::D2,
        TextureExtent {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        TextureFormat::Rgba8Unorm,
        TextureUsage::TEXTURE_BINDING,
    );
    assert!(ops.create_texture(&desc).is_ok());
    assert!(ops.create_sampler(&SamplerDescriptor::default()).is_ok());
}

// DEV-153
// DEV-154
#[test]
fn cpu_only_product_independent_answers_s2_for_create_keys() {
    assert_cpu_only(&*device(
        Profile::CpuOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

// DEV-153
// DEV-154
#[test]
fn cpu_only_shared_answers_s2_for_create_keys() {
    assert_cpu_only(&*device(Profile::CpuOnly, DeviceOwnership::Shared));
}

// DEV-153
// DEV-154
#[test]
fn cpu_only_imported_answers_s2_for_create_keys() {
    assert_cpu_only(&*device(Profile::CpuOnly, imported()));
}

// DEV-153
// DEV-154
#[test]
fn compute_only_product_independent_answers_s2_for_create_keys() {
    assert_creates(&*device(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

// DEV-153
// DEV-154
#[test]
fn compute_only_shared_answers_s2_for_create_keys() {
    assert_creates(&*device(Profile::ComputeOnly, DeviceOwnership::Shared));
}

// DEV-153
// DEV-154
#[test]
fn compute_only_imported_answers_s2_for_create_keys() {
    assert_creates(&*device(Profile::ComputeOnly, imported()));
}

// DEV-153
// DEV-154
#[test]
fn graphics_product_independent_answers_s2_for_create_keys() {
    assert_creates(&*device(
        Profile::Graphics,
        DeviceOwnership::ProductIndependent,
    ));
}

// DEV-153
// DEV-154
#[test]
fn graphics_shared_answers_s2_for_create_keys() {
    assert_creates(&*device(Profile::Graphics, DeviceOwnership::Shared));
}

// DEV-153
// DEV-154
#[test]
fn graphics_imported_answers_s2_for_create_keys() {
    assert_creates(&*device(Profile::Graphics, imported()));
}

// DEV-153
// DEV-154
#[test]
fn answers_do_not_depend_on_backend() {
    for backend in [
        BackendKind::Metal,
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        for ownership in [DeviceOwnership::Shared, DeviceOwnership::ProductIndependent] {
            let d = device_on(backend, Profile::ComputeOnly, ownership);
            for key in KEYS {
                assert_eq!(d.query_ex(key), S2);
            }
        }
    }
}

// DEV-153
// DEV-154
#[test]
fn create_key_strings_are_fixed() {
    assert_eq!(
        API_DEVICE_CREATE_TEXTURE,
        FeatureId("api:wgpu/src/api/device.rs:create_texture:294")
    );
    assert_eq!(
        API_DEVICE_CREATE_SAMPLER,
        FeatureId("api:wgpu/src/api/device.rs:create_sampler:489")
    );
}

// DEV-153
// DEV-154
#[test]
fn create_keys_are_outside_other_key_lists() {
    assert_ne!(API_DEVICE_CREATE_TEXTURE, API_DEVICE_CREATE_SAMPLER);
    for key in KEYS {
        assert!(!TEXTURE_FORMAT_QUERY_KEYS.contains(&key));
        assert!(!FEATURE_QUERY_KEYS.contains(&key));
        assert!(!LIMIT_QUERY_KEYS.contains(&key));
        assert!(!DOWNLEVEL_QUERY_KEYS.contains(&key));
        assert_ne!(key, WEBGPU_DEVICE_PORT);
        assert_ne!(key, WEBGPU_HANDLE_PORT);
    }
}

// DEV-153
// DEV-154
#[test]
fn other_keys_keep_their_answers() {
    let d = device(Profile::ComputeOnly, DeviceOwnership::Shared);
    assert_eq!(check_webgpu_ports(&*d), Ok(()));
    let unlisted = FeatureId("any-feature");
    assert_eq!(d.query_ex(unlisted), UNLISTED_FEATURE_QUERY);
    assert_eq!(d.query(unlisted), CapabilityLevel::Unsupported);
    assert_eq!(
        d.query(TEXTURE_FORMAT_QUERY_KEYS[0]),
        CapabilityLevel::Unsupported
    );
}
