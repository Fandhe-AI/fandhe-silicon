//! Metal の device を実際に作り、診断・能力・メモリ操作を呼ぶ確認（TASK-158 のポインタ。
//! DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211・TASK-265。バッファの照合は MEM-79・MEM-80 のポインタ）。
//!
//! どの試験も Metal の device に触るので `#[ignore]` を付け、専用の実機の回でだけ流す。
//! macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel, CapabilityQuery, FeatureId};
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, UnsupportedReason,
};
use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

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

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes() {
    let device = new_device();
    assert_eq!(device.backend(), BackendKind::Metal);
    assert!(matches!(
        device.ownership(),
        DeviceOwnership::ProductIndependent
    ));
    assert!(device.generation() >= 1);

    let diag = device.diagnostics();
    assert_eq!(diag.backend_name, "metal");
    assert!(!diag.adapter_name.is_empty());
    let expected_name = MTLCreateSystemDefaultDevice()
        .expect("the default Metal device must exist in this session")
        .name()
        .to_string();
    assert_eq!(diag.adapter_name, expected_name);
    // 実機の回の記録に残すため、得た値を標準出力に出す（`--show-output` で見える。この出力は照合に使わない）。
    println!("adapter_name={}", diag.adapter_name);

    let feature = FeatureId("feature:any");
    assert_eq!(device.query(feature), CapabilityLevel::Unsupported);
    assert_eq!(
        device.query_ex(feature),
        CapabilityQuery {
            level: CapabilityLevel::Unsupported,
            unsupported_reason: Some(UnsupportedReason::UnknownFeature),
            evidence: None,
        }
    );
    assert!(device.limits().max_buffer_bytes > 0);

    let memory = device.memory();
    let payload: Vec<u8> = (0..64_u8).collect();
    let handle = memory
        .allocate(
            64,
            BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD,
        )
        .expect("allocate");
    assert_eq!(handle.size(), 64);
    assert_eq!(
        handle.usage(),
        BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
    );
    memory.upload(&handle, 0, &payload).expect("upload");
    let back = memory.download_blocking(&handle, 0, 64).expect("download");
    assert_eq!(back, payload);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn two_devices_do_not_accept_each_others_buffers() {
    let first = new_device();
    let second = new_device();
    assert!(second.generation() > first.generation());

    let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
    let from_first = first.memory().allocate(16, usage).expect("allocate first");
    let from_second = second
        .memory()
        .allocate(16, usage)
        .expect("allocate second");
    assert_ne!(
        from_first.__backend_device_id(),
        from_second.__backend_device_id()
    );
    // 各 device の最初のバッファは同じ識別子を持つ（取り違えが最も起きやすい組）。
    assert_eq!(from_first.__backend_id(), from_second.__backend_id());

    second
        .memory()
        .upload(&from_second, 0, &[0xAA; 16])
        .expect("upload second");

    let up = second.memory().upload(&from_first, 0, &[0xFF; 16]);
    assert_eq!(up, Err(ContractError::DeviceMismatch));
    let down = second.memory().download_blocking(&from_first, 0, 16);
    assert_eq!(down, Err(ContractError::DeviceMismatch));

    // 2 つ目の device の同じ識別子の buffer は変わっていない。
    let kept = second
        .memory()
        .download_blocking(&from_second, 0, 16)
        .expect("download second");
    assert_eq!(kept, vec![0xAA; 16]);

    first.memory().free(from_first);
    second.memory().free(from_second);
}

fn usage() -> BufferUsage {
    BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

/// `BufferHandle` は `Clone` を持たないので、同じ識別子・世代・device の識別子で作り直した写しを返す。
fn forged_copy(h: &BufferHandle) -> BufferHandle {
    BufferHandle::__backend_new(
        h.__backend_generation(),
        h.__backend_id(),
        h.__backend_device_id(),
        h.size(),
        h.usage(),
    )
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn upload_to_a_freed_handle_is_rejected_as_invalid_argument() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    let stale = forged_copy(&handle);
    memory.free(handle);

    let up = memory.upload(&stale, 0, &[1; 16]);
    assert!(
        matches!(up, Err(ContractError::InvalidArgument(_))),
        "{up:?}"
    );
    let down = memory.download_blocking(&stale, 0, 16);
    assert!(
        matches!(down, Err(ContractError::InvalidArgument(_))),
        "{down:?}"
    );
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_async_of_a_freed_handle_calls_back_once_with_invalid_argument() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    let stale = forged_copy(&handle);
    memory.free(handle);

    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(Mutex::new(None));
    let (c, r) = (Arc::clone(&calls), Arc::clone(&received));
    memory.download_async(
        &stale,
        0,
        16,
        Box::new(move |result| {
            c.fetch_add(1, Ordering::SeqCst);
            *r.lock().unwrap() = Some(result);
        }),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let got = received.lock().unwrap().take().expect("callback ran");
    assert!(
        matches!(got, Err(ContractError::InvalidArgument(_))),
        "{got:?}"
    );
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn freeing_another_devices_handle_keeps_the_buffer_with_the_same_id() {
    let first = new_device();
    let second = new_device();
    let from_first = first
        .memory()
        .allocate(16, usage())
        .expect("allocate first");
    let from_second = second
        .memory()
        .allocate(16, usage())
        .expect("allocate second");
    assert_eq!(from_first.__backend_id(), from_second.__backend_id());
    second
        .memory()
        .upload(&from_second, 0, &[0x5A; 16])
        .expect("upload second");

    // 別の device のハンドルを渡しても、自 device の同じ識別子のバッファは解放されない。
    second.memory().free(forged_copy(&from_first));

    let kept = second
        .memory()
        .download_blocking(&from_second, 0, 16)
        .expect("download second");
    assert_eq!(kept, vec![0x5A; 16]);
    // 払い出し元の device のバッファも残っている。
    first
        .memory()
        .download_blocking(&from_first, 0, 16)
        .expect("download first");

    first.memory().free(from_first);
    second.memory().free(from_second);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn out_of_range_upload_fails_and_leaves_the_contents_unchanged() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0x11; 16]).expect("upload");

    let past_end = memory.upload(&handle, 8, &[0xEE; 16]);
    assert!(
        matches!(past_end, Err(ContractError::InvalidArgument(_))),
        "{past_end:?}"
    );
    let offset_past_end = memory.upload(&handle, 17, &[]);
    assert!(
        matches!(offset_past_end, Err(ContractError::InvalidArgument(_))),
        "{offset_past_end:?}"
    );

    let back = memory.download_blocking(&handle, 0, 16).expect("download");
    assert_eq!(back, vec![0x11; 16]);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn upload_with_an_overflowing_end_fails_and_leaves_the_contents_unchanged() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0x22; 16]).expect("upload");

    for offset in [u64::MAX, u64::MAX - 7, u64::MAX - 15] {
        let r = memory.upload(&handle, offset, &[0xEE; 16]);
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "offset={offset}: {r:?}"
        );
    }
    let r = memory.upload(&handle, 16, &[0xEE]);
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");

    let back = memory.download_blocking(&handle, 0, 16).expect("download");
    assert_eq!(back, vec![0x22; 16]);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn upload_that_ends_exactly_at_the_allocation_is_accepted() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0; 16]).expect("zero fill");
    memory.upload(&handle, 12, &[1, 2, 3, 4]).expect("tail");
    memory.upload(&handle, 16, &[]).expect("empty at the end");
    let back = memory.download_blocking(&handle, 0, 16).expect("download");
    let mut expected = vec![0_u8; 16];
    expected[12..].copy_from_slice(&[1, 2, 3, 4]);
    assert_eq!(back, expected);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_blocking_rejects_out_of_range_and_overflowing_ranges() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0x33; 16]).expect("upload");

    let cases = [
        (8_u64, 9_u64),
        (0, 17),
        (17, 0),
        (u64::MAX, 1),
        (u64::MAX, 0),
        (1, u64::MAX),
        (u64::MAX, u64::MAX),
    ];
    for (offset, len) in cases {
        let r = memory.download_blocking(&handle, offset, len);
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "offset={offset} len={len}: {r:?}"
        );
    }
    // 拒否のあとでも内容は変わらない。
    let back = memory.download_blocking(&handle, 0, 16).expect("download");
    assert_eq!(back, vec![0x33; 16]);
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_blocking_of_a_sub_range_matches_the_cpu_reference() {
    let device = new_device();
    let memory = device.memory();
    let payload: Vec<u8> = (0..32_u8)
        .map(|b| b.wrapping_mul(7).wrapping_add(3))
        .collect();
    let handle = memory.allocate(32, usage()).expect("allocate");
    memory.upload(&handle, 0, &payload).expect("upload");

    for (offset, len) in [(0_usize, 32_usize), (5, 11), (31, 1), (32, 0), (16, 16)] {
        let got = memory
            .download_blocking(&handle, offset as u64, len as u64)
            .expect("download");
        assert_eq!(
            got,
            payload[offset..offset + len],
            "offset={offset} len={len}"
        );
    }
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_async_rejects_an_out_of_range_read_with_one_callback() {
    let device = new_device();
    let memory = device.memory();
    let handle = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&handle, 0, &[0x44; 16]).expect("upload");

    for (offset, len) in [(8_u64, 9_u64), (u64::MAX, 1), (1, u64::MAX)] {
        let calls = Arc::new(AtomicUsize::new(0));
        let received = Arc::new(Mutex::new(None));
        let (c, r) = (Arc::clone(&calls), Arc::clone(&received));
        memory.download_async(
            &handle,
            offset,
            len,
            Box::new(move |result| {
                c.fetch_add(1, Ordering::SeqCst);
                *r.lock().unwrap() = Some(result);
            }),
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1, "offset={offset} len={len}");
        let got = received.lock().unwrap().take().expect("callback ran");
        assert!(
            matches!(got, Err(ContractError::InvalidArgument(_))),
            "offset={offset} len={len}: {got:?}"
        );
    }
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn download_async_calls_back_once_with_the_same_bytes() {
    let device = new_device();
    let memory = device.memory();
    let payload: Vec<u8> = (0..32_u8).collect();
    let handle = memory.allocate(32, usage()).expect("allocate");
    memory.upload(&handle, 0, &payload).expect("upload");

    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(Mutex::new(None));
    let (c, r) = (Arc::clone(&calls), Arc::clone(&received));
    memory.download_async(
        &handle,
        0,
        32,
        Box::new(move |result| {
            c.fetch_add(1, Ordering::SeqCst);
            *r.lock().unwrap() = Some(result);
        }),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let got = received.lock().unwrap().take().expect("callback ran");
    assert_eq!(got, Ok(payload));
    memory.free(handle);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn allocate_past_the_buffer_limit_is_invalid_argument() {
    let device = new_device();
    let limit = device.limits().max_buffer_bytes;
    // 上限が u64 の最大なら、それを超える大きさは作れない。あふれさせずに抜ける。
    let Some(past_limit) = limit.checked_add(1) else {
        return;
    };
    let r = device.memory().allocate(past_limit, usage());
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn allocate_of_zero_bytes_is_invalid_argument() {
    let device = new_device();
    let r = device.memory().allocate(0, usage());
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn limits_report_the_dispatch_and_texture_bounds() {
    let device = new_device();
    let limits = device.limits();
    assert_eq!(limits.max_dispatch_dims, [u32::MAX; 3]);
    assert_eq!(limits.max_texture_size, 16384);
    let native_max = MTLCreateSystemDefaultDevice()
        .expect("the default Metal device must exist in this session")
        .maxBufferLength();
    assert_eq!(limits.max_buffer_bytes, u64::try_from(native_max).unwrap());
}

/// 投入の試験が使う、最初の通し番号を確かめる補助。空の記録先を投入して印を返す。
fn submit_empty(device: &dyn Device) -> fandhe_silicon_core::execution::SubmitToken {
    let queue = device.queue();
    queue.submit(queue.create_encoder(), &[]).expect("submit")
}

/// CMD-1 のポインタ。記録先を作る・破棄するだけでは投入されず、最初の投入の通し番号は 1 のまま。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn creating_and_dropping_encoders_does_not_advance_the_sequence() {
    let device = new_device();
    let queue = device.queue();
    drop(queue.create_encoder());
    drop(queue.create_encoder());
    let token = submit_empty(device.as_ref());
    assert_eq!(token.sequence, 1);
}

/// CMD-11 のポインタ。空の `wait` の投入は成功し、`wait_blocking(None)` の後の `poll` は `Complete`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn submit_with_empty_wait_completes_after_wait_blocking() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    queue.wait_blocking(token, None).expect("wait_blocking");
    assert_eq!(queue.poll(token), CompletionState::Complete);
}

/// CMD-11 のポインタ。`Some(期限)` の待ちも完了で `Ok` を返す。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn wait_blocking_with_a_timeout_returns_ok_when_the_work_completes() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    queue
        .wait_blocking(token, Some(std::time::Duration::from_secs(10)))
        .expect("wait_blocking");
    assert_eq!(queue.poll(token), CompletionState::Complete);
}

/// CMD-12 のポインタ。空でない `wait` は、`submit`・`submit_ex` のどちらでも `Unsupported` で何も投入しない。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn non_empty_wait_is_unsupported_and_submits_nothing() {
    let device = new_device();
    let queue = device.queue();
    let r = queue.submit(
        queue.create_encoder(),
        &[ImportedSemaphoreHandle::__backend_new(1)],
    );
    assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
    let r = queue.submit_ex(
        queue.create_encoder(),
        &[ImportedSemaphoreHandle::__backend_new(1)],
        fandhe_silicon_core::error::RetryPolicy::NotIdempotent,
    );
    assert!(matches!(r, Err(ContractError::Unsupported { .. })), "{r:?}");
    assert_eq!(submit_empty(device.as_ref()).sequence, 1);
}

/// CMD-12 のポインタ。別 device の記録先は `InvalidArgument` で投入されず、元の device でも投入できる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn foreign_encoder_is_invalid_argument_and_submits_nothing() {
    let first = new_device();
    let second = new_device();
    let foreign = first.queue().create_encoder();
    let r = second.queue().submit(foreign, &[]);
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
    assert_eq!(submit_empty(second.as_ref()).sequence, 1);
    assert_eq!(submit_empty(first.as_ref()).sequence, 1);
}

/// CMD-13 のポインタ。通し番号は device ごとに 1 から数え、2 つの device の最初はどちらも 1。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn sequences_start_at_one_per_device() {
    let first = new_device();
    let second = new_device();
    let a = submit_empty(first.as_ref());
    let b = submit_empty(second.as_ref());
    assert_eq!(a.sequence, 1);
    assert_eq!(b.sequence, 1);
    assert_eq!(submit_empty(first.as_ref()).sequence, 2);
    // 別 device の印は、通し番号が同じでも `poll` では `Lost`。
    assert_eq!(second.queue().poll(a), CompletionState::Lost);
}

/// CMD-87 のポインタ。印の世代は device の世代と一致し、投入直後の `poll` は `Pending` か `Complete`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn token_generation_matches_the_device_and_poll_is_not_lost() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    assert_eq!(token.generation, device.generation());
    let state = device.queue().poll(token);
    assert!(
        matches!(state, CompletionState::Pending | CompletionState::Complete),
        "{state:?}"
    );
}

/// CMD-67 のポインタ。`submit_many` は印を 1 つ返し、完了まで待てる。空の列は `InvalidArgument`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn submit_many_returns_one_token_and_rejects_an_empty_list() {
    let device = new_device();
    let queue = device.queue();
    let first = submit_empty(device.as_ref());
    let commands = vec![
        queue.create_encoder(),
        queue.create_encoder(),
        queue.create_encoder(),
    ];
    let token = queue
        .submit_many(commands, &[first], &[])
        .expect("submit_many");
    assert_eq!(token.sequence, 2);
    queue.wait_blocking(token, None).expect("wait_blocking");
    assert_eq!(queue.poll(token), CompletionState::Complete);
    // 先の印も回収済みで `Complete`。
    assert_eq!(queue.poll(first), CompletionState::Complete);

    let r = queue.submit_many(Vec::new(), &[], &[]);
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
    // 失敗した呼び出しは通し番号を進めない。
    assert_eq!(submit_empty(device.as_ref()).sequence, 3);
}

/// CMD-67 のポインタ。別 device の印を `after` に渡すと `DeviceMismatch` で、何も投入しない。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn submit_many_rejects_an_after_token_of_another_device() {
    let first = new_device();
    let second = new_device();
    let foreign = submit_empty(first.as_ref());
    let queue = second.queue();
    let r = queue.submit_many(vec![queue.create_encoder()], &[foreign], &[]);
    assert_eq!(r, Err(ContractError::DeviceMismatch));
    assert_eq!(submit_empty(second.as_ref()).sequence, 1);
}

/// `on_complete` は 1 回だけ、その場で `poll` の値を渡す。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn on_complete_calls_back_once_with_the_polled_state() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    device.queue().wait_blocking(token, None).expect("wait");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    device.queue().on_complete(
        token,
        Box::new(move |state| sink.lock().expect("lock").push(state)),
    );
    assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
}

/// CMD-67 のポインタ。キューの未完了の上限（既定 64）を超える件数を渡しても、戻って完了まで待てる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn submit_many_with_one_hundred_encoders_returns_and_completes() {
    let device = new_device();
    let queue = device.queue();
    let commands: Vec<_> = (0..100).map(|_| queue.create_encoder()).collect();
    let token = queue.submit_many(commands, &[], &[]).expect("submit_many");
    queue.wait_blocking(token, None).expect("wait_blocking");
    assert_eq!(queue.poll(token), CompletionState::Complete);
}

/// `submit` の直後に `on_complete` を呼んでも、完了まで待ってから callback が 1 回だけ `Complete` で呼ばれる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn on_complete_right_after_submit_calls_back_once_with_complete() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    device.queue().on_complete(
        token,
        Box::new(move |state| sink.lock().expect("lock").push(state)),
    );
    assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
}

/// 別 device の印と払い出していない番号では、`on_complete` は待たずに `Lost` で 1 回だけ呼ぶ。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn on_complete_with_a_foreign_or_unissued_token_calls_back_lost_once() {
    let first = new_device();
    let second = new_device();
    let foreign = submit_empty(first.as_ref());
    // 払い出していない番号は、`second` 自身の device の識別子と世代で作る（別 device の拒否と分けて確かめる）。
    let own = submit_empty(second.as_ref());
    let unissued = fandhe_silicon_core::execution::SubmitToken::__backend_new(
        own.generation,
        own.sequence + 98,
        own.__backend_device_id(),
    );
    for token in [foreign, unissued] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        second.queue().on_complete(
            token,
            Box::new(move |state| sink.lock().expect("lock").push(state)),
        );
        assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Lost]);
    }
}

/// CMD-23 のポインタ。完了を待った後の `poll_ex` は `Complete`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn poll_ex_is_complete_after_waiting() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    queue.wait_blocking(token, None).expect("wait_blocking");
    assert_eq!(queue.poll_ex(token), CompletionStateEx::Complete);
}

/// CMD-24 のポインタ。回収された後の番号は `poll` も `poll_ex` も `Complete`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn reclaimed_tokens_poll_as_complete() {
    let device = new_device();
    let queue = device.queue();
    let tokens: Vec<_> = (0..8).map(|_| submit_empty(device.as_ref())).collect();
    for token in &tokens {
        queue.wait_blocking(*token, None).expect("wait_blocking");
    }
    for token in &tokens {
        assert_eq!(queue.poll(*token), CompletionState::Complete);
        assert_eq!(queue.poll_ex(*token), CompletionStateEx::Complete);
    }
}

/// CMD-88 のポインタ。払い出していない番号（0 を含む）は `Unknown`、待った後の自分の印は `Complete`、別 device の印は `Lost`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn poll_ex_of_unissued_is_unknown_and_of_a_foreign_token_is_lost() {
    let first = new_device();
    let second = new_device();
    let issued = submit_empty(first.as_ref());
    first
        .queue()
        .wait_blocking(issued, None)
        .expect("wait_blocking");
    let forged = |sequence: u64| {
        SubmitToken::__backend_new(first.generation(), sequence, issued.__backend_device_id())
    };
    for sequence in [0, issued.sequence + 1, 99] {
        assert_eq!(
            first.queue().poll_ex(forged(sequence)),
            CompletionStateEx::Unknown
        );
    }
    assert_eq!(first.queue().poll_ex(issued), CompletionStateEx::Complete);
    assert_eq!(second.queue().poll_ex(issued), CompletionStateEx::Lost);
}
