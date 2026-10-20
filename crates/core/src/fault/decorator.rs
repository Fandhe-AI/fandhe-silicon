//! 試験用のデコレータ。筋書きに当たると内側（背後実装）を呼ばずに合成の値を返す（TASK-271 の (2)〜(6)）。
//!
//! (3) 能力不足の返しは、指定した機能の能力を `Unsupported` に差し替え、その機能を要る呼び出しに
//! `CapabilityInsufficient` を返させる。(4) 能力の取得の失敗の返しは、問合せに `S0`（不明）を返させ、
//! `Unsupported` とは区別する。
//!
//! (5) 投入の回数の計数は、内側の投入が呼ばれた回数を `submit_call_count` で読ませる（再試行しないことの確認の入力）。
//! (6) 模擬の OOM は、確保を内側に渡さず `AllocationFailed` に要求の大きさを載せて返す。
//!
//! `Device` のトレイトはまだ無いため、包む型を型引数にし、内側の呼び出しはクロージャで受ける。
//! `Device` が入る段で、そのメソッドがここの `*_with` を呼ぶ形にする。差し込むのは模擬であって、
//! 実際の故障ではない。名前は仮。

use std::sync::atomic::{AtomicU64, Ordering};

use super::FaultHooks;
use crate::capability::{Capabilities, CapabilityLevel, CapabilityQuery, FeatureId};
use crate::error::{CompletionStateEx, ContractError, DeviceLostReason, UnsupportedReason};

/// デコレータが差し込む筋書き。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FaultScript {
    /// 何も差し込まず、内側の値をそのまま返す。
    PassThrough,
    /// 完了を報告させない（待機は `Timeout`、拡張の確認は `Unknown`）。
    CompletionUnknown,
    /// 指定した機能の能力を `Unsupported` に差し替え、その機能を要る呼び出しに能力不足を返させる。
    CapabilityInsufficient {
        /// 差し替える機能。
        feature: FeatureId,
        /// 要る呼び出しが求める段階。
        required: CapabilityLevel,
    },
    /// 能力の取得を失敗させる（すべての機能で `S0`〈不明〉を返し、`Unsupported` は返さない）。
    CapabilityQueryFails,
    /// 確保を失敗させる（内側を呼ばず `AllocationFailed` を返す。名前は仮）。
    AllocationFails,
}

/// 内側 `D` を包み、筋書きに従って差し込む試験用のデコレータ。
#[derive(Debug)]
pub struct FaultInjecting<D> {
    inner: D,
    script: FaultScript,
    submit_calls: AtomicU64,
}

impl<D> FaultInjecting<D> {
    /// 内側と筋書きを渡して作る。
    pub fn new(inner: D, script: FaultScript) -> Self {
        Self {
            inner,
            script,
            submit_calls: AtomicU64::new(0),
        }
    }

    /// 内側への参照。
    pub fn inner(&self) -> &D {
        &self.inner
    }

    /// 筋書きへの参照。
    pub fn script(&self) -> &FaultScript {
        &self.script
    }

    /// 投入の差し込み。内側を呼ぶときに計数をちょうど 1 増やし、内側の値（`Ok`・`Err` とも）をそのまま返す。
    /// どの筋書きでも内側へ渡して数える（完了不明は投入後の待ち・確認の側で差し込む）。
    pub fn submit_with<T, F>(&self, call_inner: F) -> Result<T, ContractError>
    where
        F: FnOnce(&D) -> Result<T, ContractError>,
    {
        self.submit_calls.fetch_add(1, Ordering::SeqCst);
        call_inner(&self.inner)
    }

    /// 内側の投入が呼ばれた回数。
    pub fn submit_call_count(&self) -> u64 {
        self.submit_calls.load(Ordering::SeqCst)
    }

    /// 確保の差し込み。`AllocationFails` なら `call_inner` を呼ばず
    /// `Err(AllocationFailed { requested_bytes })` を返す。それ以外は内側の値をそのまま返す。
    pub fn allocate_with<T, F>(
        &self,
        requested_bytes: u64,
        call_inner: F,
    ) -> Result<T, ContractError>
    where
        F: FnOnce(&D, u64) -> Result<T, ContractError>,
    {
        match self.script {
            FaultScript::AllocationFails => {
                Err(ContractError::AllocationFailed { requested_bytes })
            }
            _ => call_inner(&self.inner, requested_bytes),
        }
    }

    /// 完了待ちの差し込み。`CompletionUnknown` なら `call_inner` を呼ばず `Err(Timeout)` を返す。
    /// それ以外は内側の値をそのまま返す。
    pub fn wait_blocking_with<F>(&self, call_inner: F) -> Result<(), ContractError>
    where
        F: FnOnce(&D) -> Result<(), ContractError>,
    {
        match self.script {
            FaultScript::CompletionUnknown => Err(ContractError::Timeout),
            FaultScript::PassThrough
            | FaultScript::CapabilityInsufficient { .. }
            | FaultScript::CapabilityQueryFails
            | FaultScript::AllocationFails => call_inner(&self.inner),
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
            FaultScript::PassThrough
            | FaultScript::CapabilityInsufficient { .. }
            | FaultScript::CapabilityQueryFails
            | FaultScript::AllocationFails => call_inner(&self.inner),
        }
    }

    /// 能力を要る呼び出し（パイプライン生成）の差し込み。`CapabilityInsufficient` なら `call_inner` を呼ばず、
    /// `Err(CapabilityInsufficient { actual: Unsupported, .. })` を返す。それ以外は内側の値をそのまま返す。
    pub fn create_pipeline_with<T, F>(&self, call_inner: F) -> Result<T, ContractError>
    where
        F: FnOnce(&D) -> Result<T, ContractError>,
    {
        match &self.script {
            FaultScript::CapabilityInsufficient { feature, required } => {
                Err(ContractError::CapabilityInsufficient {
                    feature: *feature,
                    required: *required,
                    actual: CapabilityLevel::Unsupported,
                })
            }
            FaultScript::PassThrough
            | FaultScript::CompletionUnknown
            | FaultScript::CapabilityQueryFails
            | FaultScript::AllocationFails => call_inner(&self.inner),
        }
    }
}

/// 能力問合せの差し込み。`CapabilityQueryFails` は `S0`、対象の機能の `CapabilityInsufficient` は
/// `Unsupported` を内側を呼ばずに返す。それ以外は内側へそのまま渡す。
impl<D: Capabilities> Capabilities for FaultInjecting<D> {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        match &self.script {
            FaultScript::CapabilityQueryFails => CapabilityLevel::S0,
            FaultScript::CapabilityInsufficient { feature: f, .. } if *f == feature => {
                CapabilityLevel::Unsupported
            }
            _ => self.inner.query(feature),
        }
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        match &self.script {
            FaultScript::CapabilityQueryFails => CapabilityQuery {
                level: CapabilityLevel::S0,
                unsupported_reason: None,
                evidence: None,
            },
            FaultScript::CapabilityInsufficient { feature: f, .. } if *f == feature => {
                CapabilityQuery {
                    level: CapabilityLevel::Unsupported,
                    unsupported_reason: Some(UnsupportedReason::NotSupported),
                    evidence: None,
                }
            }
            _ => self.inner.query_ex(feature),
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
