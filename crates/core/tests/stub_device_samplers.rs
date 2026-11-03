//! feature `test-support` の裏の試験用の device の、プロファイル・所有形態ごとの `TextureOps` 越しに `create_sampler` へ届くことを確かめる統合テスト（TASK-150 のポインタ）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`sampler`）だけを使う。
//! `backend` は `BackendKind::Vulkan` に固定し、`Imported` は `NativeDeviceHandle::new(7u32)` を包む。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::sampler::SamplerDescriptor;
use fandhe_silicon_core::stub_device::StubDeviceProvider;

fn device(profile: Profile, ownership: DeviceOwnership) -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Vulkan,
        profile,
        ownership,
    })
    .expect("device")
}

fn imported() -> DeviceOwnership {
    DeviceOwnership::Imported {
        native: NativeDeviceHandle::new(7u32),
    }
}

fn assert_no_sampler_path(d: &dyn Device) {
    assert!(d.textures().is_none());
}

fn assert_creates_sampler(d: &dyn Device) {
    let ops = d.textures().expect("textures");
    let mut desc = SamplerDescriptor::default();
    desc.label = Some("reach");
    let handle = ops.create_sampler(&desc).expect("sampler");
    assert_eq!(handle.__backend_device_id(), d.generation());
    assert_eq!(handle.__backend_generation(), d.generation());
}

// DEV-154
#[test]
fn cpu_only_product_independent_has_no_sampler_path() {
    assert_no_sampler_path(&*device(
        Profile::CpuOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

// DEV-154
#[test]
fn cpu_only_shared_has_no_sampler_path() {
    assert_no_sampler_path(&*device(Profile::CpuOnly, DeviceOwnership::Shared));
}

// DEV-154
#[test]
fn cpu_only_imported_has_no_sampler_path() {
    assert_no_sampler_path(&*device(Profile::CpuOnly, imported()));
}

// DEV-154
#[test]
fn compute_only_product_independent_creates_sampler() {
    assert_creates_sampler(&*device(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

// DEV-154
#[test]
fn compute_only_shared_creates_sampler() {
    assert_creates_sampler(&*device(Profile::ComputeOnly, DeviceOwnership::Shared));
}

// DEV-154
#[test]
fn compute_only_imported_creates_sampler() {
    assert_creates_sampler(&*device(Profile::ComputeOnly, imported()));
}

// DEV-154
#[test]
fn graphics_product_independent_creates_sampler() {
    assert_creates_sampler(&*device(
        Profile::Graphics,
        DeviceOwnership::ProductIndependent,
    ));
}

// DEV-154
#[test]
fn graphics_shared_creates_sampler() {
    assert_creates_sampler(&*device(Profile::Graphics, DeviceOwnership::Shared));
}

// DEV-154
#[test]
fn graphics_imported_creates_sampler() {
    assert_creates_sampler(&*device(Profile::Graphics, imported()));
}

// DEV-154
#[test]
fn sampler_descriptor_without_label_also_reaches() {
    let d = device(Profile::ComputeOnly, DeviceOwnership::Shared);
    let ops = d.textures().expect("textures");
    let desc = SamplerDescriptor::default();
    assert!(desc.label.is_none());
    let handle = ops.create_sampler(&desc).expect("sampler");
    assert_eq!(handle.__backend_device_id(), d.generation());
}

// DEV-154
#[test]
fn two_samplers_from_one_device_have_distinct_ids() {
    let d = device(Profile::Graphics, DeviceOwnership::Shared);
    let ops = d.textures().expect("textures");
    let desc = SamplerDescriptor::default();
    let a = ops.create_sampler(&desc).expect("sampler");
    let b = ops.create_sampler(&desc).expect("sampler");
    assert_ne!(a.__backend_id(), b.__backend_id());
}
