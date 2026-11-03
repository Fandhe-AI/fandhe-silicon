//! feature `test-support` の裏の名前つきの入力の組を、試験用の device で確保したバッファに当てて、
//! 各入力が当たる検査（接頭辞か `Ok(())`）を確かめる統合テスト（TASK-2944 の 1/2）。
//!
//! 公開のパスだけを使う。拒否の文言は接頭辞より後ろを固定しない。

use std::cell::RefCell;

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::dispatch_inputs::{
    DISPATCH_INPUTS, DimsSpec, DispatchInput, ExpectedCheck, dispatch_dims_over_limit,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    OperandBinding, validate_declared_operands, validate_dispatch_operands,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};
use fandhe_silicon_core::stub_device::StubDeviceProvider;

fn device() -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Vulkan,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
    .expect("device")
}

fn run_dispatch(
    device: &dyn Device,
    input: &DispatchInput,
    handles: &[BufferHandle],
) -> Result<(), ContractError> {
    let ob = input.operand_bindings();
    let consts = input.operand_constants();
    validate_dispatch_operands(
        input.declared,
        &ob,
        &consts,
        |b: &OperandBinding| {
            ob.iter()
                .position(|x| std::ptr::eq(x, b))
                .and_then(|i| handles.get(i))
                .map(BufferHandle::size)
        },
        device.limits().max_buffer_bytes,
    )
}

fn free_all(device: &dyn Device, handles: Vec<BufferHandle>) {
    for h in handles {
        device.memory().free(h);
    }
}

fn assert_prefix(r: Result<(), ContractError>, prefix: &str, name: &str) {
    match r {
        Err(ContractError::InvalidArgument(msg)) => {
            assert!(msg.starts_with(prefix), "{name}: {msg}");
        }
        other => panic!("{name}: {other:?}"),
    }
}

#[test]
fn c_inputs_hit_their_prefix() {
    let d = device();
    let mut n = 0;
    for input in &DISPATCH_INPUTS {
        let Some(prefix) = input.expected.prefix() else {
            continue;
        };
        n += 1;
        let handles = input.allocate_buffers(d.memory()).expect("allocate");
        let declared = validate_declared_operands(input.declared, &input.layout());
        match input.expected {
            ExpectedCheck::C1 | ExpectedCheck::C3 => assert_prefix(declared, prefix, input.name),
            _ => {
                assert!(declared.is_ok(), "{}", input.name);
                assert_prefix(
                    run_dispatch(d.as_ref(), input, &handles),
                    prefix,
                    input.name,
                );
            }
        }
        free_all(d.as_ref(), handles);
    }
    assert_eq!(n, 8);
}

#[test]
fn pass_inputs_return_ok() {
    let d = device();
    let mut n = 0;
    for input in DISPATCH_INPUTS
        .iter()
        .filter(|i| i.expected == ExpectedCheck::Pass)
    {
        n += 1;
        let handles = input.allocate_buffers(d.memory()).expect("allocate");
        assert!(validate_declared_operands(input.declared, &input.layout()).is_ok());
        assert!(
            run_dispatch(d.as_ref(), input, &handles).is_ok(),
            "{}",
            input.name
        );
        free_all(d.as_ref(), handles);
    }
    assert_eq!(n, 3);
}

#[test]
fn dims_inputs_pass_the_c_checks() {
    let d = device();
    let max = d.limits().max_dispatch_dims;
    let mut n = 0;
    for input in &DISPATCH_INPUTS {
        if !matches!(
            input.expected,
            ExpectedCheck::ZeroDispatchDims | ExpectedCheck::DispatchDimsOverLimit
        ) {
            continue;
        }
        n += 1;
        let handles = input.allocate_buffers(d.memory()).expect("allocate");
        assert!(validate_declared_operands(input.declared, &input.layout()).is_ok());
        assert!(
            run_dispatch(d.as_ref(), input, &handles).is_ok(),
            "{}",
            input.name
        );
        let dims = input.dispatch_dims(max).expect("dims");
        if input.expected == ExpectedCheck::ZeroDispatchDims {
            assert!(dims.contains(&0));
        } else {
            assert!(dims[0] > max[0]);
        }
        free_all(d.as_ref(), handles);
    }
    assert_eq!(n, 2);
}

#[test]
fn non_dims_inputs_have_no_zero_axis() {
    let max = [7, 8, 9];
    for input in &DISPATCH_INPUTS {
        if matches!(
            input.expected,
            ExpectedCheck::ZeroDispatchDims | ExpectedCheck::DispatchDimsOverLimit
        ) {
            continue;
        }
        let dims = input.dispatch_dims(max).expect("dims");
        assert!(!dims.contains(&0), "{}", input.name);
        assert!(dims.iter().zip(max).all(|(a, m)| *a <= m), "{}", input.name);
    }
}

#[test]
fn over_limit_dims_exceed_a_nonzero_limit() {
    let max = [7, 8, 9];
    let input = DISPATCH_INPUTS
        .iter()
        .find(|i| i.expected == ExpectedCheck::DispatchDimsOverLimit)
        .expect("input");
    assert_eq!(input.dims, DimsSpec::OverLimit);
    let dims = input.dispatch_dims(max).expect("dims");
    assert_eq!(dims, [8, 1, 1]);
    assert_eq!(Some(dims), dispatch_dims_over_limit(max));
    assert!(dims[0] > max[0]);
    assert!(dims[1] <= max[1] && dims[2] <= max[2]);
    assert_eq!(input.dispatch_dims([u32::MAX, 1, 1]), None);
}

/// 確保の通し番号（0 始まり）が `fail_at` に等しい呼び出しで失敗し、確保した大きさと解放した大きさを記録する試験用の口。
struct FailingOps {
    fail_at: usize,
    allocated: RefCell<Vec<u64>>,
    freed: RefCell<Vec<u64>>,
    calls: RefCell<usize>,
}

impl FailingOps {
    fn new(fail_at: usize) -> Self {
        Self {
            fail_at,
            allocated: RefCell::new(Vec::new()),
            freed: RefCell::new(Vec::new()),
            calls: RefCell::new(0),
        }
    }
}

impl MemoryOps for FailingOps {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        let n = *self.calls.borrow();
        *self.calls.borrow_mut() += 1;
        if n == self.fail_at {
            return Err(ContractError::InvalidArgument("allocate".to_owned()));
        }
        self.allocated.borrow_mut().push(byte_len);
        let id = u64::try_from(n).expect("id") + 1;
        Ok(BufferHandle::__backend_new(0, id, 1, byte_len, usage))
    }

    fn upload(&self, _: &BufferHandle, _: u64, _: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, h: BufferHandle) {
        self.freed.borrow_mut().push(h.size());
    }
}

#[test]
fn allocate_failure_frees_the_buffers_taken_before() {
    let input = DISPATCH_INPUTS
        .iter()
        .find(|i| i.name == "zero_dispatch_dims")
        .expect("input");
    assert_eq!(input.buffers.len(), 3);
    let ops = FailingOps::new(2);
    let r = input.allocate_buffers(&ops);
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))));
    assert_eq!(*ops.allocated.borrow(), [4, 4]);
    assert_eq!(*ops.freed.borrow(), [4, 4]);
    assert_eq!(*ops.calls.borrow(), 3);
}

#[test]
fn allocate_success_frees_nothing() {
    let input = DISPATCH_INPUTS
        .iter()
        .find(|i| i.name == "zero_dispatch_dims")
        .expect("input");
    let ops = FailingOps::new(usize::MAX);
    let handles = input.allocate_buffers(&ops).expect("allocate");
    assert_eq!(handles.len(), 3);
    assert!(ops.freed.borrow().is_empty());
}

#[test]
fn inputs_build_contract_values() {
    let d = device();
    for input in &DISPATCH_INPUTS {
        let handles = input.allocate_buffers(d.memory()).expect("allocate");
        let bs = input.bindings(&handles);
        assert_eq!(bs.len(), input.buffers.len(), "{}", input.name);
        for (b, spec) in bs.iter().zip(input.buffers) {
            assert_eq!(b.slot, spec.slot);
            assert_eq!(b.offset, spec.offset);
            assert_eq!(b.buffer.size(), spec.byte_len);
        }
        let cs = input.constants();
        assert_eq!(cs.len(), input.constants.len());
        for (c, spec) in cs.iter().zip(input.constants) {
            assert_eq!(c.slot, spec.slot);
            assert_eq!(c.bytes.len(), spec.bytes.len());
        }
        assert_eq!(input.layout().slots, input.layout_slots);
        free_all(d.as_ref(), handles);
    }
}
