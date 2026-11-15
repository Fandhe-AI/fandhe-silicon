//! 試験の枠の、取った device を走らせる組み合わせと照らす口の試験（TASK-2945）。
//! device には触らない。device は試験用の device（`StubDeviceProvider`）で、写しは試験の中の閉包。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::stub_device::StubDeviceProvider;
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, IndeterminateReason, Label, Machine, MachineCombination,
    RunResult, RunSetting, Subject, Verdict, judge, on_acquired_device,
};
use std::cell::Cell;
use std::ffi::OsString;

fn stub(backend: BackendKind) -> Result<Box<dyn Device>, ContractError> {
    StubDeviceProvider::request(DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::ProductIndependent,
    })
}

/// 試験用の device の診断の値（`stub`）だけを `machine` に写す。
fn stub_as(machine: Machine) -> impl FnOnce(&DeviceDiagnostics) -> Option<Machine> {
    move |d| (d.backend_name == "stub" && d.adapter_name == "stub").then_some(machine)
}

fn match_body(called: &Cell<u32>) -> impl FnOnce(MachineCombination, &dyn Device) -> RunResult {
    move |_, _| {
        called.set(called.get() + 1);
        RunResult::Verdict(Verdict::Match)
    }
}

#[test]
fn an_acquire_failure_is_machine_unreachable_and_neither_the_map_nor_the_body_is_called() {
    let called = Cell::new(0);
    let mapped = Cell::new(false);
    let r = on_acquired_device(
        MachineCombination::MetalMacMini,
        |_| Err(ContractError::DeviceUnavailable("none".into())),
        |_| {
            mapped.set(true);
            Some(Machine::MacMini)
        },
        match_body(&called),
    );
    assert_eq!(
        r,
        RunResult::Indeterminate(IndeterminateReason::MachineUnreachable)
    );
    assert_eq!(called.get(), 0);
    assert!(!mapped.get());
}

#[test]
fn an_unknown_device_is_device_mismatch_and_the_body_is_not_called() {
    let called = Cell::new(0);
    let r = on_acquired_device(
        MachineCombination::MetalMacMini,
        |_| stub(BackendKind::Metal),
        |_| None,
        match_body(&called),
    );
    assert_eq!(
        r,
        RunResult::Indeterminate(IndeterminateReason::DeviceMismatch)
    );
    assert_eq!(called.get(), 0);
}

#[test]
fn another_machine_is_device_mismatch() {
    for other in [Machine::NodeA, Machine::NodeB, Machine::Amd, Machine::Intel] {
        let called = Cell::new(0);
        let r = on_acquired_device(
            MachineCombination::MetalMacMini,
            |_| stub(BackendKind::Metal),
            stub_as(other),
            match_body(&called),
        );
        assert_eq!(
            r,
            RunResult::Indeterminate(IndeterminateReason::DeviceMismatch)
        );
        assert_eq!(called.get(), 0);
    }
}

#[test]
fn another_backend_is_device_mismatch_even_when_the_map_agrees() {
    for kind in [
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ] {
        let called = Cell::new(0);
        let r = on_acquired_device(
            MachineCombination::MetalMacMini,
            |_| stub(kind),
            stub_as(Machine::MacMini),
            match_body(&called),
        );
        assert_eq!(
            r,
            RunResult::Indeterminate(IndeterminateReason::DeviceMismatch)
        );
        assert_eq!(called.get(), 0);
    }
}

#[test]
fn a_matching_device_passes_the_combination_and_the_device_to_the_body_once() {
    for (c, kind) in [
        (MachineCombination::MetalMacMini, BackendKind::Metal),
        (MachineCombination::VulkanNodeA, BackendKind::Vulkan),
        (MachineCombination::CudaNodeB, BackendKind::Cuda),
    ] {
        for v in [Verdict::Match, Verdict::Mismatch] {
            let calls = Cell::new(0);
            let r = on_acquired_device(
                c,
                |got| {
                    assert_eq!(got, c);
                    stub(kind)
                },
                stub_as(c.machine()),
                |got, d| {
                    calls.set(calls.get() + 1);
                    assert_eq!(got, c);
                    assert_eq!(d.backend(), kind);
                    RunResult::Verdict(v)
                },
            );
            assert_eq!(r, RunResult::Verdict(v));
            assert_eq!(calls.get(), 1);
        }
    }
}

#[test]
fn through_judge_the_not_run_path_does_not_acquire() {
    let s = Subject::from_parts("device_check_not_run", &[], &[]).unwrap();
    let d = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let acquired = Cell::new(false);
    let o = judge(&s, &d, &RunSetting::from_lookup(|_| None), |c| {
        on_acquired_device(
            c,
            |_| {
                acquired.set(true);
                stub(BackendKind::Metal)
            },
            stub_as(Machine::MacMini),
            |_, _| RunResult::Verdict(Verdict::Match),
        )
    });
    assert!(matches!(o.label(), Label::NotRun(_)));
    assert!(!acquired.get());
}

#[test]
fn through_judge_the_paths_give_their_labels() {
    let s = Subject::from_parts("device_check_labels", &[], &[]).unwrap();
    let d = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let st = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    let unreachable = judge(&s, &d, &st, |c| {
        on_acquired_device(
            c,
            |_| Err(ContractError::DeviceUnavailable("none".into())),
            stub_as(Machine::MacMini),
            |_, _| RunResult::Verdict(Verdict::Match),
        )
    });
    assert_eq!(
        unreachable.label(),
        &Label::Indeterminate(IndeterminateReason::MachineUnreachable)
    );
    let mismatch = judge(&s, &d, &st, |c| {
        on_acquired_device(
            c,
            |_| stub(BackendKind::Metal),
            |_| None,
            |_, _| RunResult::Verdict(Verdict::Match),
        )
    });
    assert_eq!(
        mismatch.label(),
        &Label::Indeterminate(IndeterminateReason::DeviceMismatch)
    );
    for (v, label) in [(Verdict::Match, Label::Ok), (Verdict::Mismatch, Label::Ng)] {
        let o = judge(&s, &d, &st, |c| {
            on_acquired_device(
                c,
                |_| stub(BackendKind::Metal),
                stub_as(Machine::MacMini),
                |_, _| RunResult::Verdict(v),
            )
        });
        assert_eq!(o.label(), &label);
    }
}
