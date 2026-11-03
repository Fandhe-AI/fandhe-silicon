//! 上の層の device・記録先・投入・待ちと、下-2 の経路の問合せ・入口を、呼び出しを記録する 1 つの試験用の device の上で通す統合テスト（TASK-261）。
//!
//! 両方の呼び出しが同じ device の 1 つの呼び出しの列に順に届くことを確かめる。
//! feature `native-blocking-wait` の下でだけ組む。

mod common;

use common::{Call, Overrides, Recorder};
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::feature_key::FEATURE_SHADER_F16;
use fandhe_silicon_core::memory::BufferUsage;
use fandhe_silicon_exec::{Exec, F16FmaOperands, OpId, OptIn, Outcome};
use fandhe_silicon_upper::{Buffer, SubmitId, UpperDevice};

const LEN: u64 = 16;

fn alloc(d: &dyn Device, fill: u8) -> Buffer {
    let mem = d.memory();
    let h = mem.allocate(LEN, BufferUsage::STORAGE).unwrap();
    mem.upload(&h, 0, &[fill; LEN as usize]).unwrap();
    Buffer::from_handle(h)
}

fn read(d: &dyn Device, b: &Buffer) -> Vec<u8> {
    d.memory().download_blocking(b.handle(), 0, LEN).unwrap()
}

fn free_all(d: &dyn Device, bufs: Vec<Buffer>) {
    for b in bufs {
        d.memory().free(b.into_handle());
    }
}

/// 呼び出しの列に、投入と待ちと時間の読み出し以外の queue の操作が無いことを照らす。
fn assert_no_other_queue_calls(calls: &[Call]) {
    assert!(!calls.iter().any(|c| matches!(
        c,
        Call::Poll | Call::SubmitMany | Call::OnComplete | Call::DispatchEx
    )));
}

#[test]
fn upper_and_exec_calls_reach_one_device() {
    // BUILD-59
    let rec = Recorder::new(Overrides::default());
    let d: &dyn Device = &rec;
    let upper = UpperDevice::new(d);
    let exec = Exec::deny_by_default(d);
    let mut bufs: Vec<Buffer> = (1..=4).map(|i| alloc(d, i)).collect();

    let queue = upper.queue();
    let enc = queue.create_command_encoder();
    let route = exec.query_route(OpId::F16Fma, None).unwrap();
    let mut it = bufs.iter_mut().map(Buffer::handle_mut);
    let mut next = || it.next().unwrap();
    let r = exec
        .f16_fma(
            None,
            8,
            F16FmaOperands::Device {
                a: next(),
                b: next(),
                c: next(),
                out: next(),
            },
        )
        .unwrap();
    let buf = enc.begin_compute_pass().end().finish();
    let id: SubmitId = queue.submit(buf).unwrap();
    assert_eq!(queue.wait_blocking(id, None), Ok(()));
    let _ = queue.gpu_timing(id);

    match r {
        Outcome::Unavailable { report } => assert_eq!(report, route),
        other => panic!("unexpected outcome: {other:?}"),
    }
    for (i, b) in bufs.iter().enumerate() {
        assert_eq!(read(d, b), vec![i as u8 + 1; LEN as usize]);
    }

    let calls = rec.calls();
    let Call::Submit { token: Some(t), .. } = calls[3] else {
        panic!("unexpected call: {:?}", calls[3]);
    };
    assert_eq!(
        calls,
        vec![
            Call::CreateEncoder,
            Call::QueryEx {
                feature: FEATURE_SHADER_F16
            },
            Call::QueryEx {
                feature: FEATURE_SHADER_F16
            },
            Call::Submit {
                wait_len: 0,
                token: Some(t)
            },
            Call::WaitBlocking {
                token: t,
                timeout: None
            },
            Call::GpuTiming { token: t },
        ]
    );
    assert_no_other_queue_calls(&calls);
    free_all(d, bufs);
}

#[test]
fn exec_failure_does_not_change_upper_submit() {
    let rec = Recorder::new(Overrides::default());
    let d: &dyn Device = &rec;
    let upper = UpperDevice::new(d);
    let exec = Exec::deny_by_default(d);
    let mut bufs: Vec<Buffer> = (1..=4).map(|i| alloc(d, i)).collect();

    let queue = upper.queue();
    let enc = queue.create_command_encoder();
    let over = Some(OptIn {
        allow_gpu_emulated: false,
        allow_cpu: true,
    });
    let mut it = bufs.iter_mut().map(Buffer::handle_mut);
    let mut next = || it.next().unwrap();
    let r = exec.f16_fma(
        over,
        8,
        F16FmaOperands::Device {
            a: next(),
            b: next(),
            c: next(),
            out: next(),
        },
    );
    let buf = enc.begin_compute_pass().end().finish();
    let id: SubmitId = queue.submit(buf).unwrap();
    assert_eq!(queue.wait_blocking(id, None), Ok(()));
    let _ = queue.gpu_timing(id);

    assert!(matches!(
        r,
        Err(ContractError::Unsupported {
            op: "f16_fma",
            backend: BackendKind::Metal,
            ..
        })
    ));
    for b in &bufs {
        assert_eq!(b.handle().size(), LEN);
        assert_eq!(b.handle().usage(), BufferUsage::STORAGE);
        assert_eq!(read(d, b).len(), LEN as usize);
    }

    let calls = rec.calls();
    let Call::Submit { token: Some(t), .. } = calls[2] else {
        panic!("unexpected call: {:?}", calls[2]);
    };
    assert_eq!(
        calls,
        vec![
            Call::CreateEncoder,
            Call::QueryEx {
                feature: FEATURE_SHADER_F16
            },
            Call::Submit {
                wait_len: 0,
                token: Some(t)
            },
            Call::WaitBlocking {
                token: t,
                timeout: None
            },
            Call::GpuTiming { token: t },
        ]
    );
    assert_no_other_queue_calls(&calls);
    free_all(d, bufs);
}
