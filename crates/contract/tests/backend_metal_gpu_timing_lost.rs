//! Metal の背後実装で、模擬のロスト（feature `fault-injection` の差し込み）の後の `gpu_timing` の確認（TASK-596 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::DeviceLostReason;
use fandhe_silicon_core::execution::GpuTiming;
use fandhe_silicon_core::fault::{HookFault, inject};

fn new_device() -> Box<dyn Device> {
    match MetalDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

/// CMD-47 のポインタ。`wait_blocking(None)` の後に `Available` だった印は、模擬のロストの後は `Unavailable` で、
/// 続けて読んでも `Unavailable` のまま。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_is_unavailable_after_the_simulated_loss() {
    let device = new_device();
    let queue = device.queue();
    let token = queue.submit(queue.create_encoder(), &[]).expect("submit");
    queue.wait_blocking(token, None).expect("wait_blocking");
    assert!(
        matches!(queue.gpu_timing(token), GpuTiming::Available { .. }),
        "{:?}",
        queue.gpu_timing(token)
    );

    inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Unknown),
    )
    .expect("loss");

    assert_eq!(queue.gpu_timing(token), GpuTiming::Unavailable);
    assert_eq!(queue.gpu_timing(token), GpuTiming::Unavailable);
}
