//! 上の層の記録先・投入・時刻の写し・包みを、呼び出しを記録する試験用の device の上で確かめる統合テスト（TASK-2386）。
//!
//! 公開のパスだけを使う。プロセスの環境は読まず、設定は `RunSetting::from_lookup` へ渡す。

mod common;

use std::cell::Cell;
use std::ffi::OsString;
use std::mem::size_of;

use common::{Call, Overrides, Recorder};
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{GpuTiming, SubmitToken};
use fandhe_silicon_core::memory::BufferUsage;
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, Label, MachineCombination, NotRunReason, RunResult, RunSetting,
    Subject, Verdict, judge,
};
use fandhe_silicon_upper::{Buffer, Queue, SubmitId, UpperDevice};

/// 記録の中の `Submit` が返した印を取り出す。
fn submitted_token(calls: &[Call]) -> SubmitToken {
    calls
        .iter()
        .find_map(|c| match c {
            Call::Submit { token, .. } => *token,
            _ => None,
        })
        .expect("submit recorded with a token")
}

/// 空の記録先を 1 つ作って投入し、印を返す。
fn submit_empty(queue: &Queue<'_>) -> SubmitId {
    let buffer = queue.create_command_encoder().finish();
    queue.submit(buffer).expect("submit")
}

#[test]
fn encoder_pass_and_finish_call_no_queue_operation() {
    // UPPER-17
    let dev = Recorder::new(Overrides::default());
    let queue = UpperDevice::new(&dev).queue();

    let buffer = queue
        .create_command_encoder()
        .begin_compute_pass()
        .end()
        .finish();
    assert_eq!(dev.calls(), vec![Call::CreateEncoder]);

    queue.submit(buffer).expect("submit");
    let calls = dev.calls();
    assert_eq!(calls.len(), 2);
    assert!(matches!(calls[1], Call::Submit { .. }));
}

#[test]
fn submit_passes_empty_wait_and_wraps_the_token() {
    // UPPER-38
    let dev = Recorder::new(Overrides::default());
    let queue = UpperDevice::new(&dev).queue();

    let id = submit_empty(&queue);
    let _ = queue.gpu_timing(id);

    let calls = dev.calls();
    let submits: Vec<_> = calls
        .iter()
        .filter(|c| matches!(c, Call::Submit { .. }))
        .collect();
    assert_eq!(submits.len(), 1);
    assert!(matches!(submits[0], Call::Submit { wait_len: 0, .. }));
    let token = submitted_token(&calls);
    assert_eq!(calls.last(), Some(&Call::GpuTiming { token }));
    assert_eq!(size_of::<SubmitId>(), size_of::<SubmitToken>());
}

#[test]
fn submit_error_is_returned_as_is() {
    // UPPER-38
    let err = ContractError::InvalidArgument("injected".to_string());
    let dev = Recorder::new(Overrides {
        submit: Some(err.clone()),
        ..Overrides::default()
    });
    let queue = UpperDevice::new(&dev).queue();

    let buffer = queue.create_command_encoder().finish();
    assert_eq!(queue.submit(buffer), Err(err));
    assert!(matches!(
        dev.calls().last(),
        Some(Call::Submit { token: None, .. })
    ));
}

#[test]
fn gpu_timing_only_reads_and_returns_as_is() {
    // UPPER-38
    let cases = [
        Some(GpuTiming::Pending),
        Some(GpuTiming::Reclaimed),
        Some(GpuTiming::Available {
            start_seconds: 1.5,
            end_seconds: 2.25,
        }),
        None,
    ];
    for injected in cases {
        let dev = Recorder::new(Overrides {
            gpu_timing: injected,
            ..Overrides::default()
        });
        let queue = UpperDevice::new(&dev).queue();
        let id = submit_empty(&queue);
        let before = dev.calls().len();

        let got = queue.gpu_timing(id);

        let calls = dev.calls();
        assert_eq!(calls.len(), before + 1);
        assert!(matches!(calls[before], Call::GpuTiming { .. }));
        assert!(
            !calls
                .iter()
                .any(|c| matches!(c, Call::Poll | Call::SubmitMany | Call::OnComplete))
        );
        assert_eq!(got, injected.unwrap_or(GpuTiming::Unavailable));
    }
}

#[test]
fn device_and_queue_hold_no_state() {
    // UPPER-32
    let ptr = size_of::<&'static dyn Device>();
    assert_eq!(size_of::<UpperDevice<'static>>(), ptr);
    assert_eq!(size_of::<Queue<'static>>(), ptr);

    let a = Recorder::new(Overrides::default());
    let b = Recorder::new(Overrides::default());
    let upper_a = UpperDevice::new(&a);
    let upper_b = UpperDevice::new(&b);

    submit_empty(&upper_a.queue());
    assert!(a.calls().iter().any(|c| matches!(c, Call::Submit { .. })));
    assert!(b.calls().is_empty());

    let q1 = upper_b.queue();
    let q2 = upper_b.queue();
    submit_empty(&q1);
    submit_empty(&q2);
    let submits = b
        .calls()
        .iter()
        .filter(|c| matches!(c, Call::Submit { .. }))
        .count();
    assert_eq!(submits, 2);
}

#[test]
fn buffer_wraps_the_handle() {
    // UPPER-33
    let dev = Recorder::new(Overrides::default());
    let handle = dev
        .memory()
        .allocate(64, BufferUsage::STORAGE)
        .expect("allocate");
    let buffer = Buffer::from_handle(handle);
    assert_eq!(buffer.handle().size(), 64);
    assert_eq!(buffer.handle().usage(), BufferUsage::STORAGE);

    let handle = buffer.into_handle();
    assert_eq!(handle.size(), 64);
    assert_eq!(handle.usage(), BufferUsage::STORAGE);
    dev.memory()
        .upload(&handle, 0, &[1, 2, 3, 4])
        .expect("upload");
    dev.memory().free(handle);
}

fn setting(configured: bool) -> RunSetting {
    RunSetting::from_lookup(move |n| {
        (configured && n == COMBINATION_ENV)
            .then(|| OsString::from(MachineCombination::MetalMacMini.name()))
    })
}

/// 投入と時刻の写しの確かめ。`injected` を返す device で、値がそのまま返れば `Match`。
fn verdict_for(injected: GpuTiming, expected: GpuTiming) -> RunResult {
    let dev = Recorder::new(Overrides {
        gpu_timing: Some(injected),
        ..Overrides::default()
    });
    let queue = UpperDevice::new(&dev).queue();
    let id = submit_empty(&queue);
    RunResult::Verdict(if queue.gpu_timing(id) == expected {
        Verdict::Match
    } else {
        Verdict::Mismatch
    })
}

#[test]
fn checks_compose_with_test_frame_judge() {
    let subject = Subject::from_parts("upper_compute_path", &["UPPER-38"], &[]).unwrap();
    let declared = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let calls = Cell::new(0u32);

    let o = judge(&subject, &declared, &setting(true), |_| {
        calls.set(calls.get() + 1);
        verdict_for(GpuTiming::Pending, GpuTiming::Pending)
    });
    assert_eq!(*o.label(), Label::Ok);
    assert_eq!(calls.get(), 1);

    let o = judge(&subject, &declared, &setting(true), |_| {
        calls.set(calls.get() + 1);
        verdict_for(GpuTiming::Pending, GpuTiming::Reclaimed)
    });
    assert_eq!(*o.label(), Label::Ng);
    assert_eq!(calls.get(), 2);

    let o = judge(&subject, &declared, &setting(false), |_| {
        calls.set(calls.get() + 1);
        verdict_for(GpuTiming::Pending, GpuTiming::Pending)
    });
    assert_eq!(*o.label(), Label::NotRun(NotRunReason::NotConfigured));
    assert_eq!(calls.get(), 2);
}
