//! 3 段で受け渡すエラー・完了状態・理由の型（TASK-269）。
//!
//! 背後実装（下-1）が値を作り、下-2・上が `match` で受ける。`CompletionState` 以外の
//! enum は `#[non_exhaustive]` で、別の crate の `match` は `_` の腕を要る（ERR-18）。
//! variant には付けないので、別の crate からも値を構築できる。
//! `Display`・`std::error::Error` はここでは実装しない。

use crate::capability::{BackendKind, CapabilityLevel, FeatureId};

/// 契約の操作が返すエラー。
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
