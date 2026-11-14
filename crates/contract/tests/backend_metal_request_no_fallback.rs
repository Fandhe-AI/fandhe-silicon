//! Metal の背後実装の `request` の、組み合わせの検査と失敗の伝播の確認（TASK-277 のポインタ）。
//!
//! device に触らない入口（検査で拒む要求と、既定の device の取得を `None` にする差し込みの入口）だけを使う。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use std::cell::Cell;

use fandhe_silicon_contract::backend_metal::{MetalDefaultDeviceNoneProvider, MetalDeviceProvider};
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;

const OTHER_BACKENDS: [BackendKind; 3] = [
    BackendKind::Vulkan,
    BackendKind::Cuda,
    BackendKind::Dx12Compute,
];

fn expect_err(r: Result<Box<dyn Device>, ContractError>) -> ContractError {
    match r {
        Ok(_) => panic!("request must fail"),
        Err(e) => e,
    }
}

fn request(backend: BackendKind, profile: Profile) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile,
        ownership: DeviceOwnership::ProductIndependent,
    }
}

/// ERR-1。Metal 以外の背後実装に `Graphics`・`CpuOnly` を求めても、背後実装の不一致で `InvalidCombination`。
#[test]
fn other_backends_with_graphics_or_cpu_only_are_invalid_combinations() {
    for backend in OTHER_BACKENDS {
        for profile in [Profile::Graphics, Profile::CpuOnly] {
            let real = expect_err(MetalDeviceProvider::request(request(backend, profile)));
            let injected = expect_err(MetalDefaultDeviceNoneProvider::request(request(
                backend, profile,
            )));
            assert!(
                matches!(real, ContractError::InvalidCombination(_)),
                "{backend:?} {profile:?}: {real:?}"
            );
            assert!(
                matches!(injected, ContractError::InvalidCombination(_)),
                "{backend:?} {profile:?}: {injected:?}"
            );
        }
    }
}

/// ERR-1。`graphics` のビルドでは、Metal の `Graphics` は検査を通り、device の取得まで進む（差し込みで `DeviceUnavailable`）。
#[cfg(feature = "graphics")]
#[test]
fn metal_graphics_passes_the_checks_in_a_graphics_build() {
    let e = expect_err(MetalDefaultDeviceNoneProvider::request(request(
        BackendKind::Metal,
        Profile::Graphics,
    )));
    assert!(matches!(e, ContractError::DeviceUnavailable(_)), "{e:?}");
}

thread_local! {
    /// 別の背後実装の provider（試験の中の偽物）の `request` が呼ばれた回数。試験ごとのスレッドで数える。
    static OTHER_REQUESTS: Cell<usize> = const { Cell::new(0) };
}

/// 別の背後実装を名乗る偽の provider。呼ばれた回数を数え、必ず失敗を返す。
struct CountingOtherProvider;

impl DeviceProvider for CountingOtherProvider {
    fn request(_req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        OTHER_REQUESTS.with(|n| n.set(n.get() + 1));
        Err(ContractError::DeviceUnavailable("counting provider".into()))
    }
}

fn other_requests() -> usize {
    OTHER_REQUESTS.with(Cell::get)
}

/// fail-fast の利用コードの模擬。`request` の `Err` をそのまま返す。
fn fail_fast<P: DeviceProvider>(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
    P::request(req)
}

/// 縮退の利用コードの模擬の結果。
enum Slot {
    Device,
    Unavailable,
}

/// 縮退の利用コードの模擬。`Err` を受けたら理由を記録し、`Unavailable` に倒す。
fn degrade<P: DeviceProvider>(req: DeviceRequest, events: &mut Vec<ContractError>) -> Slot {
    match P::request(req) {
        Ok(_) => Slot::Device,
        Err(e) => {
            events.push(e);
            Slot::Unavailable
        }
    }
}

/// ERR-57。差し込みの失敗と検査の失敗は、2 つの利用コードへ値のまま届き、別の背後実装の `request` は呼ばれない。
#[test]
fn failures_reach_both_policies_as_is_without_another_backend_request() {
    let unavailable = || request(BackendKind::Metal, Profile::ComputeOnly);
    let mismatch = || request(BackendKind::Vulkan, Profile::ComputeOnly);

    let e = expect_err(fail_fast::<MetalDefaultDeviceNoneProvider>(unavailable()));
    assert!(matches!(e, ContractError::DeviceUnavailable(_)), "{e:?}");
    let e = expect_err(fail_fast::<MetalDefaultDeviceNoneProvider>(mismatch()));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");
    let e = expect_err(fail_fast::<MetalDeviceProvider>(mismatch()));
    assert!(matches!(e, ContractError::InvalidCombination(_)), "{e:?}");

    let mut events = Vec::new();
    assert!(matches!(
        degrade::<MetalDefaultDeviceNoneProvider>(unavailable(), &mut events),
        Slot::Unavailable
    ));
    assert!(matches!(
        degrade::<MetalDeviceProvider>(mismatch(), &mut events),
        Slot::Unavailable
    ));
    assert!(matches!(events[0], ContractError::DeviceUnavailable(_)));
    assert!(matches!(events[1], ContractError::InvalidCombination(_)));
    assert_eq!(events.len(), 2);

    assert_eq!(other_requests(), 0);

    // 数える仕組みそのものの確かめ: 利用コードが自分で呼んだときだけ数が増える。
    let e = expect_err(fail_fast::<CountingOtherProvider>(request(
        BackendKind::Vulkan,
        Profile::ComputeOnly,
    )));
    assert!(matches!(e, ContractError::DeviceUnavailable(_)), "{e:?}");
    assert_eq!(other_requests(), 1);
}

/// ERR-57。Metal の背後実装のソースは、ほかの背後実装の provider と module を名指ししない。
#[test]
fn the_metal_source_names_no_other_backend_provider() {
    const SOURCE: &str = include_str!("../src/backend_metal.rs");
    for word in [
        "VulkanDeviceProvider",
        "CudaDeviceProvider",
        "backend_vulkan",
        "backend_cuda",
    ] {
        assert!(!SOURCE.contains(word), "{word}");
    }
}
