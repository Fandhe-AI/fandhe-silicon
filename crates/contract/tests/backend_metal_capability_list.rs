//! Metal の背後実装の能力の一覧・形式単位・表示先の鍵・上限の確認（TASK-179 のポインタ）。
//!
//! device に触る試験は `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, CapabilityQuery, KeyList};
use fandhe_silicon_core::capability_rule::check_query_ex;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::feature_key::FEATURE_SURFACE;
use fandhe_silicon_core::texture_format::TextureFormat;
use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

fn new_device(profile: Profile) -> Box<dyn Device> {
    MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("request")
}

const LISTS: [KeyList; 4] = [
    KeyList::Features,
    KeyList::Downlevel,
    KeyList::Limits,
    KeyList::TextureFormats,
];

const SURFACE_NOT_SUPPORTED: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::Unsupported,
    unsupported_reason: Some(UnsupportedReason::NotSupported),
    evidence: None,
};

// DEV-101
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn query_list_matches_query_ex_for_every_list() {
    let device = new_device(Profile::ComputeOnly);
    for list in LISTS {
        let keys = list.keys();
        let rows = device.query_list(list);
        assert_eq!(rows.len(), keys.len(), "{list:?}");
        let mut s0 = 0usize;
        let mut unsupported = 0usize;
        for (&key, (row_key, row)) in keys.iter().zip(rows.iter()) {
            assert_eq!(*row_key, key, "{list:?}");
            assert_eq!(*row, device.query_ex(key), "{list:?} {key:?}");
            check_query_ex(device.as_ref(), key).expect("common rules");
            match row.level {
                CapabilityLevel::S0 => s0 += 1,
                CapabilityLevel::Unsupported => unsupported += 1,
                _ => {}
            }
        }
        println!(
            "list: {list:?} keys={} s0={s0} unsupported={unsupported}",
            keys.len()
        );
        assert_eq!(s0, 0, "{list:?}");
    }
}

// DEV-104
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn texture_format_unit_matches_the_format_key() {
    let device = new_device(Profile::ComputeOnly);
    let formats = TextureFormat::all();
    assert_eq!(formats.len(), TEXTURE_FORMAT_QUERY_KEYS.len());
    for (&format, &key) in formats.iter().zip(TEXTURE_FORMAT_QUERY_KEYS.iter()) {
        assert_eq!(
            device.texture_format_features(format).format_query(),
            device.query_ex(key),
            "{format:?}"
        );
    }
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn surface_key_is_not_supported_on_a_requested_device() {
    let mut profiles = vec![Profile::ComputeOnly];
    if cfg!(feature = "graphics") {
        profiles.push(Profile::Graphics);
    }
    for profile in profiles {
        let device = new_device(profile);
        let q = device.query_ex(FEATURE_SURFACE);
        println!(
            "surface: {profile:?} level={:?} reason={:?} evidence={:?}",
            q.level, q.unsupported_reason, q.evidence
        );
        assert_eq!(q, SURFACE_NOT_SUPPORTED, "{profile:?}");
        assert_eq!(
            device.query(FEATURE_SURFACE),
            CapabilityLevel::Unsupported,
            "{profile:?}"
        );
    }
}

// DEV-111
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn limits_match_the_native_values_and_print_them() {
    let device = new_device(Profile::ComputeOnly);
    let limits = *device.limits();
    let native_max = MTLCreateSystemDefaultDevice()
        .expect("the default Metal device must exist in this session")
        .maxBufferLength();
    println!(
        "limits: max_buffer_bytes={} max_dispatch_dims={:?} max_texture_size={} native_max_buffer_length={native_max}",
        limits.max_buffer_bytes, limits.max_dispatch_dims, limits.max_texture_size
    );
    assert_eq!(limits.max_buffer_bytes, u64::try_from(native_max).unwrap());
    assert_eq!(limits.max_dispatch_dims, [u32::MAX; 3]);
    assert_eq!(limits.max_texture_size, 16384);
}
