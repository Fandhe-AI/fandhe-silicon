//! CUDA の provider が `CpuOnly`・`Graphics` の要求を所有形態によらず拒むことを確かめる試験（TASK-169 のポインタ）。
//!
//! 検査で弾かれる入力だけを呼ぶので、device には触らない。
//! 末尾の `#[ignore]` の 2 本は device を作り、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, FeatureId};
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, NativeDeviceHandle, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::feature_key::FEATURE_QUERY_KEYS;

/// 3 つの所有形態。`Imported` が所有する値は複製できないので、呼び出しごとに作り直す。
fn ownerships() -> [DeviceOwnership; 3] {
    [
        DeviceOwnership::ProductIndependent,
        DeviceOwnership::Shared,
        DeviceOwnership::Imported {
            native: NativeDeviceHandle::new(Box::new(0_u8)),
        },
    ]
}

fn cuda(profile: Profile, ownership: DeviceOwnership) -> DeviceRequest {
    DeviceRequest {
        backend: BackendKind::Cuda,
        profile,
        ownership,
    }
}

/// `Box<dyn Device>` は `Debug` を持たないので、失敗側だけを取り出す。成功したら試験を落とす。
fn expect_err(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

// DEV-64
#[test]
fn cpu_only_is_an_invalid_combination_for_every_ownership() {
    for ownership in ownerships() {
        let e = expect_err(CudaDeviceProvider::request(cuda(
            Profile::CpuOnly,
            ownership,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}

// DEV-209
#[test]
fn graphics_is_an_invalid_combination_for_every_ownership() {
    for ownership in ownerships() {
        let e = expect_err(CudaDeviceProvider::request(cuda(
            Profile::Graphics,
            ownership,
        )));
        assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    }
}

/// 検査で弾かれる 2 つのプロファイル。
const REJECTED_PROFILES: [Profile; 2] = [Profile::CpuOnly, Profile::Graphics];

/// 鍵の一覧に無い ID（大文字・小文字・空白・綴りの違いと、一覧の外の ID）。
const UNLISTED_IDS: [FeatureId; 5] = [
    FeatureId("feature:shader_f16"),
    FeatureId("feature:SHADER_F16 "),
    FeatureId("feature:SUBGRUOP"),
    FeatureId("feature:not-a-feature"),
    FeatureId(""),
];

/// `ComputeOnly`・`ProductIndependent` で CUDA の device を開く。失敗したら試験を落とす。
fn open_compute_only() -> Box<dyn Device> {
    match CudaDeviceProvider::request(cuda(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    )) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

#[test]
#[ignore = "requires a CUDA device"]
fn rejected_profiles_are_invalid_combinations_where_compute_only_opens_a_device() {
    let device = open_compute_only();
    assert_eq!(device.backend(), BackendKind::Cuda);
    for profile in REJECTED_PROFILES {
        for ownership in ownerships() {
            let e = expect_err(CudaDeviceProvider::request(cuda(profile, ownership)));
            assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
        }
    }
}

// DEV-99
#[test]
#[ignore = "requires a CUDA device"]
fn unlisted_ids_are_unknown_features_on_a_cuda_device() {
    let device = open_compute_only();
    for id in UNLISTED_IDS {
        assert!(!FEATURE_QUERY_KEYS.contains(&id), "{id:?}");
        let q = device.query_ex(id);
        println!(
            "query: feature={:?} level={:?} reason={:?} evidence={:?}",
            id.0, q.level, q.unsupported_reason, q.evidence
        );
        assert_eq!(q, UNLISTED_FEATURE_QUERY, "{id:?}");
        assert_eq!(device.query(id), CapabilityLevel::Unsupported, "{id:?}");
    }
}
