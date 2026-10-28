//! 能力の問合せの共通の規則を確かめるための試験用の部品（`test-support` の裏）。
//!
//! 背後実装に依らない規則（TASK-273 の (1)〜(4)、ERR-50・ERR-51・ERR-52・ERR-62 のポインタ）を、
//! 表で値を返すスタブ（`CapabilityTable`）と、呼び出しの順の記録（`StepLog`）、
//! 「列挙・取得が成功してから機能の値を読む」2 段の手順（`query_after_acquire`）で確かめられるようにする。
//! (3)・(4) は `query_ex` の戻り値 1 件の検査（`check_query`・`check_query_ex`）で、
//! 背後実装の試験（TASK-173〜TASK-175 のポインタ）から呼ぶ。
//! 製品の経路からは使わない。ここに出てくる名前は仮で、列挙・device の本物の口が入る段
//! （TASK-157 のポインタ）で形を合わせる。
//!
//! `crate::fault` は参照しない。`test-support` だけの組（`fault-injection` 無し）でもビルドされるため。
//! デコレータとの組み合わせは統合テストの側で行う。

use std::sync::{Arc, Mutex, MutexGuard};

use crate::capability::{Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable};
use crate::error::ContractError;

/// 2 段の手順で起きた呼び出し 1 件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Step {
    /// 列挙の呼び出し。
    Enumerate,
    /// device の取得の呼び出し。
    RequestDevice,
    /// 機能の値の読み出し。
    ReadFeature(FeatureId),
}

/// 呼び出しの順の記録。複数の持ち主から共有して積む。
#[derive(Debug, Default)]
pub struct StepLog {
    steps: Mutex<Vec<Step>>,
}

impl StepLog {
    /// 空の記録を作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// 毒化していても中身を取り出して錠を取る（試験の panic を引きずらない）。
    fn lock(&self) -> MutexGuard<'_, Vec<Step>> {
        self.steps.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 1 件積む。
    pub fn push(&self, step: Step) {
        self.lock().push(step);
    }

    /// これまでの記録を積んだ順に返す。
    pub fn steps(&self) -> Vec<Step> {
        self.lock().clone()
    }

    /// `ReadFeature` の件数を返す。
    pub fn read_count(&self) -> usize {
        self.lock()
            .iter()
            .filter(|s| matches!(s, Step::ReadFeature(_)))
            .count()
    }
}

/// 機能ごとの値を表で返すスタブ。確認済みの非対応（`Unsupported`）を作る用途を持つ。
///
/// 上限の表（`limits()`）の初期値は 3 項目とも 0。`with_limits` で差し替える（TASK-157。ポインタ）。
#[derive(Debug, Clone)]
pub struct CapabilityTable {
    entries: Vec<(FeatureId, CapabilityQuery)>,
    default: CapabilityQuery,
    log: Option<Arc<StepLog>>,
    limits: LimitsTable,
}

impl Default for CapabilityTable {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityTable {
    /// 表が空で、表に無い機能には `S0`（理由・根拠なし）を返すスタブを作る。
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            default: CapabilityQuery {
                level: CapabilityLevel::S0,
                unsupported_reason: None,
                evidence: None,
            },
            log: None,
            limits: LimitsTable {
                max_buffer_bytes: 0,
                max_dispatch_dims: [0, 0, 0],
                max_texture_size: 0,
            },
        }
    }

    /// `limits()` が返す上限の表を決める。
    pub fn with_limits(mut self, limits: LimitsTable) -> Self {
        self.limits = limits;
        self
    }

    /// 機能の値を表に足す。同じ機能は後から足した方が先に当たる。
    pub fn with(mut self, feature: FeatureId, value: CapabilityQuery) -> Self {
        self.entries.insert(0, (feature, value));
        self
    }

    /// 表に無い機能に返す値を決める。
    pub fn with_default(mut self, value: CapabilityQuery) -> Self {
        self.default = value;
        self
    }

    /// 読み出しのたびに `Step::ReadFeature` を積む記録を付ける。
    pub fn with_log(mut self, log: Arc<StepLog>) -> Self {
        self.log = Some(log);
        self
    }

    fn lookup(&self, feature: FeatureId) -> CapabilityQuery {
        if let Some(log) = &self.log {
            log.push(Step::ReadFeature(feature));
        }
        self.entries
            .iter()
            .find(|(f, _)| *f == feature)
            .map(|(_, v)| *v)
            .unwrap_or(self.default)
    }
}

impl Capabilities for CapabilityTable {
    fn limits(&self) -> &LimitsTable {
        &self.limits
    }

    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        self.lookup(feature).level
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        self.lookup(feature)
    }
}

/// 列挙、device の取得、機能の読み出しの順で問い合わせる 2 段の手順。
///
/// 列挙か取得が `Err` なら、後の段も機能の読み出しも呼ばずに `S0` を返す（失敗の中身は返さない）。
/// 両方 `Ok` のときだけ、取得した値の `query(feature)` を返す。
pub fn query_after_acquire<D, E, A>(enumerate: E, acquire: A, feature: FeatureId) -> CapabilityLevel
where
    D: Capabilities,
    E: FnOnce() -> Result<(), ContractError>,
    A: FnOnce() -> Result<D, ContractError>,
{
    if enumerate().is_err() {
        return CapabilityLevel::S0;
    }
    match acquire() {
        Ok(d) => d.query(feature),
        Err(_) => CapabilityLevel::S0,
    }
}

/// 能力の問合せの戻り値が共通の規則を破っている 1 件。
///
/// どの値も、検査に渡した機能と戻り値を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityRuleViolation {
    /// `Unsupported` なのに理由が無い。
    MissingUnsupportedReason {
        /// 検査した機能。
        feature: FeatureId,
        /// 違反した戻り値。
        query: CapabilityQuery,
    },
    /// `Unsupported` でないのに理由がある。
    UnexpectedUnsupportedReason {
        /// 検査した機能。
        feature: FeatureId,
        /// 違反した戻り値。
        query: CapabilityQuery,
    },
    /// S3 以上なのに根拠が無い。
    MissingEvidence {
        /// 検査した機能。
        feature: FeatureId,
        /// 違反した戻り値。
        query: CapabilityQuery,
    },
}

/// 根拠を必ず載せるべき段階か。`S3`〜`S6` が `true`、それ以外は `false`。
///
/// `CapabilityLevel` に大小の比較が無いため、明示の `match` で判定する。
/// 段階が足されたらここがコンパイルエラーになり、更新を強制する。
pub fn level_requires_evidence(level: CapabilityLevel) -> bool {
    match level {
        CapabilityLevel::S0
        | CapabilityLevel::S1
        | CapabilityLevel::S2
        | CapabilityLevel::Unsupported => false,
        CapabilityLevel::S3 | CapabilityLevel::S4 | CapabilityLevel::S5 | CapabilityLevel::S6 => {
            true
        }
    }
}

/// `query_ex` の戻り値 1 件を検査する。
///
/// 理由の規則（`Unsupported` のときだけ理由が要る。理由の値は問わない）→ 根拠の規則
/// （S3 以上は根拠が要る。それ未満に根拠が付いていても受け入れる）の順で、違反を全部返す。
pub fn check_query(
    feature: FeatureId,
    query: CapabilityQuery,
) -> Result<(), Vec<CapabilityRuleViolation>> {
    let mut violations = Vec::new();
    let is_unsupported = query.level == CapabilityLevel::Unsupported;
    if is_unsupported && query.unsupported_reason.is_none() {
        violations.push(CapabilityRuleViolation::MissingUnsupportedReason { feature, query });
    }
    if !is_unsupported && query.unsupported_reason.is_some() {
        violations.push(CapabilityRuleViolation::UnexpectedUnsupportedReason { feature, query });
    }
    if level_requires_evidence(query.level) && query.evidence.is_none() {
        violations.push(CapabilityRuleViolation::MissingEvidence { feature, query });
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// `device.query_ex(feature)` を 1 回呼んで検査し、通れば得た値を返す。
///
/// 背後実装の試験から 1 行で呼ぶための口。
pub fn check_query_ex<D: Capabilities + ?Sized>(
    device: &D,
    feature: FeatureId,
) -> Result<CapabilityQuery, Vec<CapabilityRuleViolation>> {
    let query = device.query_ex(feature);
    check_query(feature, query).map(|()| query)
}
