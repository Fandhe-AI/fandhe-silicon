//! run ごとの記録を「書く側」から作る型と、JSON への書き出し。
//!
//! 封筒の芯（版・run の ID・時刻・機材・ラベル・回への参照）と、本体の種類（いまは `perf-samples`）を持つ。
//! 測定の腕が標本を集めたあと、本モジュールで検証つきの記録にして JSON を出す。
//! 欄の値は白表の列挙・検証済みの ID・時刻・数だけで、ホスト名・パス・自由な文を受ける口を持たない。
//! 経路の証拠・環境・比較基準の版の欄と値型は `evidence` にあり、ここから再公開する。
//! 欄の取得不能は、型を持つ欄は `null`、文字列の欄は予約語（0 で埋めない）。道具側に標本数・run 数・機材ごとの制限は無い。
//! 時計は読まない（時刻は呼び手が [`UtcTime`] で渡す）。専有の確かめが後で引く手掛かりとして
//! `session_ref` と開始・終了の時刻を載せる。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-27・TASK-28（ポインタのみ）。
//! 機材とラベルの綴りは `crates/core` の `test_frame` に揃える。

use crate::PerfError;
pub use crate::evidence::*;
use crate::json;
use crate::sample::{PhaseSamples, SampleCounts, SubmissionSample};
use crate::stats::{RunStats, run_stats};
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// 記録の版。
pub const SCHEMA_VERSION: &str = "1";

/// 回に属さない run を表す予約語。回の ID の形（[`Id`]）には決して一致しない（先頭が `_`）。
pub const NO_SESSION: &str = "_none";

/// 記録を作る・書く途中の失敗。入力の値は埋め込まない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordError {
    /// 時刻が `YYYY-MM-DDTHH:MM:SSZ` の暦として正しい形でない。
    InvalidTime,
    /// ID の形が合わない。
    InvalidId,
    /// 開始が終了より後。
    TimeOrder,
    /// 標本の数の指定と標本の長さが合わない。
    CountMismatch,
    /// 計測の区間が空。
    Empty,
    /// 統計の失敗、または統計の値を正確に書けない。
    Perf(PerfError),
    /// 値の形が合わない（入力は埋め込まない）。
    InvalidValue,
    /// 環境の背後実装の欄と、機材の組み合わせの背後実装が合わない。
    BackendMismatch,
    /// ファイルの書き出しの失敗（既存のファイルがある場合を含む）。
    Io(std::io::ErrorKind),
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTime => f.write_str("invalid UTC time"),
            Self::InvalidId => f.write_str("invalid id"),
            Self::TimeOrder => f.write_str("start time is after end time"),
            Self::CountMismatch => f.write_str("sample counts do not match the samples"),
            Self::Empty => f.write_str("empty samples"),
            Self::InvalidValue => f.write_str("invalid value"),
            Self::BackendMismatch => {
                f.write_str("backend env does not match the machine combination")
            }
            Self::Perf(e) => write!(f, "perf error: {e}"),
            Self::Io(k) => write!(f, "io error: {k}"),
        }
    }
}

impl std::error::Error for RecordError {}

impl From<PerfError> for RecordError {
    fn from(e: PerfError) -> Self {
        Self::Perf(e)
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ`（20 文字）の検証済みの UTC 時刻。固定長なので文字列順が時刻順になる。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtcTime(String);

fn is_leap(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(y) => 29,
        _ => 28,
    }
}

impl UtcTime {
    /// 文字列を検証して受ける。形または暦が合わなければ [`RecordError::InvalidTime`]。
    pub fn parse(s: &str) -> Result<Self, RecordError> {
        let b = s.as_bytes();
        if b.len() != 20 {
            return Err(RecordError::InvalidTime);
        }
        let sep = [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'Z'),
        ];
        if sep.iter().any(|&(i, c)| b.get(i) != Some(&c)) {
            return Err(RecordError::InvalidTime);
        }
        let num = |from: usize, to: usize| -> Result<u32, RecordError> {
            let part = b.get(from..to).ok_or(RecordError::InvalidTime)?;
            if !part.iter().all(u8::is_ascii_digit) {
                return Err(RecordError::InvalidTime);
            }
            Ok(part.iter().fold(0, |a, d| a * 10 + u32::from(d - b'0')))
        };
        let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
        let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
        let ok = (1..=12).contains(&mo)
            && (1..=days_in_month(y, mo)).contains(&d)
            && h < 24
            && mi < 60
            && se < 60;
        if !ok {
            return Err(RecordError::InvalidTime);
        }
        Ok(Self(s.to_owned()))
    }

    /// Unix 秒から作る純粋な変換（時計は読まない）。西暦 9999 年を超える値は `InvalidTime`。
    pub fn from_unix_secs(secs: u64) -> Result<Self, RecordError> {
        let days = i64::try_from(secs / 86_400).map_err(|_| RecordError::InvalidTime)?;
        let rem = secs % 86_400;
        // 日数から暦日への変換（proleptic Gregorian。紀元を 0000-03-01 にずらす）。
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        if !(0..=9999).contains(&y) {
            return Err(RecordError::InvalidTime);
        }
        Self::parse(&format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        ))
    }

    /// 文字列表現。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// run・回の ID。`[a-z0-9]+(-[a-z0-9]+)*` で 1〜64 文字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Id(String);

impl Id {
    /// 形を検証して受ける。
    pub fn parse(s: &str) -> Result<Self, RecordError> {
        let ok = (1..=64).contains(&s.len())
            && s.split('-').all(|seg| {
                !seg.is_empty()
                    && seg
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            });
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(RecordError::InvalidId)
        }
    }

    /// 文字列表現。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// タスク・ビヘイビアの ID への参照（ポインタ）。`[A-Z]+(-[A-Z0-9]+)+` で 64 文字以内。
///
/// 許す文字を白表で狭く取る。空白・制御文字・非 ASCII・小文字は受けない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefId(String);

impl RefId {
    /// 形を検証して受ける。
    pub fn parse(s: &str) -> Result<Self, RecordError> {
        let mut segs = s.split('-');
        let head_ok = segs
            .next()
            .is_some_and(|h| !h.is_empty() && h.bytes().all(|c| c.is_ascii_uppercase()));
        let mut rest = 0;
        let rest_ok = segs.all(|seg| {
            rest += 1;
            !seg.is_empty()
                && seg
                    .bytes()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        });
        if s.len() <= 64 && head_ok && rest_ok && rest >= 1 {
            Ok(Self(s.to_owned()))
        } else {
            Err(RecordError::InvalidId)
        }
    }

    /// 文字列表現。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 回への参照。回に属さない run は [`SessionRef::NoSession`]（予約語 [`NO_SESSION`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRef {
    /// 回に属する。
    Session(Id),
    /// 回に属さない。
    NoSession,
}

impl SessionRef {
    /// 記録に書く綴り。
    pub fn as_str(&self) -> &str {
        match self {
            Self::Session(id) => id.as_str(),
            Self::NoSession => NO_SESSION,
        }
    }
}

/// 機材の呼び名。綴りは `crates/core` の `Machine::name` に揃える（公開テストで突き合わせる）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    /// Metal の実機（Mac mini）。
    MacMini,
    /// 別名 node-A。
    NodeA,
    /// 別名 node-B。
    NodeB,
    /// AMD の実機。
    Amd,
    /// Intel の実機。
    Intel,
}

impl Machine {
    /// 全ての機材。
    pub const ALL: [Machine; 5] = [
        Self::MacMini,
        Self::NodeA,
        Self::NodeB,
        Self::Amd,
        Self::Intel,
    ];

    /// 記録に書く綴り。
    pub const fn name(self) -> &'static str {
        match self {
            Self::MacMini => "mac-mini",
            Self::NodeA => "node-a",
            Self::NodeB => "node-b",
            Self::Amd => "amd",
            Self::Intel => "intel",
        }
    }
}

/// 背後実装と機材の組み合わせ。綴りは `crates/core` の `MachineCombination::name` に揃える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineCombination {
    /// Metal と Mac mini。
    MetalMacMini,
    /// Vulkan と node-B。
    VulkanNodeB,
    /// Vulkan と node-A。
    VulkanNodeA,
    /// Vulkan と AMD の実機。
    VulkanAmd,
    /// Vulkan と Intel の実機。
    VulkanIntel,
    /// CUDA と node-B。
    CudaNodeB,
    /// CUDA と node-A。
    CudaNodeA,
}

impl MachineCombination {
    /// 全ての組み合わせ。
    pub const ALL: [MachineCombination; 7] = [
        Self::MetalMacMini,
        Self::VulkanNodeB,
        Self::VulkanNodeA,
        Self::VulkanAmd,
        Self::VulkanIntel,
        Self::CudaNodeB,
        Self::CudaNodeA,
    ];

    /// 背後実装。
    pub const fn backend(self) -> AdapterBackend {
        match self {
            Self::MetalMacMini => AdapterBackend::Metal,
            Self::VulkanNodeB | Self::VulkanNodeA | Self::VulkanAmd | Self::VulkanIntel => {
                AdapterBackend::Vulkan
            }
            Self::CudaNodeB | Self::CudaNodeA => AdapterBackend::Cuda,
        }
    }

    /// 記録に書く綴り。
    pub const fn name(self) -> &'static str {
        match self {
            Self::MetalMacMini => "metal-mac-mini",
            Self::VulkanNodeB => "vulkan-node-b",
            Self::VulkanNodeA => "vulkan-node-a",
            Self::VulkanAmd => "vulkan-amd",
            Self::VulkanIntel => "vulkan-intel",
            Self::CudaNodeB => "cuda-node-b",
            Self::CudaNodeA => "cuda-node-a",
        }
    }
}

/// 走らせなかった理由。綴りは `crates/core` の `NotRunReason::name` に揃える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotRunReason {
    /// 組み合わせが未設定。
    NotConfigured,
    /// 組み合わせの設定が不正。
    InvalidCombinationSetting,
    /// 許可の設定が不正。
    InvalidPermissionSetting,
    /// 試験が組み合わせを宣言していない。
    CombinationNotDeclared,
    /// 必要な許可が無い。
    PermissionMissing,
}

impl NotRunReason {
    /// 全ての理由。
    pub const ALL: [NotRunReason; 5] = [
        Self::NotConfigured,
        Self::InvalidCombinationSetting,
        Self::InvalidPermissionSetting,
        Self::CombinationNotDeclared,
        Self::PermissionMissing,
    ];

    /// 理由のトークン。
    pub const fn name(self) -> &'static str {
        match self {
            Self::NotConfigured => "not-configured",
            Self::InvalidCombinationSetting => "invalid-combination-setting",
            Self::InvalidPermissionSetting => "invalid-permission-setting",
            Self::CombinationNotDeclared => "combination-not-declared",
            Self::PermissionMissing => "permission-missing",
        }
    }
}

/// 判定不能の理由。綴りは `crates/core` の `IndeterminateReason::name` に揃える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndeterminateReason {
    /// 取れた device が宣言した機材と合わない。
    DeviceMismatch,
    /// 機材に届かない。
    MachineUnreachable,
    /// その run のログに、合格の証拠（試験の名前と、同じログの `test result: ok`）がそろっていない。
    EvidenceMissing,
    /// 比べる 2 つの run で、関わる版や組み合わせが違う。
    EnvironmentDiffers,
}

impl IndeterminateReason {
    /// 全ての理由。
    pub const ALL: [IndeterminateReason; 4] = [
        Self::DeviceMismatch,
        Self::MachineUnreachable,
        Self::EvidenceMissing,
        Self::EnvironmentDiffers,
    ];

    /// 理由のトークン。
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeviceMismatch => "device-mismatch",
            Self::MachineUnreachable => "machine-unreachable",
            Self::EvidenceMissing => "evidence-missing",
            Self::EnvironmentDiffers => "environment-differs",
        }
    }
}

/// 判定保留の理由のトークン（`crates/core` の `PENDING_REASON` と同じ綴り）。
pub const PENDING_REASON: &str = "threshold-unapproved";

/// 結果のラベル。理由を持つ種類は理由なしでは作れない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// 一致した。
    Ok,
    /// 一致しなかった。
    Ng,
    /// 判定保留（理由は常に [`PENDING_REASON`]）。
    Pending,
    /// 走らせなかった。
    NotRun(NotRunReason),
    /// 判定不能。
    Indeterminate(IndeterminateReason),
}

impl Label {
    /// 種類のトークン。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Ng => "ng",
            Self::Pending => "pending",
            Self::NotRun(_) => "not-run",
            Self::Indeterminate(_) => "indeterminate",
        }
    }

    /// 理由のトークン。理由を持つ種類だけ `Some`。
    pub const fn reason_name(self) -> Option<&'static str> {
        match self {
            Self::Pending => Some(PENDING_REASON),
            Self::NotRun(r) => Some(r.name()),
            Self::Indeterminate(r) => Some(r.name()),
            _ => None,
        }
    }
}

/// `perf-samples` の本体の種類名。
pub const BODY_KIND_PERF_SAMPLES: &str = "perf-samples";

/// `perf-samples` の本体。標本と統計を記録の形にしたもの。
#[derive(Debug, Clone, PartialEq)]
pub struct PerfSamplesBody {
    counts: SampleCounts,
    samples: PhaseSamples,
    submissions: u64,
    dispatches: u64,
    processed_count: Option<u64>,
    instructions_found: BTreeMap<Name, bool>,
    cpu_stats: RunStats,
    gpu_stats: Option<RunStats>,
}

impl PerfSamplesBody {
    /// 標本の数の指定・集めた標本・投入件数・dispatch 数（呼び手の値をそのまま持つ）から作る。
    ///
    /// 数の指定と長さの食い違いは [`RecordError::CountMismatch`]、計測の区間が空なら [`RecordError::Empty`]。
    /// 計測の区間に GPU 時間の取得不能が 1 つでもあれば GPU の統計は持たず `null` で出す。
    pub fn new(
        counts: SampleCounts,
        samples: PhaseSamples,
        submissions: u64,
        dispatches: u64,
    ) -> Result<Self, RecordError> {
        if samples.measured.is_empty() {
            return Err(RecordError::Empty);
        }
        if samples.cold.len() != counts.cold
            || samples.warmup.len() != counts.warmup
            || samples.measured.len() != counts.measured
        {
            return Err(RecordError::CountMismatch);
        }
        let cpu_stats = run_stats(&samples.measured_cpu_ns())?;
        let gpu_stats = samples
            .measured_gpu_ns()
            .map(|v| run_stats(&v))
            .transpose()?;
        Ok(Self {
            counts,
            samples,
            submissions,
            dispatches,
            processed_count: None,
            instructions_found: BTreeMap::new(),
            cpu_stats,
            gpu_stats,
        })
    }

    /// 処理件数を足す（既定は取得不能で `null`）。
    pub fn with_processed_count(mut self, n: Option<u64>) -> Self {
        self.processed_count = n;
        self
    }

    /// 生成コードの該当命令の表（名前 → 見つかったか）を足す（既定は空）。
    pub fn with_instructions_found(mut self, m: BTreeMap<Name, bool>) -> Self {
        self.instructions_found = m;
        self
    }

    /// 本体を JSON にして `out` に足す。統計の値を正確に書けなければ `false`。
    fn write_json(&self, out: &mut String) -> bool {
        let c = &self.counts;
        let _ = write!(
            out,
            "{{\"sample_counts\":{{\"cold\":{},\"warmup\":{},\"measured\":{}}},\"submission_count\":{},\"dispatch_count\":{},",
            c.cold, c.warmup, c.measured, self.submissions, self.dispatches
        );
        out.push_str("\"processed_count\":");
        json::push_opt_u64(out, self.processed_count);
        out.push_str(",\"instructions_found\":");
        json::push_bool_map(
            out,
            self.instructions_found
                .iter()
                .map(|(k, v)| (k.as_str(), *v)),
        );
        out.push(',');
        type Pick = fn(&SubmissionSample) -> Option<u64>;
        let picks: [(&str, Pick); 2] = [("cpu_ns", |s| Some(s.cpu_ns)), ("gpu_ns", |s| s.gpu_ns)];
        for (key, pick) in picks {
            let _ = write!(out, "\"{key}\":{{");
            let phases = [
                ("cold", &self.samples.cold),
                ("warmup", &self.samples.warmup),
                ("measured", &self.samples.measured),
            ];
            for (i, (name, v)) in phases.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(out, "\"{name}\":");
                json::push_opt_array(out, v.iter().map(pick));
            }
            out.push_str("},");
        }
        out.push_str("\"stats\":{\"cpu_ns\":");
        let cpu_ok = write_stats(out, Some(&self.cpu_stats));
        out.push_str(",\"gpu_ns\":");
        let gpu_ok = write_stats(out, self.gpu_stats.as_ref());
        out.push_str("}}");
        cpu_ok && gpu_ok
    }
}

/// 統計を JSON にして足す。`None` は `null`。値を正確に書けなければ `false`。
fn write_stats(out: &mut String, s: Option<&RunStats>) -> bool {
    let Some(s) = s else {
        out.push_str("null");
        return true;
    };
    out.push_str("{\"median\":");
    let ok = json::push_median(out, s.median);
    let _ = write!(
        out,
        ",\"q1\":{},\"q3\":{},\"p95\":{},\"min\":{},\"max\":{}}}",
        s.q1, s.q3, s.p95, s.min, s.max
    );
    ok
}

/// 本体。種類ごとに 1 つの列挙子を持つ（封筒は本体の種類を固定しない）。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Body {
    /// 投入ごとの標本と統計。
    PerfSamples(PerfSamplesBody),
}

impl Body {
    /// `body_kind` の値。
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::PerfSamples(_) => BODY_KIND_PERF_SAMPLES,
        }
    }
}

/// 封筒の芯の入力。
#[derive(Debug, Clone)]
pub struct Envelope {
    /// run の ID。
    pub run_id: Id,
    /// 関連するタスクの ID（無ければ空）。
    pub task_ids: Vec<RefId>,
    /// 関連するビヘイビアの ID（無ければ空）。
    pub behavior_ids: Vec<RefId>,
    /// 背後実装と機材の組み合わせ。
    pub machine_combination: MachineCombination,
    /// 機材。
    pub machine: Machine,
    /// 結果のラベル。
    pub label: Label,
    /// run の開始。
    pub started_utc: UtcTime,
    /// run の終了。
    pub ended_utc: UtcTime,
    /// 回への参照。
    pub session_ref: SessionRef,
    /// 経路の証拠。
    pub evidence: Evidence,
    /// 環境。
    pub environment: Environment,
    /// 比較基準の wgpu の版（比較用の run のときだけ `Some`。`None` ならキーごと出さない）。
    pub wgpu_baseline: Option<WgpuBaseline>,
}

/// 検証済みの run ごとの記録。
#[derive(Debug, Clone)]
pub struct RunRecord {
    envelope: Envelope,
    body: Body,
}

fn push_str_field(out: &mut String, key: &str, value: &str) {
    let _ = write!(out, ",\"{key}\":");
    json::push_string(out, value);
}

fn push_ids(out: &mut String, key: &str, ids: &[RefId]) {
    let _ = write!(out, ",\"{key}\":[");
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json::push_string(out, id.as_str());
    }
    out.push(']');
}

impl RunRecord {
    /// 封筒と本体から作る。開始が終了より後なら [`RecordError::TimeOrder`]（同時刻は受ける）。
    /// 環境の背後実装の欄が組み合わせの背後実装と合わなければ [`RecordError::BackendMismatch`]。
    pub fn new(envelope: Envelope, body: Body) -> Result<Self, RecordError> {
        if envelope.started_utc > envelope.ended_utc {
            return Err(RecordError::TimeOrder);
        }
        if envelope.environment.backend_env.backend() != envelope.machine_combination.backend() {
            return Err(RecordError::BackendMismatch);
        }
        Ok(Self { envelope, body })
    }

    /// 決定的な JSON（1 行・ASCII だけ）にする。欄の順序は固定。
    ///
    /// 統計に正確に書けない値があれば [`RecordError::Perf`]（暗黙に丸めない）。
    pub fn to_json(&self) -> Result<String, RecordError> {
        let e = &self.envelope;
        let mut out = String::from("{\"schema_version\":");
        json::push_string(&mut out, SCHEMA_VERSION);
        push_str_field(&mut out, "run_id", e.run_id.as_str());
        push_ids(&mut out, "task_ids", &e.task_ids);
        push_ids(&mut out, "behavior_ids", &e.behavior_ids);
        push_str_field(
            &mut out,
            "machine_combination",
            e.machine_combination.name(),
        );
        push_str_field(&mut out, "machine", e.machine.name());
        push_str_field(&mut out, "label", e.label.name());
        // 理由は理由を持つラベルのときだけキーごと出す。
        if let Some(reason) = e.label.reason_name() {
            push_str_field(&mut out, "label_reason", reason);
        }
        push_str_field(&mut out, "body_kind", self.body.kind());
        push_str_field(&mut out, "started_utc", e.started_utc.as_str());
        push_str_field(&mut out, "ended_utc", e.ended_utc.as_str());
        push_str_field(&mut out, "session_ref", e.session_ref.as_str());
        out.push_str(",\"evidence\":");
        e.evidence.write_json(&mut out);
        out.push_str(",\"environment\":");
        e.environment.write_json(&mut out);
        if let Some(w) = &e.wgpu_baseline {
            out.push_str(",\"wgpu_baseline\":");
            w.write_json(&mut out);
        }
        out.push_str(",\"body\":");
        let ok = match &self.body {
            Body::PerfSamples(b) => b.write_json(&mut out),
        };
        out.push('}');
        if ok {
            Ok(out)
        } else {
            Err(RecordError::Perf(PerfError::InvalidValue))
        }
    }
}

/// 記録を新しいファイルへ書く。既存のファイルは上書きせず `Err(Io(AlreadyExists))`。
///
/// ディレクトリは作らない（呼び手の責務）。末尾に改行を 1 つ足す。
/// 書き込みに失敗したときは、この呼び出しが作ったファイルを消して再試行できる状態に戻す。
pub fn write_new(path: &Path, record: &RunRecord) -> Result<(), RecordError> {
    let json = record.to_json()?;
    write_new_with(path, |f| {
        f.write_all(json.as_bytes())
            .and_then(|()| f.write_all(b"\n"))
    })
}

/// `create_new` で作ったファイルへ `body` で書く。`body` が失敗したら作ったファイルを消す。
///
/// 既存ファイルを開けなかった（`AlreadyExists` 等）ときは何も作っていないので消さない。
fn write_new_with(
    path: &Path,
    body: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), RecordError> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| RecordError::Io(e.kind()))?;
    body(&mut f).map_err(|e| {
        drop(f);
        // 部分ファイルを残すと再試行が AlreadyExists になるため、自分が作ったものを消す。
        let _ = std::fs::remove_file(path);
        RecordError::Io(e.kind())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_write_leaves_no_partial_file_and_allows_retry() {
        let dir = std::env::temp_dir().join(format!("perf-wn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rec.json");
        let err = write_new_with(&path, |f| {
            f.write_all(b"partial")?;
            Err(std::io::Error::other("boom"))
        })
        .unwrap_err();
        assert_eq!(err, RecordError::Io(std::io::ErrorKind::Other));
        assert!(!path.exists());
        write_new_with(&path, |f| f.write_all(b"ok")).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"ok");
        // 既存ファイルは失敗しても消さない。
        let err = write_new_with(&path, |_| Ok(())).unwrap_err();
        assert_eq!(err, RecordError::Io(std::io::ErrorKind::AlreadyExists));
        assert!(path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
