//! CPU の演算（契約の crate の `DefaultCpuOps`）の時間を腕ごとに測り、run ごとの記録を書く測定の部品。
//!
//! 測定専用の別 workspace `measure/` に置く crate で、製品の crate からは使われない。
//! bin `cpu-ops-arm`（`src/main.rs`）が [`parse_args`] と [`run`] を呼ぶ。
//! 腕は [`ARMS`] の 10 個（行列積の scalar・simd × 3 形、要素ごと演算の scalar・simd × 2 演算）。
//! 標本は `measure_perf::sample::collect` で集め、記録は `measure_perf::record` で検証つきの JSON にして腕ごとに 1 ファイル書く。
//! 入力は `wasm/crates/parity-host` の `gen` と同じ生成器・同じ値で作る（入力の sha256 が一致する）。
//! aarch64 で `neon` が有効なビルドでは、simd の腕の出力の sha256 を既知の値と照合し、
//! 一致しなければその腕の記録を書かずに失敗する（[`check_known_output`]）。
//! GPU 時間は無く、記録の `gpu_ns` は取得不能の形で出る。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-2960（ポインタのみ）。

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, UnaryElementwiseOp};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, SplitMix64, generate_f32,
};
use measure_perf::PerfError;
use measure_perf::record::{
    AdapterBackend, BackendEnv, Body, BuildProfile, CommitSha, DeviceName, EnvSnapshot, Envelope,
    Environment, Evidence, HashTable, Id, Label, Machine, MachineCombination, Name, OptionToken,
    OsName, OutputHash, PerfSamplesBody, RecordError, RefId, RunRecord, SessionRef, Sha256,
    UtcTime, VersionText, write_new,
};
use measure_perf::sample::{CollectError, SampleCounts, collect};
use measure_perf::stats::run_stats;
use std::collections::BTreeMap;
use std::fmt;
use std::hint::black_box;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 記録の `task_ids` に載せるタスクの ID。
pub const TASK_ID: &str = "TASK-2960";

/// 既定の cold の投入回数。
pub const DEFAULT_COLD: usize = 1;
/// 既定の warmup の投入回数。
pub const DEFAULT_WARMUP: usize = 20;
/// 既定の計測の投入回数。
pub const DEFAULT_MEASURED: usize = 200;

/// 要素ごと演算の入力の長さ（`wasm-parity-host` の `ELEMENTWISE_LEN` と同じ値）。
pub const ELEMENTWISE_LEN: usize = 1_000_003;

/// 既定の標本の数の指定。
pub const DEFAULT_COUNTS: SampleCounts = SampleCounts {
    cold: DEFAULT_COLD,
    warmup: DEFAULT_WARMUP,
    measured: DEFAULT_MEASURED,
};

/// aarch64 で `neon` が有効なビルドか。真のビルドでだけ simd の腕の出力を既知の sha256 と照合する。
pub const NEON: bool = cfg!(all(target_arch = "aarch64", target_feature = "neon"));

/// 腕が測る計算の入力の名前（既知の出力の表と入力の生成の鍵）。
pub const CASE_GEMM_64: &str = "gemm-64";
/// 256³ の行列積のケース名。
pub const CASE_GEMM_256: &str = "gemm-256";
/// 37×41×53 の行列積のケース名。
pub const CASE_GEMM_37X41X53: &str = "gemm-37x41x53";
/// `Relu` のケース名。
pub const CASE_RELU: &str = "elementwise-relu";
/// `AddScalar(0.5)` のケース名。
pub const CASE_ADD_SCALAR: &str = "elementwise-add-scalar";

/// aarch64 で `neon` が有効なビルドの `gemm_simd`・`elementwise_simd` の出力（f32 のリトルエンディアンのバイト列）の sha256。
/// `wasm-parity-host` の `*.native-neon.bin` の sha256 と同じ定義。
pub const KNOWN_NEON_OUTPUT_SHA256: [(&str, &str); 5] = [
    (
        CASE_GEMM_64,
        "1fe06a7542848dc200ebb3fa0d9f81b3041e6f78b664b00a0b74ee1c39bcffa1",
    ),
    (
        CASE_GEMM_256,
        "bd580c4cc14dfeeb2fce80cfc354a1af66d3f6764674f04d2269d31e1f8c4718",
    ),
    (
        CASE_GEMM_37X41X53,
        "c8e2f4dc4e6c39be0600b34005bf456ae01c55cd01ca4fac9433a3d3c9a1d0b3",
    ),
    (
        CASE_RELU,
        "52444bc37b060fd87456b23132c1f5b2a4e32e2d008fc1d1d453cac5b9913588",
    ),
    (
        CASE_ADD_SCALAR,
        "25996e901beab1e6891ba2b25366fee752e1c350cad2cc239f4a6878c0c3d509",
    ),
];

/// 腕 1 個の定義。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmSpec {
    /// 腕の名前（run の ID の末尾とファイル名になる。`[a-z0-9]+(-[a-z0-9]+)*`）。
    pub name: &'static str,
    /// 入力のケース名。
    pub case: &'static str,
    /// simd 版か（偽なら scalar 版）。
    pub simd: bool,
}

/// 測る 10 個の腕。この順に測る。
pub const ARMS: [ArmSpec; 10] = [
    ArmSpec {
        name: "gemm-scalar-64",
        case: CASE_GEMM_64,
        simd: false,
    },
    ArmSpec {
        name: "gemm-simd-64",
        case: CASE_GEMM_64,
        simd: true,
    },
    ArmSpec {
        name: "gemm-scalar-256",
        case: CASE_GEMM_256,
        simd: false,
    },
    ArmSpec {
        name: "gemm-simd-256",
        case: CASE_GEMM_256,
        simd: true,
    },
    ArmSpec {
        name: "gemm-scalar-37x41x53",
        case: CASE_GEMM_37X41X53,
        simd: false,
    },
    ArmSpec {
        name: "gemm-simd-37x41x53",
        case: CASE_GEMM_37X41X53,
        simd: true,
    },
    ArmSpec {
        name: "elementwise-scalar-relu",
        case: CASE_RELU,
        simd: false,
    },
    ArmSpec {
        name: "elementwise-simd-relu",
        case: CASE_RELU,
        simd: true,
    },
    ArmSpec {
        name: "elementwise-scalar-add-scalar",
        case: CASE_ADD_SCALAR,
        simd: false,
    },
    ArmSpec {
        name: "elementwise-simd-add-scalar",
        case: CASE_ADD_SCALAR,
        simd: true,
    },
];

/// 測定の失敗。`Display` は固定の英語の文言で、入力の値・パスを含まない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArmError {
    /// 引数が決めた形でない。
    Usage(&'static str),
    /// 出力先のディレクトリが無い。
    MissingDirectory,
    /// 書く予定の名前のファイルが既にある。
    FileExists,
    /// run の ID が長すぎる、または形が合わない。
    InvalidRunId,
    /// 入力を作れなかった。
    InputFailed,
    /// CPU の演算が `Err` を返した。
    OpFailed,
    /// 標本を集められなかった。
    CollectFailed,
    /// 時計を読めない、または時刻に直せない。
    ClockFailed,
    /// 記録を作る・書く途中の失敗。
    Record(RecordError),
    /// 統計を出せなかった。
    Stats(PerfError),
    /// 標準出力に書けなかった。
    OutputFailed,
    /// simd の腕の出力の sha256 が既知の値と一致しない。
    OutputMismatch,
    /// 既知の出力の表に無いケース。
    UnknownCase,
}

impl fmt::Display for ArmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(m) => write!(f, "usage: {m}"),
            Self::MissingDirectory => f.write_str("output directory does not exist"),
            Self::FileExists => f.write_str("a record file already exists"),
            Self::InvalidRunId => f.write_str("invalid run id"),
            Self::InputFailed => f.write_str("failed to build the inputs"),
            Self::OpFailed => f.write_str("cpu op returned an error"),
            Self::CollectFailed => f.write_str("failed to collect samples"),
            Self::ClockFailed => f.write_str("failed to read the clock"),
            Self::Record(e) => write!(f, "record error: {e}"),
            Self::Stats(e) => write!(f, "stats error: {e}"),
            Self::OutputFailed => f.write_str("failed to write to standard output"),
            Self::OutputMismatch => f.write_str("output sha256 differs from the known value"),
            Self::UnknownCase => f.write_str("no known output for the case"),
        }
    }
}

impl std::error::Error for ArmError {}

impl From<RecordError> for ArmError {
    fn from(e: RecordError) -> Self {
        Self::Record(e)
    }
}

/// 解析した引数。
#[derive(Debug, Clone)]
pub struct Config {
    /// run の ID の前半（腕ごとの run の ID はこれに腕の名前を足す）。
    pub run_id: String,
    /// 回への参照。
    pub session_ref: SessionRef,
    /// 記録を書くディレクトリ（呼び手が作る）。
    pub out_dir: PathBuf,
    /// 本リポの commit。
    pub repo_commit: Option<CommitSha>,
    /// CPU の名前。
    pub cpu: Option<DeviceName>,
    /// OS の版。
    pub os_version: Option<VersionText>,
    /// rustc の版。
    pub rustc_version: Option<VersionText>,
    /// 標本の数の指定。
    pub counts: SampleCounts,
}

/// 引数（プログラム名を除く）を解析する。未知の引数・重複・値の欠け・不正な値は `Err`。
///
/// 必須は `--run-id` と `--out-dir`。`--cold`・`--warmup`・`--measured` は試験用に回数を変える口で、
/// 既定は [`DEFAULT_COUNTS`]。
pub fn parse_args(args: &[String]) -> Result<Config, ArmError> {
    const KEYS: [&str; 10] = [
        "--run-id",
        "--session-ref",
        "--out-dir",
        "--repo-commit",
        "--cpu",
        "--os-version",
        "--rustc-version",
        "--cold",
        "--warmup",
        "--measured",
    ];
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut it = args.iter();
    while let Some(key) = it.next() {
        let key = KEYS
            .iter()
            .find(|k| **k == key.as_str())
            .ok_or(ArmError::Usage("unknown argument"))?;
        let value = it.next().ok_or(ArmError::Usage("missing value"))?;
        if values.insert(key, value).is_some() {
            return Err(ArmError::Usage("duplicate argument"));
        }
    }
    let bad = |_: RecordError| ArmError::Usage("invalid value");
    let count = |key: &str, default: usize| -> Result<usize, ArmError> {
        match values.get(key) {
            None => Ok(default),
            Some(v) => v
                .parse::<usize>()
                .map_err(|_| ArmError::Usage("invalid count")),
        }
    };
    let counts = SampleCounts {
        cold: count("--cold", DEFAULT_COLD)?,
        warmup: count("--warmup", DEFAULT_WARMUP)?,
        measured: count("--measured", DEFAULT_MEASURED)?,
    };
    if counts.measured == 0 {
        return Err(ArmError::Usage("measured must be at least 1"));
    }
    let run_id = values
        .get("--run-id")
        .ok_or(ArmError::Usage("--run-id is required"))?;
    // 腕の名前を足した ID が形に合うことを先に確かめる。
    for arm in &ARMS {
        arm_run_id(run_id, arm)?;
    }
    let out_dir = values
        .get("--out-dir")
        .ok_or(ArmError::Usage("--out-dir is required"))?;
    let session_ref = match values.get("--session-ref") {
        Some(v) => SessionRef::Session(Id::parse(v).map_err(bad)?),
        None => SessionRef::NoSession,
    };
    Ok(Config {
        run_id: (*run_id).to_owned(),
        session_ref,
        out_dir: PathBuf::from(out_dir),
        repo_commit: values
            .get("--repo-commit")
            .map(|v| CommitSha::parse(v))
            .transpose()
            .map_err(bad)?,
        cpu: values
            .get("--cpu")
            .map(|v| DeviceName::parse(v))
            .transpose()
            .map_err(bad)?,
        os_version: values
            .get("--os-version")
            .map(|v| VersionText::parse(v))
            .transpose()
            .map_err(bad)?,
        rustc_version: values
            .get("--rustc-version")
            .map(|v| VersionText::parse(v))
            .transpose()
            .map_err(bad)?,
        counts,
    })
}

/// 腕ごとの run の ID（`<run-id>-<腕の名前>`）。形に合わなければ [`ArmError::InvalidRunId`]。
pub fn arm_run_id(run_id: &str, arm: &ArmSpec) -> Result<Id, ArmError> {
    Id::parse(&format!("{run_id}-{}", arm.name)).map_err(|_| ArmError::InvalidRunId)
}

/// 1 ケースの入力。
enum Case {
    /// 行列積。`a` は m×k、`b` は k×n（行優先）。
    Gemm {
        m: usize,
        k: usize,
        n: usize,
        a: Vec<f32>,
        b: Vec<f32>,
    },
    /// 要素ごとの演算。
    Elementwise {
        op: UnaryElementwiseOp,
        input: Vec<f32>,
    },
}

/// 正方の行列積の 1 辺の入力を作る。ケースごとに新しい生成器から `a`、続けて `b` を取る。
fn square_gemm(size: usize) -> Result<Case, ArmError> {
    let len = size.checked_mul(size).ok_or(ArmError::InputFailed)?;
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let a = (0..len).map(|_| rng.next_signed_unit_f32()).collect();
    let b = (0..len).map(|_| rng.next_signed_unit_f32()).collect();
    Ok(Case::Gemm {
        m: size,
        k: size,
        n: size,
        a,
        b,
    })
}

/// 要素ごとの演算の入力を作る。新しい生成器から `0.0` と等しい値を除いて [`ELEMENTWISE_LEN`] 個取る。
fn elementwise(op: UnaryElementwiseOp) -> Case {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let mut input = Vec::with_capacity(ELEMENTWISE_LEN);
    while input.len() < ELEMENTWISE_LEN {
        let x = rng.next_signed_unit_f32();
        if x != 0.0 {
            input.push(x);
        }
    }
    Case::Elementwise { op, input }
}

fn build_case(name: &str) -> Result<Case, ArmError> {
    match name {
        CASE_GEMM_64 => square_gemm(64),
        CASE_GEMM_256 => square_gemm(256),
        CASE_GEMM_37X41X53 => {
            let inputs =
                generate_f32(GemmGateRow::Odd37x41x53).map_err(|_| ArmError::InputFailed)?;
            let row = GemmGateRow::Odd37x41x53;
            Ok(Case::Gemm {
                m: row.m(),
                k: row.k(),
                n: row.n(),
                a: inputs.a().to_vec(),
                b: inputs.b().to_vec(),
            })
        }
        CASE_RELU => Ok(elementwise(UnaryElementwiseOp::Relu)),
        CASE_ADD_SCALAR => Ok(elementwise(UnaryElementwiseOp::AddScalar(0.5))),
        _ => Err(ArmError::UnknownCase),
    }
}

/// f32 の列を、添字の順のリトルエンディアンのバイト列にする。
pub fn f32_le_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// バイト列の小文字 16 進。
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// f32 の列（のリトルエンディアンのバイト列）の sha256 の小文字 16 進。
pub fn f32_sha256_hex(values: &[f32]) -> String {
    hex(&sha256_bytes(&f32_le_bytes(values)))
}

/// 入力の sha256 の表（`wasm-parity-host gen` の `a_sha256`・`b_sha256`・`input_sha256` と同じ定義）。
/// 行列積は `a`・`b`、要素ごと演算は `input`。
pub fn input_sha256_hex(case: &str) -> Result<Vec<(&'static str, String)>, ArmError> {
    Ok(match build_case(case)? {
        Case::Gemm { a, b, .. } => {
            vec![("a", f32_sha256_hex(&a)), ("b", f32_sha256_hex(&b))]
        }
        Case::Elementwise { input, .. } => vec![("input", f32_sha256_hex(&input))],
    })
}

/// 既知の出力の sha256（小文字 16 進）。表に無いケースは `None`。
pub fn known_output_sha256(case: &str) -> Option<&'static str> {
    KNOWN_NEON_OUTPUT_SHA256
        .iter()
        .find(|(c, _)| *c == case)
        .map(|(_, h)| *h)
}

/// 出力の sha256 を既知の値と照合する。`enabled` が偽なら照合しない（`Ok`）。
///
/// [`run`] は simd の腕に対し [`NEON`] を渡す。関数に分けてあるので、neon でないビルドでも試験から直接呼べる。
/// 一致しなければ [`ArmError::OutputMismatch`]、表に無いケースは [`ArmError::UnknownCase`]。
pub fn check_known_output(case: &str, actual_hex: &str, enabled: bool) -> Result<(), ArmError> {
    if !enabled {
        return Ok(());
    }
    let expected = known_output_sha256(case).ok_or(ArmError::UnknownCase)?;
    if expected == actual_hex {
        Ok(())
    } else {
        Err(ArmError::OutputMismatch)
    }
}

/// 1 腕の測定結果。
struct Measured {
    samples: measure_perf::sample::PhaseSamples,
    output: Vec<f32>,
}

/// 腕を測る。計算が消えないように入力・出力に `black_box` を当てる。
fn measure(arm: &ArmSpec, case: &Case, counts: SampleCounts) -> Result<Measured, ArmError> {
    let ops = DefaultCpuOps;
    let (samples, output) = match case {
        Case::Gemm { m, k, n, a, b } => {
            let mut out = vec![0.0_f32; m.checked_mul(*n).ok_or(ArmError::InputFailed)?];
            let samples = collect(counts, || {
                let r = if arm.simd {
                    ops.gemm_simd(black_box(a), black_box(b), *m, *k, *n, black_box(&mut out))
                } else {
                    ops.gemm_scalar(black_box(a), black_box(b), *m, *k, *n, black_box(&mut out))
                };
                black_box(&out);
                r.map(|()| None).map_err(|_| ArmError::OpFailed)
            });
            (samples, out)
        }
        Case::Elementwise { op, input } => {
            let mut out = vec![0.0_f32; input.len()];
            let samples = collect(counts, || {
                let r = if arm.simd {
                    ops.elementwise_simd(*op, black_box(input), black_box(&mut out))
                } else {
                    ops.elementwise_scalar(*op, black_box(input), black_box(&mut out))
                };
                black_box(&out);
                r.map(|()| None).map_err(|_| ArmError::OpFailed)
            });
            (samples, out)
        }
    };
    let samples = samples.map_err(|e| match e {
        CollectError::Submit(e) => e,
        CollectError::Perf(_) => ArmError::CollectFailed,
    })?;
    Ok(Measured { samples, output })
}

/// 現在の UTC 時刻。時計は記録を作る側（本関数）だけが読む。
fn now_utc() -> Result<UtcTime, ArmError> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ArmError::ClockFailed)?
        .as_secs();
    UtcTime::from_unix_secs(secs).map_err(|_| ArmError::ClockFailed)
}

fn name(s: &str) -> Result<Name, ArmError> {
    Name::parse(s).map_err(ArmError::from)
}

fn sha(s: &str) -> Result<Sha256, ArmError> {
    Sha256::parse(s).map_err(ArmError::from)
}

fn environment(config: &Config, start: &UtcTime, end: &UtcTime) -> Result<Environment, ArmError> {
    let snapshot = |t: &UtcTime| EnvSnapshot {
        utc: Some(t.clone()),
        ..EnvSnapshot::default()
    };
    let os_name = if cfg!(target_os = "macos") {
        Some(OsName::Macos)
    } else if cfg!(target_os = "linux") {
        Some(OsName::Linux)
    } else if cfg!(target_os = "windows") {
        Some(OsName::Windows)
    } else {
        None
    };
    let target_features = if NEON {
        vec![OptionToken::parse("neon")?]
    } else {
        Vec::new()
    };
    Ok(Environment {
        gpu: None,
        cpu: config.cpu.clone(),
        os_name,
        os_version: config.os_version.clone(),
        rustc_version: config.rustc_version.clone(),
        repo_commit: config.repo_commit.clone(),
        cargo_lock_sha256: None,
        build_profile: Some(if cfg!(debug_assertions) {
            BuildProfile::Dev
        } else {
            BuildProfile::Release
        }),
        feature_set: Some(vec![OptionToken::parse("cpu-ops")?]),
        target_features: Some(target_features),
        shader_compile_options: None,
        library_versions: None,
        validation_layers: None,
        backend_env: BackendEnv::Metal {
            xcode_version: None,
            metal_toolchain: None,
        },
        before: snapshot(start),
        after: snapshot(end),
    })
}

/// 腕 1 個の記録の書き先（`<out-dir>/<run の ID>.json`）。
fn record_path(config: &Config, arm: &ArmSpec) -> Result<PathBuf, ArmError> {
    let id = arm_run_id(&config.run_id, arm)?;
    Ok(config.out_dir.join(format!("{}.json", id.as_str())))
}

/// 測る前に、出力先が在り、書く予定の 10 ファイルがどれも無いことを確かめる。
fn precheck(config: &Config) -> Result<(), ArmError> {
    if !Path::new(&config.out_dir).is_dir() {
        return Err(ArmError::MissingDirectory);
    }
    for arm in &ARMS {
        if record_path(config, arm)?.symlink_metadata().is_ok() {
            return Err(ArmError::FileExists);
        }
    }
    Ok(())
}

/// 10 個の腕を順に測り、腕ごとに記録を 1 ファイル書く。結果の行を `out` に出す。
///
/// 先頭の行は `neon=<true|false>`、続けて腕ごとに `arm=<名前> neon=<..> median_ns=<..> output_sha256=<..>`。
/// neon が真のビルドで simd の腕の出力が既知の sha256 と一致しなければ、その腕の記録を書かずに `Err`。
pub fn run(config: &Config, out: &mut impl Write) -> Result<(), ArmError> {
    precheck(config)?;
    let say = |out: &mut dyn Write, line: String| -> Result<(), ArmError> {
        writeln!(out, "{line}").map_err(|_| ArmError::OutputFailed)
    };
    say(out, format!("neon={NEON}"))?;
    let submissions = u64::try_from(
        config
            .counts
            .cold
            .saturating_add(config.counts.warmup)
            .saturating_add(config.counts.measured),
    )
    .map_err(|_| ArmError::CollectFailed)?;
    for arm in &ARMS {
        let case = build_case(arm.case)?;
        let input_hashes: HashTable = input_sha256_hex(arm.case)?
            .into_iter()
            .map(|(k, v)| Ok((name(k)?, sha(&v)?)))
            .collect::<Result<_, ArmError>>()?;
        let started = now_utc()?;
        let measured = measure(arm, &case, config.counts)?;
        let ended = now_utc()?;
        let output_hex = f32_sha256_hex(&measured.output);
        check_known_output(arm.case, &output_hex, NEON && arm.simd)?;
        let stats = run_stats(&measured.samples.measured_cpu_ns()).map_err(ArmError::Stats)?;
        let body = PerfSamplesBody::new(config.counts, measured.samples, submissions, submissions)?;
        let envelope = Envelope {
            run_id: arm_run_id(&config.run_id, arm)?,
            task_ids: vec![RefId::parse(TASK_ID)?],
            behavior_ids: Vec::new(),
            machine_combination: MachineCombination::MetalMacMini,
            machine: Machine::MacMini,
            label: Label::Pending,
            started_utc: started.clone(),
            ended_utc: ended.clone(),
            session_ref: config.session_ref.clone(),
            evidence: Evidence {
                adapter_backend: None::<AdapterBackend>,
                adapter_name: None,
                kernel_sha256: None,
                artifact_sha256: None,
                input_sha256: Some(input_hashes),
                output_sha256: OutputHash::Sha256(sha(&output_hex)?),
            },
            environment: environment(config, &started, &ended)?,
            wgpu_baseline: None,
        };
        let record = RunRecord::new(envelope, Body::PerfSamples(body))?;
        write_new(&record_path(config, arm)?, &record).map_err(|e| match e {
            RecordError::Io(std::io::ErrorKind::AlreadyExists) => ArmError::FileExists,
            other => ArmError::Record(other),
        })?;
        say(
            out,
            format!(
                "arm={} neon={} median_ns={} output_sha256={}",
                arm.name,
                NEON && arm.simd,
                stats.median,
                output_hex
            ),
        )?;
    }
    Ok(())
}
