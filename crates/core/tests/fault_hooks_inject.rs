//! `inject` が種類ごとに対応する口を 1 回だけ呼ぶことの確認（spec TASK-271 の 2/8）。
//! 呼び出しを記録する偽の実装はこの試験の中だけのもの。

use std::cell::RefCell;

use fandhe_silicon_core::error::{ContractError, DeviceLostReason};
use fandhe_silicon_core::fault::{FaultHooks, HookFault, inject};

#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    fail: bool,
}

impl Recorder {
    fn record(&self, s: String) -> Result<(), ContractError> {
        self.calls.borrow_mut().push(s);
        if self.fail {
            Err(ContractError::UnsupportedExtensionDefault { op: "recorder" })
        } else {
            Ok(())
        }
    }
}

impl FaultHooks for Recorder {
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        self.record(format!("lost:{reason:?}"))
    }
    fn arm_gate(&self) -> Result<(), ContractError> {
        self.record("arm".into())
    }
    fn open_gate(&self) -> Result<(), ContractError> {
        self.record("open".into())
    }
    fn simulate_runtime_error(&self, detail: String) -> Result<(), ContractError> {
        self.record(format!("err:{detail}"))
    }
}

#[test]
fn each_kind_calls_matching_hook_once() {
    let cases = [
        (
            HookFault::DeviceLost(DeviceLostReason::Destroyed),
            "lost:Destroyed",
        ),
        (
            HookFault::DeviceLost(DeviceLostReason::Unknown),
            "lost:Unknown",
        ),
        (
            HookFault::DeviceLost(DeviceLostReason::ValidationError("v".into())),
            "lost:ValidationError(\"v\")",
        ),
        (HookFault::ArmGate, "arm"),
        (HookFault::OpenGate, "open"),
        (HookFault::RuntimeError("d".into()), "err:d"),
    ];
    for (fault, want) in cases {
        let r = Recorder::default();
        assert_eq!(inject(Some(&r), fault), Ok(()));
        assert_eq!(*r.calls.borrow(), vec![want.to_string()]);
    }
}

#[test]
fn hook_error_is_returned_as_is() {
    let r = Recorder {
        fail: true,
        ..Default::default()
    };
    assert_eq!(
        inject(Some(&r), HookFault::ArmGate),
        Err(ContractError::UnsupportedExtensionDefault { op: "recorder" })
    );
    assert_eq!(r.calls.borrow().len(), 1);
}

#[test]
fn none_returns_unsupported_without_calling() {
    assert_eq!(
        inject(None, HookFault::OpenGate),
        Err(ContractError::UnsupportedExtensionDefault { op: "fault_hooks" })
    );
}
