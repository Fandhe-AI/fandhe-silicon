//! 試験用のデコレータ。筋書きに当たると内側（背後実装）を呼ばずに合成の値を返す（TASK-271 の (2)〜(8)）。
//!
//! (3) 能力不足の返しは、指定した機能の能力を `Unsupported` に差し替え、その機能を要る呼び出しに
//! `CapabilityInsufficient` を返させる。(4) 能力の取得の失敗の返しは、問合せに `S0`（不明）を返させ、
//! `Unsupported` とは区別する。
//!
//! (5) 投入の回数の計数は、内側の投入が呼ばれた回数を `submit_call_count` で読ませる（再試行しないことの確認の入力）。
//! (6) 模擬の OOM は、確保を内側に渡さず `AllocationFailed` に要求の大きさを載せて返す。
//! (7) 模擬の実行時エラーは、投入は通したまま完了の報告だけを実行時エラーにする（閉じた状態は `Lost`、
//! `wait_blocking` の戻りは TASK-264 (3) に従い `BackendFailure`）。
//! (8) 上限の差し替えは、型を消した値で内側の上限を置き換える口。`LimitsTable`（TASK-157）の置き換えは
//! `Capabilities::limits()` にも効く。別の型の置き換えは `limits()` に効かず、内側の表を返す。
//!
//! `Device` のトレイトはまだ無いため、包む型を型引数にし、内側の呼び出しはクロージャで受ける。
//! `Device` が入る段で、そのメソッドがここの `*_with` を呼ぶ形にする。差し込むのは模擬であって、
//! 実際の故障ではない。名前は仮。

use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

use super::FaultHooks;
use crate::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use crate::error::{
    CompletionState, CompletionStateEx, ContractError, DeviceLostReason, UnsupportedReason,
};

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
    /// 完了の報告を実行時エラーにする（投入は内側へ通す。模擬であって実際の故障ではない。名前は仮）。
    RuntimeError {
        /// `wait_blocking` の `BackendFailure` に載せる背後実装の種類（デコレータは内側の種類を知れないため筋書きが持つ）。
        backend: BackendKind,
        /// `BackendFailure` に載せる説明。
        detail: String,
    },
}

/// 内側 `D` を包み、筋書きに従って差し込む試験用のデコレータ。
#[derive(Debug)]
pub struct FaultInjecting<D> {
    inner: D,
    script: FaultScript,
    submit_calls: AtomicU64,
    limits_override: Option<Box<dyn Any + Send + Sync>>,
}

impl<D> FaultInjecting<D> {
    /// 内側と筋書きを渡して作る。
    pub fn new(inner: D, script: FaultScript) -> Self {
        Self {
            inner,
            script,
            submit_calls: AtomicU64::new(0),
            limits_override: None,
        }
    }

    /// 上限の置き換えを持たせる。筋書きとは独立で、同時に使える。
    /// 型を消して持つ。`L` が `LimitsTable` なら `limits()` と `limits_with::<LimitsTable>` の両方に効く。
    /// 別の型なら `limits()` には効かず内側の表を返し、`limits_with::<LimitsTable>` は `InvalidArgument` を返す。
    pub fn with_limits_override<L: Any + Send + Sync>(mut self, limits: L) -> Self {
        self.limits_override = Some(Box::new(limits));
        self
    }

    /// 上限の差し込み。置き換えが無ければ内側を 1 回呼んでその値を返す。置き換えが `L` なら内側を呼ばず
    /// その複製を返す。置き換えの型が `L` でなければ、実際の上限を黙って返さず `InvalidArgument` を返す。
    pub fn limits_with<L, F>(&self, call_inner: F) -> Result<L, ContractError>
    where
        L: Any + Clone,
        F: FnOnce(&D) -> L,
    {
        match &self.limits_override {
            None => Ok(call_inner(&self.inner)),
            Some(o) => o.downcast_ref::<L>().cloned().ok_or_else(|| {
                ContractError::InvalidArgument("limits override type mismatch".into())
            }),
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
    /// `RuntimeError` なら `call_inner` を呼ばず、筋書きの `backend`・`detail` を載せた
    /// `Err(BackendFailure { op: "wait_blocking", .. })` を返す。
    /// それ以外の筋書きは内側の値をそのまま返す。
    pub fn wait_blocking_with<F>(&self, call_inner: F) -> Result<(), ContractError>
    where
        F: FnOnce(&D) -> Result<(), ContractError>,
    {
        match &self.script {
            FaultScript::CompletionUnknown => Err(ContractError::Timeout),
            FaultScript::RuntimeError { backend, detail } => Err(ContractError::BackendFailure {
                op: "wait_blocking",
                backend: *backend,
                detail: detail.clone(),
            }),
            FaultScript::PassThrough
            | FaultScript::CapabilityInsufficient { .. }
            | FaultScript::CapabilityQueryFails
            | FaultScript::AllocationFails => call_inner(&self.inner),
        }
    }

    /// 拡張の完了状態の差し込み。`CompletionUnknown` なら `call_inner` を呼ばず `Unknown` を返す。
    /// `RuntimeError` なら `call_inner` を呼ばず `RuntimeError` を返す。
    /// それ以外の筋書きは内側の値をそのまま返す。
    pub fn poll_ex_with<F>(&self, call_inner: F) -> CompletionStateEx
    where
        F: FnOnce(&D) -> CompletionStateEx,
    {
        match &self.script {
            FaultScript::CompletionUnknown => CompletionStateEx::Unknown,
            FaultScript::RuntimeError { .. } => CompletionStateEx::RuntimeError,
            FaultScript::PassThrough
            | FaultScript::CapabilityInsufficient { .. }
            | FaultScript::CapabilityQueryFails
            | FaultScript::AllocationFails => call_inner(&self.inner),
        }
    }

    /// 閉じた完了状態の差し込み。`RuntimeError` なら `call_inner` を呼ばず `Lost` を返す。
    /// それ以外は（`CompletionUnknown` を含め）内側の値をそのまま返す。
    pub fn poll_with<F>(&self, call_inner: F) -> CompletionState
    where
        F: FnOnce(&D) -> CompletionState,
    {
        match &self.script {
            FaultScript::RuntimeError { .. } => CompletionState::Lost,
            FaultScript::PassThrough
            | FaultScript::CompletionUnknown
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
            | FaultScript::AllocationFails
            | FaultScript::RuntimeError { .. } => call_inner(&self.inner),
        }
    }
}

/// 能力問合せの差し込み。`CapabilityQueryFails` は `S0`、対象の機能の `CapabilityInsufficient` は
/// `Unsupported` を内側を呼ばずに返す。それ以外は内側へそのまま渡す。
impl<D: Capabilities> Capabilities for FaultInjecting<D> {
    fn limits(&self) -> &LimitsTable {
        self.limits_override
            .as_ref()
            .and_then(|o| o.downcast_ref::<LimitsTable>())
            .unwrap_or_else(|| self.inner.limits())
    }

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
