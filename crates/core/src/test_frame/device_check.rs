//! 試験の側が取った device を、枠が走らせる組み合わせと照らす口（spec TASK-2945）。
//!
//! 試験の本体（[`judge`](super::judge)・[`judge_and_export`](super::judge_and_export) に渡す閉包）の中から呼ばれる。
//! device を取る関数と、device の診断の値を機材の呼び名に写す関数は試験の側が渡す。この module は device を取らず、
//! 写しの規則も持たない。照らす順は次のとおりで、どれかに当たると本体を呼ばない。
//!
//! 1. device が取れない（`acquire` が `Err`）→ 判定不能（機材に届かない）
//! 2. device の背後実装が組み合わせの背後実装と違う → 判定不能（device の食い違い）
//! 3. 写しが `None`、または組み合わせの機材と違う → 判定不能（device の食い違い）
//!
//! 写しが知らない診断の値は `None` を返す決まりなので、知らない device はどれも食い違いになる。

use super::combination::{Backend, Machine, MachineCombination};
use super::label::IndeterminateReason;
use super::outcome::RunResult;
use crate::capability::BackendKind;
use crate::device::{Device, DeviceDiagnostics};
use crate::error::ContractError;

/// 組み合わせの背後実装に当たる device の背後実装の値。
const fn backend_kind(backend: Backend) -> BackendKind {
    match backend {
        Backend::Metal => BackendKind::Metal,
        Backend::Vulkan => BackendKind::Vulkan,
        Backend::Cuda => BackendKind::Cuda,
    }
}

/// `acquire` で device を取り、`machine_of` で写した機材の呼び名を `combination` と照らしてから `body` を呼ぶ。
///
/// 照らしに当たったときの値は module の説明の 1〜3 のとおり。当たらなければ `body(combination, device)` の結果を返す。
/// `acquire` は枠が渡した組み合わせを受ける。`machine_of` は 1 回だけ、取れた device の診断の値で呼ぶ。
pub fn on_acquired_device<A, M, B>(
    combination: MachineCombination,
    acquire: A,
    machine_of: M,
    body: B,
) -> RunResult
where
    A: FnOnce(MachineCombination) -> Result<Box<dyn Device>, ContractError>,
    M: FnOnce(&DeviceDiagnostics) -> Option<Machine>,
    B: FnOnce(MachineCombination, &dyn Device) -> RunResult,
{
    let device = match acquire(combination) {
        Ok(d) => d,
        Err(_) => return RunResult::Indeterminate(IndeterminateReason::MachineUnreachable),
    };
    if device.backend() != backend_kind(combination.backend()) {
        return RunResult::Indeterminate(IndeterminateReason::DeviceMismatch);
    }
    if machine_of(&device.diagnostics()) != Some(combination.machine()) {
        return RunResult::Indeterminate(IndeterminateReason::DeviceMismatch);
    }
    body(combination, &*device)
}
