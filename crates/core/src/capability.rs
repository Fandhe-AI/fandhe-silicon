//! エラーの型のフィールドが参照する、能力まわりの最小の型。
//!
//! `error` モジュールの `ContractError` が、背後実装の種類・機能の識別子・能力の段階を
//! フィールドに持つために置く。device・能力問合せの残りの型は TASK-136 の持ち分（ポインタ）で、ここには置かない。
//! ここの 3 型の定義の持ち主は TASK-136 で、`#[non_exhaustive]` の確かめは TASK-269 の試験の対象に入れない。
//! 背後実装（下-1）が値を作り、上・下-2 が受け取って比べる。

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
