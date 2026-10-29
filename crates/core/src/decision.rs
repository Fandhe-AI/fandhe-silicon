//! 決定表。能力の段階・データ規模・常駐位置・数値型から、使える経路の候補とその理由を返す。
//!
//! 純関数 [`decide`] と、その入力・出力の型を置く。引数は [`DecisionInput`] だけで、戻り値は [`Decision`] だけ。
//! 出力の型に、別の経路へ切り替えることや CPU の経路を表す値は無い。
//! 上位の利用コードが `decide` を呼び、戻り値を受けて分岐する。
//! 対応づけ: TASK-141（ポインタ）。

use crate::capability::CapabilityLevel;

/// データ規模の区分。実際のデータ量からこの区分を求める操作は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DataScale {
    /// 小さい。
    Small,
    /// 大きい。
    Large,
}

/// データの常駐位置。`decide` の出力は常駐位置で変わらず、理由にも残らない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Residency {
    /// ホストにだけある。
    HostOnly,
    /// device 側にある。
    DeviceResident,
    /// ホストと device が同じ実体を共有する。
    Unified,
}

/// 数値型の区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScalarKind {
    /// 32 ビット浮動小数点。
    F32,
    /// 16 ビット浮動小数点。
    F16,
    /// 8 ビット整数。
    I8,
}

/// [`decide`] の入力。公開フィールドは 4 つ（能力の段階・データ規模・常駐位置・数値型）。
///
/// `#[non_exhaustive]` なので、他の crate からは [`DecisionInput::new`] で作る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DecisionInput {
    /// 能力の段階。
    pub capability: CapabilityLevel,
    /// データ規模。
    pub scale: DataScale,
    /// 常駐位置。
    pub residency: Residency,
    /// 数値型。
    pub scalar: ScalarKind,
}

/// 使える経路の候補。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RouteCandidate {
    /// 汎用の GPU の経路。
    GpuGeneric,
    /// 行列の演算器を使う GPU の経路。
    GpuMatrixUnit,
}

/// 経路の候補が無い理由の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NoRouteReason {
    /// 能力の段階が不明。
    Unknown,
    /// 能力の段階が非対応。
    Unsupported,
    /// 経路の候補にしてよいことが確かめられていない。
    NotVerified,
}

/// [`decide`] の出力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RouteOutput {
    /// 使える経路の候補がある。
    Route(RouteCandidate),
    /// 使える経路の候補が無い。
    NoRoute(NoRouteReason),
}

/// どの入力の値で [`RouteOutput`] になったかを表す理由。常駐位置を表す値は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecisionReason {
    /// 能力の段階が `S0`。
    CapabilityS0,
    /// 能力の段階が `Unsupported`。
    CapabilityUnsupported,
    /// 能力の段階がこの crate の知らない値。
    CapabilityUnrecognized,
    /// 能力の段階が `S1`〜`S4` で、`S5` に届かない。
    CapabilityBelowS5,
    /// データ規模が `Small`。
    ScaleSmall,
    /// 能力の段階が `S5` で、データ規模が `Large`。
    CapabilityS5ScaleLarge,
    /// 能力の段階が `S6` で、データ規模が `Large` で、数値型が `F16`。
    CapabilityS6ScaleLargeF16,
    /// 能力の段階が `S6` で、データ規模が `Large` で、数値型が `F16` 以外。
    CapabilityS6ScaleLargeOtherScalar,
}

/// [`decide`] の戻り値。出力と理由の組。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decision {
    /// 経路の候補、または無い理由の種類。
    pub output: RouteOutput,
    /// 出力になった理由。
    pub reason: DecisionReason,
}

impl DecisionInput {
    /// 4 つの値から入力を作る。
    pub fn new(
        capability: CapabilityLevel,
        scale: DataScale,
        residency: Residency,
        scalar: ScalarKind,
    ) -> Self {
        Self {
            capability,
            scale,
            residency,
            scalar,
        }
    }
}

fn no_route(kind: NoRouteReason, reason: DecisionReason) -> Decision {
    Decision {
        output: RouteOutput::NoRoute(kind),
        reason,
    }
}

fn route(candidate: RouteCandidate, reason: DecisionReason) -> Decision {
    Decision {
        output: RouteOutput::Route(candidate),
        reason,
    }
}

/// 入力から経路の判断を返す関数。引数 1 つ・戻り値 1 つだけで、同じ入力には同じ値を返す。
///
/// 段階が `S0`・`Unsupported`・この crate の知らない値のときは経路の候補を返さない。
/// 常駐位置は参照しないので、出力にも理由にも影響しない。
pub fn decide(input: DecisionInput) -> Decision {
    decide_level(Some(input.capability), input.scale, input.scalar)
}

/// 段階が `None` のものを「この crate の知らない段階」として扱う内側の関数。
///
/// 同じ crate の中では `CapabilityLevel` の値はすべて個別の腕に当たるので、`Some(_)` の末尾の腕には到達できない。
/// 他の crate から見た `non_exhaustive` の「知らない値」と同じ結果になる分岐を、試験が `None` を渡して確かめる。
fn decide_level(level: Option<CapabilityLevel>, scale: DataScale, scalar: ScalarKind) -> Decision {
    match level {
        Some(CapabilityLevel::S0) => no_route(NoRouteReason::Unknown, DecisionReason::CapabilityS0),
        Some(CapabilityLevel::Unsupported) => no_route(
            NoRouteReason::Unsupported,
            DecisionReason::CapabilityUnsupported,
        ),
        Some(
            CapabilityLevel::S1 | CapabilityLevel::S2 | CapabilityLevel::S3 | CapabilityLevel::S4,
        ) => no_route(
            NoRouteReason::NotVerified,
            DecisionReason::CapabilityBelowS5,
        ),
        Some(CapabilityLevel::S5) => match scale {
            DataScale::Large => route(
                RouteCandidate::GpuGeneric,
                DecisionReason::CapabilityS5ScaleLarge,
            ),
            DataScale::Small => no_route(NoRouteReason::NotVerified, DecisionReason::ScaleSmall),
        },
        Some(CapabilityLevel::S6) => match (scale, scalar) {
            (DataScale::Small, _) => {
                no_route(NoRouteReason::NotVerified, DecisionReason::ScaleSmall)
            }
            (DataScale::Large, ScalarKind::F16) => route(
                RouteCandidate::GpuMatrixUnit,
                DecisionReason::CapabilityS6ScaleLargeF16,
            ),
            (DataScale::Large, ScalarKind::F32 | ScalarKind::I8) => route(
                RouteCandidate::GpuGeneric,
                DecisionReason::CapabilityS6ScaleLargeOtherScalar,
            ),
        },
        // 同じ crate 内では上の腕で網羅になり、`Some(_)` は到達不能と見なされるので lint を許可する。
        #[allow(unreachable_patterns)]
        Some(_) | None => no_route(
            NoRouteReason::Unknown,
            DecisionReason::CapabilityUnrecognized,
        ),
    }
}

#[cfg(test)]
mod tests;
