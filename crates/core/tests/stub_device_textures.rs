//! feature `test-support` の裏の試験用の device の、プロファイル・所有形態ごとの `textures()` の答えと、`create_texture` へ届くことを確かめる統合テスト（TASK-150 の 3/4。DEV-153 のポインタ）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`texture`）だけを使う。
//! `backend` は `BackendKind::Vulkan` に固定し、`Imported` は `NativeDeviceHandle::new(7u32)` を包む。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::stub_device::StubDeviceProvider;
use fandhe_silicon_core::texture::{
    TextureDescriptor, TextureDimension, TextureExtent, TextureUsage,
};
use fandhe_silicon_core::texture_format::TextureFormat;

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

fn extent() -> TextureExtent {
    TextureExtent {
        width: 64,
        height: 32,
        depth_or_array_layers: 1,
    }
}

fn unlabeled() -> TextureDescriptor<'static> {
    TextureDescriptor::new(
        TextureDimension::D2,
        extent(),
        TextureFormat::Rgba8Unorm,
        TextureUsage::TEXTURE_BINDING,
    )
}

fn labeled() -> TextureDescriptor<'static> {
    let mut desc = unlabeled();
    desc.label = Some("reach");
    desc
}

fn assert_no_textures(d: &dyn Device) {
    assert!(d.textures().is_none());
}

fn assert_creates_texture(d: &dyn Device, desc: &TextureDescriptor<'_>) {
    let ops = d.textures().expect("textures");
    let handle = ops.create_texture(desc).expect("texture");
    assert_eq!(handle.__backend_device_id(), d.generation());
    assert_eq!(handle.size(), desc.size);
    assert_eq!(handle.width(), desc.size.width);
    assert_eq!(handle.height(), desc.size.height);
    assert_eq!(
        handle.depth_or_array_layers(),
        desc.size.depth_or_array_layers
    );
    assert_eq!(handle.mip_level_count(), desc.mip_level_count);
    assert_eq!(handle.sample_count(), desc.sample_count);
    assert_eq!(handle.dimension(), desc.dimension);
    assert_eq!(handle.format(), desc.format);
    assert_eq!(handle.usage(), desc.usage);
}

#[test]
fn cpu_only_product_independent_textures_is_none() {
    assert_no_textures(&*device(
        Profile::CpuOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

#[test]
fn cpu_only_shared_textures_is_none() {
    assert_no_textures(&*device(Profile::CpuOnly, DeviceOwnership::Shared));
}

#[test]
fn cpu_only_imported_textures_is_none() {
    assert_no_textures(&*device(Profile::CpuOnly, imported()));
}

#[test]
fn compute_only_product_independent_textures_creates_texture() {
    assert_creates_texture(
        &*device(Profile::ComputeOnly, DeviceOwnership::ProductIndependent),
        &labeled(),
    );
}

#[test]
fn compute_only_shared_textures_creates_texture() {
    assert_creates_texture(
        &*device(Profile::ComputeOnly, DeviceOwnership::Shared),
        &labeled(),
    );
}

#[test]
fn compute_only_imported_textures_creates_texture() {
    assert_creates_texture(&*device(Profile::ComputeOnly, imported()), &labeled());
}

#[test]
fn graphics_product_independent_textures_creates_texture() {
    assert_creates_texture(
        &*device(Profile::Graphics, DeviceOwnership::ProductIndependent),
        &labeled(),
    );
}

#[test]
fn graphics_shared_textures_creates_texture() {
    assert_creates_texture(
        &*device(Profile::Graphics, DeviceOwnership::Shared),
        &labeled(),
    );
}

#[test]
fn graphics_imported_textures_creates_texture() {
    assert_creates_texture(&*device(Profile::Graphics, imported()), &labeled());
}

#[test]
fn descriptor_without_label_also_reaches() {
    let desc = unlabeled();
    assert!(desc.label.is_none());
    assert_creates_texture(
        &*device(Profile::ComputeOnly, DeviceOwnership::Shared),
        &desc,
    );
}

#[test]
fn two_textures_from_one_device_have_distinct_ids() {
    let d = device(Profile::ComputeOnly, DeviceOwnership::Shared);
    let ops = d.textures().expect("textures");
    let desc = labeled();
    let a = ops.create_texture(&desc).expect("first");
    let b = ops.create_texture(&desc).expect("second");
    assert_ne!(a.__backend_id(), b.__backend_id());
    assert_eq!(a.__backend_device_id(), d.generation());
    assert_eq!(b.__backend_device_id(), d.generation());
}
