//! 試験用のデコレータ。筋書きに当たると内側（背後実装）を呼ばずに合成の値を返す（TASK-271 の (2)）。
//!
//! `Device` のトレイトはまだ無いため、包む型を型引数にし、内側の呼び出しはクロージャで受ける。
//! `Device` が入る段で、そのメソッドがここの `*_with` を呼ぶ形にする。差し込むのは模擬であって、
//! 実際の故障ではない。名前は仮。

use super::FaultHooks;
use crate::error::{CompletionStateEx, ContractError, DeviceLostReason};

/// デコレータが差し込む筋書き。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FaultScript {
    /// 何も差し込まず、内側の値をそのまま返す。
    PassThrough,
    /// 完了を報告させない（待機は `Timeout`、拡張の確認は `Unknown`）。
    CompletionUnknown,
}

/// 内側 `D` を包み、筋書きに従って差し込む試験用のデコレータ。
#[derive(Debug)]
pub struct FaultInjecting<D> {
    inner: D,
    script: FaultScript,
}

impl<D> FaultInjecting<D> {
    /// 内側と筋書きを渡して作る。
    pub fn new(inner: D, script: FaultScript) -> Self {
        Self { inner, script }
    }

    /// 内側への参照。
    pub fn inner(&self) -> &D {
        &self.inner
    }

    /// 筋書きへの参照。
    pub fn script(&self) -> &FaultScript {
        &self.script
    }

    /// 完了待ちの差し込み。`CompletionUnknown` なら `call_inner` を呼ばず `Err(Timeout)` を返す。
    /// それ以外は内側の値をそのまま返す。
    pub fn wait_blocking_with<F>(&self, call_inner: F) -> Result<(), ContractError>
    where
        F: FnOnce(&D) -> Result<(), ContractError>,
    {
        match self.script {
            FaultScript::CompletionUnknown => Err(ContractError::Timeout),
            FaultScript::PassThrough => call_inner(&self.inner),
        }
    }

    /// 拡張の完了状態の差し込み。`CompletionUnknown` なら `call_inner` を呼ばず `Unknown` を返す。
    /// それ以外は内側の値をそのまま返す。
    pub fn poll_ex_with<F>(&self, call_inner: F) -> CompletionStateEx
    where
        F: FnOnce(&D) -> CompletionStateEx,
    {
        match self.script {
            FaultScript::CompletionUnknown => CompletionStateEx::Unknown,
            FaultScript::PassThrough => call_inner(&self.inner),
        }
    }
}

/// 口は内側へそのまま渡す。内側が既定の実装の口では、その `Err` がそのまま返る。
impl<D: FaultHooks> FaultHooks for FaultInjecting<D> {
    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
        self.inner.simulate_device_lost(reason)
    }

    fn arm_gate(&self) -> Result<(), ContractError> {
        self.inner.arm_gate()
    }

    fn open_gate(&self) -> Result<(), ContractError> {
        self.inner.open_gate()
    }

    fn simulate_runtime_error(&self, detail: String) -> Result<(), ContractError> {
        self.inner.simulate_runtime_error(detail)
    }
}
