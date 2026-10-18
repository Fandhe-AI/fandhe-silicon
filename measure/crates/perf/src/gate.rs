//! 性能の値を採る前の門の規則（一致の run だけ採る）。
//!
//! 呼び手は、照合の処理が出した「一致・不一致」の結果を run ごとに集めて渡す測定のハーネスである。
//! 結果の出どころは結果の一致の道具と画素の一致の道具の 2 種で、本 crate は閾値も比べ方も持たず、
//! 一致・不一致の結果だけを受ける。製品の crate（`core`）には依存しないため、`core` の比べの結果から
//! [`GateVerdict`] への写しは呼び手が行う。
//!
//! 規則は fail-closed で、明示の一致以外（不一致・結果なし）は採らない。採らない run には白表の理由を残し、
//! 件数を数える。同じ腕 × 行の組に採らない run が 1 件でもあれば、その組の他の run も採らない。
//! 純粋な関数で、時計にもファイルにも触れず、`record::RunRecord` の JSON は変えない。
//! 呼び手は [`GateOutcome::taken_run_ids`] で `runs::aggregate` へ渡す run を絞る。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-1026（ポインタのみ）。

use crate::json;
use crate::record::{Id, Name};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// 照合の結果の出どころ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GateSource {
    /// 結果の一致の道具。
    ResultParity,
    /// 画素の一致の道具。
    PixelParity,
}

impl GateSource {
    /// 出力用の綴り（英小文字 ASCII）。
    pub const fn name(self) -> &'static str {
        match self {
            Self::ResultParity => "result-parity",
            Self::PixelParity => "pixel-parity",
        }
    }
}

/// 照合の結果（一致・不一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    /// 一致。
    Match,
    /// 不一致。
    Mismatch,
}

/// 照合の処理が出した結果 1 件（出どころ付き）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateResult {
    /// 結果の出どころ。
    pub source: GateSource,
    /// 一致・不一致。
    pub verdict: GateVerdict,
}

/// 性能の run 1 件に対する門の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateInput {
    /// 腕の名前。
    pub arm: Name,
    /// 行の名前。
    pub row: Name,
    /// 性能の run の ID。
    pub run_id: Id,
    /// 照合の結果。`None` は結果が無い。
    pub result: Option<GateResult>,
}

/// 採らない理由（白表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotTakenReason {
    /// 結果の一致の道具が不一致を返した。
    ResultParityMismatch,
    /// 画素の一致の道具が不一致を返した。
    PixelParityMismatch,
    /// 照合の結果が無い。
    ResultMissing,
    /// 同じ腕 × 行の別の run が採られなかったので、この run も採らない。
    PairNotTaken,
}

impl NotTakenReason {
    /// 全理由。件数の表の順を決める。
    pub const ALL: [NotTakenReason; 4] = [
        Self::ResultParityMismatch,
        Self::PixelParityMismatch,
        Self::ResultMissing,
        Self::PairNotTaken,
    ];

    /// 出力用の綴り（英小文字 ASCII）。
    pub const fn name(self) -> &'static str {
        match self {
            Self::ResultParityMismatch => "result-parity-mismatch",
            Self::PixelParityMismatch => "pixel-parity-mismatch",
            Self::ResultMissing => "result-missing",
            Self::PairNotTaken => "pair-not-taken",
        }
    }
}

/// run 1 件の採否。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// 採る。
    Taken,
    /// 採らない。
    NotTaken(NotTakenReason),
}

impl Decision {
    /// 出力用の綴り（英小文字 ASCII）。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Taken => "taken",
            Self::NotTaken(_) => "not-taken",
        }
    }

    /// 採らない理由の綴り。採るときは `None`。
    pub const fn reason_name(self) -> Option<&'static str> {
        match self {
            Self::Taken => None,
            Self::NotTaken(r) => Some(r.name()),
        }
    }
}

/// run 1 件の採否の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunDecision {
    arm: Name,
    row: Name,
    run_id: Id,
    decision: Decision,
}

impl RunDecision {
    /// 腕の名前。
    pub fn arm(&self) -> &Name {
        &self.arm
    }

    /// 行の名前。
    pub fn row(&self) -> &Name {
        &self.row
    }

    /// run の ID。
    pub fn run_id(&self) -> &Id {
        &self.run_id
    }

    /// 採否。
    pub fn decision(&self) -> Decision {
        self.decision
    }
}

/// 門の失敗。入力の値は埋め込まない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GateError {
    /// 入力が 0 件。
    Empty,
    /// 同じ `run_id` が複数ある。
    DuplicateRunId,
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "empty gate input",
            Self::DuplicateRunId => "duplicate run id in gate input",
        })
    }
}

impl std::error::Error for GateError {}

/// 門を通した結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateOutcome {
    decisions: Vec<RunDecision>,
    taken_runs: u64,
    not_taken_runs: u64,
    taken_pairs: u64,
    not_taken_pairs: u64,
    reason_counts: [u64; 4],
}

impl GateOutcome {
    /// run ごとの採否。(arm, row, run_id) の昇順。
    pub fn decisions(&self) -> &[RunDecision] {
        &self.decisions
    }

    /// 採った run の件数。
    pub fn taken_run_count(&self) -> u64 {
        self.taken_runs
    }

    /// 採らなかった run の件数。
    pub fn not_taken_run_count(&self) -> u64 {
        self.not_taken_runs
    }

    /// 採った組（腕 × 行）の数。
    pub fn taken_pair_count(&self) -> u64 {
        self.taken_pairs
    }

    /// 採らなかった組の数。
    pub fn not_taken_pair_count(&self) -> u64 {
        self.not_taken_pairs
    }

    /// 理由ごとの採らなかった run の件数。[`NotTakenReason::ALL`] の順。
    pub fn reason_counts(&self) -> [(NotTakenReason, u64); 4] {
        let mut out = [(NotTakenReason::ALL[0], 0); 4];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = (NotTakenReason::ALL[i], self.reason_counts[i]);
        }
        out
    }

    /// 採った run の ID。呼び手が `runs::aggregate` へ渡す入力を絞るのに使う。
    pub fn taken_run_ids(&self) -> Vec<&Id> {
        self.decisions
            .iter()
            .filter(|d| d.decision == Decision::Taken)
            .map(|d| &d.run_id)
            .collect()
    }
}

/// 照合の結果から run 1 件の一次判定を出す。明示の一致だけが `None`（採らない理由なし）になる。
fn first_pass(result: Option<GateResult>) -> Option<NotTakenReason> {
    match result {
        Some(GateResult {
            verdict: GateVerdict::Match,
            ..
        }) => None,
        Some(GateResult {
            source,
            verdict: GateVerdict::Mismatch,
        }) => Some(match source {
            GateSource::ResultParity => NotTakenReason::ResultParityMismatch,
            GateSource::PixelParity => NotTakenReason::PixelParityMismatch,
        }),
        None => Some(NotTakenReason::ResultMissing),
    }
}

/// 門の規則を適用する。
///
/// 一致の run だけ採る。組（腕 × 行）の中に採らない run が 1 件でもあれば、その組の一致の run も
/// [`NotTakenReason::PairNotTaken`] で採らない。入力が 0 件、または `run_id` の重複はエラーにする。
pub fn apply(inputs: &[GateInput]) -> Result<GateOutcome, GateError> {
    if inputs.is_empty() {
        return Err(GateError::Empty);
    }
    let mut seen = BTreeSet::new();
    for i in inputs {
        if !seen.insert(i.run_id.as_str()) {
            return Err(GateError::DuplicateRunId);
        }
    }

    let mut pair_blocked: BTreeMap<(&Name, &Name), bool> = BTreeMap::new();
    for i in inputs {
        let blocked = first_pass(i.result).is_some();
        *pair_blocked.entry((&i.arm, &i.row)).or_insert(false) |= blocked;
    }

    let mut decisions: Vec<RunDecision> = inputs
        .iter()
        .map(|i| {
            let blocked = pair_blocked.get(&(&i.arm, &i.row)).copied().unwrap_or(true);
            let decision = match first_pass(i.result) {
                Some(r) => Decision::NotTaken(r),
                None if blocked => Decision::NotTaken(NotTakenReason::PairNotTaken),
                None => Decision::Taken,
            };
            RunDecision {
                arm: i.arm.clone(),
                row: i.row.clone(),
                run_id: i.run_id.clone(),
                decision,
            }
        })
        .collect();
    decisions.sort_by(|a, b| {
        (&a.arm, &a.row, a.run_id.as_str()).cmp(&(&b.arm, &b.row, b.run_id.as_str()))
    });

    // 件数は入力の件数以下なので、usize から u64 への変換と加算は溢れない。
    let mut taken_runs = 0u64;
    let mut reason_counts = [0u64; 4];
    for d in &decisions {
        match d.decision {
            Decision::Taken => taken_runs += 1,
            Decision::NotTaken(r) => {
                if let Some(idx) = NotTakenReason::ALL.iter().position(|x| *x == r) {
                    reason_counts[idx] += 1;
                }
            }
        }
    }
    let total = decisions.len() as u64;
    let taken_pairs = pair_blocked.values().filter(|b| !**b).count() as u64;
    let not_taken_pairs = pair_blocked.len() as u64 - taken_pairs;
    Ok(GateOutcome {
        decisions,
        taken_runs,
        not_taken_runs: total - taken_runs,
        taken_pairs,
        not_taken_pairs,
        reason_counts,
    })
}

/// 結果を決定的な 1 行の JSON にする（ASCII・欄順固定・末尾の改行なし。ファイルは書かない）。
pub fn render(o: &GateOutcome) -> String {
    let mut s = String::from("{\"output_kind\":\"gate\"");
    s.push_str(&format!(
        ",\"taken_run_count\":{},\"not_taken_run_count\":{},\"taken_pair_count\":{},\"not_taken_pair_count\":{}",
        o.taken_runs, o.not_taken_runs, o.taken_pairs, o.not_taken_pairs
    ));
    s.push_str(",\"not_taken_by_reason\":{");
    for (i, (r, n)) in o.reason_counts().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        json::push_string(&mut s, r.name());
        s.push_str(&format!(":{n}"));
    }
    s.push_str("},\"runs\":[");
    for (i, d) in o.decisions.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"arm\":");
        json::push_string(&mut s, d.arm.as_str());
        s.push_str(",\"row\":");
        json::push_string(&mut s, d.row.as_str());
        s.push_str(",\"run_id\":");
        json::push_string(&mut s, d.run_id.as_str());
        s.push_str(",\"decision\":");
        json::push_string(&mut s, d.decision.name());
        if let Some(r) = d.decision.reason_name() {
            s.push_str(",\"reason\":");
            json::push_string(&mut s, r);
        }
        s.push('}');
    }
    s.push_str("]}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    type R = Option<(GateSource, GateVerdict)>;

    fn inp(arm: &str, row: &str, id: &str, r: R) -> GateInput {
        GateInput {
            arm: Name::parse(arm).unwrap(),
            row: Name::parse(row).unwrap(),
            run_id: Id::parse(id).unwrap(),
            result: r.map(|(source, verdict)| GateResult { source, verdict }),
        }
    }

    const M: R = Some((GateSource::ResultParity, GateVerdict::Match));
    const RX: R = Some((GateSource::ResultParity, GateVerdict::Mismatch));
    const PX: R = Some((GateSource::PixelParity, GateVerdict::Mismatch));

    fn reason(o: &GateOutcome, id: &str) -> Decision {
        o.decisions()
            .iter()
            .find(|d| d.run_id().as_str() == id)
            .unwrap()
            .decision()
    }

    #[test]
    fn all_match_taken() {
        let o = apply(&[inp("a", "r", "run-1", M), inp("a", "r", "run-2", M)]).unwrap();
        assert_eq!(o.taken_run_count(), 2);
        assert_eq!(o.not_taken_run_count(), 0);
        assert_eq!(o.taken_run_ids().len(), 2);
    }

    #[test]
    fn mismatch_blocks_pair_only() {
        for (bad, why) in [
            (RX, NotTakenReason::ResultParityMismatch),
            (PX, NotTakenReason::PixelParityMismatch),
        ] {
            let o = apply(&[
                inp("a", "r", "run-1", M),
                inp("a", "r", "run-2", bad),
                inp("b", "r", "run-3", M),
            ])
            .unwrap();
            assert_eq!(reason(&o, "run-2"), Decision::NotTaken(why));
            assert_eq!(
                reason(&o, "run-1"),
                Decision::NotTaken(NotTakenReason::PairNotTaken)
            );
            assert_eq!(reason(&o, "run-3"), Decision::Taken);
            assert_eq!(o.taken_run_count(), 1);
            assert_eq!(o.not_taken_run_count(), 2);
            assert_eq!(o.not_taken_pair_count(), 1);
            assert_eq!(o.taken_pair_count(), 1);
        }
    }

    #[test]
    fn missing_result_not_taken() {
        let o = apply(&[inp("a", "r", "run-1", None)]).unwrap();
        assert_eq!(
            reason(&o, "run-1"),
            Decision::NotTaken(NotTakenReason::ResultMissing)
        );
        assert!(o.taken_run_ids().is_empty());
    }

    #[test]
    fn empty_and_duplicate_rejected() {
        assert_eq!(apply(&[]), Err(GateError::Empty));
        assert_eq!(
            apply(&[inp("a", "r", "run-1", M), inp("b", "r", "run-1", M)]),
            Err(GateError::DuplicateRunId)
        );
    }

    #[test]
    fn names_are_lowercase_ascii_and_unique() {
        let mut seen = BTreeSet::new();
        for r in NotTakenReason::ALL {
            assert!(
                r.name()
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c == b'-')
            );
            assert!(seen.insert(r.name()));
        }
        assert_eq!(GateSource::ResultParity.name(), "result-parity");
        assert_eq!(GateSource::PixelParity.name(), "pixel-parity");
    }

    #[test]
    fn render_is_deterministic_and_order_independent() {
        let a = [inp("a", "r", "run-1", M), inp("a", "r", "run-2", RX)];
        let b = [a[1].clone(), a[0].clone()];
        let sa = render(&apply(&a).unwrap());
        assert_eq!(sa, render(&apply(&b).unwrap()));
        assert!(sa.starts_with("{\"output_kind\":\"gate\""));
        assert!(sa.contains("\"reason\":\"pair-not-taken\""));
        assert!(sa.is_ascii() && !sa.ends_with('\n'));
    }
}
