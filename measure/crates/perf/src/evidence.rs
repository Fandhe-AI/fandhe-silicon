//! 封筒の `evidence`・`environment`・`wgpu_baseline` の欄と、それらが使う検証済みの値型。
//!
//! `record` の封筒（`Envelope`）に載り、`RunRecord::to_json` から書き出される。
//! 値は白表の列挙・形を狭く検証した文字列・数だけで、ホスト名・絶対パス・資格情報つきの URL・自由な文を受ける口を持たない。
//! 取得不能の規則は 2 つに分ける。型を持つ欄（真偽・数・配列・表）は `null`、文字列の欄は予約語 [`UNAVAILABLE`]。
//! 出力が無いことは取得不能と区別し、予約語 [`NO_OUTPUT`] で書く。取得不能を 0・空・既定値で埋めない。
//! 条件付きの欄は条件に当たらないときキーごと出さない（背後実装ごとの欄は [`BackendEnv`]、git の欄は [`BaselineSourceSpec`] の型で表す）。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-27・TASK-28（ポインタのみ）。

use crate::json;
use crate::record::{RecordError, UtcTime};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// 文字列の欄の取得不能を表す予約語。どの値型の形にも一致しない（先頭が `_`）。
pub const UNAVAILABLE: &str = "_unavailable";

/// `evidence.output_sha256` の「出力が無い」を表す予約語。取得不能とは別で、どの値型の形にも一致しない。
pub const NO_OUTPUT: &str = "_no-output";

fn invalid() -> RecordError {
    RecordError::InvalidValue
}

macro_rules! token {
    ($(#[$m:meta])* $name:ident, |$s:ident| $ok:expr) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// 形を検証して受ける。合わなければ [`RecordError::InvalidValue`]（入力は埋め込まない）。
            pub fn parse($s: &str) -> Result<Self, RecordError> {
                if $ok { Ok(Self($s.to_owned())) } else { Err(invalid()) }
            }

            /// 文字列表現。
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

token!(
    /// 64 桁の小文字 16 進（SHA-256）。
    Sha256, |s| is_hex(s, 64)
);

token!(
    /// 40 桁の小文字 16 進（commit SHA）。
    CommitSha, |s| is_hex(s, 40)
);

token!(
    /// `x.y.z` の版（各部は先頭 0 なしの 10 進で 9 桁以内）。
    SemVer, |s| {
        let mut n = 0;
        let ok = s.split('.').all(|p| {
            n += 1;
            (1..=9).contains(&p.len())
                && p.bytes().all(|c| c.is_ascii_digit())
                && (p == "0" || !p.starts_with('0'))
        });
        ok && n == 3
    }
);

token!(
    /// 英数字を `.`・`-`・`+` でつないだ版の綴り（1〜64 文字、先頭は英数字）。
    VersionText, |s| {
        let b = s.as_bytes();
        (1..=64).contains(&b.len())
            && b[0].is_ascii_alphanumeric()
            && b.iter()
                .all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'+'))
    }
);

token!(
    /// 表のキーに使う名前。`[A-Za-z0-9][A-Za-z0-9._-]*` で 1〜64 文字（`/`・空白・`_` 始まりを受けない）。
    Name, |s| {
        let b = s.as_bytes();
        (1..=64).contains(&b.len())
            && b[0].is_ascii_alphanumeric()
            && b.iter()
                .all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    }
);

token!(
    /// device が返した名前。ASCII の英数字・空白・`._()+,-` だけで 1〜128 文字、前後が空白でなく `_` 始まりでない。
    DeviceName, |s| {
        let b = s.as_bytes();
        (1..=128).contains(&b.len())
            && b[0] != b' '
            && b[0] != b'_'
            && b[b.len() - 1] != b' '
            && b.iter().all(|&c| {
                c.is_ascii_alphanumeric()
                    || matches!(c, b' ' | b'.' | b'_' | b'(' | b')' | b'+' | b',' | b'-')
            })
    }
);

token!(
    /// 機能・オプションの指定 1 つ。空白・引用符・`\`・`/`・`~`・制御文字を含まない ASCII で 1〜128 文字、`_` 始まりでない。
    OptionToken, |s| {
        let b = s.as_bytes();
        (1..=128).contains(&b.len())
            && b[0] != b'_'
            && b.iter().all(|&c| {
                c.is_ascii_graphic() && !matches!(c, b'"' | b'\'' | b'`' | b'\\' | b'/' | b'~')
            })
    }
);

token!(
    /// リポジトリの URL。`https://` 始まりの ASCII で、空白・引用符・`\`・`@`・`?`・`#` を含まず 256 文字以内。
    GitRepository, |s| {
        s.len() <= 256
            && s.strip_prefix("https://").is_some_and(|rest| !rest.is_empty())
            && s.bytes().all(|c| {
                c.is_ascii_graphic()
                    && !matches!(c, b'"' | b'\'' | b'`' | b'\\' | b'@' | b'?' | b'#')
            })
    }
);

/// 固定したタグ名（形は [`Name`] と同じ）。
pub type GitTag = Name;

/// 比較基準の区分。名前の形だけを検証して受ける（区分の名前・数・意味は private spec 側の記録に従う。ポインタのみ）。
pub type Basis = Name;

/// 負荷平均（1・5・15 分）。千分の一単位の整数で持ち、浮動小数点を使わない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadAvg([u64; 3]);

impl LoadAvg {
    /// 空白 1 つ区切りの非負の 10 進 3 つ（整数部 6 桁以内、小数部は 3 桁以内）から作る。
    pub fn parse(s: &str) -> Result<Self, RecordError> {
        let mut out = [0u64; 3];
        let mut parts = s.split(' ');
        for slot in &mut out {
            *slot = parse_milli(parts.next().ok_or_else(invalid)?)?;
        }
        if parts.next().is_some() {
            return Err(invalid());
        }
        Ok(Self(out))
    }

    /// 千分の一単位の 3 つの値。
    pub fn as_milli(&self) -> [u64; 3] {
        self.0
    }
}

fn parse_milli(p: &str) -> Result<u64, RecordError> {
    let (int, frac) = p.split_once('.').unwrap_or((p, ""));
    let digits = |t: &str| t.bytes().all(|c| c.is_ascii_digit());
    if !(1..=6).contains(&int.len())
        || frac.len() > 3
        || (p.contains('.') && frac.is_empty())
        || !digits(int)
        || !digits(frac)
    {
        return Err(invalid());
    }
    let i: u64 = int.parse().map_err(|_| invalid())?;
    let f: u64 = if frac.is_empty() {
        0
    } else {
        let scale = 10u64.pow(3 - u32::try_from(frac.len()).map_err(|_| invalid())?);
        frac.parse::<u64>().map_err(|_| invalid())? * scale
    };
    Ok(i * 1000 + f)
}

macro_rules! spelled_enum {
    ($(#[$m:meta])* $name:ident, $n:literal, { $($(#[$vm:meta])* $v:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($(#[$vm])* $v),+ }

        impl $name {
            /// 全ての値。
            pub const ALL: [$name; $n] = [$(Self::$v),+];

            /// 記録に書く綴り。
            pub const fn name(self) -> &'static str {
                match self { $(Self::$v => $s),+ }
            }
        }
    };
}

spelled_enum!(
    /// 実際に使った背後実装の呼び名。
    AdapterBackend, 3, {
        /// Metal。
        Metal => "metal",
        /// Vulkan。
        Vulkan => "vulkan",
        /// CUDA。
        Cuda => "cuda",
    }
);

spelled_enum!(
    /// OS の呼び名。
    OsName, 3, {
        /// macOS。
        Macos => "macos",
        /// Linux。
        Linux => "linux",
        /// Windows。
        Windows => "windows",
    }
);

spelled_enum!(
    /// ビルドのプロファイル。
    BuildProfile, 4, {
        /// dev。
        Dev => "dev",
        /// release。
        Release => "release",
        /// test。
        Test => "test",
        /// bench。
        Bench => "bench",
    }
);

spelled_enum!(
    /// 検証レイヤの有無。
    ValidationLayers, 2, {
        /// 有効。
        Enabled => "enabled",
        /// 無効。
        Disabled => "disabled",
    }
);

spelled_enum!(
    /// 電源の種類（綴りは `scripts/metal-session.sh` の probe に揃える）。
    PowerSource, 3, {
        /// AC 電源。
        Ac => "ac",
        /// バッテリ。
        Battery => "battery",
        /// UPS。
        Ups => "ups",
    }
);

/// 経路の証拠の出力ハッシュ。出力が無いことは取得不能と別に持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputHash {
    /// 出力のハッシュ。
    Sha256(Sha256),
    /// 出力が無い（[`NO_OUTPUT`]）。
    NoOutput,
    /// 取得不能（[`UNAVAILABLE`]）。
    Unavailable,
}

/// 名前 → SHA-256 の表。
pub type HashTable = BTreeMap<Name, Sha256>;

/// 経路の証拠。`adapter_backend` は呼び手の入力のまま持ち、組み合わせから導かない・寄せない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// 実際に使った背後実装。
    pub adapter_backend: Option<AdapterBackend>,
    /// device が返した名前。
    pub adapter_name: Option<DeviceName>,
    /// カーネルごとのハッシュ。
    pub kernel_sha256: Option<HashTable>,
    /// 成果物ごとのハッシュ。
    pub artifact_sha256: Option<HashTable>,
    /// 入力ごとのハッシュ。
    pub input_sha256: Option<HashTable>,
    /// 出力のハッシュ。
    pub output_sha256: OutputHash,
}

/// 開始時・終了時の環境の断面。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvSnapshot {
    /// 取った時刻。
    pub utc: Option<UtcTime>,
    /// 電源の種類。
    pub power_source: Option<PowerSource>,
    /// 低電力モード。
    pub low_power_mode: Option<bool>,
    /// 熱の警告。
    pub thermal_warning: Option<bool>,
    /// 負荷平均。
    pub loadavg: Option<LoadAvg>,
}

/// 名前 → 版の表。
pub type VersionTable = BTreeMap<Name, VersionText>;

/// 背後実装ごとの環境の欄。組み合わせの背後実装と列挙子が合わなければ記録を作れない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEnv {
    /// Metal の欄。
    Metal {
        /// Xcode の版。
        xcode_version: Option<VersionText>,
        /// Metal toolchain の版。
        metal_toolchain: Option<VersionText>,
    },
    /// Vulkan の欄。
    Vulkan {
        /// driver の版。
        driver_version: Option<VersionText>,
        /// SDK の版の表。
        sdk_versions: Option<VersionTable>,
    },
    /// CUDA の欄。
    Cuda {
        /// driver の版。
        driver_version: Option<VersionText>,
        /// SDK の版の表。
        sdk_versions: Option<VersionTable>,
    },
}

impl BackendEnv {
    /// この欄が属する背後実装。
    pub const fn backend(&self) -> AdapterBackend {
        match self {
            Self::Metal { .. } => AdapterBackend::Metal,
            Self::Vulkan { .. } => AdapterBackend::Vulkan,
            Self::Cuda { .. } => AdapterBackend::Cuda,
        }
    }
}

/// 環境の欄。取得不能は `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// GPU の名前。
    pub gpu: Option<DeviceName>,
    /// CPU の名前。
    pub cpu: Option<DeviceName>,
    /// OS。
    pub os_name: Option<OsName>,
    /// OS の版。
    pub os_version: Option<VersionText>,
    /// rustc の版。
    pub rustc_version: Option<VersionText>,
    /// 本リポの commit。
    pub repo_commit: Option<CommitSha>,
    /// `Cargo.lock` のハッシュ。
    pub cargo_lock_sha256: Option<Sha256>,
    /// ビルドのプロファイル。
    pub build_profile: Option<BuildProfile>,
    /// 有効にした feature。
    pub feature_set: Option<Vec<OptionToken>>,
    /// target feature。
    pub target_features: Option<Vec<OptionToken>>,
    /// シェーダのコンパイルオプション。
    pub shader_compile_options: Option<Vec<OptionToken>>,
    /// ライブラリの版の表。
    pub library_versions: Option<VersionTable>,
    /// 検証レイヤの有無。
    pub validation_layers: Option<ValidationLayers>,
    /// 背後実装ごとの欄。
    pub backend_env: BackendEnv,
    /// 開始時の断面。
    pub before: EnvSnapshot,
    /// 終了時の断面。
    pub after: EnvSnapshot,
}

/// 比較基準の wgpu の取得元。git のときだけ git の欄を持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineSourceSpec {
    /// crates.io。
    CratesIo,
    /// git。
    Git {
        /// リポジトリ。
        repository: Option<GitRepository>,
        /// タグ。
        tag: Option<GitTag>,
        /// commit。
        commit: Option<CommitSha>,
    },
}

/// 比較用の run に載せる比較基準の wgpu の版の欄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgpuBaseline {
    /// 解決された版。
    pub version: Option<SemVer>,
    /// 取得元。
    pub source: BaselineSourceSpec,
    /// 別 workspace の `Cargo.lock` のハッシュ。
    pub lockfile_sha256: Option<Sha256>,
    /// 比較基準の区分（省略しない）。
    pub basis: Basis,
}

fn ver(v: &Option<VersionText>) -> Option<&str> {
    v.as_ref().map(VersionText::as_str)
}

fn key(out: &mut String, first: &mut bool, k: &str) {
    if !*first {
        out.push(',');
    }
    *first = false;
    let _ = write!(out, "\"{k}\":");
}

/// 文字列の欄。取得不能は予約語。
fn text(out: &mut String, first: &mut bool, k: &str, v: Option<&str>) {
    key(out, first, k);
    json::push_string(out, v.unwrap_or(UNAVAILABLE));
}

fn opt_array(out: &mut String, first: &mut bool, k: &str, v: Option<&[OptionToken]>) {
    key(out, first, k);
    match v {
        Some(v) => json::push_str_array(out, v.iter().map(OptionToken::as_str)),
        None => out.push_str("null"),
    }
}

fn opt_map<V: AsRef<str>>(
    out: &mut String,
    first: &mut bool,
    k: &str,
    v: Option<&BTreeMap<Name, V>>,
) {
    key(out, first, k);
    match v {
        Some(m) => json::push_str_map(out, m.iter().map(|(a, b)| (a.as_str(), b.as_ref()))),
        None => out.push_str("null"),
    }
}

impl Evidence {
    pub(crate) fn write_json(&self, out: &mut String) {
        let f = &mut true;
        out.push('{');
        text(
            out,
            f,
            "adapter_backend",
            self.adapter_backend.map(AdapterBackend::name),
        );
        text(
            out,
            f,
            "adapter_name",
            self.adapter_name.as_ref().map(DeviceName::as_str),
        );
        opt_map(out, f, "kernel_sha256", self.kernel_sha256.as_ref());
        opt_map(out, f, "artifact_sha256", self.artifact_sha256.as_ref());
        opt_map(out, f, "input_sha256", self.input_sha256.as_ref());
        let o = match &self.output_sha256 {
            OutputHash::Sha256(h) => h.as_str(),
            OutputHash::NoOutput => NO_OUTPUT,
            OutputHash::Unavailable => UNAVAILABLE,
        };
        text(out, f, "output_sha256", Some(o));
        out.push('}');
    }
}

impl EnvSnapshot {
    fn write_json(&self, out: &mut String) {
        let f = &mut true;
        out.push('{');
        text(out, f, "utc", self.utc.as_ref().map(UtcTime::as_str));
        text(
            out,
            f,
            "power_source",
            self.power_source.map(PowerSource::name),
        );
        key(out, f, "low_power_mode");
        json::push_opt_bool(out, self.low_power_mode);
        key(out, f, "thermal_warning");
        json::push_opt_bool(out, self.thermal_warning);
        key(out, f, "loadavg");
        match self.loadavg {
            Some(l) => {
                out.push('[');
                for (i, m) in l.as_milli().into_iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    json::push_milli(out, m);
                }
                out.push(']');
            }
            None => out.push_str("null"),
        }
        out.push('}');
    }
}

impl Environment {
    pub(crate) fn write_json(&self, out: &mut String) {
        let f = &mut true;
        out.push('{');
        text(out, f, "gpu", self.gpu.as_ref().map(DeviceName::as_str));
        text(out, f, "cpu", self.cpu.as_ref().map(DeviceName::as_str));
        text(out, f, "os_name", self.os_name.map(OsName::name));
        text(out, f, "os_version", ver(&self.os_version));
        text(out, f, "rustc_version", ver(&self.rustc_version));
        text(
            out,
            f,
            "repo_commit",
            self.repo_commit.as_ref().map(CommitSha::as_str),
        );
        text(
            out,
            f,
            "cargo_lock_sha256",
            self.cargo_lock_sha256.as_ref().map(Sha256::as_str),
        );
        text(
            out,
            f,
            "build_profile",
            self.build_profile.map(BuildProfile::name),
        );
        opt_array(out, f, "feature_set", self.feature_set.as_deref());
        opt_array(out, f, "target_features", self.target_features.as_deref());
        opt_array(
            out,
            f,
            "shader_compile_options",
            self.shader_compile_options.as_deref(),
        );
        opt_map(out, f, "library_versions", self.library_versions.as_ref());
        text(
            out,
            f,
            "validation_layers",
            self.validation_layers.map(ValidationLayers::name),
        );
        match &self.backend_env {
            BackendEnv::Metal {
                xcode_version,
                metal_toolchain,
            } => {
                text(out, f, "xcode_version", ver(xcode_version));
                text(out, f, "metal_toolchain", ver(metal_toolchain));
            }
            BackendEnv::Vulkan {
                driver_version,
                sdk_versions,
            }
            | BackendEnv::Cuda {
                driver_version,
                sdk_versions,
            } => {
                text(out, f, "driver_version", ver(driver_version));
                opt_map(out, f, "sdk_versions", sdk_versions.as_ref());
            }
        }
        key(out, f, "before");
        self.before.write_json(out);
        key(out, f, "after");
        self.after.write_json(out);
        out.push('}');
    }
}

impl WgpuBaseline {
    pub(crate) fn write_json(&self, out: &mut String) {
        let f = &mut true;
        out.push('{');
        text(out, f, "version", self.version.as_ref().map(SemVer::as_str));
        match &self.source {
            BaselineSourceSpec::CratesIo => text(out, f, "source", Some("crates-io")),
            BaselineSourceSpec::Git {
                repository,
                tag,
                commit,
            } => {
                text(out, f, "source", Some("git"));
                text(
                    out,
                    f,
                    "git_repository",
                    repository.as_ref().map(GitRepository::as_str),
                );
                text(out, f, "git_tag", tag.as_ref().map(Name::as_str));
                text(out, f, "git_commit", commit.as_ref().map(CommitSha::as_str));
            }
        }
        text(
            out,
            f,
            "lockfile_sha256",
            self.lockfile_sha256.as_ref().map(Sha256::as_str),
        );
        text(out, f, "basis", Some(self.basis.as_str()));
        out.push('}');
    }
}
