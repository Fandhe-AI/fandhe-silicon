//! 試験用の device を `Box<dyn Device>` 越しに使い、プロファイル × 所有形態の組ごとに確かめる統合テスト（TASK-137。DEV-15・DEV-59・DEV-60・DEV-63・DEV-66 のポインタ）。
//!
//! 組ごとに `#[test]` を 1 本置く。`Graphics` の 3 本は feature `graphics` の裏にあるので、
//! `graphics` の有無で流れる本数が変わる。
//! 共通の確かめには `compute()` の口（`CpuOnly` は `None`、それ以外は `create_pipeline` が `Ok`）も含み、feature `graphics` のビルドでは `graphics()` の口（`Graphics` だけ `Some` で `create_render_pipeline` が `Ok`）も含み、feature `artifact-inspection` のビルドでは `artifact_hooks()` の口（どのプロファイルでも `None`）も含む。試験用の device は core の feature `test-support` の裏にある。

use std::sync::{Arc, Mutex};

use fandhe_silicon_core::capability::{BackendKind, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{BindingLayout, CommandQueue, ComputeOps, PipelineInput};
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

    // compute の口は CpuOnly だけ無く、それ以外は入力を見ずにパイプラインを返す。
    if matches!(profile, Profile::CpuOnly) {
        assert!(device.compute().is_none());
    } else {
        let c: &dyn ComputeOps = device.compute().expect("compute");
        let p = c
            .create_pipeline(
                PipelineInput::Msl {
                    source: "",
                    entry_point: "main",
                },
                &BindingLayout::new(vec![]),
            )
            .expect("pipeline");
        assert_eq!(p.__backend_device_id(), device.generation());
    }

    // feature `graphics` のビルドでは描画の口も確かめる。CpuOnly・ComputeOnly は無く、Graphics は操作まで届く。
    #[cfg(feature = "graphics")]
    {
        use fandhe_silicon_core::graphics::GraphicsOps;

        if matches!(profile, Profile::Graphics) {
            let g: &dyn GraphicsOps = device.graphics().expect("graphics");
            let p = g
                .create_render_pipeline(
                    PipelineInput::Msl {
                        source: "",
                        entry_point: "vs",
                    },
                    PipelineInput::Msl {
                        source: "",
                        entry_point: "fs",
                    },
                    &BindingLayout::new(vec![]),
                )
                .expect("render pipeline");
            assert_eq!(p.__backend_device_id(), device.generation());
        } else {
            assert!(device.graphics().is_none());
        }
    }

    // feature `artifact-inspection` のビルドでは成果物の検査の口も確かめる。試験用の device は上書きしないので、どのプロファイルでも `None`。
    #[cfg(feature = "artifact-inspection")]
    {
        use fandhe_silicon_core::artifact::ArtifactHooks;

        let hooks: Option<&dyn ArtifactHooks> = device.artifact_hooks();
        assert!(hooks.is_none());
    }
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
