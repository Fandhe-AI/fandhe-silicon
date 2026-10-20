//! 能力問合せの型。
//!
//! 背後実装の種類・機能の識別子・能力の段階・判定の根拠・問合せの結果・問合せのトレイトを置く。
//! `error` モジュールの `ContractError` もこれらを参照する。
//! 定義の持ち主は TASK-136（ポインタ）。`limits()` は TASK-157 の持ち分でここには置かない。
//! 背後実装（下-1）が値を作り、上・下-2 が受け取って比べる。

use crate::error::UnsupportedReason;

/// 背後実装の種類。`ContractError` のフィールドで、どの背後実装の失敗かを示す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BackendKind {
    /// Metal。
    Metal,
    /// Vulkan。
    Vulkan,
    /// CUDA。
    Cuda,
    /// DX12 の compute。
    Dx12Compute,
}

/// 機能の識別子。静的な名前で機能を指す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureId(pub &'static str);

/// 能力の段階。`S0` と `Unsupported` は別の値。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityLevel {
    /// 段階 S0。
    S0,
    /// 段階 S1。
    S1,
    /// 段階 S2。
    S2,
    /// 段階 S3。
    S3,
    /// 段階 S4。
    S4,
    /// 段階 S5。
    S5,
    /// 段階 S6。
    S6,
    /// 非対応。
    Unsupported,
}

/// 能力の判定の根拠の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityEvidence {
    /// 実行時の API の問合せ。
    RuntimeApi,
    /// ベンダーごとの静的な表。
    StaticVendorTable,
    /// 試しのコンパイル。
    ProbeCompile,
    /// 実装の台帳。
    ImplementationLedger,
    /// 実機の記録。
    FieldRecord,
    /// 性能の比較。
    PerformanceComparison,
}

/// 能力問合せの結果。段階に、非対応の理由と根拠を添える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityQuery {
    /// 能力の段階。
    pub level: CapabilityLevel,
    /// 非対応の理由。分からなければ `None`。
    pub unsupported_reason: Option<UnsupportedReason>,
    /// 判定の根拠。分からなければ `None`。
    pub evidence: Option<CapabilityEvidence>,
}

/// 能力問合せの口。背後実装が実装し、上・下-2 が呼ぶ。
///
/// `limits()` は別の持ち分（TASK-157。ポインタ）でここには無い。
pub trait Capabilities {
    /// 機能の能力の段階を返す。
    fn query(&self, feature: FeatureId) -> CapabilityLevel;

    /// 段階に理由と根拠を添えて返す。既定の実装は理由・根拠なしで `query` の値を返す。
    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        CapabilityQuery {
            level: self.query(feature),
            unsupported_reason: None,
            evidence: None,
        }
    }
}
