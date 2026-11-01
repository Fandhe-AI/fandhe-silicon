//! 試験用の device を `Box<dyn Device>` 越しに使い、プロファイル × 所有形態の組ごとに確かめる統合テスト（TASK-137。DEV-15・DEV-59・DEV-66 のポインタ）。
//!
//! 組ごとに `#[test]` を 1 本置く。`Graphics` の 3 本は feature `graphics` の裏にあるので、
//! `graphics` の有無で流れる本数が変わる。試験用の device は core の feature `test-support` の裏にある。

use std::sync::{Arc, Mutex};

use fandhe_silicon_core::capability::{BackendKind, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::CommandQueue;
use fandhe_silicon_core::memory::{BufferUsage, MemoryOps};
use fandhe_silicon_core::stub_device::StubDeviceProvider;

/// 要求する所有形態の種類。`Imported` は組ごとに作る値を持つ。
#[derive(Clone, Copy)]
enum Kind {
    ProductIndependent,
    Shared,
    Imported(u32),
}

type Downloaded = Vec<Result<Vec<u8>, ContractError>>;

/// 1 つの組について、要求から所有形態・メモリ・投入・能力の問合せまでを通す。
fn check(profile: Profile, kind: Kind) {
    let ownership = match kind {
        Kind::ProductIndependent => DeviceOwnership::ProductIndependent,
        Kind::Shared => DeviceOwnership::Shared,
        Kind::Imported(tag) => DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(tag),
        },
    };
    let device: Box<dyn Device> = StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile,
        ownership,
    })
    .expect("device");

    match (kind, device.ownership()) {
        (Kind::ProductIndependent, DeviceOwnership::ProductIndependent) => {}
        (Kind::Shared, DeviceOwnership::Shared) => {}
        (Kind::Imported(tag), DeviceOwnership::Imported { native }) => {
            assert_eq!(native.__backend_downcast_ref::<u32>(), Some(&tag));
        }
        _ => panic!("ownership does not match the request"),
    }

    let m: &dyn MemoryOps = device.memory();
    let q: &dyn CommandQueue = device.queue();

    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    let h = m.allocate(4, usage).expect("allocate");
    m.upload(&h, 0, &[1, 2, 3, 4]).expect("upload");
    let slot: Arc<Mutex<Downloaded>> = Arc::default();
    let sink = Arc::clone(&slot);
    m.download_async(
        &h,
        0,
        4,
        Box::new(move |r| sink.lock().expect("lock").push(r)),
    );
    {
        let mut got = slot.lock().expect("lock");
        assert_eq!(got.len(), 1);
        assert_eq!(got.remove(0).expect("download"), vec![1, 2, 3, 4]);
    }
    m.free(h);

    let token = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(q.poll(token), CompletionState::Complete);

    let _level = device.query(FeatureId("any-feature"));
    let _limits = device.limits();
}

#[test]
fn cpu_only_product_independent() {
    check(Profile::CpuOnly, Kind::ProductIndependent);
}

#[test]
fn cpu_only_shared() {
    check(Profile::CpuOnly, Kind::Shared);
}

#[test]
fn cpu_only_imported() {
    check(Profile::CpuOnly, Kind::Imported(11));
}

#[test]
fn compute_only_product_independent() {
    check(Profile::ComputeOnly, Kind::ProductIndependent);
}

#[test]
fn compute_only_shared() {
    check(Profile::ComputeOnly, Kind::Shared);
}

#[test]
fn compute_only_imported() {
    check(Profile::ComputeOnly, Kind::Imported(22));
}

#[cfg(feature = "graphics")]
#[test]
fn graphics_product_independent() {
    check(Profile::Graphics, Kind::ProductIndependent);
}

#[cfg(feature = "graphics")]
#[test]
fn graphics_shared() {
    check(Profile::Graphics, Kind::Shared);
}

#[cfg(feature = "graphics")]
#[test]
fn graphics_imported() {
    check(Profile::Graphics, Kind::Imported(33));
}
