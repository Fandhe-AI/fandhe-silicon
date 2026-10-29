//! テスト専用の故障の差し込み口（背後実装が実装する `FaultHooks`）と、口を呼ぶ差し込みの部品（`inject`）。
//!
//! feature `fault-injection` の配下でだけ見える。契約の公開 API ではなく、`default` と本番の組には入らない。
//! 差し込むのは模擬であって、実際の故障ではない。口は下-1（背後実装）が実装し、試験の部品が `inject` 経由で呼ぶ。
//! 口ごとに実装の持ち主が違うため、すべて既定の実装を持つ。既定の実装は黙って成功せず、
//! `ContractError::UnsupportedExtensionDefault` を返す。
//! 使い分け: (A) 試験側の部品は `decorator`（内側を包む）と `stub`（背後実装を持たない見本）、
//! (B) 背後実装が実装するのは `FaultHooks`。
//! 型・メソッド・種類の名前は仮。spec との対応は TASK-271（ポインタ表記）。

pub mod decorator;
pub mod stub;

use crate::error::{ContractError, DeviceLostReason};

/// 背後実装が実装する故障の差し込み口。実装していない口は既定の実装が `Err` を返す。
pub trait FaultHooks {
    /// 背後実装自身を、指定した理由で模擬のロストにする。2 回目以降の呼び出しは無視して `Ok` を返すこと。
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        let _ = reason;
        Err(ContractError::UnsupportedExtensionDefault {
            op: "simulate_device_lost",
        })
    }

    /// 次に作る投入を未完了のまま止める門を仕込む。
    fn arm_gate(&self) -> Result<(), ContractError> {
        Err(ContractError::UnsupportedExtensionDefault { op: "arm_gate" })
    }

    /// 仕込んだ門を開ける。何度呼んでも安全であること。
    fn open_gate(&self) -> Result<(), ContractError> {
        Err(ContractError::UnsupportedExtensionDefault { op: "open_gate" })
    }

    /// 次に作る投入の完了を、実行時エラーとして報告させる。
    fn simulate_runtime_error(&self, detail: String) -> Result<(), ContractError> {
        let _ = detail;
        Err(ContractError::UnsupportedExtensionDefault {
            op: "simulate_runtime_error",
        })
    }

    /// 検証レイヤーの報告を模して、受け口（`ValidationIntake`）へ `ValidationFailed(detail)` を届けさせる（ERR-8。TASK-554）。
    /// 報告だけを模し、device はロストさせない。
    fn simulate_validation_report(&self, detail: String) -> Result<(), ContractError> {
        let _ = detail;
        Err(ContractError::UnsupportedExtensionDefault {
            op: "simulate_validation_report",
        })
    }
}

/// `inject` が呼ぶ口の種類。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HookFault {
    /// `FaultHooks::simulate_device_lost` を呼ぶ。
    DeviceLost(DeviceLostReason),
    /// `FaultHooks::arm_gate` を呼ぶ。
    ArmGate,
    /// `FaultHooks::open_gate` を呼ぶ。
    OpenGate,
    /// `FaultHooks::simulate_runtime_error` を呼ぶ。説明を持つ。
    RuntimeError(String),
    /// `FaultHooks::simulate_validation_report` を呼ぶ。説明を持つ。
    ValidationReport(String),
}

/// 種類に対応する口を 1 回だけ呼び、戻り値をそのまま返す。
///
/// `hooks` は口を返す側の戻り値をそのまま受ける。口を持たない（`None`）ときは何も呼ばず、
/// `ContractError::UnsupportedExtensionDefault { op: "fault_hooks" }` を返す。
pub fn inject(hooks: Option<&dyn FaultHooks>, fault: HookFault) -> Result<(), ContractError> {
    let Some(hooks) = hooks else {
        return Err(ContractError::UnsupportedExtensionDefault { op: "fault_hooks" });
    };
    match fault {
        HookFault::DeviceLost(reason) => hooks.simulate_device_lost(reason),
        HookFault::ArmGate => hooks.arm_gate(),
        HookFault::OpenGate => hooks.open_gate(),
        HookFault::RuntimeError(detail) => hooks.simulate_runtime_error(detail),
        HookFault::ValidationReport(detail) => hooks.simulate_validation_report(detail),
    }
}
