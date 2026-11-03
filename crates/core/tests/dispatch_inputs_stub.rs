//! feature `test-support` の裏の名前つきの入力の組を、試験用の device で確保したバッファに当てて、
//! 各入力が当たる検査（接頭辞か `Ok(())`）を確かめる統合テスト（TASK-2944 の 1/2）。
//!
//! 公開のパスだけを使う。拒否の文言は接頭辞より後ろを固定しない。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::dispatch_inputs::{DISPATCH_INPUTS, DispatchInput, ExpectedCheck};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    OperandBinding, validate_declared_operands, validate_dispatch_operands,
};
use fandhe_silicon_core::memory::BufferHandle;
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
    for input in &DISPATCH_INPUTS {
        if !matches!(
            input.expected,
            ExpectedCheck::ZeroDispatchDims | ExpectedCheck::DispatchDimsOverLimit
        ) {
            continue;
        }
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
