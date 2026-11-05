//! GEMM の 3 ケースの入力と、host の `DefaultCpuOps::gemm_scalar` の出力をファイルに書く host 側の道具
//! （spec TASK-994・PARITY-20 のポインタ）。
//!
//! 別 workspace `wasm/` の member。`scripts/wasm-node-check.sh` が Node で glue の出力と比べるときに読むファイルを、
//! `wasm-parity-host gen <dir>` が `<dir>` に書く。入力の生成器は `fandhe-silicon-core` の `parity::gemm_inputs`
//! （`test-support` の裏）、参照は `fandhe-silicon-contract` の `DefaultCpuOps` を使う。
//!
//! 書くファイルは 3 ケースそれぞれの `.a.bin`・`.b.bin`・`.native-scalar.bin`（f32 のリトルエンディアン）と、
//! 各ファイルの sha256 を持つ `manifest.json`。既存のファイルは上書きしない。

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, SplitMix64, generate_f32,
};

/// ケースの入力の出どころ。
#[derive(Debug, Clone, Copy)]
pub enum GemmCaseSource {
    /// 1 辺が `usize` の正方のケース。ケースごとに新しい生成器から `a`、続けて `b` を取る。
    Square(usize),
    /// 生成器の行から作るケース。
    GateRow(GemmGateRow),
}

/// ケースの表の 1 行。
#[derive(Debug, Clone, Copy)]
pub struct GemmCaseSpec {
    /// ファイル名の前半になるケース名（`[a-z0-9.-]` だけ）。
    pub name: &'static str,
    /// 入力の出どころ。
    pub source: GemmCaseSource,
}

/// 書き出す 3 ケース（64³・256³・37×41×53）。
pub const GEMM_CASES: [GemmCaseSpec; 3] = [
    GemmCaseSpec {
        name: "gemm-64",
        source: GemmCaseSource::Square(64),
    },
    GemmCaseSpec {
        name: "gemm-256",
        source: GemmCaseSource::Square(256),
    },
    GemmCaseSpec {
        name: "gemm-37x41x53",
        source: GemmCaseSource::GateRow(GemmGateRow::Odd37x41x53),
    },
];

/// 1 ケースの入力。`a` は m×k、`b` は k×n（行優先）。
#[derive(Debug, Clone)]
pub struct GemmCase {
    /// ケース名。
    pub name: &'static str,
    /// 行数。
    pub m: usize,
    /// 内積の長さ。
    pub k: usize,
    /// 列数。
    pub n: usize,
    /// 左の行列。
    pub a: Vec<f32>,
    /// 右の行列。
    pub b: Vec<f32>,
}

/// 実行の失敗。`Display` は固定の英語の文言で、パスの値を含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenError {
    /// 引数が `gen <dir>` の形でない。
    Usage,
    /// 出力先のディレクトリが無い。
    MissingDirectory,
    /// 書く予定の名前のファイルが既にある。
    FileExists,
    /// ファイルを書けなかった。
    WriteFailed,
    /// 入力を作れなかった。
    InputFailed,
    /// 参照の計算が `Err` を返した。
    NativeFailed,
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            GenError::Usage => "usage: wasm-parity-host gen <dir>",
            GenError::MissingDirectory => "error: output directory does not exist",
            GenError::FileExists => "error: output file already exists",
            GenError::WriteFailed => "error: cannot write output file",
            GenError::InputFailed => "error: cannot generate inputs",
            GenError::NativeFailed => "error: native gemm_scalar failed",
        };
        f.write_str(text)
    }
}

impl std::error::Error for GenError {}

/// 長さ `x * y` を返す。溢れたら入力を作れないものとして扱う。
fn checked_len(x: usize, y: usize) -> Result<usize, GenError> {
    x.checked_mul(y).ok_or(GenError::InputFailed)
}

fn square_values(rng: &mut SplitMix64, len: usize) -> Vec<f32> {
    (0..len).map(|_| rng.next_signed_unit_f32()).collect()
}

/// 表の 1 行から入力を作る。
pub fn build_case(spec: &GemmCaseSpec) -> Result<GemmCase, GenError> {
    match spec.source {
        GemmCaseSource::Square(size) => {
            let len = checked_len(size, size)?;
            let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
            let a = square_values(&mut rng, len);
            let b = square_values(&mut rng, len);
            Ok(GemmCase {
                name: spec.name,
                m: size,
                k: size,
                n: size,
                a,
                b,
            })
        }
        GemmCaseSource::GateRow(row) => {
            let inputs = generate_f32(row).map_err(|_| GenError::InputFailed)?;
            Ok(GemmCase {
                name: spec.name,
                m: row.m(),
                k: row.k(),
                n: row.n(),
                a: inputs.a().to_vec(),
                b: inputs.b().to_vec(),
            })
        }
    }
}

/// 3 ケースの入力を作る。
pub fn build_cases() -> Result<Vec<GemmCase>, GenError> {
    GEMM_CASES.iter().map(build_case).collect()
}

/// `DefaultCpuOps::gemm_scalar` の出力（m×n）を返す。
pub fn native_scalar(case: &GemmCase) -> Result<Vec<f32>, GenError> {
    let mut out = vec![0.0f32; checked_len(case.m, case.n)?];
    DefaultCpuOps
        .gemm_scalar(&case.a, &case.b, case.m, case.k, case.n, &mut out)
        .map_err(|_| GenError::NativeFailed)?;
    Ok(out)
}

/// f32 の列を、添字の順のリトルエンディアンのバイト列にする。
pub fn f32_le_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

/// 書く 1 ファイル（名前と中身）。
struct OutFile {
    name: String,
    bytes: Vec<u8>,
}

/// 1 ケースの 3 ファイルと、manifest の 1 要素の文字列。
struct CaseOutput {
    files: [OutFile; 3],
    manifest_entry: String,
}

fn case_output(case: &GemmCase) -> Result<CaseOutput, GenError> {
    let native = native_scalar(case)?;
    let a_file = format!("{}.a.bin", case.name);
    let b_file = format!("{}.b.bin", case.name);
    let n_file = format!("{}.native-scalar.bin", case.name);
    let a_bytes = f32_le_bytes(&case.a);
    let b_bytes = f32_le_bytes(&case.b);
    let n_bytes = f32_le_bytes(&native);
    let entry = [
        "    {".to_string(),
        format!("      \"name\": \"{}\",", case.name),
        "      \"kind\": \"gemm\",".to_string(),
        format!("      \"m\": {},", case.m),
        format!("      \"k\": {},", case.k),
        format!("      \"n\": {},", case.n),
        format!("      \"a_file\": \"{a_file}\","),
        format!("      \"b_file\": \"{b_file}\","),
        format!("      \"native_scalar_file\": \"{n_file}\","),
        format!("      \"a_sha256\": \"{}\",", hex(&sha256_bytes(&a_bytes))),
        format!("      \"b_sha256\": \"{}\",", hex(&sha256_bytes(&b_bytes))),
        format!(
            "      \"native_scalar_sha256\": \"{}\"",
            hex(&sha256_bytes(&n_bytes))
        ),
        "    }".to_string(),
    ]
    .join("\n");
    Ok(CaseOutput {
        files: [
            OutFile {
                name: a_file,
                bytes: a_bytes,
            },
            OutFile {
                name: b_file,
                bytes: b_bytes,
            },
            OutFile {
                name: n_file,
                bytes: n_bytes,
            },
        ],
        manifest_entry: entry,
    })
}

/// 同名のものが（シンボリックリンクを含め）あるか。
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// 新しいファイルとして書く。既にあれば `FileExists`。
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), GenError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                GenError::FileExists
            } else {
                GenError::WriteFailed
            }
        })?;
    file.write_all(bytes).map_err(|_| GenError::WriteFailed)
}

/// `dir` に 3 ケースの入力・参照・`manifest.json` を書く。
///
/// 入力と参照を全部作ってから、書く名前の不在を確かめ、`.bin`、最後に `manifest.json` の順に書く。
pub fn generate(dir: &Path) -> Result<(), GenError> {
    if !fs::metadata(dir).map(|m| m.is_dir()).unwrap_or(false) {
        return Err(GenError::MissingDirectory);
    }
    let cases = build_cases()?;
    let outputs = cases
        .iter()
        .map(case_output)
        .collect::<Result<Vec<_>, _>>()?;

    let manifest = format!(
        "{{\n  \"cases\": [\n{}\n  ]\n}}\n",
        outputs
            .iter()
            .map(|o| o.manifest_entry.as_str())
            .collect::<Vec<_>>()
            .join(",\n")
    );
    let manifest_path: PathBuf = dir.join("manifest.json");
    let targets: Vec<PathBuf> = outputs
        .iter()
        .flat_map(|o| o.files.iter().map(|f| dir.join(&f.name)))
        .chain(std::iter::once(manifest_path.clone()))
        .collect();
    if targets.iter().any(|p| exists(p)) {
        return Err(GenError::FileExists);
    }
    for output in &outputs {
        for file in &output.files {
            write_new(&dir.join(&file.name), &file.bytes)?;
        }
    }
    write_new(&manifest_path, manifest.as_bytes())
}

/// 実行の入口。`args` は実行ファイル名を除いた列で、ちょうど `gen <dir>` の 2 つを受ける。
pub fn run(args: &[OsString]) -> Result<(), GenError> {
    match args {
        [cmd, dir] if cmd == "gen" => generate(Path::new(dir)),
        _ => Err(GenError::Usage),
    }
}
