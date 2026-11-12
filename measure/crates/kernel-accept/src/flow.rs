//! 特殊化する GEMM・特殊化しない GEMM・加算・Relu の 4 つの役割の、device に触れない部品（TASK-485 のポインタ）。
//!
//! 入口名の受け取りとソースの照合（[`resolve_entry`]・[`load_kernel`]・[`FlowKernels::load`]）、
//! 役割ごとの起動の形（[`DispatchShape`]）、入力と CPU の参照を置く。device を作る前に済ませる手順はここで閉じ、
//! `run` の device の口は [`FlowKernels`] だけを受ける。エラーは固定の文言で、値・パス・入口名・ソースを含めない。
//! 呼び出し元は `run` と、後続の段の device の試験。

use std::ffi::OsString;
use std::fmt;
use std::path::Path;

use fandhe_silicon_core::execution::{FunctionConstant, FunctionConstantValue};
use fandhe_silicon_core::parity::cpu_reference::{ReferenceError, reference_gemm_f32};
use fandhe_silicon_core::parity::gemm_inputs::{
    GemmGateRow, GemmInputDtype, GemmInputsF32, SplitMix64,
};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_metal_gemm_arm::launch::{ConstantValue, Launch, launch_for};
use measure_metal_gemm_arm::source::load_source_against;

use crate::accept::source_file;
use crate::manifest::{Manifest, ManifestEntry, Mode, is_entry_name};

/// 一覧のファイルの絶対パスを受ける環境変数の名前。
pub const MANIFEST_ENV: &str = "FANDHE_SILICON_KERNEL_MANIFEST";
/// 取得したソースを置いたディレクトリを受ける環境変数の名前。
pub const DIR_ENV: &str = "FANDHE_SILICON_KERNEL_DIR";
/// 特殊化する GEMM の入口名を受ける環境変数の名前。
pub const SPECIALIZED_GEMM_ENV: &str = "FANDHE_SILICON_GEMM_SPECIALIZED_ENTRY";
/// 特殊化しない GEMM の入口名を受ける環境変数の名前。
pub const PLAIN_GEMM_ENV: &str = "FANDHE_SILICON_GEMM_PLAIN_ENTRY";
/// 加算の入口名を受ける環境変数の名前。
pub const ADD_ENV: &str = "FANDHE_SILICON_ADD_ENTRY";
/// Relu の入口名を受ける環境変数の名前。
pub const RELU_ENV: &str = "FANDHE_SILICON_RELU_ENTRY";

/// 加算の第 2 項を作る擬似乱数の seed。
pub const ADDEND_SEED: u64 = 0x5EED_0A11_0000_0001;

/// このモジュールが扱う形状。
pub const FLOW_ROWS: [GemmGateRow; 3] = [
    GemmGateRow::Sq512,
    GemmGateRow::Sq2048,
    GemmGateRow::Odd37x41x53,
];

/// カーネルの役割。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// function constant で特殊化する GEMM。
    SpecializedGemm,
    /// 特殊化しない GEMM。
    PlainGemm,
    /// 加算。
    Add,
    /// Relu。
    Relu,
}

impl Role {
    /// 入口名を受ける環境変数の名前。
    pub const fn env_name(self) -> &'static str {
        match self {
            Role::SpecializedGemm => SPECIALIZED_GEMM_ENV,
            Role::PlainGemm => PLAIN_GEMM_ENV,
            Role::Add => ADD_ENV,
            Role::Relu => RELU_ENV,
        }
    }

    /// 一覧の行がこの役割で持つべき `mode`。
    pub const fn mode(self) -> Mode {
        match self {
            Role::SpecializedGemm => Mode::Specialized,
            _ => Mode::Msl,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Role::SpecializedGemm => "specialized gemm",
            Role::PlainGemm => "plain gemm",
            Role::Add => "add",
            Role::Relu => "relu",
        }
    }
}

/// 形状に使う GEMM の役割。扱わない形状は `None`。
pub fn gemm_role(row: GemmGateRow) -> Option<Role> {
    match row {
        GemmGateRow::Sq512 | GemmGateRow::Sq2048 => Some(Role::SpecializedGemm),
        GemmGateRow::Odd37x41x53 => Some(Role::PlainGemm),
        _ => None,
    }
}

/// この段の部品の失敗。文言は固定で、値・パス・入口名・ソースを含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowError {
    /// 役割の入口名が与えられていない。
    EntryMissing(Role),
    /// 役割の入口名の形が不正。
    EntryMalformed(Role),
    /// 役割の入口名が一覧に無い。
    EntryNotListed(Role),
    /// 役割の入口名が一覧に 2 回以上ある。
    EntryListedTwice(Role),
    /// 一覧の行の `mode` が役割と合わない。
    ModeMismatch(Role),
    /// 役割のソースを読めない・照合に通らない。
    SourceUnreadable(Role),
    /// 扱わない形状。
    UnsupportedRow,
    /// 大きさの計算か変換が収まらない。
    SizeOverflow,
    /// 列の長さが合わない。
    LengthMismatch,
    /// 領域を確保できない。
    AllocationFailed,
    /// カーネルの役割が段・行の要る役割と合わない。
    RoleMismatch(Role),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::EntryMissing(r) => write!(f, "{}: entry name is not given", r.label()),
            FlowError::EntryMalformed(r) => write!(f, "{}: entry name is malformed", r.label()),
            FlowError::EntryNotListed(r) => {
                write!(f, "{}: entry is not in the manifest", r.label())
            }
            FlowError::EntryListedTwice(r) => {
                write!(f, "{}: entry is listed more than once", r.label())
            }
            FlowError::ModeMismatch(r) => write!(f, "{}: manifest mode does not match", r.label()),
            FlowError::SourceUnreadable(r) => {
                write!(f, "{}: source cannot be read or verified", r.label())
            }
            FlowError::UnsupportedRow => f.write_str("unsupported shape"),
            FlowError::SizeOverflow => f.write_str("size overflow"),
            FlowError::LengthMismatch => f.write_str("length mismatch"),
            FlowError::AllocationFailed => f.write_str("allocation failed"),
            FlowError::RoleMismatch(r) => write!(f, "{}: kernel role does not match", r.label()),
        }
    }
}

impl std::error::Error for FlowError {}

/// 役割の入口名を受け取り、一覧のちょうど 1 行に結び付け、`mode` を照らす。
///
/// `lookup` は環境変数の読み出しの差し替え口。device には触れない。
pub fn resolve_entry<'m>(
    m: &'m Manifest,
    role: Role,
    lookup: &dyn Fn(&str) -> Option<OsString>,
) -> Result<&'m ManifestEntry, FlowError> {
    let raw = lookup(role.env_name())
        .filter(|v| !v.is_empty())
        .ok_or(FlowError::EntryMissing(role))?;
    let name = raw.to_str().ok_or(FlowError::EntryMalformed(role))?;
    if !is_entry_name(name) {
        return Err(FlowError::EntryMalformed(role));
    }
    let mut hits = m.entries().iter().filter(|e| e.entry() == name);
    let row = hits.next().ok_or(FlowError::EntryNotListed(role))?;
    if hits.next().is_some() {
        return Err(FlowError::EntryListedTwice(role));
    }
    if row.mode() != role.mode() {
        return Err(FlowError::ModeMismatch(role));
    }
    Ok(row)
}

/// 照合を通ったカーネル 1 つ。入口名とソース。
#[derive(Clone, PartialEq, Eq)]
pub struct FlowKernel {
    role: Role,
    source: String,
    entry: String,
}

impl fmt::Debug for FlowKernel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowKernel")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl FlowKernel {
    /// 役割。
    pub fn role(&self) -> Role {
        self.role
    }
    /// 照合済みのソース。
    pub fn source(&self) -> &str {
        &self.source
    }
    /// 入口名。
    pub fn entry(&self) -> &str {
        &self.entry
    }
}

/// 入口を受け取り、その行のパスと sha256 でソースを読む。
pub fn load_kernel(
    m: &Manifest,
    dir: &Path,
    role: Role,
    lookup: &dyn Fn(&str) -> Option<OsString>,
) -> Result<FlowKernel, FlowError> {
    let row = resolve_entry(m, role, lookup)?;
    let source = load_source_against(&source_file(dir, row), row.sha256())
        .map_err(|_| FlowError::SourceUnreadable(role))?;
    Ok(FlowKernel {
        role,
        source,
        entry: row.entry().to_owned(),
    })
}

/// 1 つの形状で GEMM → 加算 → Relu を流すのに要るカーネルの組。device を作る前に全部そろえる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowKernels {
    /// GEMM の段のカーネル。
    pub gemm: FlowKernel,
    /// 加算の段のカーネル。
    pub add: FlowKernel,
    /// Relu の段のカーネル。
    pub relu: FlowKernel,
}

impl FlowKernels {
    /// 形状の GEMM の役割と加算・Relu の入口を受け取り、ソースまで照合する。
    pub fn load(
        m: &Manifest,
        dir: &Path,
        row: GemmGateRow,
        lookup: &dyn Fn(&str) -> Option<OsString>,
    ) -> Result<Self, FlowError> {
        let role = gemm_role(row).ok_or(FlowError::UnsupportedRow)?;
        Ok(Self {
            gemm: load_kernel(m, dir, role, lookup)?,
            add: load_kernel(m, dir, Role::Add, lookup)?,
            relu: load_kernel(m, dir, Role::Relu, lookup)?,
        })
    }
}

/// 1 回の起動の形。
#[derive(Debug, Clone)]
pub struct DispatchShape {
    /// threadgroup あたりのスレッド数。
    pub threads: [u32; 3],
    /// threadgroup の数。
    pub groups: [u32; 3],
    /// 束縛と定数の slot の並び。
    pub layout: Vec<u32>,
    /// 定数の slot とネイティブエンディアンのバイト列。
    pub constants: Vec<(u32, Vec<u8>)>,
    /// threadgroup のメモリの index とバイト数。
    pub threadgroup_memory: Option<(u32, u32)>,
    /// 特殊化する GEMM だけが持つ function constant。
    pub function_constants: Vec<FunctionConstant>,
}

/// `u32` の列をネイティブエンディアンのバイト列にする。
pub fn u32_ne_bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// `f32` の列をネイティブエンディアンのバイト列にする。
pub fn f32_ne_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// ネイティブエンディアンのバイト列を `f32` の列に戻す。長さが 4 の倍数でなければ `Err`。
pub fn f32_from_ne_bytes(bytes: &[u8]) -> Result<Vec<f32>, FlowError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(FlowError::LengthMismatch);
    }
    let (chunks, _) = bytes.as_chunks::<4>();
    Ok(chunks.iter().map(|c| f32::from_ne_bytes(*c)).collect())
}

/// `Launch` の function constant を契約の型に写す。
pub fn launch_function_constants(launch: &Launch) -> Vec<FunctionConstant> {
    launch
        .constants
        .iter()
        .map(|&(index, v)| FunctionConstant {
            index,
            value: match v {
                ConstantValue::UInt(u) => FunctionConstantValue::U32(u),
                ConstantValue::Bool(b) => FunctionConstantValue::Bool(b),
            },
        })
        .collect()
}

fn to_u32(x: usize) -> Result<u32, FlowError> {
    u32::try_from(x).map_err(|_| FlowError::SizeOverflow)
}

/// 特殊化する GEMM の起動の形。数値は `launch_for` の `Launch` をそのまま使う。
pub fn specialized_gemm_shape(row: GemmGateRow) -> Result<DispatchShape, FlowError> {
    let cell = gate_cells()
        .into_iter()
        .find(|c| c.arm() == GemmGateArm::G6 && c.row() == row && c.dtype() == GemmInputDtype::F32)
        .ok_or(FlowError::UnsupportedRow)?;
    let launch = launch_for(cell).ok_or(FlowError::UnsupportedRow)?;
    let (m, n, k) = (to_u32(row.m())?, to_u32(row.n())?, to_u32(row.k())?);
    let gh = to_u32(launch.groups_height)?;
    let gw = to_u32(launch.groups_width)?;
    Ok(DispatchShape {
        threads: [to_u32(launch.threads)?, 1, 1],
        groups: [gw, gh, 1],
        layout: (0..=6).collect(),
        constants: vec![
            (3, u32_ne_bytes(&[m, n, k])),
            (4, u32_ne_bytes(&[k, n, 0, 0])),
            (5, u32_ne_bytes(&[0, 0, gh, gw])),
            (6, u32_ne_bytes(&[1, k, 0, 0])),
        ],
        threadgroup_memory: Some((0, to_u32(launch.threadgroup_memory)?)),
        function_constants: launch_function_constants(&launch),
    })
}

/// 特殊化しない GEMM の起動の形。
pub fn plain_gemm_shape(row: GemmGateRow) -> Result<DispatchShape, FlowError> {
    let (m, n, k) = (to_u32(row.m())?, to_u32(row.n())?, to_u32(row.k())?);
    Ok(DispatchShape {
        threads: [16, 16, 1],
        groups: [n.div_ceil(16), m.div_ceil(16), 1],
        layout: vec![0, 1, 2, 3],
        constants: vec![(3, u32_ne_bytes(&[m, n, k]))],
        threadgroup_memory: None,
        function_constants: Vec::new(),
    })
}

/// 形状の GEMM の起動の形。扱わない形状は `Err`。
pub fn gemm_shape(row: GemmGateRow) -> Result<DispatchShape, FlowError> {
    match gemm_role(row) {
        Some(Role::SpecializedGemm) => specialized_gemm_shape(row),
        Some(_) => plain_gemm_shape(row),
        None => Err(FlowError::UnsupportedRow),
    }
}

fn elementwise_groups(len: u32) -> Result<[u32; 3], FlowError> {
    if len == 0 {
        return Err(FlowError::LengthMismatch);
    }
    Ok([len.div_ceil(256), 1, 1])
}

/// 加算の起動の形。`len` は要素数（0 は `Err`）。
pub fn add_shape(len: usize) -> Result<DispatchShape, FlowError> {
    let len = to_u32(len)?;
    Ok(DispatchShape {
        threads: [256, 1, 1],
        groups: elementwise_groups(len)?,
        layout: vec![0, 1, 2, 3],
        constants: vec![(3, u32_ne_bytes(&[len]))],
        threadgroup_memory: None,
        function_constants: Vec::new(),
    })
}

/// Relu の起動の形。`len` は要素数（0 は `Err`）。
pub fn relu_shape(len: usize) -> Result<DispatchShape, FlowError> {
    let len = to_u32(len)?;
    Ok(DispatchShape {
        threads: [256, 1, 1],
        groups: elementwise_groups(len)?,
        layout: vec![0, 1, 2],
        constants: vec![(2, u32_ne_bytes(&[len]))],
        threadgroup_memory: None,
        function_constants: Vec::new(),
    })
}

/// 加算の第 2 項（M×N）。固定の seed の擬似乱数で作る。
pub fn addend(row: GemmGateRow) -> Result<Vec<f32>, FlowError> {
    let len = row
        .m()
        .checked_mul(row.n())
        .ok_or(FlowError::SizeOverflow)?;
    let mut v = Vec::new();
    v.try_reserve_exact(len)
        .map_err(|_| FlowError::AllocationFailed)?;
    let mut rng = SplitMix64::new(ADDEND_SEED);
    for _ in 0..len {
        v.push(rng.next_signed_unit_f32());
    }
    Ok(v)
}

/// 加算の CPU の参照。GEMM の出力の各要素に第 2 項を足す。
pub fn reference_add(gemm: &[f32], addend: &[f32]) -> Result<Vec<f32>, FlowError> {
    if gemm.len() != addend.len() {
        return Err(FlowError::LengthMismatch);
    }
    Ok(gemm.iter().zip(addend).map(|(g, a)| g + a).collect())
}

/// Relu の CPU の参照。`x > 0` ならそのまま、そうでなければ `+0.0`（`-0.0` と NaN も `+0.0`）。
pub fn reference_relu(x: &[f32]) -> Vec<f32> {
    x.iter().map(|&v| if v > 0.0 { v } else { 0.0 }).collect()
}

/// 3 段の CPU の参照。
#[derive(Debug, Clone, PartialEq)]
pub struct FlowReference {
    /// GEMM の出力。
    pub gemm: Vec<f32>,
    /// 加算の出力。
    pub add: Vec<f32>,
    /// Relu の出力。
    pub relu: Vec<f32>,
}

/// カーネルの役割が、段の要る役割と合うか確かめる（device に触る前の検査）。
pub fn check_role(kernel: &FlowKernel, expected: Role) -> Result<(), FlowError> {
    if kernel.role() == expected {
        Ok(())
    } else {
        Err(FlowError::RoleMismatch(expected))
    }
}

/// GEMM のカーネルの役割が行の要る役割と合うか確かめる。扱わない行は `UnsupportedRow`。
pub fn check_gemm_role(row: GemmGateRow, kernel: &FlowKernel) -> Result<(), FlowError> {
    let expected = gemm_role(row).ok_or(FlowError::UnsupportedRow)?;
    check_role(kernel, expected)
}

/// GEMM の入力の長さが行の `M×K`・`K×N` と合うか確かめる（device に触る前の検査）。
pub fn check_gemm_input_lengths(row: GemmGateRow, inputs: &GemmInputsF32) -> Result<(), FlowError> {
    let a_len = row
        .m()
        .checked_mul(row.k())
        .ok_or(FlowError::SizeOverflow)?;
    let b_len = row
        .k()
        .checked_mul(row.n())
        .ok_or(FlowError::SizeOverflow)?;
    if inputs.a().len() != a_len || inputs.b().len() != b_len {
        return Err(FlowError::LengthMismatch);
    }
    Ok(())
}

/// 3 段の CPU の参照を作る。
pub fn reference(
    row: GemmGateRow,
    inputs: &GemmInputsF32,
    addend: &[f32],
) -> Result<FlowReference, FlowError> {
    let gemm = reference_gemm_f32(inputs.a(), inputs.b(), row.m(), row.k(), row.n()).map_err(
        |e| match e {
            ReferenceError::AllocationFailed => FlowError::AllocationFailed,
            ReferenceError::SizeOverflow => FlowError::SizeOverflow,
            _ => FlowError::LengthMismatch,
        },
    )?;
    let add = reference_add(&gemm, addend)?;
    let relu = reference_relu(&add);
    Ok(FlowReference { gemm, add, relu })
}
