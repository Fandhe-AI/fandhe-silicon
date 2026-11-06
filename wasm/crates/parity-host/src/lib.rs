//! GEMM と部分 Top-k の 3 ケースずつ、要素ごと演算の 2 ケース、-0.0 を含む部分 Top-k の 5 ケース、特殊値の GEMM の 14 ケースの入力と、
//! host の `DefaultCpuOps::gemm_scalar`・`topk_partial_scalar`・`elementwise_scalar` の出力、特殊値の GEMM の CPU 参照をファイルに書く
//! host 側の道具（spec TASK-994・PARITY-20・TASK-996・TASK-997・TASK-2451 のポインタ）。
//! 特殊値の GEMM の 14 ケースは spec TASK-999 のポインタ。
//!
//! 別 workspace `wasm/` の member。`scripts/wasm-node-check.sh` が Node で glue の出力と比べるときに読むファイルを、
//! `wasm-parity-host gen <dir>` が `<dir>` に書く。入力の生成器は `fandhe-silicon-core` の `parity::gemm_inputs`・
//! `parity::case_inputs`（`test-support` の裏）、参照は `fandhe-silicon-contract` の `DefaultCpuOps` を使う。
//!
//! 書くファイルは GEMM の 3 ケースそれぞれの `.a.bin`・`.b.bin`・`.native-scalar.bin`（f32 のリトルエンディアン）、
//! 部分 Top-k の 3 ケースそれぞれの `.scores.bin`（f32 のリトルエンディアン）・`.native-scalar.bin`
//! （1 件が、スロット番号の u32、スコアの f32 のビット列の順のリトルエンディアン 8 バイト）、
//! 要素ごと演算の 2 ケースそれぞれの `.input.bin`・`.native-scalar.bin`（f32 のリトルエンディアン）、
//! -0.0 を含む部分 Top-k の 5 ケースそれぞれの `.scores.bin`・`.native-scalar.bin`（部分 Top-k の 3 ケースと同じ形）と、
//! 特殊値の GEMM の 14 ケース（core の `CaseId::G01`〜`G13`・`E1`）それぞれの `.a.bin`・`.b.bin`・`.native-scalar.bin`・
//! `.cpu-reference.bin`（f32 のリトルエンディアン。最後は core の CPU 参照）と、
//! 各ファイルの sha256 を持つ `manifest.json`。既存のファイルは上書きしない。

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, UnaryElementwiseOp};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::case_inputs::{CaseId, CaseInput, additional_topk_inputs};
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
    /// 部分 Top-k の参照の計算が `Err` を返した。
    NativeTopkFailed,
    /// 要素ごと演算の参照の計算が `Err` を返した。
    NativeElementwiseFailed,
    /// core の CPU 参照を作れなかった（`Err`、または f32 の出力でない）。
    CpuReferenceFailed,
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
            GenError::NativeTopkFailed => "error: native topk_partial_scalar failed",
            GenError::NativeElementwiseFailed => "error: native elementwise_scalar failed",
            GenError::CpuReferenceFailed => "error: cpu reference failed",
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

/// 部分 Top-k のケースの入力の作り方。どれもケースごとに新しい `SplitMix64::new(GEMM_INPUT_SEED)` から作る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopkInput {
    /// `next_signed_unit_f32` を件数だけ取る。
    Batch,
    /// 添字 `i` が `i % 10 == 3` なら `0.0`（生成器を進めない）、それ以外は `((next_u64() >> 61) as f32) + 1.0`。
    Ties,
    /// `Batch` と同じ列の添字 5・17・30・77 を、NaN・負の NaN（`0xffc0_0000`）・`+Inf`・`-Inf` に置く。
    NonFinite,
}

/// 部分 Top-k のケースの表の 1 行。
#[derive(Debug, Clone, Copy)]
pub struct TopkCaseSpec {
    /// ファイル名の前半になるケース名（`[a-z0-9.-]` だけ）。
    pub name: &'static str,
    /// スコアの件数。
    pub len: usize,
    /// 取り出す件数。
    pub k: usize,
    /// 入力の作り方。
    pub input: TopkInput,
}

/// 書き出す部分 Top-k の 3 ケース（spec PARITY-22 のポインタ）。
pub const TOPK_CASES: [TopkCaseSpec; 3] = [
    TopkCaseSpec {
        name: "topk-batch",
        len: 20_000,
        k: 64,
        input: TopkInput::Batch,
    },
    TopkCaseSpec {
        name: "topk-ties",
        len: 200,
        k: 10,
        input: TopkInput::Ties,
    },
    TopkCaseSpec {
        name: "topk-nonfinite",
        len: 200,
        k: 10,
        input: TopkInput::NonFinite,
    },
];

/// 部分 Top-k の 1 ケースの入力。
#[derive(Debug, Clone)]
pub struct TopkCase {
    /// ケース名。
    pub name: &'static str,
    /// 取り出す件数。
    pub k: usize,
    /// スコアの列。
    pub scores: Vec<f32>,
}

/// `TopkInput::NonFinite` で差し込む（添字, ビット列）。
const NON_FINITE_SLOTS: [(usize, u32); 4] = [
    (5, 0x7fc0_0000),
    (17, 0xffc0_0000),
    (30, 0x7f80_0000),
    (77, 0xff80_0000),
];

/// 表の 1 行から部分 Top-k の入力を作る。
pub fn build_topk_case(spec: &TopkCaseSpec) -> Result<TopkCase, GenError> {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let mut scores: Vec<f32> = match spec.input {
        TopkInput::Batch | TopkInput::NonFinite => square_values(&mut rng, spec.len),
        TopkInput::Ties => (0..spec.len)
            .map(|i| {
                if i % 10 == 3 {
                    0.0
                } else {
                    ((rng.next_u64() >> 61) as f32) + 1.0
                }
            })
            .collect(),
    };
    if spec.input == TopkInput::NonFinite {
        for (index, word) in NON_FINITE_SLOTS {
            *scores.get_mut(index).ok_or(GenError::InputFailed)? = f32::from_bits(word);
        }
    }
    Ok(TopkCase {
        name: spec.name,
        k: spec.k,
        scores,
    })
}

/// 部分 Top-k の 3 ケースの入力を作る。
pub fn build_topk_cases() -> Result<Vec<TopkCase>, GenError> {
    TOPK_CASES.iter().map(build_topk_case).collect()
}

/// -0.0 を含む部分 Top-k のケースの入力の出どころ。
#[derive(Debug, Clone, Copy)]
pub enum SignedZeroSource {
    /// core の `CaseId::E4` の入力。
    CaseE4,
    /// core の `additional_topk_inputs()` のうち、同じ名前の入力。
    Additional(&'static str),
}

/// -0.0 を含む部分 Top-k のケースの表の 1 行。
#[derive(Debug, Clone, Copy)]
pub struct SignedZeroCaseSpec {
    /// ファイル名の前半になるケース名（`[a-z0-9.-]` だけ）。
    pub name: &'static str,
    /// 入力の出どころ。
    pub source: SignedZeroSource,
}

/// 書き出す -0.0 を含む部分 Top-k の 5 ケース（CPU-13）。長さと k は出どころの入力から取る。
pub const SIGNED_ZERO_CASES: [SignedZeroCaseSpec; 5] = [
    SignedZeroCaseSpec {
        name: "topk-e-4",
        source: SignedZeroSource::CaseE4,
    },
    SignedZeroCaseSpec {
        name: "topk-signed-zero-adjacent",
        source: SignedZeroSource::Additional("signed_zero_adjacent"),
    },
    SignedZeroCaseSpec {
        name: "topk-signed-zero-separated",
        source: SignedZeroSource::Additional("signed_zero_separated"),
    },
    SignedZeroCaseSpec {
        name: "topk-signed-zero-cut-at-k",
        source: SignedZeroSource::Additional("signed_zero_cut_at_k"),
    },
    SignedZeroCaseSpec {
        name: "topk-signed-zero-repeated",
        source: SignedZeroSource::Additional("signed_zero_repeated"),
    },
];

/// 表の 1 行から、core の入力をそのまま使って部分 Top-k の入力を作る。
pub fn build_signed_zero_case(spec: &SignedZeroCaseSpec) -> Result<TopkCase, GenError> {
    match spec.source {
        SignedZeroSource::CaseE4 => match CaseId::E4.input() {
            Ok(CaseInput::TopK { scores, k }) => Ok(TopkCase {
                name: spec.name,
                k,
                scores,
            }),
            _ => Err(GenError::InputFailed),
        },
        SignedZeroSource::Additional(name) => {
            let input = additional_topk_inputs()
                .iter()
                .find(|i| i.name() == name)
                .ok_or(GenError::InputFailed)?;
            Ok(TopkCase {
                name: spec.name,
                k: input.k(),
                scores: input.scores().to_vec(),
            })
        }
    }
}

/// -0.0 を含む部分 Top-k の 5 ケースの入力を作る。
pub fn build_signed_zero_cases() -> Result<Vec<TopkCase>, GenError> {
    SIGNED_ZERO_CASES
        .iter()
        .map(build_signed_zero_case)
        .collect()
}

/// `DefaultCpuOps::topk_partial_scalar` の出力（長さ k の（スロット番号, スコア））を返す。
pub fn native_topk(case: &TopkCase) -> Result<Vec<(u32, f32)>, GenError> {
    let mut out = vec![(0_u32, 0.0_f32); case.k];
    DefaultCpuOps
        .topk_partial_scalar(&case.scores, case.k, &mut out)
        .map_err(|_| GenError::NativeTopkFailed)?;
    Ok(out)
}

/// 特殊値の GEMM のケースの表の 1 行。
#[derive(Debug, Clone, Copy)]
pub struct GemmSpecialCaseSpec {
    /// ファイル名の前半になるケース名（`[a-z0-9.-]` だけ）。
    pub name: &'static str,
    /// 入力と CPU 参照の出どころの core のケース。
    pub id: CaseId,
}

/// 書き出す特殊値の GEMM の 14 ケース（`CaseId::G01`〜`G13`・`E1`。spec PARITY-25 のポインタ）。
pub const GEMM_SPECIAL_CASES: [GemmSpecialCaseSpec; 14] = [
    GemmSpecialCaseSpec {
        name: "special-gemm-g-01",
        id: CaseId::G01,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-02",
        id: CaseId::G02,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-03",
        id: CaseId::G03,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-04",
        id: CaseId::G04,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-05",
        id: CaseId::G05,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-06",
        id: CaseId::G06,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-07",
        id: CaseId::G07,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-08",
        id: CaseId::G08,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-09",
        id: CaseId::G09,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-10",
        id: CaseId::G10,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-11",
        id: CaseId::G11,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-12",
        id: CaseId::G12,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-g-13",
        id: CaseId::G13,
    },
    GemmSpecialCaseSpec {
        name: "special-gemm-e-1",
        id: CaseId::E1,
    },
];

/// 表の 1 行から、core の入力をそのまま使って GEMM の入力を作る。GEMM の入力でなければ `InputFailed`。
pub fn build_gemm_special_case(spec: &GemmSpecialCaseSpec) -> Result<GemmCase, GenError> {
    match spec.id.input() {
        Ok(CaseInput::Gemm(g)) => Ok(GemmCase {
            name: spec.name,
            m: g.m(),
            k: g.k(),
            n: g.n(),
            a: g.a().to_vec(),
            b: g.b().to_vec(),
        }),
        _ => Err(GenError::InputFailed),
    }
}

/// 特殊値の GEMM の 14 ケースの入力を作る。
pub fn build_gemm_special_cases() -> Result<Vec<GemmCase>, GenError> {
    GEMM_SPECIAL_CASES
        .iter()
        .map(build_gemm_special_case)
        .collect()
}

/// core の CPU 参照（`CaseId::reference`）の f32 の出力（m×n）を返す。`Err` と f32 でない出力は `CpuReferenceFailed`。
pub fn gemm_special_cpu_reference(spec: &GemmSpecialCaseSpec) -> Result<Vec<f32>, GenError> {
    let out = spec
        .id
        .reference()
        .map_err(|_| GenError::CpuReferenceFailed)?;
    out.as_f32()
        .map(<[f32]>::to_vec)
        .ok_or(GenError::CpuReferenceFailed)
}

/// 要素ごと演算のケースの入力の件数。
pub const ELEMENTWISE_LEN: usize = 1_000_003;

/// 要素ごと演算のケースの表の 1 行。
#[derive(Debug, Clone, Copy)]
pub struct ElementwiseCaseSpec {
    /// ファイル名の前半になるケース名（`[a-z0-9.-]` だけ）。
    pub name: &'static str,
    /// 参照に渡す演算。
    pub op: UnaryElementwiseOp,
    /// glue の export に渡す演算の番号。
    pub op_code: u32,
    /// glue の export に渡す定数の f32 のビット列。
    pub c_bits: u32,
}

/// 書き出す要素ごと演算の 2 ケース（spec PARITY-23 のポインタ）。
pub const ELEMENTWISE_CASES: [ElementwiseCaseSpec; 2] = [
    ElementwiseCaseSpec {
        name: "elementwise-relu",
        op: UnaryElementwiseOp::Relu,
        op_code: 0,
        c_bits: 0x0000_0000,
    },
    ElementwiseCaseSpec {
        name: "elementwise-add-scalar",
        op: UnaryElementwiseOp::AddScalar(0.5),
        op_code: 1,
        c_bits: 0.5_f32.to_bits(),
    },
];

/// 要素ごと演算の 1 ケースの入力。
#[derive(Debug, Clone)]
pub struct ElementwiseCase {
    /// ケース名。
    pub name: &'static str,
    /// 参照に渡す演算。
    pub op: UnaryElementwiseOp,
    /// glue の export に渡す演算の番号。
    pub op_code: u32,
    /// glue の export に渡す定数の f32 のビット列。
    pub c_bits: u32,
    /// 入力の列（`ELEMENTWISE_LEN` 個）。
    pub input: Vec<f32>,
}

/// 表の 1 行から要素ごと演算の入力を作る。
///
/// ケースごとに新しい `SplitMix64::new(GEMM_INPUT_SEED)` の `next_signed_unit_f32` を順に取り、
/// `0.0` と等しい値は入れずに引き直して `ELEMENTWISE_LEN` 個にする。
pub fn build_elementwise_case(spec: &ElementwiseCaseSpec) -> ElementwiseCase {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let mut input = Vec::with_capacity(ELEMENTWISE_LEN);
    while input.len() < ELEMENTWISE_LEN {
        let x = rng.next_signed_unit_f32();
        if x != 0.0 {
            input.push(x);
        }
    }
    ElementwiseCase {
        name: spec.name,
        op: spec.op,
        op_code: spec.op_code,
        c_bits: spec.c_bits,
        input,
    }
}

/// 要素ごと演算の 2 ケースの入力を作る。
pub fn build_elementwise_cases() -> Result<Vec<ElementwiseCase>, GenError> {
    Ok(ELEMENTWISE_CASES
        .iter()
        .map(build_elementwise_case)
        .collect())
}

/// `DefaultCpuOps::elementwise_scalar` の出力（入力と同じ長さ）を返す。
pub fn native_elementwise(case: &ElementwiseCase) -> Result<Vec<f32>, GenError> {
    let mut out = vec![0.0_f32; case.input.len()];
    DefaultCpuOps
        .elementwise_scalar(case.op, &case.input, &mut out)
        .map_err(|_| GenError::NativeElementwiseFailed)?;
    Ok(out)
}

/// 部分 Top-k の出力を、1 件ごとにスロット番号の 4 バイト、スコアのビット列の 4 バイトの順の
/// リトルエンディアンのバイト列にする。
pub fn topk_record_bytes(records: &[(u32, f32)]) -> Vec<u8> {
    records
        .iter()
        .flat_map(|(slot, score)| {
            let mut record = [0u8; 8];
            record[..4].copy_from_slice(&slot.to_le_bytes());
            record[4..].copy_from_slice(&score.to_bits().to_le_bytes());
            record
        })
        .collect()
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

/// 1 ケースのファイル（GEMM は 3、特殊値の GEMM は 4、部分 Top-k と要素ごと演算は 2）と、manifest の 1 要素の文字列。
struct CaseOutput {
    files: Vec<OutFile>,
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
        files: vec![
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

fn gemm_special_case_output(spec: &GemmSpecialCaseSpec) -> Result<CaseOutput, GenError> {
    let case = build_gemm_special_case(spec)?;
    let native = native_scalar(&case)?;
    let reference = gemm_special_cpu_reference(spec)?;
    let a_file = format!("{}.a.bin", case.name);
    let b_file = format!("{}.b.bin", case.name);
    let n_file = format!("{}.native-scalar.bin", case.name);
    let r_file = format!("{}.cpu-reference.bin", case.name);
    let a_bytes = f32_le_bytes(&case.a);
    let b_bytes = f32_le_bytes(&case.b);
    let n_bytes = f32_le_bytes(&native);
    let r_bytes = f32_le_bytes(&reference);
    let entry = [
        "    {".to_string(),
        format!("      \"name\": \"{}\",", case.name),
        "      \"kind\": \"gemm-special\",".to_string(),
        format!("      \"m\": {},", case.m),
        format!("      \"k\": {},", case.k),
        format!("      \"n\": {},", case.n),
        format!("      \"a_file\": \"{a_file}\","),
        format!("      \"b_file\": \"{b_file}\","),
        format!("      \"native_scalar_file\": \"{n_file}\","),
        format!("      \"cpu_reference_file\": \"{r_file}\","),
        format!("      \"a_sha256\": \"{}\",", hex(&sha256_bytes(&a_bytes))),
        format!("      \"b_sha256\": \"{}\",", hex(&sha256_bytes(&b_bytes))),
        format!(
            "      \"native_scalar_sha256\": \"{}\",",
            hex(&sha256_bytes(&n_bytes))
        ),
        format!(
            "      \"cpu_reference_sha256\": \"{}\"",
            hex(&sha256_bytes(&r_bytes))
        ),
        "    }".to_string(),
    ]
    .join("\n");
    Ok(CaseOutput {
        files: vec![
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
            OutFile {
                name: r_file,
                bytes: r_bytes,
            },
        ],
        manifest_entry: entry,
    })
}

fn topk_case_output(case: &TopkCase) -> Result<CaseOutput, GenError> {
    let native = native_topk(case)?;
    let scores_file = format!("{}.scores.bin", case.name);
    let n_file = format!("{}.native-scalar.bin", case.name);
    let scores_bytes = f32_le_bytes(&case.scores);
    let n_bytes = topk_record_bytes(&native);
    let entry = [
        "    {".to_string(),
        format!("      \"name\": \"{}\",", case.name),
        "      \"kind\": \"topk\",".to_string(),
        format!("      \"n\": {},", case.scores.len()),
        format!("      \"k\": {},", case.k),
        format!("      \"scores_file\": \"{scores_file}\","),
        format!("      \"native_scalar_file\": \"{n_file}\","),
        format!(
            "      \"scores_sha256\": \"{}\",",
            hex(&sha256_bytes(&scores_bytes))
        ),
        format!(
            "      \"native_scalar_sha256\": \"{}\"",
            hex(&sha256_bytes(&n_bytes))
        ),
        "    }".to_string(),
    ]
    .join("\n");
    Ok(CaseOutput {
        files: vec![
            OutFile {
                name: scores_file,
                bytes: scores_bytes,
            },
            OutFile {
                name: n_file,
                bytes: n_bytes,
            },
        ],
        manifest_entry: entry,
    })
}

fn elementwise_case_output(case: &ElementwiseCase) -> Result<CaseOutput, GenError> {
    let native = native_elementwise(case)?;
    let input_file = format!("{}.input.bin", case.name);
    let n_file = format!("{}.native-scalar.bin", case.name);
    let input_bytes = f32_le_bytes(&case.input);
    let n_bytes = f32_le_bytes(&native);
    let entry = [
        "    {".to_string(),
        format!("      \"name\": \"{}\",", case.name),
        "      \"kind\": \"elementwise\",".to_string(),
        format!("      \"n\": {},", case.input.len()),
        format!("      \"op_code\": {},", case.op_code),
        format!("      \"c_bits\": \"{:08x}\",", case.c_bits),
        format!("      \"input_file\": \"{input_file}\","),
        format!("      \"native_scalar_file\": \"{n_file}\","),
        format!(
            "      \"input_sha256\": \"{}\",",
            hex(&sha256_bytes(&input_bytes))
        ),
        format!(
            "      \"native_scalar_sha256\": \"{}\"",
            hex(&sha256_bytes(&n_bytes))
        ),
        "    }".to_string(),
    ]
    .join("\n");
    Ok(CaseOutput {
        files: vec![
            OutFile {
                name: input_file,
                bytes: input_bytes,
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

/// `dir` に GEMM と部分 Top-k の 3 ケースずつ、要素ごと演算の 2 ケース、-0.0 を含む部分 Top-k の 5 ケース、特殊値の GEMM の 14 ケースの入力・参照・`manifest.json` を書く。
///
/// 入力と参照を全部作ってから、書く名前の不在を確かめ、`.bin`、最後に `manifest.json` の順に書く。
pub fn generate(dir: &Path) -> Result<(), GenError> {
    if !fs::metadata(dir).map(|m| m.is_dir()).unwrap_or(false) {
        return Err(GenError::MissingDirectory);
    }
    let mut outputs = build_cases()?
        .iter()
        .map(case_output)
        .collect::<Result<Vec<_>, _>>()?;
    for case in &build_topk_cases()? {
        outputs.push(topk_case_output(case)?);
    }
    for case in &build_elementwise_cases()? {
        outputs.push(elementwise_case_output(case)?);
    }
    for case in &build_signed_zero_cases()? {
        outputs.push(topk_case_output(case)?);
    }
    for spec in &GEMM_SPECIAL_CASES {
        outputs.push(gemm_special_case_output(spec)?);
    }

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
