//! 試験用の device を下-2 の値と上の層の device に渡し、上の層の buffer を下-2 の device の形に渡す統合テスト（TASK-260）。
//!
//! feature `native-blocking-wait` の下でだけ組む。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{Device, DeviceOwnership, DeviceRequest, Profile};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::feature_key::{
    FEATURE_SHADER_F16, FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_SUBGROUP,
};
use fandhe_silicon_core::memory::BufferUsage;
use fandhe_silicon_core::stub_device::StubDevice;
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OpId, OptIn, Outcome, SubgroupSumF32Operands,
    SubgroupSumU32Operands, UnavailableReason,
};
use fandhe_silicon_upper::{Buffer, SubmitId, UpperDevice};

const LEN: u64 = 16;
/// f16_fma が 4 つ、残りの 3 つの入口が 2 つずつ使う buffer の数。
const BUFFERS: usize = 10;

fn stub() -> StubDevice {
    StubDevice::new(DeviceRequest {
        backend: BackendKind::Metal,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::Shared,
    })
}

fn alloc(d: &dyn Device, fill: u8) -> Buffer {
    let mem = d.memory();
    let h = mem.allocate(LEN, BufferUsage::STORAGE).unwrap();
    mem.upload(&h, 0, &[fill; LEN as usize]).unwrap();
    Buffer::from_handle(h)
}

fn alloc_all(d: &dyn Device) -> Vec<Buffer> {
    (0..BUFFERS).map(|i| alloc(d, i as u8 + 1)).collect()
}

fn read(d: &dyn Device, b: &Buffer) -> Vec<u8> {
    d.memory().download_blocking(b.handle(), 0, LEN).unwrap()
}

fn free_all(d: &dyn Device, bufs: Vec<Buffer>) {
    for b in bufs {
        d.memory().free(b.into_handle());
    }
}

/// 4 つの入口を device の形で順に呼ぶ。
fn call_all(
    exec: &Exec<'_>,
    opt_in: Option<OptIn>,
    bufs: &mut [Buffer],
) -> [Result<Outcome, ContractError>; 4] {
    let mut it = bufs.iter_mut().map(Buffer::handle_mut);
    let mut next = || it.next().unwrap();
    let f16 = exec.f16_fma(
        opt_in,
        8,
        F16FmaOperands::Device {
            a: next(),
            b: next(),
            c: next(),
            out: next(),
        },
    );
    let su32 = exec.subgroup_sum_u32(
        opt_in,
        8,
        SubgroupSumU32Operands::Device {
            x: next(),
            out: next(),
        },
    );
    let sf32 = exec.subgroup_sum_f32(
        opt_in,
        8,
        SubgroupSumF32Operands::Device {
            x: next(),
            out: next(),
        },
    );
    let add = exec.atomic_add(
        opt_in,
        8,
        1,
        AtomicAddOperands::Device {
            x: next(),
            out: next(),
        },
    );
    [f16, su32, sf32, add]
}

#[test]
fn upper_buffers_pass_to_exec_device_operands() {
    // BUILD-60
    let dev = stub();
    let d: &dyn Device = &dev;
    let exec = Exec::deny_by_default(d);
    let _upper = UpperDevice::new(d);
    let mut bufs = alloc_all(d);

    let results = call_all(&exec, None, &mut bufs);
    let ops = [
        OpId::F16Fma,
        OpId::SubgroupSumU32,
        OpId::SubgroupSumF32,
        OpId::AtomicAdd,
    ];
    for (r, op) in results.into_iter().zip(ops) {
        match r {
            Ok(Outcome::Unavailable { report }) => {
                assert_eq!(report.route, None);
                assert_eq!(report.op, op);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
    for (i, b) in bufs.iter().enumerate() {
        assert_eq!(read(d, b), vec![i as u8 + 1; LEN as usize], "buffer {i}");
    }
    free_all(d, bufs);
}

#[test]
fn override_call_returns_contract_error_type() {
    let dev = stub();
    let d: &dyn Device = &dev;
    let exec = Exec::deny_by_default(d);
    let mut bufs = alloc_all(d);
    let opt_in = Some(OptIn {
        allow_gpu_emulated: false,
        allow_cpu: true,
    });

    let results = call_all(&exec, opt_in, &mut bufs);
    for r in results {
        let r: Result<Outcome, ContractError> = r;
        assert!(!matches!(r, Ok(Outcome::Unavailable { .. })));
    }
    for b in &bufs {
        assert_eq!(b.handle().size(), LEN);
        assert_eq!(b.handle().usage(), BufferUsage::STORAGE);
        assert_eq!(read(d, b).len(), LEN as usize);
    }
    free_all(d, bufs);
}

#[test]
fn exec_report_level_matches_device_query() {
    let dev = stub();
    let d: &dyn Device = &dev;
    let exec = Exec::deny_by_default(d);

    let pairs = [
        (OpId::F16Fma, FEATURE_SHADER_F16),
        (OpId::SubgroupSumU32, FEATURE_SUBGROUP),
        (OpId::SubgroupSumF32, FEATURE_SUBGROUP),
        (OpId::AtomicAdd, FEATURE_SHADER_FLOAT32_ATOMIC),
    ];
    for (op, key) in pairs {
        let q = d.query_ex(key);
        let report = exec.query_route(op, None).unwrap();
        assert_eq!(report.observed_level, Some(q.level), "{op:?}");
        assert_eq!(
            report.native_reason,
            Some(UnavailableReason::CapabilityUnsupported(
                q.unsupported_reason
            )),
            "{op:?}"
        );
    }

    let mut bufs = alloc_all(d);
    let mut it = bufs.iter_mut().map(Buffer::handle_mut);
    let r = exec.f16_fma(
        None,
        8,
        F16FmaOperands::Device {
            a: it.next().unwrap(),
            b: it.next().unwrap(),
            c: it.next().unwrap(),
            out: it.next().unwrap(),
        },
    );
    match r {
        Ok(Outcome::Unavailable { report }) => {
            assert_eq!(report, exec.query_route(OpId::F16Fma, None).unwrap());
        }
        other => panic!("unexpected: {other:?}"),
    }
    free_all(d, bufs);
}

#[test]
fn upper_submit_and_wait_on_the_same_device() {
    let dev = stub();
    let d: &dyn Device = &dev;
    let exec = Exec::deny_by_default(d);
    let upper = UpperDevice::new(d);
    let mut bufs = alloc_all(d);

    let _ = call_all(&exec, None, &mut bufs);
    let queue = upper.queue();
    let s: Result<SubmitId, ContractError> = queue.submit(queue.create_command_encoder().finish());
    let id = s.unwrap();
    let w: Result<(), ContractError> = queue.wait_blocking(id, None);
    assert_eq!(w, Ok(()));
    assert!(exec.query_route(OpId::F16Fma, None).is_ok());
    free_all(d, bufs);
}
