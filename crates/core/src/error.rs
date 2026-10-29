//! 3 段で受け渡すエラー・完了状態・理由の型（TASK-269）。
//!
//! 背後実装（下-1）が値を作り、下-2・上が `match` で受ける。`CompletionState` 以外の
//! enum は `#[non_exhaustive]` で、別の crate の `match` は `_` の腕を要る（ERR-18）。
//! variant には付けないので、別の crate からも値を構築できる。
//! `ContractError` には `Display` と `std::error::Error` を実装する（他の型には実装しない）。
//!
//! 失敗の種類 `FailureClass` は、`ContractError` と `CompletionStateEx` の `failure_class` から得る（ERR-21。TASK-554 のポインタ）。
//!
//! ここにはエラーに関する型だけを置き、その定義と `#[non_exhaustive]` の確かめは TASK-269 が持つ。
//! 能力・診断・device の型は TASK-136 の持ち分（ポインタ）。
//! `CompletionState`・`CompletionStateEx` の値の一覧は TASK-306 の決定に従う（ポインタのみ）。

use std::fmt;

use crate::capability::{BackendKind, CapabilityLevel, FeatureId};

/// 契約の操作が返すエラー。
///
/// `Display` は値ごとに英語の決まった句で始まり、フィールドが続く（ERR-54。ポインタのみ）。
/// 文字列のフィールドは背後実装が載せたものを変えずに出すので、改行や非 ASCII を含みうる
/// （1 行のログにしたい利用側は `{:?}` を使うか自分でエスケープする）。`source()` は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContractError {
    /// 要求の組み合わせが成り立たない。
    InvalidCombination(String),
    /// device が使えない。
    DeviceUnavailable(String),
    /// device が失われた。`generation` は失われた時点の世代。
    DeviceLost {
        /// 失われた device の世代。
        generation: u64,
    },
    /// 渡された資源が別の device のものだった。
    DeviceMismatch,
    /// 引数が不正。
    InvalidArgument(String),
    /// 確保に失敗した。
    AllocationFailed {
        /// 要求したバイト数。
        requested_bytes: u64,
    },
    /// 成果物が受け付けられなかった。
    ArtifactRejected {
        /// 成果物の種類の名前。
        kind: &'static str,
        /// 受け付けなかった理由の説明。
        reason: String,
    },
    /// 検証に失敗した（ERR-8）。
    ValidationFailed(String),
    /// 時間切れ。
    Timeout,
    /// 背後実装がその操作に対応していない。
    Unsupported {
        /// 操作の名前。
        op: &'static str,
        /// 対応していない背後実装。
        backend: BackendKind,
    },
    /// 能力の段階が足りない。
    CapabilityInsufficient {
        /// 対象の機能。
        feature: FeatureId,
        /// 要る段階。
        required: CapabilityLevel,
        /// 実際の段階。
        actual: CapabilityLevel,
    },
    /// device の識別が一致しない、または判定できない。
    DeviceIdentityMismatch {
        /// 一致しない理由。
        reason: IdentityMismatchReason,
    },
    /// 拡張の操作の既定の実装が対応していない。
    UnsupportedExtensionDefault {
        /// 操作の名前。
        op: &'static str,
    },
    /// 背後実装の呼び出しが失敗した。
    BackendFailure {
        /// 操作の名前。
        op: &'static str,
        /// 失敗した背後実装。
        backend: BackendKind,
        /// 失敗の説明。
        detail: String,
    },
    /// 数に上限のある資源が上限に達した。
    CapacityExceeded {
        /// 操作の名前。
        op: &'static str,
        /// 上限に達した背後実装。
        backend: BackendKind,
        /// 上限の個数。
        capacity: u32,
    },
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCombination(s) => write!(f, "invalid combination: {s}"),
            Self::DeviceUnavailable(s) => write!(f, "device unavailable: {s}"),
            Self::DeviceLost { generation } => write!(f, "device lost (generation {generation})"),
            Self::DeviceMismatch => f.write_str("handle belongs to a different device"),
            Self::InvalidArgument(s) => write!(f, "invalid argument: {s}"),
            Self::AllocationFailed { requested_bytes } => {
                write!(f, "allocation failed ({requested_bytes} bytes requested)")
            }
            Self::ArtifactRejected { kind, reason } => {
                write!(f, "artifact rejected ({kind}): {reason}")
            }
            Self::ValidationFailed(s) => write!(f, "validation failed: {s}"),
            Self::Timeout => f.write_str("timed out waiting for completion"),
            Self::Unsupported { op, backend } => write!(f, "unsupported on {backend:?}: {op}"),
            Self::CapabilityInsufficient {
                feature,
                required,
                actual,
            } => write!(
                f,
                "capability insufficient for {}: requires {required:?}, actual {actual:?}",
                feature.0
            ),
            Self::DeviceIdentityMismatch { reason } => {
                write!(f, "physical device identity mismatch ({reason:?})")
            }
            Self::UnsupportedExtensionDefault { op } => {
                write!(f, "extension operation not implemented: {op}")
            }
            Self::BackendFailure {
                op,
                backend,
                detail,
            } => write!(f, "{backend:?} call failed: {op}: {detail}"),
            Self::CapacityExceeded {
                op,
                backend,
                capacity,
            } => write!(
                f,
                "capacity exceeded on {backend:?}: {op} (capacity {capacity})"
            ),
        }
    }
}

impl std::error::Error for ContractError {}

/// 失敗の種類。`ContractError` と `CompletionStateEx` の値から `failure_class` で得る（ERR-21。TASK-554 のポインタ）。
///
/// 名前は仮。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FailureClass {
    /// 初期化の失敗（device や成果物を用意できない）。
    Initialization,
    /// 実行時エラー（device は失われていない呼び出し・実行の失敗）。
    Runtime,
    /// device のロスト。
    DeviceLost,
    /// 完了不明（完了したかを確かめられない）。
    CompletionUnknown,
    /// 能力不足（その device・環境ではその操作を行えない）。
    Capability,
}

impl ContractError {
    /// この値の失敗の種類。すべての値が種類を持つ。
    ///
    /// 内側の `match` は `_` を持たない。値を足すときは同じ変更で種類も決める（コンパイルが落ちて気づける）。
    pub fn failure_class(&self) -> FailureClass {
        match self {
            Self::InvalidCombination(_)
            | Self::DeviceUnavailable(_)
            | Self::ArtifactRejected { .. } => FailureClass::Initialization,
            Self::InvalidArgument(_)
            | Self::DeviceMismatch
            | Self::AllocationFailed { .. }
            | Self::ValidationFailed(_)
            | Self::BackendFailure { .. }
            | Self::DeviceIdentityMismatch { .. }
            | Self::CapacityExceeded { .. } => FailureClass::Runtime,
            Self::DeviceLost { .. } => FailureClass::DeviceLost,
            Self::Timeout => FailureClass::CompletionUnknown,
            Self::CapabilityInsufficient { .. }
            | Self::Unsupported { .. }
            | Self::UnsupportedExtensionDefault { .. } => FailureClass::Capability,
        }
    }
}

impl CompletionStateEx {
    /// この完了状態が表す失敗の種類。失敗でない `Pending`・`Complete` は `None`。
    pub fn failure_class(&self) -> Option<FailureClass> {
        match self {
            Self::Pending | Self::Complete => None,
            Self::RuntimeError => Some(FailureClass::Runtime),
            Self::Lost => Some(FailureClass::DeviceLost),
            Self::Unknown => Some(FailureClass::CompletionUnknown),
        }
    }
}

/// device が失われた理由。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeviceLostReason {
    /// 利用側が破棄した。
    Destroyed,
    /// 理由が分からない。
    Unknown,
    /// 検証のエラーによる。説明を持つ。
    ValidationError(String),
    /// 回復できない失敗による。説明を持つ。
    Unrecoverable(String),
}

/// 実行の完了状態。閉じた 3 値（`#[non_exhaustive]` を付けない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionState {
    /// 未完了。
    Pending,
    /// 完了した。
    Complete,
    /// device の喪失で完了しない。
    Lost,
}

/// 実行の完了状態の拡張版。実行時エラーと不明を区別する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompletionStateEx {
    /// 未完了。
    Pending,
    /// 完了した。
    Complete,
    /// 実行時にエラーが起きた。
    RuntimeError,
    /// device の喪失で完了しない。
    Lost,
    /// 状態が分からない。
    Unknown,
}

/// 失敗した操作を再試行してよいかの宣言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetryPolicy {
    /// 冪等で、再試行して安全。
    IdempotentSafe,
    /// 冪等でなく、再試行は安全でない。
    NotIdempotent,
}

/// device の識別が合わない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdentityMismatchReason {
    /// 識別が一致しない。
    Mismatch,
    /// 一致するか判定できない。
    Undeterminable,
}

/// 非対応の理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsupportedReason {
    /// 対応する API が無い。
    NoApi,
    /// API はあるが対応していない。
    NotSupported,
    /// 機能の識別子が未知。
    UnknownFeature,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `CompletionStateEx` の 5 値の一覧を固定する。定義した crate の内側なので
    /// `#[non_exhaustive]` でも `_` なしで網羅できる。値を足すときはこの試験も直す。
    #[test]
    fn completion_state_ex_has_five_values() {
        let all = [
            CompletionStateEx::Pending,
            CompletionStateEx::Complete,
            CompletionStateEx::RuntimeError,
            CompletionStateEx::Lost,
            CompletionStateEx::Unknown,
        ];
        let mut seen = [false; 5];
        for v in all {
            let i = match v {
                CompletionStateEx::Pending => 0,
                CompletionStateEx::Complete => 1,
                CompletionStateEx::RuntimeError => 2,
                CompletionStateEx::Lost => 3,
                CompletionStateEx::Unknown => 4,
            };
            seen[i] = true;
        }
        assert!(seen.iter().all(|b| *b));
    }
}
