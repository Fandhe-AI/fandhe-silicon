//! feature `test-support` の裏の試験用の device の、投入の口の振る舞いを確かめる統合テスト（TASK-156 の 3/5。DEV-34 のポインタ）。
//!
//! 記録先の作成・投入・完了の問合せ・dispatch の拒否を、公開のパスだけで確かめる。

use std::sync::{Arc, Mutex};

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    CommandEncoder, CommandQueue, ImportedSemaphoreHandle, PipelineHandle, SubmitToken,
};
use fandhe_silicon_core::stub_device::StubDeviceProvider;

fn device_for(backend: BackendKind) -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::Shared,
    })
    .expect("device")
}

fn device() -> Box<dyn Device> {
    device_for(BackendKind::Vulkan)
}

fn is_invalid<T: std::fmt::Debug>(r: Result<T, ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

/// 別の device で払い出した印を作る。
fn foreign_token() -> SubmitToken {
    let other = device();
    other
        .queue()
        .submit(other.queue().create_encoder(), &[])
        .expect("submit")
}

/// `on_complete` を流し、callback に届いた状態の列を返す。
fn on_complete_states(q: &dyn CommandQueue, token: SubmitToken) -> Vec<CompletionState> {
    let slot: Arc<Mutex<Vec<CompletionState>>> = Arc::default();
    let sink = Arc::clone(&slot);
    q.on_complete(token, Box::new(move |s| sink.lock().expect("lock").push(s)));
    slot.lock().expect("lock").clone()
}

#[test]
fn submit_then_poll_is_complete() {
    let d = device();
    let q = d.queue();
    let token = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(q.poll(token), CompletionState::Complete);
    assert_eq!(token.generation, d.generation());
}

#[test]
fn on_complete_reports_complete_once() {
    let d = device();
    let q = d.queue();
    let token = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(
        on_complete_states(q, token),
        vec![CompletionState::Complete]
    );
    assert_eq!(
        on_complete_states(q, foreign_token()),
        vec![CompletionState::Lost]
    );
}

#[test]
fn submit_many_empty_is_invalid_argument() {
    let d = device();
    assert!(is_invalid(d.queue().submit_many(vec![], &[], &[])));
}

#[test]
fn submit_with_wait_is_unsupported() {
    let d = device_for(BackendKind::Cuda);
    let q = d.queue();
    let wait = [ImportedSemaphoreHandle::__backend_new(1)];
    assert!(matches!(
        q.submit(q.create_encoder(), &wait),
        Err(ContractError::Unsupported {
            backend: BackendKind::Cuda,
            ..
        })
    ));
    assert!(matches!(
        q.submit_many(vec![q.create_encoder()], &[], &wait),
        Err(ContractError::Unsupported {
            backend: BackendKind::Cuda,
            ..
        })
    ));
}

#[test]
fn submit_rejects_foreign_submitted_and_zero_encoders() {
    let d = device();
    let q = d.queue();
    let other = device();
    // 別の device の記録先
    assert!(is_invalid(q.submit(other.queue().create_encoder(), &[])));
    // 投入済みの id を偽造した記録先
    let enc = q.create_encoder();
    let id = enc.__backend_id();
    q.submit(enc, &[]).expect("submit");
    assert!(is_invalid(
        q.submit(CommandEncoder::__backend_new(id, None), &[])
    ));
    // id 0 の記録先
    assert!(is_invalid(
        q.submit(CommandEncoder::__backend_new(0, None), &[])
    ));
    // 拒否の後も新しい記録先で投入できる。
    q.submit(q.create_encoder(), &[])
        .expect("submit after rejection");
}

#[test]
fn dropped_encoder_does_not_block_next_submit() {
    let d = device();
    let q = d.queue();
    drop(q.create_encoder());
    q.submit(q.create_encoder(), &[]).expect("submit");
}

#[test]
fn submit_many_with_foreign_after_is_device_mismatch() {
    let d = device();
    let q = d.queue();
    assert!(matches!(
        q.submit_many(vec![q.create_encoder()], &[foreign_token()], &[]),
        Err(ContractError::DeviceMismatch)
    ));
}

#[test]
fn poll_reports_lost_for_foreign_or_unissued_tokens() {
    let d = device();
    let q = d.queue();
    let token = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(q.poll(foreign_token()), CompletionState::Lost);
    let mut zero = token;
    zero.sequence = 0;
    let mut unissued = token;
    unissued.sequence = token.sequence + 1;
    let mut other_generation = token;
    other_generation.generation = token.generation + 1;
    for forged in [zero, unissued, other_generation] {
        assert_eq!(q.poll(forged), CompletionState::Lost);
    }
}

#[test]
fn dispatch_ex_and_dispatch_are_unsupported() {
    for backend in [
        BackendKind::Metal,
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        let d = device_for(backend);
        let q = d.queue();
        let mut enc = q.create_encoder();
        let pipeline = PipelineHandle::__backend_new(1, 1);
        let expected = Err(ContractError::Unsupported {
            op: "dispatch_ex",
            backend,
        });
        assert_eq!(
            q.dispatch_ex(&mut enc, &pipeline, &[], &[], [1, 1, 1], [1, 1, 1], &[]),
            expected
        );
        assert_eq!(q.dispatch(&mut enc, &pipeline, &[], [1, 1, 1]), expected);
    }
}

#[cfg(feature = "native-blocking-wait")]
#[test]
fn wait_blocking_returns_ok_for_issued_token() {
    let d = device();
    let q = d.queue();
    let token = q.submit(q.create_encoder(), &[]).expect("submit");
    assert_eq!(q.wait_blocking(token, None), Ok(()));
}
