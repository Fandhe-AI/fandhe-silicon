//! feature `test-support` の裏の試験用の device の、プロファイル・所有形態ごとの `compute()` の答えを確かめる統合テスト（TASK-2956 の 1/3。DEV-59・DEV-60・DEV-164 のポインタ）。
//!
//! 公開のパス（`fandhe_silicon_core::stub_device` と `device`・`execution`）だけを使う。
//! `backend` は `BackendKind::Vulkan` に固定し、`Imported` は `NativeDeviceHandle::new(7u32)` を包む。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::execution::{BindingLayout, PipelineInput};
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

fn input() -> PipelineInput<'static> {
    PipelineInput::Msl {
        source: "",
        entry_point: "main",
    }
}

fn assert_no_compute(d: &dyn Device) {
    assert!(d.compute().is_none());
}

fn assert_creates_pipeline(d: &dyn Device) {
    let ops = d.compute().expect("compute");
    let handle = ops
        .create_pipeline(input(), &BindingLayout::new(vec![]))
        .expect("pipeline");
    assert_eq!(handle.__backend_device_id(), d.generation());
}

#[test]
fn cpu_only_product_independent_compute_is_none() {
    assert_no_compute(&*device(
        Profile::CpuOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

#[test]
fn cpu_only_shared_compute_is_none() {
    assert_no_compute(&*device(Profile::CpuOnly, DeviceOwnership::Shared));
}

#[test]
fn cpu_only_imported_compute_is_none() {
    assert_no_compute(&*device(Profile::CpuOnly, imported()));
}

#[test]
fn compute_only_product_independent_compute_creates_pipeline() {
    assert_creates_pipeline(&*device(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    ));
}

#[test]
fn compute_only_shared_compute_creates_pipeline() {
    assert_creates_pipeline(&*device(Profile::ComputeOnly, DeviceOwnership::Shared));
}

#[test]
fn compute_only_imported_compute_creates_pipeline() {
    assert_creates_pipeline(&*device(Profile::ComputeOnly, imported()));
}

#[test]
fn graphics_product_independent_compute_creates_pipeline() {
    assert_creates_pipeline(&*device(
        Profile::Graphics,
        DeviceOwnership::ProductIndependent,
    ));
}

#[test]
fn graphics_shared_compute_creates_pipeline() {
    assert_creates_pipeline(&*device(Profile::Graphics, DeviceOwnership::Shared));
}

#[test]
fn graphics_imported_compute_creates_pipeline() {
    assert_creates_pipeline(&*device(Profile::Graphics, imported()));
}

#[test]
fn two_pipelines_from_one_device_have_distinct_ids() {
    let d = device(Profile::ComputeOnly, DeviceOwnership::Shared);
    let ops = d.compute().expect("compute");
    let layout = BindingLayout::new(vec![]);
    let a = ops.create_pipeline(input(), &layout).expect("first");
    let b = ops.create_pipeline(input(), &layout).expect("second");
    assert_ne!(a.__backend_id(), b.__backend_id());
    assert_eq!(a.__backend_device_id(), d.generation());
    assert_eq!(b.__backend_device_id(), d.generation());
}
