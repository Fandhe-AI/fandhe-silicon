//! 返した経路と、呼び出しの後に読んだ値を突き合わせて、1 回ごとに一致・不一致を出す純粋関数の置き場。
//!
//! 突き合わせる値は、印の値・呼び出しの前後の実行の回数・出力の列（番兵のバイト `0xA5` で埋めた列）・
//! 使った成果物の SHA-256 の 4 つ。入力は素の値とこの module の型だけで、device・buffer には触れず、
//! ファイルと環境変数を読まない。
//!
//! feature `test-support` の裏にだけあり、試験が使う。製品の経路からは使わない。
//! ポインタ: `docs/spec/05-tasks.md` の TASK-922。

use crate::hash::sha256_bytes;
use crate::test_frame::{IndeterminateReason, RunResult, Verdict};

/// 経路 `Native` の呼び出しの後に期待する印の値。
pub const MARKER_NATIVE: u32 = 0x4E;

/// 経路 `GpuEmulated` の呼び出しの後に期待する印の値。
pub const MARKER_GPU_EMULATED: u32 = 0x47;

/// 経路 `Cpu` と経路なしの呼び出しの後に期待する印の値（書き換わらないまま 0）。
pub const MARKER_CLEAR: u32 = 0;

/// [`fill_sentinel`] が出力の列に埋めるバイト。
pub const SENTINEL_BYTE: u8 = 0xA5;

/// 返された経路。経路なしは `Option<ReportedRoute>` の `None` で表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportedRoute {
    /// 背後実装の本来の経路。
    Native,
    /// GPU 上の代わりの経路。
    GpuEmulated,
    /// CPU の代わりの経路。
    Cpu,
}

/// 呼び出しの前または後に読んだ実行の回数の組。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunCounts {
    /// 経路 `Native` の実行の回数。
    pub native: u64,
    /// 経路 `GpuEmulated` の実行の回数。
    pub gpu_emulated: u64,
    /// 経路 `Cpu` の実行の回数。
    pub cpu: u64,
    /// 経路の実行に数えない試しの実行の回数。
    pub probe: u64,
}

/// 入力の誤り。値の不一致はこの型ではなく [`CallCheck`] の `false` で返る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvidenceError {
    /// 出力の列が空。
    EmptyOutput,
    /// 後の回数が前の回数より小さい欄がある。
    CountsDecreased,
    /// [`ArtifactDigests`] の 2 つの SHA-256 が等しい。
    SameArtifactDigest,
    /// 数え上げの欄があふれる。
    TallyOverflow,
}

impl EvidenceError {
    /// 固定の英語の文言を返す。入力の値は含まない。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmptyOutput => "output is empty",
            Self::CountsDecreased => "run counts decreased",
            Self::SameArtifactDigest => "artifact digests are equal",
            Self::TallyOverflow => "evidence tally overflowed",
        }
    }
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for EvidenceError {}

/// 1 つの操作の `Native` の成果物と `GpuEmulated` の成果物の SHA-256 の組。2 つは等しくない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactDigests {
    native: [u8; 32],
    gpu_emulated: [u8; 32],
}

impl ArtifactDigests {
    /// SHA-256 の組を作る。2 つが等しければ [`EvidenceError::SameArtifactDigest`]。
    pub fn new(native: [u8; 32], gpu_emulated: [u8; 32]) -> Result<Self, EvidenceError> {
        if native == gpu_emulated {
            return Err(EvidenceError::SameArtifactDigest);
        }
        Ok(Self {
            native,
            gpu_emulated,
        })
    }

    /// 2 つの成果物のバイト列から SHA-256 を作って組にする。
    pub fn from_artifacts(native: &[u8], gpu_emulated: &[u8]) -> Result<Self, EvidenceError> {
        Self::new(sha256_bytes(native), sha256_bytes(gpu_emulated))
    }

    /// `Native` の成果物の SHA-256。
    pub fn native(&self) -> [u8; 32] {
        self.native
    }

    /// `GpuEmulated` の成果物の SHA-256。
    pub fn gpu_emulated(&self) -> [u8; 32] {
        self.gpu_emulated
    }
}

/// 1 回の呼び出しの後に読んだ値。
#[derive(Debug, Clone, Copy)]
pub struct CallEvidence<'a> {
    /// 読んだ印の値。
    pub marker: u32,
    /// 呼び出しの前に読んだ回数。
    pub before: RunCounts,
    /// 呼び出しの後に読んだ回数。
    pub after: RunCounts,
    /// 呼び出しの後の出力の列。呼び出しの前に [`fill_sentinel`] で埋めた列を渡した物。
    pub output: &'a [u8],
    /// その呼び出しが使った成果物の SHA-256。使わなければ `None`。
    pub artifact: Option<[u8; 32]>,
}

/// 1 回の呼び出しの照合の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallCheck {
    marker: bool,
    counts: bool,
    output: bool,
    artifact: Option<bool>,
    probe_runs: u64,
}

impl CallCheck {
    /// 印の値が期待と一致したか。
    pub fn marker(&self) -> bool {
        self.marker
    }

    /// 経路の実行の回数の差が期待と一致したか。試しの実行の回数は含まない。
    pub fn counts(&self) -> bool {
        self.counts
    }

    /// 出力の列が期待と一致したか。
    pub fn output(&self) -> bool {
        self.output
    }

    /// 成果物の SHA-256 が期待と一致したか。比べなかったときは `None`。
    pub fn artifact(&self) -> Option<bool> {
        self.artifact
    }

    /// 試しの実行の回数の差。
    pub fn probe_runs(&self) -> u64 {
        self.probe_runs
    }

    /// 印・回数・出力がすべて一致し、成果物が不一致でないとき真。
    pub fn is_match(&self) -> bool {
        self.marker && self.counts && self.output && self.artifact != Some(false)
    }
}

/// 出力の列の全バイトを [`SENTINEL_BYTE`] にする。
pub fn fill_sentinel(out: &mut [u8]) {
    out.fill(SENTINEL_BYTE);
}

/// 出力の列の全バイトが [`SENTINEL_BYTE`] なら真。空の列は真。
pub fn is_untouched(out: &[u8]) -> bool {
    out.iter().all(|&b| b == SENTINEL_BYTE)
}

/// 返した経路と読んだ値を突き合わせる。
///
/// `artifacts` が `Some` のときだけ成果物の SHA-256 を比べる（`None` なら結果の `artifact` は `None`）。
/// 出力の列が空、または後の回数が前の回数より小さい欄があるときは `Err`。
/// 値の不一致は `Ok` の [`CallCheck`] の `false` で返る。
pub fn check_call(
    route: Option<ReportedRoute>,
    evidence: &CallEvidence<'_>,
    artifacts: Option<&ArtifactDigests>,
) -> Result<CallCheck, EvidenceError> {
    if evidence.output.is_empty() {
        return Err(EvidenceError::EmptyOutput);
    }
    let diff = |after: u64, before: u64| {
        after
            .checked_sub(before)
            .ok_or(EvidenceError::CountsDecreased)
    };
    let native = diff(evidence.after.native, evidence.before.native)?;
    let gpu_emulated = diff(evidence.after.gpu_emulated, evidence.before.gpu_emulated)?;
    let cpu = diff(evidence.after.cpu, evidence.before.cpu)?;
    let probe_runs = diff(evidence.after.probe, evidence.before.probe)?;

    // 経路ごとの期待: 印の値・回数の差・出力が書き換わったか・成果物の SHA-256。
    let (marker, expected_counts, touched, expected_artifact) = match route {
        Some(ReportedRoute::Native) => (
            MARKER_NATIVE,
            (1, 0, 0),
            true,
            artifacts.map(|a| Some(a.native())),
        ),
        Some(ReportedRoute::GpuEmulated) => (
            MARKER_GPU_EMULATED,
            (0, 1, 0),
            true,
            artifacts.map(|a| Some(a.gpu_emulated())),
        ),
        Some(ReportedRoute::Cpu) => (MARKER_CLEAR, (0, 0, 1), true, artifacts.map(|_| None)),
        None => (MARKER_CLEAR, (0, 0, 0), false, artifacts.map(|_| None)),
    };

    Ok(CallCheck {
        marker: evidence.marker == marker,
        counts: (native, gpu_emulated, cpu) == expected_counts,
        output: is_untouched(evidence.output) != touched,
        artifact: expected_artifact.map(|expected| evidence.artifact == expected),
        probe_runs,
    })
}

/// 1 回ごとの照合の結果 [`CallCheck`] を呼び出しの列として数える。
///
/// 試験の本体が [`EvidenceTally::run_result`] を試験の枠の `judge` の戻り値にして使う。
/// 欄はすべて `u64` で、あふれる足し算は [`EvidenceError::TallyOverflow`] で拒否し、そのとき数え上げは変わらない。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvidenceTally {
    calls: u64,
    matched: u64,
    mismatched: u64,
    marker_mismatches: u64,
    count_mismatches: u64,
    output_mismatches: u64,
    artifact_mismatches: u64,
    artifact_not_compared: u64,
    probe_runs: u64,
}

impl EvidenceTally {
    /// 0 件の数え上げを返す。
    pub fn new() -> Self {
        Self::default()
    }

    /// 照合の結果を 1 件数える。
    ///
    /// 呼び出しの数は常に 1 増え、`is_match()` が真なら一致、偽なら不一致に数える。
    /// 印・回数・出力の `false` はそれぞれの不一致の欄に、成果物は `Some(false)` を不一致の欄に、`None` を比べなかった欄に数える。
    /// 試しの実行の回数は合計に足す。あふれたら `Err` で、数え上げは変わらない。
    pub fn record(&mut self, check: &CallCheck) -> Result<(), EvidenceError> {
        let add = |field: u64, by: u64| field.checked_add(by).ok_or(EvidenceError::TallyOverflow);
        let flag = |b: bool| u64::from(b);
        let matched = check.is_match();
        let mut next = self.clone();
        next.calls = add(next.calls, 1)?;
        next.matched = add(next.matched, flag(matched))?;
        next.mismatched = add(next.mismatched, flag(!matched))?;
        next.marker_mismatches = add(next.marker_mismatches, flag(!check.marker()))?;
        next.count_mismatches = add(next.count_mismatches, flag(!check.counts()))?;
        next.output_mismatches = add(next.output_mismatches, flag(!check.output()))?;
        next.artifact_mismatches = add(
            next.artifact_mismatches,
            flag(check.artifact() == Some(false)),
        )?;
        next.artifact_not_compared =
            add(next.artifact_not_compared, flag(check.artifact().is_none()))?;
        next.probe_runs = add(next.probe_runs, check.probe_runs())?;
        *self = next;
        Ok(())
    }

    /// [`check_call`] を呼び、成功した結果を数えて返す。
    ///
    /// `check_call` の `Err` はそのまま返し、数えない。数え上げのあふれは `Err(TallyOverflow)` で、数え上げは変わらない。
    pub fn record_call(
        &mut self,
        route: Option<ReportedRoute>,
        evidence: &CallEvidence<'_>,
        artifacts: Option<&ArtifactDigests>,
    ) -> Result<CallCheck, EvidenceError> {
        let check = check_call(route, evidence, artifacts)?;
        self.record(&check)?;
        Ok(check)
    }

    /// 数えた呼び出しの数。
    pub fn calls(&self) -> u64 {
        self.calls
    }

    /// 一致に数えた呼び出しの数。
    pub fn matched(&self) -> u64 {
        self.matched
    }

    /// 不一致に数えた呼び出しの数。
    pub fn mismatched(&self) -> u64 {
        self.mismatched
    }

    /// 印の値が期待と違った呼び出しの数。
    pub fn marker_mismatches(&self) -> u64 {
        self.marker_mismatches
    }

    /// 回数の差が期待と違った呼び出しの数。
    pub fn count_mismatches(&self) -> u64 {
        self.count_mismatches
    }

    /// 出力の列が期待と違った呼び出しの数。
    pub fn output_mismatches(&self) -> u64 {
        self.output_mismatches
    }

    /// 成果物の SHA-256 が期待と違った呼び出しの数。
    pub fn artifact_mismatches(&self) -> u64 {
        self.artifact_mismatches
    }

    /// 成果物を比べなかった呼び出しの数。
    pub fn artifact_not_compared(&self) -> u64 {
        self.artifact_not_compared
    }

    /// 試しの実行の回数の合計。一致・不一致の判定には入らない。
    pub fn probe_runs(&self) -> u64 {
        self.probe_runs
    }

    /// 試験の枠の `judge` の本体が返す結果にする。
    ///
    /// 0 件なら `Indeterminate(EvidenceMissing)`、不一致が 1 件以上なら `Verdict(Mismatch)`、それ以外は `Verdict(Match)`。
    pub fn run_result(&self) -> RunResult {
        if self.calls == 0 {
            RunResult::Indeterminate(IndeterminateReason::EvidenceMissing)
        } else if self.mismatched > 0 {
            RunResult::Verdict(Verdict::Mismatch)
        } else {
            RunResult::Verdict(Verdict::Match)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WRITTEN: &[u8] = b"\x01\x02\x03\x04";
    const UNTOUCHED: &[u8] = &[SENTINEL_BYTE; 4];

    const ROUTES: [Option<ReportedRoute>; 4] = [
        Some(ReportedRoute::Native),
        Some(ReportedRoute::GpuEmulated),
        Some(ReportedRoute::Cpu),
        None,
    ];

    fn digests() -> ArtifactDigests {
        ArtifactDigests::from_artifacts(b"native", b"gpu-emulated").unwrap()
    }

    fn counts(native: u64, gpu_emulated: u64, cpu: u64) -> RunCounts {
        RunCounts {
            native,
            gpu_emulated,
            cpu,
            probe: 0,
        }
    }

    /// 経路ごとの、すべて一致する入力と期待の SHA-256 の組。
    fn good(route: Option<ReportedRoute>) -> (CallEvidence<'static>, ArtifactDigests) {
        let d = digests();
        let (marker, after, artifact, output) = match route {
            Some(ReportedRoute::Native) => {
                (MARKER_NATIVE, counts(1, 0, 0), Some(d.native()), WRITTEN)
            }
            Some(ReportedRoute::GpuEmulated) => (
                MARKER_GPU_EMULATED,
                counts(0, 1, 0),
                Some(d.gpu_emulated()),
                WRITTEN,
            ),
            Some(ReportedRoute::Cpu) => (MARKER_CLEAR, counts(0, 0, 1), None, WRITTEN),
            None => (MARKER_CLEAR, counts(0, 0, 0), None, UNTOUCHED),
        };
        let ev = CallEvidence {
            marker,
            before: RunCounts::default(),
            after,
            output,
            artifact,
        };
        (ev, d)
    }

    #[test]
    fn matching_values_match_for_every_route() {
        for route in ROUTES {
            let (ev, d) = good(route);
            let c = check_call(route, &ev, Some(&d)).unwrap();
            assert!(c.is_match(), "{route:?}");
            assert_eq!(c.artifact(), Some(true));
            assert_eq!(c.probe_runs(), 0);
        }
    }

    #[test]
    fn each_value_mismatch_flips_only_its_field() {
        for route in ROUTES {
            let (ev, d) = good(route);

            let mut m = ev;
            m.marker = 0x99;
            let c = check_call(route, &m, Some(&d)).unwrap();
            assert!(!c.marker() && c.counts() && c.output() && c.artifact() == Some(true));
            assert!(!c.is_match());

            let mut m = ev;
            m.after.cpu += 2;
            let c = check_call(route, &m, Some(&d)).unwrap();
            assert!(c.marker() && !c.counts() && c.output() && c.artifact() == Some(true));
            assert!(!c.is_match());

            let mut m = ev;
            m.output = if route.is_some() { UNTOUCHED } else { WRITTEN };
            let c = check_call(route, &m, Some(&d)).unwrap();
            assert!(c.marker() && c.counts() && !c.output() && c.artifact() == Some(true));
            assert!(!c.is_match());

            let mut m = ev;
            m.artifact = Some([7; 32]);
            let c = check_call(route, &m, Some(&d)).unwrap();
            assert!(c.marker() && c.counts() && c.output() && c.artifact() == Some(false));
            assert!(!c.is_match());
        }
    }

    #[test]
    fn missing_artifact_is_a_mismatch_for_artifact_routes() {
        let (mut ev, d) = good(Some(ReportedRoute::Native));
        ev.artifact = None;
        let c = check_call(Some(ReportedRoute::Native), &ev, Some(&d)).unwrap();
        assert_eq!(c.artifact(), Some(false));
    }

    #[test]
    fn counts_ignore_probe_and_reject_other_routes() {
        let route = Some(ReportedRoute::Native);

        let (mut ev, d) = good(route);
        ev.after.native = 2;
        assert!(!check_call(route, &ev, Some(&d)).unwrap().counts());

        let (mut ev, d) = good(route);
        ev.after.gpu_emulated = 1;
        assert!(!check_call(route, &ev, Some(&d)).unwrap().counts());

        let (mut ev, d) = good(route);
        ev.before.probe = 3;
        ev.after.probe = 8;
        let c = check_call(route, &ev, Some(&d)).unwrap();
        assert!(c.counts() && c.is_match());
        assert_eq!(c.probe_runs(), 5);
    }

    #[test]
    fn decreased_counts_are_errors() {
        let route = Some(ReportedRoute::Native);

        let (mut ev, d) = good(route);
        ev.before.native = 5;
        ev.after.native = 4;
        assert_eq!(
            check_call(route, &ev, Some(&d)),
            Err(EvidenceError::CountsDecreased)
        );

        let (mut ev, d) = good(route);
        ev.before.probe = 5;
        ev.after.probe = 4;
        assert_eq!(
            check_call(route, &ev, Some(&d)),
            Err(EvidenceError::CountsDecreased)
        );
    }

    #[test]
    fn empty_output_is_an_error() {
        let (mut ev, d) = good(None);
        ev.output = &[];
        assert_eq!(
            check_call(None, &ev, Some(&d)),
            Err(EvidenceError::EmptyOutput)
        );
    }

    #[test]
    fn equal_digests_are_rejected() {
        assert_eq!(
            ArtifactDigests::new([1; 32], [1; 32]),
            Err(EvidenceError::SameArtifactDigest)
        );
        assert_eq!(
            ArtifactDigests::from_artifacts(b"same", b"same"),
            Err(EvidenceError::SameArtifactDigest)
        );
    }

    #[test]
    fn no_expected_digests_means_not_compared() {
        for route in ROUTES {
            let (ev, _) = good(route);
            let c = check_call(route, &ev, None).unwrap();
            assert_eq!(c.artifact(), None);
            assert!(c.is_match());

            let mut other = ev;
            other.artifact = Some([9; 32]);
            let c = check_call(route, &other, None).unwrap();
            assert_eq!(c.artifact(), None);
            assert!(c.is_match());
        }
    }

    #[test]
    fn sentinel_helpers() {
        let mut buf = [0u8; 5];
        assert!(!is_untouched(&buf));
        fill_sentinel(&mut buf);
        assert!(is_untouched(&buf));
        buf[3] = 0;
        assert!(!is_untouched(&buf));
        assert!(is_untouched(&[]));
    }

    #[test]
    fn digests_match_sha256() {
        let d = digests();
        assert_eq!(d.native(), sha256_bytes(b"native"));
        assert_eq!(d.gpu_emulated(), sha256_bytes(b"gpu-emulated"));
    }

    #[test]
    fn error_display_is_fixed() {
        for e in [
            EvidenceError::EmptyOutput,
            EvidenceError::CountsDecreased,
            EvidenceError::SameArtifactDigest,
            EvidenceError::TallyOverflow,
        ] {
            assert!(!e.to_string().is_empty());
            assert_eq!(e.to_string(), e.as_str());
        }
    }

    #[test]
    fn tally_counts_each_value() {
        let mut t = EvidenceTally::new();
        for route in ROUTES {
            let (ev, d) = good(route);
            t.record_call(route, &ev, Some(&d)).unwrap();
        }
        assert_eq!((t.calls(), t.matched(), t.mismatched()), (4, 4, 0));
        assert_eq!(t.run_result(), RunResult::Verdict(Verdict::Match));

        let (mut ev, d) = good(Some(ReportedRoute::Native));
        ev.marker = MARKER_CLEAR;
        t.record_call(Some(ReportedRoute::Native), &ev, Some(&d))
            .unwrap();
        assert_eq!((t.calls(), t.matched(), t.mismatched()), (5, 4, 1));
        assert_eq!(t.marker_mismatches(), 1);
        assert_eq!(t.count_mismatches(), 0);
        assert_eq!(t.output_mismatches(), 0);
        assert_eq!(t.artifact_mismatches(), 0);
        assert_eq!(t.run_result(), RunResult::Verdict(Verdict::Mismatch));
    }

    #[test]
    fn tally_counts_artifact_not_compared() {
        let mut t = EvidenceTally::new();
        let (ev, _) = good(Some(ReportedRoute::Cpu));
        t.record_call(Some(ReportedRoute::Cpu), &ev, None).unwrap();
        assert_eq!(t.artifact_not_compared(), 1);
        assert_eq!(t.matched(), 1);
    }

    #[test]
    fn empty_tally_is_indeterminate() {
        assert_eq!(
            EvidenceTally::new().run_result(),
            RunResult::Indeterminate(IndeterminateReason::EvidenceMissing)
        );
    }

    #[test]
    fn tally_overflow_keeps_value() {
        let (ev, d) = good(Some(ReportedRoute::Native));
        for field in 0..3 {
            let mut t = EvidenceTally::new();
            match field {
                0 => t.calls = u64::MAX,
                1 => t.matched = u64::MAX,
                _ => t.probe_runs = u64::MAX,
            }
            let mut ev2 = ev;
            ev2.after.probe = 1;
            let c2 = check_call(Some(ReportedRoute::Native), &ev2, Some(&d)).unwrap();
            let before = t.clone();
            assert_eq!(t.record(&c2), Err(EvidenceError::TallyOverflow));
            assert_eq!(t, before);
        }
    }
}
