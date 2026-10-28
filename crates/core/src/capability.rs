//! 能力問合せの型。
//!
//! 背後実装の種類・機能の識別子・能力の段階・判定の根拠・問合せの結果・問合せのトレイトを置く。
//! `error` モジュールの `ContractError` もこれらを参照する。
//! 定義の持ち主は TASK-136（ポインタ）。上限の表 `LimitsTable` と `Capabilities::limits()` もここに置く（TASK-157・DEV-110・DEV-114・DEV-115。ポインタ）。
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

/// 上限の表。バッファの大きさ・dispatch の次元・テクスチャの大きさの 3 項目を持つ（TASK-157。ポインタ）。
///
/// 背後実装が値を作り、`Capabilities::limits()` で上・下-2 へ渡す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitsTable {
    /// バッファ 1 本の大きさの上限（バイト）。
    pub max_buffer_bytes: u64,
    /// dispatch の各次元の上限。
    pub max_dispatch_dims: [u32; 3],
    /// テクスチャの 1 辺の大きさの上限。
    pub max_texture_size: u32,
}

/// 能力問合せの口。背後実装が実装し、上・下-2 が呼ぶ。
///
/// `limits()` は必須の操作で、既定の実装は無い（TASK-157。ポインタ）。
pub trait Capabilities {
    /// 機能の能力の段階を返す。
    fn query(&self, feature: FeatureId) -> CapabilityLevel;

    /// 上限の表を返す。
    fn limits(&self) -> &LimitsTable;

    /// 段階に理由と根拠を添えて返す。既定の実装は理由・根拠なしで `query` の値を返す。
    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        CapabilityQuery {
            level: self.query(feature),
            unsupported_reason: None,
            evidence: None,
        }
    }
}
