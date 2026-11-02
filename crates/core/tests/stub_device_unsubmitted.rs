//! feature `test-support` の裏の試験用の device が、印（`SubmitToken`）を `submit` の戻り値でだけ払い出すことを確かめる統合テスト（TASK-313）。
//!
//! 記録先を投入せずに落とした場合と、拒まれた投入の場合に、印の通し番号が進まないことを公開のパスだけで確かめる。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::CompletionState;
use fandhe_silicon_core::execution::ImportedSemaphoreHandle;
use fandhe_silicon_core::stub_device::StubDeviceProvider;

fn device() -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Vulkan,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::Shared,
    })
    .expect("device")
}

#[test]
fn submit_returns_token() {
    let d = device();
    let q = d.queue();
    let token = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(q.poll(token), CompletionState::Complete);
}

#[test]
fn dropped_encoder_issues_no_token() {
    let d = device();
    let q = d.queue();
    let t1 = q.submit(q.create_encoder(), &[]).expect("submit");
    let unsubmitted = q.create_encoder();
    drop(unsubmitted);
    let t2 = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(t2.sequence, t1.sequence + 1);
}

#[test]
fn rejected_submit_issues_no_token() {
    let d = device();
    let q = d.queue();
    let t1 = q.submit(q.create_encoder(), &[]).expect("submit");
    assert!(q.submit_many(vec![], &[], &[]).is_err());
    let wait = [ImportedSemaphoreHandle::__backend_new(1)];
    assert!(q.submit(q.create_encoder(), &wait).is_err());
    let t2 = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(t2.sequence, t1.sequence + 1);
}
