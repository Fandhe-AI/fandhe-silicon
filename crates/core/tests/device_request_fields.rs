//! `DeviceRequest` の公開フィールドの集合と型を別の crate から固定する公開テスト（TASK-136 の 3/6）。
//!
//! 仕組み: `..` なしの分解で束縛する。フィールドが増えると（非公開のものでも）分解が通らずコンパイルが落ち、
//! 束縛した値の型注釈でフィールドの型を固定する（DEV-4。ポインタのみ）。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{DeviceOwnership, DeviceRequest, NativeDeviceHandle, Profile};

#[test]
fn device_request_has_exactly_three_fields() {
    let req = DeviceRequest {
        backend: BackendKind::Vulkan,
        profile: Profile::Graphics,
        ownership: DeviceOwnership::Shared,
    };
    let DeviceRequest {
        backend,
        profile,
        ownership,
    } = req;
    let _: BackendKind = backend;
    let _: Profile = profile;
    let _: DeviceOwnership = ownership;
}

fn ownership_label(o: &DeviceOwnership) -> &'static str {
    match o {
        DeviceOwnership::ProductIndependent => "ProductIndependent",
        DeviceOwnership::Shared => "Shared",
        DeviceOwnership::Imported { .. } => "Imported",
        _ => "other",
    }
}

#[test]
fn device_request_fields_read_back() {
    let cases = [
        (
            BackendKind::Metal,
            Profile::CpuOnly,
            DeviceOwnership::ProductIndependent,
            "ProductIndependent",
        ),
        (
            BackendKind::Cuda,
            Profile::ComputeOnly,
            DeviceOwnership::Shared,
            "Shared",
        ),
        (
            BackendKind::Vulkan,
            Profile::Graphics,
            DeviceOwnership::Imported {
                native: NativeDeviceHandle::new(1u8),
            },
            "Imported",
        ),
    ];
    for (b, p, o, label) in cases {
        let DeviceRequest {
            backend,
            profile,
            ownership,
        } = DeviceRequest {
            backend: b,
            profile: p,
            ownership: o,
        };
        assert_eq!(backend, b);
        assert_eq!(profile, p);
        assert_eq!(ownership_label(&ownership), label);
    }
}
