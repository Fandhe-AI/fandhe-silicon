//! Metal の背後実装で、模擬のロスト（feature `fault-injection` の差し込み）の後の `poll`・`poll_ex` の確認（TASK-593 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, CompletionStateEx, DeviceLostReason};
use fandhe_silicon_core::execution::SubmitToken;
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

/// CMD-88 のポインタ。待った後の自分の印は `Complete`、払い出していない番号は `Unknown` で、
/// 模擬のロストの後は 2 つの印の `poll_ex` がどちらも `Lost`、待った印の `poll` も `Lost` になる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn poll_and_poll_ex_are_lost_after_the_simulated_loss() {
    let device = new_device();
    let queue = device.queue();
    let issued = queue.submit(queue.create_encoder(), &[]).expect("submit");
    queue.wait_blocking(issued, None).expect("wait_blocking");
    let unissued = SubmitToken::__backend_new(
        issued.generation,
        issued.sequence + 98,
        issued.__backend_device_id(),
    );
    assert_eq!(queue.poll_ex(issued), CompletionStateEx::Complete);
    assert_eq!(queue.poll(issued), CompletionState::Complete);
    assert_eq!(queue.poll_ex(unissued), CompletionStateEx::Unknown);

    inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Unknown),
    )
    .expect("loss");

    assert_eq!(queue.poll_ex(issued), CompletionStateEx::Lost);
    assert_eq!(queue.poll(issued), CompletionState::Lost);
    assert_eq!(queue.poll_ex(unissued), CompletionStateEx::Lost);
}
