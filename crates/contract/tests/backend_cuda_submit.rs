//! CUDA の device で投入の窓口を呼ぶ試験（TASK-604 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{CompletionState, ContractError, RetryPolicy};
use fandhe_silicon_core::execution::{CommandEncoder, ImportedSemaphoreHandle, SubmitToken};
use std::time::{Duration, Instant};

fn new_device() -> Box<dyn Device> {
    match CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    }) {
        Ok(d) => d,
        Err(e) => panic!("request failed: {e:?}"),
    }
}

/// `SubmitToken` は `Clone` を持たないので、同じ値で作り直した写しを返す。
fn copy_of(token: &SubmitToken) -> SubmitToken {
    SubmitToken::__backend_new(
        token.generation,
        token.sequence,
        token.__backend_device_id(),
    )
}

/// `poll` を 1 ms 間隔で繰り返し、`Complete` になるまで待つ。10 秒で失敗とする。
fn poll_until_complete(device: &dyn Device, token: &SubmitToken) {
    let start = Instant::now();
    loop {
        match device.queue().poll(copy_of(token)) {
            CompletionState::Complete => return,
            CompletionState::Pending => {}
            other => panic!("unexpected state: {other:?}"),
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "submission did not complete in time"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn submit_empty(device: &dyn Device) -> SubmitToken {
    let queue = device.queue();
    match queue.submit(queue.create_encoder(), &[]) {
        Ok(t) => t,
        Err(e) => panic!("submit failed: {e:?}"),
    }
}

// CMD-87
#[test]
#[ignore = "requires a CUDA device"]
fn submit_returns_a_token_of_the_device_generation_without_waiting() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    assert_eq!(token.generation, device.generation());
    let after = device.queue().poll(copy_of(&token));
    assert!(matches!(
        after,
        CompletionState::Pending | CompletionState::Complete
    ));
    println!(
        "first_sequence={} poll_after_submit={after:?}",
        token.sequence
    );
    poll_until_complete(device.as_ref(), &token);
}

// CMD-1
#[test]
#[ignore = "requires a CUDA device"]
fn creating_and_dropping_encoders_submits_nothing() {
    let device = new_device();
    let queue = device.queue();
    drop(queue.create_encoder());
    drop(queue.create_encoder());
    let token = submit_empty(device.as_ref());
    assert_eq!(token.sequence, 1);
    poll_until_complete(device.as_ref(), &token);
}

// CMD-12
#[test]
#[ignore = "requires a CUDA device"]
fn submit_ex_with_an_empty_wait_returns_a_token_that_completes() {
    let device = new_device();
    let queue = device.queue();
    let token = match queue.submit_ex(queue.create_encoder(), &[], RetryPolicy::IdempotentSafe) {
        Ok(t) => t,
        Err(e) => panic!("submit_ex failed: {e:?}"),
    };
    assert_eq!(token.generation, device.generation());
    poll_until_complete(device.as_ref(), &token);
}

// CMD-11
#[test]
#[ignore = "requires a CUDA device"]
fn foreign_encoders_and_a_nonempty_wait_are_rejected_without_consuming_a_sequence() {
    let device = new_device();
    let other = new_device();
    let queue = device.queue();
    let foreign: CommandEncoder = other.queue().create_encoder();
    assert!(matches!(
        queue.submit(foreign, &[]),
        Err(ContractError::InvalidArgument(_))
    ));
    assert!(matches!(
        queue.submit(
            queue.create_encoder(),
            &[ImportedSemaphoreHandle::__backend_new(1)]
        ),
        Err(ContractError::Unsupported { .. })
    ));
    let token = submit_empty(device.as_ref());
    assert_eq!(token.sequence, 1);
    poll_until_complete(device.as_ref(), &token);
}

// CMD-67
#[test]
#[ignore = "requires a CUDA device"]
fn submit_many_returns_one_token_for_the_whole_list() {
    let device = new_device();
    let queue = device.queue();
    let first = submit_empty(device.as_ref());
    let list = vec![
        queue.create_encoder(),
        queue.create_encoder(),
        queue.create_encoder(),
    ];
    let first_sequence = first.sequence;
    let many = match queue.submit_many(list, &[first], &[]) {
        Ok(t) => t,
        Err(e) => panic!("submit_many failed: {e:?}"),
    };
    assert_eq!(many.sequence, first_sequence + 1);
    poll_until_complete(device.as_ref(), &many);
}

#[test]
#[ignore = "requires a CUDA device"]
fn submit_many_rejects_bad_lists_without_consuming_a_sequence() {
    let device = new_device();
    let other = new_device();
    let queue = device.queue();
    assert!(matches!(
        queue.submit_many(Vec::new(), &[], &[]),
        Err(ContractError::InvalidArgument(_))
    ));
    let foreign_token = submit_empty(other.as_ref());
    assert!(matches!(
        queue.submit_many(vec![queue.create_encoder()], &[foreign_token], &[]),
        Err(ContractError::DeviceMismatch)
    ));
    let list = vec![queue.create_encoder(), other.queue().create_encoder()];
    assert!(matches!(
        queue.submit_many(list, &[], &[]),
        Err(ContractError::InvalidArgument(_))
    ));
    let token = submit_empty(device.as_ref());
    assert_eq!(token.sequence, 1);
    poll_until_complete(device.as_ref(), &token);
}

#[test]
#[ignore = "requires a CUDA device"]
fn poll_of_a_foreign_or_unissued_token_is_lost() {
    let device = new_device();
    let other = new_device();
    let ta = submit_empty(device.as_ref());
    let tb = submit_empty(other.as_ref());
    assert_eq!(device.queue().poll(tb), CompletionState::Lost);
    let unissued =
        SubmitToken::__backend_new(ta.generation, ta.sequence + 1, ta.__backend_device_id());
    assert_eq!(device.queue().poll(unissued), CompletionState::Lost);
}
