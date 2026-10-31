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
use fandhe_silicon_core::execution::{GpuTiming, ImportedSemaphoreHandle, SubmitToken};
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

/// 自 device の識別子で、表に無い識別子（払い出していない大きな値）の偽造のハンドルを作る。
fn forged_unknown_id(own: &BufferHandle) -> BufferHandle {
    BufferHandle::__backend_new(
        own.__backend_generation(),
        u64::MAX - 1,
        own.__backend_device_id(),
        own.size(),
        own.usage(),
    )
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn forged_handles_are_rejected_by_upload_and_download_without_touching_own_buffers() {
    let device = new_device();
    let memory = device.memory();
    let own = memory.allocate(16, usage()).expect("allocate");
    memory.upload(&own, 0, &[0x3C; 16]).expect("upload own");

    // 自 device の識別子だが表に無い識別子。
    let unknown = forged_unknown_id(&own);
    // 表にある識別子だが世代が違う。
    let wrong_generation = BufferHandle::__backend_new(
        own.__backend_generation() + 1,
        own.__backend_id(),
        own.__backend_device_id(),
        own.size(),
        own.usage(),
    );
    for forged in [&unknown, &wrong_generation] {
        let up = memory.upload(forged, 0, &[0xEE; 16]);
        assert!(
            matches!(up, Err(ContractError::InvalidArgument(_))),
            "{up:?}"
        );
        let down = memory.download_blocking(forged, 0, 16);
        assert!(
            matches!(down, Err(ContractError::InvalidArgument(_))),
            "{down:?}"
        );
    }
    // どの device のものでもない識別子は取り違えとして拒否する。
    let foreign = BufferHandle::__backend_new(
        own.__backend_generation(),
        own.__backend_id(),
        own.__backend_device_id() ^ u64::MAX,
        own.size(),
        own.usage(),
    );
    assert_eq!(
        memory.upload(&foreign, 0, &[0xEE; 16]),
        Err(ContractError::DeviceMismatch)
    );
    assert_eq!(
        memory.download_blocking(&foreign, 0, 16),
        Err(ContractError::DeviceMismatch)
    );

    // 自 device のバッファの中身は変わっていない。
    let kept = memory.download_blocking(&own, 0, 16).expect("download own");
    assert_eq!(kept, vec![0x3C; 16]);
    memory.free(own);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn another_devices_buffer_is_rejected_by_download_async_and_the_same_id_stays_intact() {
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
        .upload(&from_second, 0, &[0x77; 16])
        .expect("upload second");

    let received = Arc::new(Mutex::new(Vec::new()));
    let r = Arc::clone(&received);
    second.memory().download_async(
        &from_first,
        0,
        16,
        Box::new(move |result| r.lock().unwrap().push(result)),
    );
    let got = received.lock().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0], Err(ContractError::DeviceMismatch));
    drop(got);

    let kept = second
        .memory()
        .download_blocking(&from_second, 0, 16)
        .expect("download second");
    assert_eq!(kept, vec![0x77; 16]);
    first.memory().free(from_first);
    second.memory().free(from_second);
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn freeing_a_forged_or_unknown_handle_is_ignored_and_keeps_own_buffers() {
    let device = new_device();
    let memory = device.memory();
    let a = memory.allocate(16, usage()).expect("allocate a");
    let b = memory.allocate(16, usage()).expect("allocate b");
    memory.upload(&a, 0, &[0x11; 16]).expect("upload a");
    memory.upload(&b, 0, &[0x22; 16]).expect("upload b");

    // 表に無い識別子、世代違い、どの device のものでもない識別子の 3 通りを渡す。
    memory.free(forged_unknown_id(&a));
    memory.free(BufferHandle::__backend_new(
        a.__backend_generation() + 1,
        a.__backend_id(),
        a.__backend_device_id(),
        a.size(),
        a.usage(),
    ));
    memory.free(BufferHandle::__backend_new(
        a.__backend_generation(),
        a.__backend_id(),
        a.__backend_device_id() ^ u64::MAX,
        a.size(),
        a.usage(),
    ));

    assert_eq!(
        memory.download_blocking(&a, 0, 16).expect("a"),
        vec![0x11; 16]
    );
    assert_eq!(
        memory.download_blocking(&b, 0, 16).expect("b"),
        vec![0x22; 16]
    );
    memory.free(a);
    memory.free(b);
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

/// TASK-592 のポインタ。空の `wait` の `submit_ex` は成功して印を返し、`RetryPolicy` は宣言として受けるだけで再試行しない。
/// 投入は 1 件だけ進み（通し番号が前の印の次）、印は `wait_blocking(None)` の後の `poll` で `Complete` になる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn submit_ex_with_an_empty_wait_returns_a_token_that_completes() {
    use fandhe_silicon_core::error::RetryPolicy;
    for policy in [RetryPolicy::IdempotentSafe, RetryPolicy::NotIdempotent] {
        let device = new_device();
        let first = submit_empty(device.as_ref());
        let queue = device.queue();
        let token = queue
            .submit_ex(queue.create_encoder(), &[], policy)
            .expect("submit_ex");
        assert_eq!(token.sequence, first.sequence + 1, "{policy:?}");
        queue.wait_blocking(token, None).expect("wait_blocking");
        assert_eq!(queue.poll(token), CompletionState::Complete, "{policy:?}");
        // 再試行で投入が増えていないので、次の投入の通し番号はさらに 1 だけ進む。
        assert_eq!(
            submit_empty(device.as_ref()).sequence,
            token.sequence + 1,
            "{policy:?}"
        );
    }
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

/// 完了済みの印への `on_complete` は、登録の呼び出しの中でその場で `Complete` を 1 回だけ渡す。
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

/// 完了を待たずに登録した callback は、基盤の呼び出し（`poll`）の中で終端になったと分かった時に、`Complete` で 1 回だけ呼ばれる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn on_complete_registered_right_after_submit_is_driven_by_poll_and_called_once() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    device.queue().on_complete(
        token,
        Box::new(move |state| sink.lock().expect("lock").push(state)),
    );
    let started = std::time::Instant::now();
    while seen.lock().expect("lock").is_empty() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "callback was not called"
        );
        device.queue().poll(token);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // 呼ばれた後の駆動では、もう呼ばれない。
    for _ in 0..3 {
        device.queue().poll(token);
    }
    assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
}

/// `wait_blocking` の中でも登録済みの callback が `Complete` で 1 回呼ばれる。登録の呼び出しはブロックしない。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn wait_blocking_drives_a_registered_callback() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    device.queue().on_complete(
        token,
        Box::new(move |state| sink.lock().expect("lock").push(state)),
    );
    device.queue().wait_blocking(token, None).expect("wait");
    assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
}

/// 登録が残ったまま device を落とすと、`Drop` の中で完了を待ってから、残りの callback を 1 回だけ呼ぶ。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn dropping_the_device_calls_remaining_callbacks_once() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    device.queue().on_complete(
        token,
        Box::new(move |state| sink.lock().expect("lock").push(state)),
    );
    drop(device);
    assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
}

/// callback が panic しても残りの callback は呼ばれ、panic は登録の呼び出し（登録の時点で終端のとき）か
/// 後の駆動の呼び出しのどちらか一方からちょうど 1 回伝わる。device は使い続けられる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn a_panicking_callback_does_not_starve_the_others_or_break_the_device() {
    let device = new_device();
    let first = submit_empty(device.as_ref());
    let second = submit_empty(device.as_ref());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    // 空の command buffer はすぐ終わるので、`first` は登録の時点で終端のことがある。その場合 panic は登録から伝わる。
    let at_registration = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        device
            .queue()
            .on_complete(first, Box::new(|_| panic!("callback failed")));
    }))
    .is_err();
    device.queue().on_complete(
        second,
        Box::new(move |state| sink.lock().expect("lock").push(state)),
    );
    let mut from_drive = false;
    if !at_registration {
        // 登録の時点で未完了だった。`first` が終端になって最初に駆動した呼び出しから panic が伝わる。伝わるまで 10 秒まで駆動する。
        let started = std::time::Instant::now();
        while !from_drive {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "the callback's panic did not propagate"
            );
            from_drive = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                device.queue().poll(second);
            }))
            .is_err();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(
        at_registration ^ from_drive,
        "the panic must propagate from exactly one of registration and drive"
    );
    println!(
        "panic_propagated_from={}",
        if at_registration {
            "registration"
        } else {
            "drive"
        }
    );
    // panic のあとも、後ろの callback は 1 回呼ばれる（同じ駆動で呼ばれるか、次の駆動で呼ばれる）。
    drive_until_called(&seen, || {
        device.queue().poll(second);
    });
    assert_eq!(device.queue().poll(first), CompletionState::Complete);
}

fn registered_recorder(
    device: &dyn Device,
    token: fandhe_silicon_core::execution::SubmitToken,
) -> Arc<Mutex<Vec<CompletionState>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    device.queue().on_complete(
        token,
        Box::new(move |state| sink.lock().expect("lock").push(state)),
    );
    // 登録の直後にもう呼ばれていれば、登録の時点で終端だった（その場で呼ばれた）。
    println!(
        "called_at_registration={}",
        !seen.lock().expect("lock").is_empty()
    );
    seen
}

/// `drive` を、callback が呼ばれるまで（10 秒まで）繰り返す。`poll` は呼ばない。
fn drive_until_called(seen: &Arc<Mutex<Vec<CompletionState>>>, mut drive: impl FnMut()) {
    let started = std::time::Instant::now();
    while seen.lock().expect("lock").is_empty() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "callback was not called"
        );
        drive();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    drive();
    assert_eq!(*seen.lock().expect("lock"), vec![CompletionState::Complete]);
}

/// 登録のあと `poll` を呼ばずに、後続の `submit` の中で callback が `Complete` で 1 回呼ばれる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn submit_drives_a_registered_callback_without_poll() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = registered_recorder(device.as_ref(), token);
    drive_until_called(&seen, || {
        submit_empty(device.as_ref());
    });
}

/// 登録のあと `poll` を呼ばずに、後続の `submit_many` の中で callback が呼ばれる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn submit_many_drives_a_registered_callback_without_poll() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = registered_recorder(device.as_ref(), token);
    drive_until_called(&seen, || {
        let queue = device.queue();
        queue
            .submit_many(vec![queue.create_encoder()], &[], &[])
            .expect("submit_many");
    });
}

/// 登録のあと `poll` を呼ばずに、後続の `allocate` の中で callback が呼ばれる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn allocate_drives_a_registered_callback_without_poll() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = registered_recorder(device.as_ref(), token);
    drive_until_called(&seen, || {
        let buffer = device
            .memory()
            .allocate(16, BufferUsage::STORAGE)
            .expect("allocate");
        device.memory().free(buffer);
    });
}

/// 登録のあと `poll_ex` の中で callback が呼ばれる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn poll_ex_drives_a_registered_callback() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let seen = registered_recorder(device.as_ref(), token);
    drive_until_called(&seen, || {
        device.queue().poll_ex(token);
    });
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

/// CMD-44〜CMD-48 のポインタ。`wait_blocking(None)` の後の `gpu_timing` は `Available` で、終了は開始以上、両方 0 より大きい。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_is_available_after_wait_blocking() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    queue.wait_blocking(token, None).expect("wait_blocking");
    match queue.gpu_timing(token) {
        GpuTiming::Available {
            start_seconds,
            end_seconds,
        } => {
            println!("gpu_timing start_seconds={start_seconds} end_seconds={end_seconds}");
            assert!(start_seconds > 0.0);
            assert!(end_seconds > 0.0);
            assert!(end_seconds >= start_seconds);
        }
        other => panic!("expected Available, got {other:?}"),
    }
    // 読み取り専用なので、続けて読んでも同じ値が返る。
    assert_eq!(queue.gpu_timing(token), queue.gpu_timing(token));
}

/// TASK-596 のポインタ。`submit` の後に間を置いてから `wait_blocking(None)` を呼んでも、その後の `gpu_timing` は
/// `Available` で、時刻が残る（開始は 0 より大きく、終了は開始以上）。間を置くのは、待つ前に完了している経路を
/// 通りやすくするためで、完了済みになることは保証しない（どちらの経路でも `Available`）。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_is_available_after_a_sleep_then_wait_blocking() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    std::thread::sleep(std::time::Duration::from_millis(50));
    let queue = device.queue();
    queue.wait_blocking(token, None).expect("wait_blocking");
    match queue.gpu_timing(token) {
        GpuTiming::Available {
            start_seconds,
            end_seconds,
        } => {
            println!("start_seconds={start_seconds} end_seconds={end_seconds}");
            assert!(start_seconds > 0.0);
            assert!(end_seconds >= start_seconds);
        }
        other => panic!("expected Available, got {other:?}"),
    }
    assert_eq!(queue.gpu_timing(token), queue.gpu_timing(token));
}

/// 待つ前の `gpu_timing` は `Available` にならない。空の command buffer はすぐ終わるので `Pending` を確かめられるとは限らず、
/// 確かめられた場合と、完了済みで時刻を残していない `Reclaimed` だった場合を出力で分ける。どちらでも待った後は `Available`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_before_waiting_is_not_available_and_does_not_reclaim() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    match queue.gpu_timing(token) {
        GpuTiming::Pending => println!("gpu_timing before wait: Pending observed"),
        GpuTiming::Reclaimed => {
            println!("gpu_timing before wait: Pending not observable (already completed)")
        }
        other => panic!("expected Pending or Reclaimed before waiting, got {other:?}"),
    }
    queue.wait_blocking(token, None).expect("wait_blocking");
    assert!(matches!(
        queue.gpu_timing(token),
        GpuTiming::Available { .. }
    ));
}

/// `poll` で回収した投入は、時刻を書かないまま `Reclaimed` になる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_of_a_token_reclaimed_by_poll_is_reclaimed() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    while queue.poll(token) == CompletionState::Pending {
        std::thread::yield_now();
    }
    assert_eq!(queue.gpu_timing(token), GpuTiming::Reclaimed);
    // 回収済みの投入を `wait_blocking(None)` が後から観測しても、時刻は書かれず `Reclaimed` のまま。
    queue.wait_blocking(token, None).expect("wait_blocking");
    assert_eq!(queue.gpu_timing(token), GpuTiming::Reclaimed);
}

/// 期限付きの `wait_blocking(Some(..))` が完了を検出しても時刻は書かれず、`Available` にならない。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_after_a_bounded_wait_is_not_available() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    queue
        .wait_blocking(token, Some(std::time::Duration::from_secs(30)))
        .expect("wait_blocking");
    assert_eq!(queue.gpu_timing(token), GpuTiming::Reclaimed);
}

/// `on_complete` の callback が呼ばれた後も、時刻は書かれず `Available` にならない。
///
/// callback は後の駆動の呼び出しの中で呼ばれる（Metal の completion handler からは呼ばれない）ので、
/// 駆動は `poll` で行う。時刻を残す `wait_blocking(None)` は呼ばない。止まらないよう上限を設ける。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_after_on_complete_is_not_available() {
    let device = new_device();
    let token = submit_empty(device.as_ref());
    let queue = device.queue();
    let (sender, receiver) = std::sync::mpsc::channel();
    queue.on_complete(
        token,
        Box::new(move |state| {
            let _ = sender.send(state);
        }),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let state = loop {
        let _ = queue.poll(token);
        match receiver.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(state) => break state,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "on_complete callback was not called within 30 s of polling"
                );
            }
            Err(error) => panic!("receiver: {error:?}"),
        }
    };
    assert_eq!(state, CompletionState::Complete);
    // 以後の駆動でも状態は変わらない。callback は 1 回呼ばれた後に破棄され、送り手が無くなる。
    for _ in 0..3 {
        assert_eq!(queue.poll(token), CompletionState::Complete);
    }
    assert_eq!(
        receiver.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Disconnected)
    );
    assert_eq!(queue.gpu_timing(token), GpuTiming::Reclaimed);
}

/// 別 device の印・払い出していない番号（0 を含む）は `Unavailable`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_of_a_foreign_or_unissued_token_is_unavailable() {
    let first = new_device();
    let second = new_device();
    let foreign = submit_empty(first.as_ref());
    let own = submit_empty(second.as_ref());
    second.queue().wait_blocking(own, None).expect("wait");
    assert_eq!(second.queue().gpu_timing(foreign), GpuTiming::Unavailable);
    for sequence in [0, own.sequence + 98] {
        let unissued =
            SubmitToken::__backend_new(own.generation, sequence, own.__backend_device_id());
        assert_eq!(second.queue().gpu_timing(unissued), GpuTiming::Unavailable);
    }
}

/// 1,025 回の `submit` + `wait_blocking(None)` の後は、最初の印が `Reclaimed`（表から捨てられた）、最後の印が `Available`。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn gpu_timing_table_drops_the_oldest_after_1025_waits() {
    let device = new_device();
    let queue = device.queue();
    let mut tokens = Vec::new();
    for _ in 0..1025 {
        let token = submit_empty(device.as_ref());
        queue.wait_blocking(token, None).expect("wait_blocking");
        tokens.push(token);
    }
    let first = queue.gpu_timing(tokens[0]);
    let last = queue.gpu_timing(tokens[1024]);
    println!("gpu_timing first={first:?} last={last:?}");
    assert_eq!(first, GpuTiming::Reclaimed);
    assert!(matches!(last, GpuTiming::Available { .. }));
}
