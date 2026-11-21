//! 決定表の出力を受ける 2 つの利用コードの模擬と、能力の段階から決定表を呼ぶ口（TASK-219 のポインタ）。
//!
//! 試験のファイルが `mod policy_mocks;` で取り込む。fail-fast の模擬は経路の候補が無ければ `Err` を返し、
//! 縮退の模擬は経路の候補が無ければ `PrimarySlot::Unavailable` に倒して理由つきのイベントを残す。
//! どちらも共通の crate の `decide` の戻り値だけを受け、`decide` の入力にも出力にも手を加えない。
//! 縮退の模擬は、外から経路を上書きする引数を持たない。

use fandhe_silicon_core::capability::{CapabilityLevel, FeatureId};
use fandhe_silicon_core::decision::{
    DataScale, Decision, DecisionInput, Residency, RouteCandidate, RouteOutput, ScalarKind, decide,
};
use fandhe_silicon_core::device::{Device, DeviceProvider, DeviceRequest};
use fandhe_silicon_core::error::ContractError;

/// 決定表に渡すデータ規模。
pub const SCALE: DataScale = DataScale::Large;
/// 決定表に渡す常駐位置。
pub const RESIDENCY: Residency = Residency::Unified;
/// 決定表に渡す数値型。
pub const SCALAR: ScalarKind = ScalarKind::F32;

/// 能力の段階から決定表の入力を作り、`decide` を 1 回呼ぶ。段階のほかの 3 つは上の定数。
pub fn decision_for(level: CapabilityLevel) -> Decision {
    decide(DecisionInput::new(level, SCALE, RESIDENCY, SCALAR))
}

/// fail-fast の模擬が返すエラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailFastError {
    /// `request` の失敗。受けた値のまま持つ。
    Request(ContractError),
    /// 決定表が経路の候補を返さなかった。受けた `Decision` のまま持つ。
    NoRoute(Decision),
}

/// fail-fast の模擬の device の取得。名指した provider の `request` を 1 回だけ呼び、失敗は値のまま返す。
pub fn request_or_fail<P: DeviceProvider>(
    req: DeviceRequest,
) -> Result<Box<dyn Device>, FailFastError> {
    P::request(req).map_err(FailFastError::Request)
}

/// fail-fast の模擬の経路の選び。経路の候補があれば `Ok`、それ以外は受けた `Decision` を載せた `Err`。
pub fn route_or_fail(decision: Decision) -> Result<RouteCandidate, FailFastError> {
    match decision.output {
        RouteOutput::Route(candidate) => Ok(candidate),
        _ => Err(FailFastError::NoRoute(decision)),
    }
}

/// 縮退の模擬の経路の置き場。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimarySlot {
    /// 決定表の経路の候補。
    Route(RouteCandidate),
    /// 経路の候補が無く、縮退した。
    Unavailable,
}

/// 縮退の模擬が残すイベント。機能と、縮退の理由になった `Decision` を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DegradeEvent {
    /// 問い合わせた機能。
    pub feature: FeatureId,
    /// 決定表の戻り値。
    pub decision: Decision,
}

/// 縮退の模擬。経路の候補が無い `Decision` ごとにイベントを 1 つ残す。
#[derive(Debug, Default)]
pub struct Degrading {
    events: Vec<DegradeEvent>,
}

impl Degrading {
    /// 経路の候補があれば `Route`、それ以外はイベントを残して `Unavailable`。
    pub fn slot(&mut self, feature: FeatureId, decision: Decision) -> PrimarySlot {
        match decision.output {
            RouteOutput::Route(candidate) => PrimarySlot::Route(candidate),
            _ => {
                self.events.push(DegradeEvent { feature, decision });
                PrimarySlot::Unavailable
            }
        }
    }

    /// 残したイベント。残した順。
    pub fn events(&self) -> &[DegradeEvent] {
        &self.events
    }
}
