//! GEMM と GEMM→加算→Relu の device の要る試験と、その純粋な部品（TASK-485 のポインタ）。
//!
//! 契約の crate の単体試験のビルドの中にだけ置く。Metal の背後実装の投入の門は単体試験のビルドで開くので、
//! 投入を通す試験はこの置き場で流す。純粋な部分（一覧の 1 行の引き当て・入口の受け取り・ソースの照合・
//! 起動の形・入力と CPU の参照）はどの OS でも流れる。device の部分は macOS で `native-blocking-wait` を
//! 有効にしたビルドの `device` module にあり、`#[ignore]` 付きで専用の実機の回でだけ流す。
//! 実行時に受けるソースは device を作る前に一覧の sha256 で照合し、エラーは固定の英語の文言で、
//! 値・パス・入口名・ソースを含めない。

use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::Path;

use fandhe_silicon_core::execution::{FunctionConstant, FunctionConstantValue};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::cpu_reference::{ReferenceError, reference_gemm_f32};
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputsF32, SplitMix64};

/// 一覧のファイルの絶対パスを受ける環境変数の名前。
const MANIFEST_ENV: &str = "FANDHE_SILICON_KERNEL_MANIFEST";
/// 取得したソースを置いたディレクトリを受ける環境変数の名前。
const DIR_ENV: &str = "FANDHE_SILICON_KERNEL_DIR";
/// 特殊化する GEMM の入口名を受ける環境変数の名前。
const SPECIALIZED_GEMM_ENV: &str = "FANDHE_SILICON_GEMM_SPECIALIZED_ENTRY";
/// 特殊化しない GEMM の入口名を受ける環境変数の名前。
const PLAIN_GEMM_ENV: &str = "FANDHE_SILICON_GEMM_PLAIN_ENTRY";
/// 加算の入口名を受ける環境変数の名前。
const ADD_ENV: &str = "FANDHE_SILICON_ADD_ENTRY";
/// Relu の入口名を受ける環境変数の名前。
const RELU_ENV: &str = "FANDHE_SILICON_RELU_ENTRY";

/// 加算の第 2 項を作る擬似乱数の seed。
const ADDEND_SEED: u64 = 0x5EED_0A11_0000_0001;

/// 一覧のファイルの大きさの上限（バイト）。
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// ソースのファイルの大きさの上限（バイト）。
const MAX_SOURCE_BYTES: u64 = 1024 * 1024;
/// 入口名の長さの上限（バイト）。
const MAX_ENTRY_BYTES: usize = 64;

/// この module が扱う形状。
const FLOW_ROWS: [GemmGateRow; 3] = [
    GemmGateRow::Sq512,
    GemmGateRow::Sq2048,
    GemmGateRow::Odd37x41x53,
];

/// 環境変数の読み出しの差し替え口の型。device の試験は `std::env::var_os` を渡す。
type Lookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// カーネルの役割。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// function constant で特殊化する GEMM。
    SpecializedGemm,
    /// 特殊化しない GEMM。
    PlainGemm,
    /// 加算。
    Add,
    /// Relu。
    Relu,
}

/// 一覧の行の `mode`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Msl,
    Specialized,
}

impl Role {
    /// 入口名を受ける環境変数の名前。
    const fn env_name(self) -> &'static str {
        match self {
            Role::SpecializedGemm => SPECIALIZED_GEMM_ENV,
            Role::PlainGemm => PLAIN_GEMM_ENV,
            Role::Add => ADD_ENV,
            Role::Relu => RELU_ENV,
        }
    }

    /// 一覧の行がこの役割で持つべき `mode`。
    const fn mode(self) -> Mode {
        match self {
            Role::SpecializedGemm => Mode::Specialized,
            _ => Mode::Msl,
        }
    }

    /// エラーの文言に使う固定の名前。
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
fn gemm_role(row: GemmGateRow) -> Option<Role> {
    match row {
        GemmGateRow::Sq512 | GemmGateRow::Sq2048 => Some(Role::SpecializedGemm),
        GemmGateRow::Odd37x41x53 => Some(Role::PlainGemm),
        _ => None,
    }
}

/// この module の部品の失敗。文言は固定で、値・パス・入口名・ソースを含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowError {
    /// 一覧のパスが与えられていない。
    ManifestPathMissing,
    /// ソースのディレクトリが与えられていない。
    DirMissing,
    /// 一覧を読めない。
    ManifestUnreadable,
    /// 一覧が大きさの上限を超える。
    ManifestTooLarge,
    /// 一覧が UTF-8 でない。
    ManifestNotUtf8,
    /// 一覧の行の形が不正（行の番号つき）。
    ManifestMalformed(usize),
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
            FlowError::ManifestPathMissing => f.write_str("manifest path is not given"),
            FlowError::DirMissing => f.write_str("source directory is not given"),
            FlowError::ManifestUnreadable => f.write_str("manifest cannot be read"),
            FlowError::ManifestTooLarge => f.write_str("manifest is too large"),
            FlowError::ManifestNotUtf8 => f.write_str("manifest is not valid UTF-8"),
            FlowError::ManifestMalformed(n) => write!(f, "manifest line {n}: malformed"),
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

/// 一覧の 1 行から使う欄。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestLine {
    file_name: String,
    sha256: [u8; 32],
    mode: Mode,
    entry: String,
}

/// 入口名の形（`[A-Za-z_][A-Za-z0-9_]{0,63}`）か。
fn is_entry_name(s: &str) -> bool {
    let mut it = s.bytes();
    s.len() <= MAX_ENTRY_BYTES
        && matches!(it.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && it.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// 64 桁の小文字の 16 進を 32 バイトに解く。
fn parse_sha256_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in s.as_bytes().chunks(2).enumerate() {
        let hi = hex_digit(pair[0])?;
        let lo = hex_digit(pair[1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// パスの最後の要素。空・`.`・`..`・`\` を含むパスは `None`。
fn path_file_name(path: &str) -> Option<&str> {
    let last = path.rsplit('/').next()?;
    if last.is_empty() || last == "." || last == ".." || path.contains('\\') {
        return None;
    }
    Some(last)
}

/// 一覧のテキストを行に分け、各行の 5 欄から使う欄を取り出す。
///
/// 改行は LF で、最後の LF は付けても付けなくてもよい。欄は半角の空白 1 つで区切る。
fn parse_manifest_lines(text: &str) -> Result<Vec<ManifestLine>, FlowError> {
    let body = text.strip_suffix('\n').unwrap_or(text);
    if body.is_empty() {
        return Err(FlowError::ManifestMalformed(0));
    }
    let mut out = Vec::new();
    for (i, raw) in body.split('\n').enumerate() {
        let n = i + 1;
        let bad = || FlowError::ManifestMalformed(n);
        if raw.is_empty() || raw.chars().any(char::is_control) {
            return Err(bad());
        }
        let fields: Vec<&str> = raw.split(' ').collect();
        let [_rev, path, sha, mode, entry] = fields[..] else {
            return Err(bad());
        };
        let file_name = path_file_name(path).ok_or_else(bad)?;
        let sha256 = parse_sha256_hex(sha).ok_or_else(bad)?;
        let mode = match mode {
            "msl" => Mode::Msl,
            "specialized" => Mode::Specialized,
            _ => return Err(bad()),
        };
        out.push(ManifestLine {
            file_name: file_name.to_owned(),
            sha256,
            mode,
            entry: entry.to_owned(),
        });
    }
    Ok(out)
}

/// 通常のファイルを、大きさの上限を読む前に照らしてから読む。
fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ReadFailure> {
    let meta = std::fs::metadata(path).map_err(|_| ReadFailure::Unreadable)?;
    if !meta.is_file() {
        return Err(ReadFailure::Unreadable);
    }
    if meta.len() > limit {
        return Err(ReadFailure::TooLarge);
    }
    let file = std::fs::File::open(path).map_err(|_| ReadFailure::Unreadable)?;
    let mut buf = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ReadFailure::Unreadable)?;
    if buf.len() as u64 > limit {
        return Err(ReadFailure::TooLarge);
    }
    Ok(buf)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadFailure {
    Unreadable,
    TooLarge,
}

/// 一覧のファイルを読み、行に分ける。
fn read_manifest(path: &Path) -> Result<Vec<ManifestLine>, FlowError> {
    let bytes = read_limited(path, MAX_MANIFEST_BYTES).map_err(|e| match e {
        ReadFailure::Unreadable => FlowError::ManifestUnreadable,
        ReadFailure::TooLarge => FlowError::ManifestTooLarge,
    })?;
    let text = String::from_utf8(bytes).map_err(|_| FlowError::ManifestNotUtf8)?;
    parse_manifest_lines(&text)
}

/// ソースの照合の失敗。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceError {
    /// 大きさが上限を超える。
    TooLarge,
    /// sha256 が一覧の値と合わない。
    DigestMismatch,
    /// UTF-8 でない。
    NotUtf8,
}

/// ソースのバイト列を、長さ → sha256 → UTF-8 の順に照らして文字列にする。device を作る前に済ませる。
fn verify_source(bytes: &[u8], expected: &[u8; 32]) -> Result<String, SourceError> {
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(SourceError::TooLarge);
    }
    if &sha256_bytes(bytes) != expected {
        return Err(SourceError::DigestMismatch);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| SourceError::NotUtf8)
}

/// 役割の入口名を受け取り、一覧のちょうど 1 行に結び付け、`mode` を照らす。
fn resolve_entry<'m>(
    lines: &'m [ManifestLine],
    role: Role,
    lookup: Lookup<'_>,
) -> Result<&'m ManifestLine, FlowError> {
    let raw = lookup(role.env_name())
        .filter(|v| !v.is_empty())
        .ok_or(FlowError::EntryMissing(role))?;
    let name = raw.to_str().ok_or(FlowError::EntryMalformed(role))?;
    if !is_entry_name(name) {
        return Err(FlowError::EntryMalformed(role));
    }
    let mut hits = lines.iter().filter(|l| l.entry == name);
    let row = hits.next().ok_or(FlowError::EntryNotListed(role))?;
    if hits.next().is_some() {
        return Err(FlowError::EntryListedTwice(role));
    }
    if row.mode != role.mode() {
        return Err(FlowError::ModeMismatch(role));
    }
    Ok(row)
}

/// 照合を通ったカーネル 1 つ。入口名とソース。
#[derive(Clone, PartialEq, Eq)]
struct FlowKernel {
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

/// 入口を受け取り、その行のファイル名と sha256 でソースを読んで照合する。
fn load_kernel(
    lines: &[ManifestLine],
    dir: &Path,
    role: Role,
    lookup: Lookup<'_>,
) -> Result<FlowKernel, FlowError> {
    let row = resolve_entry(lines, role, lookup)?;
    let bytes = read_limited(&dir.join(&row.file_name), MAX_SOURCE_BYTES)
        .map_err(|_| FlowError::SourceUnreadable(role))?;
    let source =
        verify_source(&bytes, &row.sha256).map_err(|_| FlowError::SourceUnreadable(role))?;
    Ok(FlowKernel {
        role,
        source,
        entry: row.entry.clone(),
    })
}

/// 1 つの形状で GEMM → 加算 → Relu を流すのに要るカーネルの組。device を作る前に全部そろえる。
#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowKernels {
    gemm: FlowKernel,
    add: FlowKernel,
    relu: FlowKernel,
}

impl FlowKernels {
    /// 形状の GEMM の役割と加算・Relu の入口を受け取り、ソースまで照合する。
    fn load(
        lines: &[ManifestLine],
        dir: &Path,
        row: GemmGateRow,
        lookup: Lookup<'_>,
    ) -> Result<Self, FlowError> {
        let role = gemm_role(row).ok_or(FlowError::UnsupportedRow)?;
        Ok(Self {
            gemm: load_kernel(lines, dir, role, lookup)?,
            add: load_kernel(lines, dir, Role::Add, lookup)?,
            relu: load_kernel(lines, dir, Role::Relu, lookup)?,
        })
    }

    /// 一覧とディレクトリも差し替え口から受けて、全部そろえる。
    fn from_lookup(row: GemmGateRow, lookup: Lookup<'_>) -> Result<Self, FlowError> {
        let manifest = lookup(MANIFEST_ENV)
            .filter(|v| !v.is_empty())
            .ok_or(FlowError::ManifestPathMissing)?;
        let dir = lookup(DIR_ENV)
            .filter(|v| !v.is_empty())
            .ok_or(FlowError::DirMissing)?;
        let lines = read_manifest(Path::new(&manifest))?;
        Self::load(&lines, Path::new(&dir), row, lookup)
    }
}

/// 1 回の起動の形。
#[derive(Debug, Clone)]
struct DispatchShape {
    /// threadgroup あたりのスレッド数。
    threads: [u32; 3],
    /// threadgroup の数。
    groups: [u32; 3],
    /// 束縛と定数の slot の並び。
    layout: Vec<u32>,
    /// 定数の slot とネイティブエンディアンのバイト列。
    constants: Vec<(u32, Vec<u8>)>,
    /// threadgroup のメモリの index とバイト数。
    threadgroup_memory: Option<(u32, u32)>,
    /// 特殊化する GEMM だけが持つ function constant。
    function_constants: Vec<FunctionConstant>,
}

/// `u32` の列をネイティブエンディアンのバイト列にする。
fn u32_ne_bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// `f32` の列をネイティブエンディアンのバイト列にする。
fn f32_ne_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// ネイティブエンディアンのバイト列を `f32` の列に戻す。長さが 4 の倍数でなければ `Err`。
fn f32_from_ne_bytes(bytes: &[u8]) -> Result<Vec<f32>, FlowError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(FlowError::LengthMismatch);
    }
    let (chunks, _) = bytes.as_chunks::<4>();
    Ok(chunks.iter().map(|c| f32::from_ne_bytes(*c)).collect())
}

fn to_u32(x: usize) -> Result<u32, FlowError> {
    u32::try_from(x).map_err(|_| FlowError::SizeOverflow)
}

const fn fc_u32(index: u32, v: u32) -> FunctionConstant {
    FunctionConstant {
        index,
        value: FunctionConstantValue::U32(v),
    }
}

const fn fc_bool(index: u32, v: bool) -> FunctionConstant {
    FunctionConstant {
        index,
        value: FunctionConstantValue::Bool(v),
    }
}

/// 特殊化する GEMM の起動の形。行ごとの値を直に置く。
fn specialized_gemm_shape(row: GemmGateRow) -> Result<DispatchShape, FlowError> {
    // (スレッド数, threadgroup 数の幅, 同 高さ, threadgroup のメモリのバイト数, function constant 18 個)
    let (threads, groups_width, groups_height, memory, function_constants) = match row {
        GemmGateRow::Sq512 => (
            128u32,
            16u32,
            8u32,
            13824u32,
            vec![
                fc_u32(0, 64),
                fc_u32(1, 32),
                fc_u32(2, 32),
                fc_u32(3, 2),
                fc_u32(4, 2),
                fc_bool(5, true),
                fc_u32(6, 4),
                fc_bool(7, false),
                fc_bool(8, false),
                fc_bool(9, false),
                fc_bool(10, false),
                fc_bool(11, false),
                fc_bool(12, false),
                fc_u32(13, 1),
                fc_u32(14, 0),
                fc_u32(15, 0),
                fc_bool(16, false),
                fc_u32(17, 0),
            ],
        ),
        GemmGateRow::Sq2048 => (
            128u32,
            64u32,
            32u32,
            7424u32,
            vec![
                fc_u32(0, 64),
                fc_u32(1, 32),
                fc_u32(2, 16),
                fc_u32(3, 2),
                fc_u32(4, 2),
                fc_bool(5, true),
                fc_u32(6, 4),
                fc_bool(7, false),
                fc_bool(8, false),
                fc_bool(9, false),
                fc_bool(10, false),
                fc_bool(11, false),
                fc_bool(12, false),
                fc_u32(13, 1),
                fc_u32(14, 0),
                fc_u32(15, 0),
                fc_bool(16, false),
                fc_u32(17, 0),
            ],
        ),
        _ => return Err(FlowError::UnsupportedRow),
    };
    let (m, n, k) = (to_u32(row.m())?, to_u32(row.n())?, to_u32(row.k())?);
    Ok(DispatchShape {
        threads: [threads, 1, 1],
        groups: [groups_width, groups_height, 1],
        layout: (0..=6).collect(),
        constants: vec![
            (3, u32_ne_bytes(&[m, n, k])),
            (4, u32_ne_bytes(&[k, n, 0, 0])),
            (5, u32_ne_bytes(&[0, 0, groups_height, groups_width])),
            (6, u32_ne_bytes(&[1, k, 0, 0])),
        ],
        threadgroup_memory: Some((0, memory)),
        function_constants,
    })
}

/// 特殊化しない GEMM の起動の形。
fn plain_gemm_shape(row: GemmGateRow) -> Result<DispatchShape, FlowError> {
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
fn gemm_shape(row: GemmGateRow) -> Result<DispatchShape, FlowError> {
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
fn add_shape(len: usize) -> Result<DispatchShape, FlowError> {
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
fn relu_shape(len: usize) -> Result<DispatchShape, FlowError> {
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
fn addend(row: GemmGateRow) -> Result<Vec<f32>, FlowError> {
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

/// 加算の CPU の参照。
fn reference_add(gemm: &[f32], addend: &[f32]) -> Result<Vec<f32>, FlowError> {
    if gemm.len() != addend.len() {
        return Err(FlowError::LengthMismatch);
    }
    Ok(gemm.iter().zip(addend).map(|(g, a)| g + a).collect())
}

/// Relu の CPU の参照。`x > 0` ならそのまま、そうでなければ `+0.0`（`-0.0` と NaN も `+0.0`）。
fn reference_relu(x: &[f32]) -> Vec<f32> {
    x.iter().map(|&v| if v > 0.0 { v } else { 0.0 }).collect()
}

/// 3 段の CPU の参照。
#[derive(Debug, Clone, PartialEq)]
struct FlowReference {
    gemm: Vec<f32>,
    add: Vec<f32>,
    relu: Vec<f32>,
}

/// 3 段の CPU の参照を作る。
fn reference(
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

/// カーネルの役割が、段の要る役割と合うか確かめる（device に触る前の検査）。
fn check_role(kernel: &FlowKernel, expected: Role) -> Result<(), FlowError> {
    if kernel.role == expected {
        Ok(())
    } else {
        Err(FlowError::RoleMismatch(expected))
    }
}

/// GEMM のカーネルの役割が行の要る役割と合うか確かめる。扱わない行は `UnsupportedRow`。
fn check_gemm_role(row: GemmGateRow, kernel: &FlowKernel) -> Result<(), FlowError> {
    let expected = gemm_role(row).ok_or(FlowError::UnsupportedRow)?;
    check_role(kernel, expected)
}

/// GEMM の入力の長さが行の `M×K`・`K×N` と合うか確かめる（device に触る前の検査）。
fn check_gemm_input_lengths(row: GemmGateRow, inputs: &GemmInputsF32) -> Result<(), FlowError> {
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

/// 2 つの列の要素ごとの差の絶対値の最大。NaN を含めば無限大。長さが違えば `None`。
fn max_abs_diff(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() {
        return None;
    }
    let mut max = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        let d = (x - y).abs();
        if d.is_nan() {
            return Some(f32::INFINITY);
        }
        max = max.max(d);
    }
    Some(max)
}

/// device に投入して待ち、出力を読む部分と、それを使う `#[ignore]` 付きの試験。
#[cfg(all(target_os = "macos", feature = "native-blocking-wait"))]
mod device {
    use std::time::Duration;

    use fandhe_silicon_core::capability::BackendKind;
    use fandhe_silicon_core::device::{
        Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
    };
    use fandhe_silicon_core::error::{CompletionState, ContractError};
    use fandhe_silicon_core::execution::{
        Binding, BindingLayout, ConstantBinding, PipelineInput, ThreadgroupMemory,
    };
    use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
    use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputsF32, generate_f32};
    use fandhe_silicon_core::parity::tolerance::{T1, compare_f32};
    use fandhe_silicon_core::test_frame::{
        Declaration, Label, MachineCombination, RunResult, RunSetting, Subject, Verdict,
        judge_and_export,
    };

    use super::{
        DispatchShape, FlowError, FlowKernel, FlowKernels, Role, add_shape, addend,
        check_gemm_input_lengths, check_gemm_role, check_role, f32_from_ne_bytes, f32_ne_bytes,
        gemm_shape, max_abs_diff, reference, relu_shape,
    };
    use crate::backend_metal::MetalDeviceProvider;

    /// 失敗した段。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Stage {
        Gemm,
        Add,
        Relu,
    }

    /// 段の中で失敗した手順。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum StepKind {
        Request,
        NoCompute,
        Pipeline,
        Allocate,
        Upload,
        Dispatch,
        Submit,
        Wait,
        NotComplete,
        Download,
        Convert,
    }

    /// 口の失敗。`kind` は契約のエラーを写した固定の種類の文字列か、手順の固定の語。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RunError {
        stage: Option<Stage>,
        step: StepKind,
        kind: &'static str,
    }

    impl std::fmt::Display for RunError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "run failed: stage={:?} step={:?} kind={}",
                self.stage, self.step, self.kind
            )
        }
    }

    impl RunError {
        fn from_flow(stage: Stage, e: FlowError) -> Self {
            let kind = match e {
                FlowError::SizeOverflow => "size-overflow",
                FlowError::LengthMismatch => "length-mismatch",
                FlowError::AllocationFailed => "allocation-failed",
                FlowError::RoleMismatch(_) => "role-mismatch",
                _ => "invalid-flow",
            };
            Self {
                stage: Some(stage),
                step: StepKind::Convert,
                kind,
            }
        }
    }

    /// 3 段の出力。
    struct FlowOutputs {
        gemm: Vec<f32>,
        add: Vec<f32>,
        relu: Vec<f32>,
    }

    /// 契約のエラーを、値を含まない固定の種類の文字列に写す。
    fn error_kind(e: &ContractError) -> &'static str {
        match e {
            ContractError::InvalidCombination(_) => "invalid-combination",
            ContractError::DeviceUnavailable(_) => "device-unavailable",
            ContractError::DeviceLost { .. } => "device-lost",
            ContractError::DeviceMismatch => "device-mismatch",
            ContractError::InvalidArgument(_) => "invalid-argument",
            ContractError::AllocationFailed { .. } => "allocation-failed",
            ContractError::ArtifactRejected { kind, .. } => match *kind {
                "msl" => "artifact-rejected-msl",
                "compute-pipeline" => "artifact-rejected-pipeline",
                _ => "artifact-rejected",
            },
            ContractError::ValidationFailed(_) => "validation-failed",
            ContractError::Timeout => "timeout",
            ContractError::Unsupported { .. } => "unsupported",
            ContractError::CapabilityInsufficient { .. } => "capability-insufficient",
            ContractError::DeviceIdentityMismatch { .. } => "device-identity-mismatch",
            ContractError::UnsupportedExtensionDefault { .. } => "unsupported-extension-default",
            ContractError::BackendFailure { .. } => "backend-failure",
            ContractError::CapacityExceeded { .. } => "capacity-exceeded",
            _ => "other",
        }
    }

    fn err(stage: Stage, step: StepKind, kind: &'static str) -> RunError {
        RunError {
            stage: Some(stage),
            step,
            kind,
        }
    }

    /// Metal の device を 1 つ作る（ComputeOnly・ProductIndependent）。
    fn open_device() -> Result<Box<dyn Device>, RunError> {
        MetalDeviceProvider::request(DeviceRequest {
            backend: BackendKind::Metal,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        })
        .map_err(|e| RunError {
            stage: None,
            step: StepKind::Request,
            kind: error_kind(&e),
        })
    }

    fn pipeline_input<'a>(kernel: &'a FlowKernel, shape: &'a DispatchShape) -> PipelineInput<'a> {
        if kernel.role == Role::SpecializedGemm {
            PipelineInput::MslSpecialized {
                source: &kernel.source,
                entry_point: &kernel.entry,
                function_constants: &shape.function_constants,
            }
        } else {
            PipelineInput::Msl {
                source: &kernel.source,
                entry_point: &kernel.entry,
            }
        }
    }

    fn alloc_f32(
        device: &dyn Device,
        stage: Stage,
        len: usize,
        held: &mut Vec<BufferHandle>,
    ) -> Result<usize, RunError> {
        let bytes = len
            .checked_mul(4)
            .and_then(|b| u64::try_from(b).ok())
            .ok_or_else(|| err(stage, StepKind::Convert, "size-overflow"))?;
        let usage = BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD;
        let buf = device
            .memory()
            .allocate(bytes, usage)
            .map_err(|e| err(stage, StepKind::Allocate, error_kind(&e)))?;
        held.push(buf);
        Ok(held.len() - 1)
    }

    /// 1 段を流す。確保したバッファは成否に関わらず解放する。
    fn run_stage(
        device: &dyn Device,
        stage: Stage,
        kernel: &FlowKernel,
        shape: &DispatchShape,
        inputs: &[&[f32]],
        out_len: usize,
    ) -> Result<Vec<f32>, RunError> {
        let mut held: Vec<BufferHandle> = Vec::new();
        let result = stage_body(device, stage, kernel, shape, inputs, out_len, &mut held);
        for buf in held {
            device.memory().free(buf);
        }
        result
    }

    fn stage_body(
        device: &dyn Device,
        stage: Stage,
        kernel: &FlowKernel,
        shape: &DispatchShape,
        inputs: &[&[f32]],
        out_len: usize,
        held: &mut Vec<BufferHandle>,
    ) -> Result<Vec<f32>, RunError> {
        let compute = device
            .compute()
            .ok_or_else(|| err(stage, StepKind::NoCompute, "no-compute"))?;
        let pipeline = compute
            .create_pipeline(
                pipeline_input(kernel, shape),
                &BindingLayout::new(shape.layout.clone()),
            )
            .map_err(|e| err(stage, StepKind::Pipeline, error_kind(&e)))?;

        let mut ids = Vec::new();
        for data in inputs {
            let id = alloc_f32(device, stage, data.len(), held)?;
            device
                .memory()
                .upload(&held[id], 0, &f32_ne_bytes(data))
                .map_err(|e| err(stage, StepKind::Upload, error_kind(&e)))?;
            ids.push(id);
        }
        let out_id = alloc_f32(device, stage, out_len, held)?;
        ids.push(out_id);

        let bindings: Vec<Binding<'_>> = ids
            .iter()
            .enumerate()
            .map(|(slot, &id)| Binding::new(slot as u32, &held[id], 0))
            .collect();
        let consts: Vec<ConstantBinding<'_>> = shape
            .constants
            .iter()
            .map(|(slot, bytes)| ConstantBinding::new(*slot, bytes))
            .collect();
        let tg: Vec<ThreadgroupMemory> = shape
            .threadgroup_memory
            .iter()
            .map(|&(index, bytes)| ThreadgroupMemory::new(index, bytes))
            .collect();

        let queue = device.queue();
        let mut encoder = queue.create_encoder();
        queue
            .dispatch_ex(
                &mut encoder,
                &pipeline,
                &bindings,
                &consts,
                shape.groups,
                shape.threads,
                &tg,
            )
            .map_err(|e| err(stage, StepKind::Dispatch, error_kind(&e)))?;
        let token = queue
            .submit(encoder, &[])
            .map_err(|e| err(stage, StepKind::Submit, error_kind(&e)))?;
        queue
            .wait_blocking(token, None::<Duration>)
            .map_err(|e| err(stage, StepKind::Wait, error_kind(&e)))?;
        if queue.poll(token) != CompletionState::Complete {
            return Err(err(stage, StepKind::NotComplete, "not-complete"));
        }

        let out_bytes = out_len
            .checked_mul(4)
            .and_then(|b| u64::try_from(b).ok())
            .ok_or_else(|| err(stage, StepKind::Convert, "size-overflow"))?;
        let bytes = device
            .memory()
            .download_blocking(&held[out_id], 0, out_bytes)
            .map_err(|e| err(stage, StepKind::Download, error_kind(&e)))?;
        f32_from_ne_bytes(&bytes).map_err(|e| RunError::from_flow(stage, e))
    }

    /// GEMM の段だけを 1 回の投入で流し、出力を返す。
    fn run_gemm(
        device: &dyn Device,
        row: GemmGateRow,
        kernel: &FlowKernel,
        inputs: &GemmInputsF32,
    ) -> Result<Vec<f32>, RunError> {
        check_gemm_role(row, kernel).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        check_gemm_input_lengths(row, inputs).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        let out_len = row
            .m()
            .checked_mul(row.n())
            .ok_or_else(|| err(Stage::Gemm, StepKind::Convert, "size-overflow"))?;
        let shape = gemm_shape(row).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        run_stage(
            device,
            Stage::Gemm,
            kernel,
            &shape,
            &[inputs.a(), inputs.b()],
            out_len,
        )
    }

    /// GEMM → 加算 → Relu を 3 回の投入で流す。前の段の出力を次の段の入力にする。
    fn run_flow(
        device: &dyn Device,
        row: GemmGateRow,
        kernels: &FlowKernels,
        inputs: &GemmInputsF32,
        addend: &[f32],
    ) -> Result<FlowOutputs, RunError> {
        // 先頭の段を流す前に、後続の段の役割も確かめる（途中で投入済みにしない）。
        check_gemm_role(row, &kernels.gemm).map_err(|e| RunError::from_flow(Stage::Gemm, e))?;
        check_role(&kernels.add, Role::Add).map_err(|e| RunError::from_flow(Stage::Add, e))?;
        check_role(&kernels.relu, Role::Relu).map_err(|e| RunError::from_flow(Stage::Relu, e))?;
        let gemm = run_gemm(device, row, &kernels.gemm, inputs)?;
        let len = gemm.len();
        if addend.len() != len {
            return Err(RunError::from_flow(Stage::Add, FlowError::LengthMismatch));
        }
        let add_s = add_shape(len).map_err(|e| RunError::from_flow(Stage::Add, e))?;
        let add = run_stage(
            device,
            Stage::Add,
            &kernels.add,
            &add_s,
            &[&gemm, addend],
            len,
        )?;
        let relu_s = relu_shape(len).map_err(|e| RunError::from_flow(Stage::Relu, e))?;
        let relu = run_stage(device, Stage::Relu, &kernels.relu, &relu_s, &[&add], len)?;
        Ok(FlowOutputs { gemm, add, relu })
    }

    /// 環境変数から、形状に要るカーネルをそろえる。失敗は固定の文言で試験を失敗にする（skip にしない）。
    fn kernels_for(row: GemmGateRow) -> FlowKernels {
        FlowKernels::from_lookup(row, &|n| std::env::var_os(n))
            .unwrap_or_else(|e| panic!("kernel setup failed: {e}"))
    }

    fn subject(test_id: &str, behavior: &str) -> Subject {
        Subject::from_parts(test_id, &[behavior], &["t-1"]).expect("subject")
    }

    /// 試験の枠で判定して書き出し、OK でなければ失敗にする。
    fn judge(subject: &Subject, body: impl FnOnce() -> Verdict) {
        let declaration =
            Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).expect("declaration");
        let setting = RunSetting::from_env();
        let outcome = judge_and_export(subject, &declaration, &setting, |_| {
            RunResult::Verdict(body())
        })
        .expect("export failed");
        assert_eq!(outcome.label(), &Label::Ok);
    }

    fn print_line(row: GemmGateRow, stage: &str, reference: &[f32], actual: &[f32], v: Verdict) {
        let diff = max_abs_diff(reference, actual);
        println!(
            "shape={row:?} stage={stage} len={} max_abs_diff={diff:?} verdict={v:?}",
            reference.len()
        );
    }

    fn gemm_case(test_id: &str, behavior: &str, row: GemmGateRow) {
        let kernels = kernels_for(row);
        let inputs = generate_f32(row).expect("inputs");
        let second = addend(row).expect("addend");
        let expected = reference(row, &inputs, &second).expect("reference");
        judge(&subject(test_id, behavior), || {
            let device = match open_device() {
                Ok(d) => d,
                Err(e) => {
                    println!("{e}");
                    return Verdict::Mismatch;
                }
            };
            let out = match run_gemm(&*device, row, &kernels.gemm, &inputs) {
                Ok(o) => o,
                Err(e) => {
                    println!("{e}");
                    return Verdict::Mismatch;
                }
            };
            let Ok(cmp) = compare_f32(&expected.gemm, &out, T1) else {
                println!("compare failed: length");
                return Verdict::Mismatch;
            };
            let v = Verdict::from(&cmp);
            print_line(row, "gemm", &expected.gemm, &out, v);
            v
        });
    }

    fn flow_case(test_id: &str, behavior: &str, row: GemmGateRow) {
        let kernels = kernels_for(row);
        let inputs = generate_f32(row).expect("inputs");
        let second = addend(row).expect("addend");
        let expected = reference(row, &inputs, &second).expect("reference");
        judge(&subject(test_id, behavior), || {
            let device = match open_device() {
                Ok(d) => d,
                Err(e) => {
                    println!("{e}");
                    return Verdict::Mismatch;
                }
            };
            let out = match run_flow(&*device, row, &kernels, &inputs, &second) {
                Ok(o) => o,
                Err(e) => {
                    println!("{e}");
                    return Verdict::Mismatch;
                }
            };
            let mut all = Verdict::Match;
            for (name, want, got) in [
                ("gemm", &expected.gemm, &out.gemm),
                ("add", &expected.add, &out.add),
                ("relu", &expected.relu, &out.relu),
            ] {
                let v = match compare_f32(want, got, T1) {
                    Ok(cmp) => Verdict::from(&cmp),
                    Err(_) => Verdict::Mismatch,
                };
                print_line(row, name, want, got, v);
                if v != Verdict::Match {
                    all = Verdict::Mismatch;
                }
            }
            all
        });
    }

    mod gemm {
        use super::*;

        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn specialized_gemm_sq512_matches_reference() {
            // 項目 ID のポインタ: ART-2
            gemm_case("gemm_specialized_sq512", "ART-2", GemmGateRow::Sq512);
        }

        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn specialized_gemm_sq2048_matches_reference() {
            // 項目 ID のポインタ: ART-2
            gemm_case("gemm_specialized_sq2048", "ART-2", GemmGateRow::Sq2048);
        }
    }

    mod flow {
        use super::*;

        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn gemm_add_relu_sq512_matches_reference() {
            // 項目 ID のポインタ: ART-79
            flow_case("gemm_add_relu_sq512", "ART-79", GemmGateRow::Sq512);
        }

        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn gemm_add_relu_sq2048_matches_reference() {
            // 項目 ID のポインタ: ART-79
            flow_case("gemm_add_relu_sq2048", "ART-79", GemmGateRow::Sq2048);
        }

        #[test]
        #[ignore = "touches the Metal device; run only in a dedicated device session"]
        fn gemm_add_relu_odd_matches_reference() {
            // 項目 ID のポインタ: ART-79
            flow_case("gemm_add_relu_odd", "ART-79", GemmGateRow::Odd37x41x53);
        }
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;
    use fandhe_silicon_core::parity::gemm_inputs::generate_f32;
    use std::path::PathBuf;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn as_u32s(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks(4)
            .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    fn const_at(shape: &DispatchShape, slot: u32) -> Vec<u32> {
        let (_, b) = shape
            .constants
            .iter()
            .find(|(s, _)| *s == slot)
            .expect("slot");
        as_u32s(b)
    }

    /// 試験ごとに一意の一時ディレクトリ。`Drop` で消す。
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("metal-flow-tests-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const SRC_ONE: &str = "// synthetic one\nkernel void k_spec() {}\nkernel void k_plain() {}\n";
    const SRC_TWO: &str = "// synthetic two\nkernel void k_add() {}\nkernel void k_relu() {}\n";

    /// 合成の 2 ファイルを書き、その一覧のテキストを返す。
    fn write_fixture(dir: &Path) -> String {
        std::fs::write(dir.join("one.src"), SRC_ONE).expect("write");
        std::fs::write(dir.join("two.src"), SRC_TWO).expect("write");
        let rev = hex(&sha256_bytes(b"rev"))[..40].to_owned();
        let h1 = hex(&sha256_bytes(SRC_ONE.as_bytes()));
        let h2 = hex(&sha256_bytes(SRC_TWO.as_bytes()));
        [
            format!("{rev} a/one.src {h1} specialized k_spec"),
            format!("{rev} a/one.src {h1} msl k_plain"),
            format!("{rev} b/two.src {h2} msl k_add"),
            format!("{rev} b/two.src {h2} msl k_relu"),
        ]
        .join("\n")
    }

    fn lookup_with<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    const GOOD: [(&str, &str); 4] = [
        (SPECIALIZED_GEMM_ENV, "k_spec"),
        (PLAIN_GEMM_ENV, "k_plain"),
        (ADD_ENV, "k_add"),
        (RELU_ENV, "k_relu"),
    ];

    fn line(sha: &str, mode: &str, entry: &str) -> String {
        let rev = hex(&sha256_bytes(b"rev"))[..40].to_owned();
        format!("{rev} a/x.src {sha} {mode} {entry}")
    }

    #[test]
    fn roles_map_rows_and_modes() {
        assert_eq!(gemm_role(GemmGateRow::Sq512), Some(Role::SpecializedGemm));
        assert_eq!(gemm_role(GemmGateRow::Sq2048), Some(Role::SpecializedGemm));
        assert_eq!(gemm_role(GemmGateRow::Odd37x41x53), Some(Role::PlainGemm));
        assert_eq!(gemm_role(GemmGateRow::Sq4096), None);
        assert_eq!(FLOW_ROWS.len(), 3);
        assert_eq!(Role::SpecializedGemm.mode(), Mode::Specialized);
        for r in [Role::PlainGemm, Role::Add, Role::Relu] {
            assert_eq!(r.mode(), Mode::Msl);
        }
    }

    #[test]
    fn specialized_shapes_carry_eighteen_constants_per_row() {
        for (row, threads, groups, memory) in [
            (GemmGateRow::Sq512, 128, [16, 8, 1], 13824),
            (GemmGateRow::Sq2048, 128, [64, 32, 1], 7424),
        ] {
            let s = specialized_gemm_shape(row).expect("shape");
            let (m, n, k) = (row.m() as u32, row.n() as u32, row.k() as u32);
            assert_eq!(s.threads, [threads, 1, 1]);
            assert_eq!(s.groups, groups);
            assert_eq!(s.layout, vec![0, 1, 2, 3, 4, 5, 6]);
            assert_eq!(s.threadgroup_memory, Some((0, memory)));
            assert_eq!(memory % 16, 0);
            assert_eq!(s.function_constants.len(), 18);
            for (i, c) in s.function_constants.iter().enumerate() {
                assert_eq!(c.index as usize, i);
            }
            assert_eq!(s.constants.len(), 4);
            assert_eq!(s.constants[0].1.len(), 12);
            assert_eq!(const_at(&s, 3), vec![m, n, k]);
            assert_eq!(const_at(&s, 4), vec![k, n, 0, 0]);
            assert_eq!(const_at(&s, 5), vec![0, 0, groups[1], groups[0]]);
            assert_eq!(const_at(&s, 6), vec![1, k, 0, 0]);
            assert_eq!(gemm_shape(row).expect("same").groups, s.groups);
        }
        let a = specialized_gemm_shape(GemmGateRow::Sq512).expect("a");
        let b = specialized_gemm_shape(GemmGateRow::Sq2048).expect("b");
        assert_ne!(
            format!("{:?}", a.function_constants),
            format!("{:?}", b.function_constants)
        );
        assert_eq!(
            specialized_gemm_shape(GemmGateRow::Odd37x41x53).unwrap_err(),
            FlowError::UnsupportedRow
        );
    }

    #[test]
    fn plain_shape_uses_sixteen_square_groups() {
        let row = GemmGateRow::Odd37x41x53;
        let s = plain_gemm_shape(row).expect("shape");
        let (m, n, k) = (row.m() as u32, row.n() as u32, row.k() as u32);
        assert_eq!(s.threads, [16, 16, 1]);
        assert_eq!(s.groups, [n.div_ceil(16), m.div_ceil(16), 1]);
        assert_eq!(s.layout, vec![0, 1, 2, 3]);
        assert_eq!(s.constants.len(), 1);
        assert_eq!(s.constants[0].0, 3);
        assert_eq!(s.constants[0].1.len(), 12);
        assert_eq!(const_at(&s, 3), vec![m, n, k]);
        assert_eq!(s.threadgroup_memory, None);
        assert!(s.function_constants.is_empty());
        assert_eq!(gemm_shape(row).expect("same").groups, s.groups);
        assert_eq!(
            gemm_shape(GemmGateRow::Sq4096).unwrap_err(),
            FlowError::UnsupportedRow
        );
    }

    #[test]
    fn elementwise_shapes_follow_the_length() {
        for (len, groups) in [(1usize, 1u32), (256, 1), (257, 2), (512, 2), (513, 3)] {
            let a = add_shape(len).expect("add");
            assert_eq!(a.threads, [256, 1, 1]);
            assert_eq!(a.groups, [groups, 1, 1]);
            assert_eq!(a.layout, vec![0, 1, 2, 3]);
            assert_eq!(const_at(&a, 3), vec![len as u32]);
            let r = relu_shape(len).expect("relu");
            assert_eq!(r.threads, [256, 1, 1]);
            assert_eq!(r.groups, [groups, 1, 1]);
            assert_eq!(r.layout, vec![0, 1, 2]);
            assert_eq!(const_at(&r, 2), vec![len as u32]);
            assert!(a.threadgroup_memory.is_none() && r.threadgroup_memory.is_none());
        }
        assert!(add_shape(0).is_err());
        assert!(relu_shape(0).is_err());
        if usize::BITS > 32 {
            assert_eq!(add_shape(usize::MAX).unwrap_err(), FlowError::SizeOverflow);
        }
    }

    #[test]
    fn addend_is_deterministic_with_the_gemm_output_length() {
        for row in FLOW_ROWS {
            let a = addend(row).expect("addend");
            assert_eq!(a.len(), row.m() * row.n());
        }
        let mut rng = SplitMix64::new(ADDEND_SEED);
        let a = addend(GemmGateRow::Odd37x41x53).expect("addend");
        assert_eq!(a, addend(GemmGateRow::Odd37x41x53).expect("again"));
        assert_eq!(a[0].to_bits(), rng.next_signed_unit_f32().to_bits());
        assert_eq!(a[1].to_bits(), rng.next_signed_unit_f32().to_bits());
    }

    #[test]
    fn reference_add_and_relu_follow_the_formulas() {
        let g = [1.0f32, -2.0, 0.5, 0.0];
        let b = [0.5f32, 1.0, -0.5, 0.0];
        assert_eq!(
            reference_add(&g, &b).expect("add"),
            vec![1.5, -1.0, 0.0, 0.0]
        );
        assert_eq!(
            reference_add(&g, &b[..3]).unwrap_err(),
            FlowError::LengthMismatch
        );
        let r = reference_relu(&[-1.0, 0.0, -0.0, 2.5, f32::NAN]);
        assert_eq!(r[0].to_bits(), 0.0f32.to_bits());
        assert_eq!(r[1].to_bits(), 0.0f32.to_bits());
        assert_eq!(r[2].to_bits(), 0.0f32.to_bits());
        assert_eq!(r[3], 2.5);
        assert_eq!(r[4].to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn flow_reference_chains_the_three_stages() {
        let row = GemmGateRow::Odd37x41x53;
        let inputs = generate_f32(row).expect("inputs");
        let second = addend(row).expect("addend");
        let r = reference(row, &inputs, &second).expect("reference");
        assert_eq!(r.gemm.len(), row.m() * row.n());
        assert_eq!(r.add, reference_add(&r.gemm, &second).expect("add"));
        assert_eq!(r.relu, reference_relu(&r.add));
        assert_eq!(max_abs_diff(&r.relu, &r.relu), Some(0.0));
        assert_eq!(max_abs_diff(&[1.0, 2.0], &[1.5, 0.0]), Some(2.0));
        assert_eq!(max_abs_diff(&[1.0], &[f32::NAN]), Some(f32::INFINITY));
        assert_eq!(max_abs_diff(&[1.0], &[1.0, 2.0]), None);
    }

    #[test]
    fn byte_conversions_round_trip_and_reject_odd_lengths() {
        let v = [1.0f32, -0.0, 3.5];
        let b = f32_ne_bytes(&v);
        assert_eq!(b.len(), 12);
        let back = f32_from_ne_bytes(&b).expect("back");
        assert_eq!(
            back.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            v.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(
            f32_from_ne_bytes(&b[..5]).unwrap_err(),
            FlowError::LengthMismatch
        );
        assert_eq!(u32_ne_bytes(&[1, 2]).len(), 8);
    }

    #[test]
    fn manifest_lines_parse_with_or_without_trailing_lf() {
        let h = hex(&sha256_bytes(b"x"));
        let text = [line(&h, "msl", "name_a"), line(&h, "specialized", "name_b")].join("\n");
        for t in [text.clone(), format!("{text}\n")] {
            let lines = parse_manifest_lines(&t).expect("lines");
            assert_eq!(lines.len(), 2);
            assert_eq!(lines[0].file_name, "x.src");
            assert_eq!(lines[0].mode, Mode::Msl);
            assert_eq!(lines[1].mode, Mode::Specialized);
            assert_eq!(lines[1].entry, "name_b");
            assert_eq!(lines[0].sha256, sha256_bytes(b"x"));
        }
    }

    #[test]
    fn malformed_manifest_lines_are_rejected() {
        let h = hex(&sha256_bytes(b"x"));
        let good = line(&h, "msl", "name_a");
        let rev = hex(&sha256_bytes(b"rev"))[..40].to_owned();
        let cases: Vec<String> = vec![
            String::new(),
            "\n".to_owned(),
            format!("{rev} a/x.src {h} msl"),
            format!("{good} extra"),
            format!("{good}\n\n"),
            format!("{good}\n\n{good}"),
            format!("{rev} a/x.src {} msl name_a", &h[..63]),
            format!("{rev} a/x.src {} msl name_a", h.to_uppercase()),
            format!("{rev} a/x.src {h} other name_a"),
            format!("{rev} a/ {h} msl name_a"),
            format!("{rev} a/.. {h} msl name_a"),
            format!("{rev} a/. {h} msl name_a"),
            format!("{rev} a/x\\y {h} msl name_a"),
            format!("{rev}  a/x.src {h} msl name_a"),
            format!("{good}\r"),
        ];
        for c in cases {
            assert!(
                matches!(
                    parse_manifest_lines(&c),
                    Err(FlowError::ManifestMalformed(_))
                ),
                "case accepted"
            );
        }
    }

    #[test]
    fn entries_resolve_and_load_with_the_expected_rows() {
        let tmp = TempDir::new("ok");
        let lines = parse_manifest_lines(&write_fixture(&tmp.0)).expect("lines");
        let lookup = lookup_with(&GOOD);
        for role in [
            Role::SpecializedGemm,
            Role::PlainGemm,
            Role::Add,
            Role::Relu,
        ] {
            resolve_entry(&lines, role, &lookup).expect("row");
        }
        let k = load_kernel(&lines, &tmp.0, Role::SpecializedGemm, &lookup).expect("kernel");
        assert_eq!(k.entry, "k_spec");
        assert_eq!(k.source, SRC_ONE);
        for row in FLOW_ROWS {
            let ks = FlowKernels::load(&lines, &tmp.0, row, &lookup).expect("kernels");
            assert_eq!(ks.add.entry, "k_add");
            assert_eq!(ks.relu.entry, "k_relu");
            assert_eq!(Some(ks.gemm.role), gemm_role(row));
            assert_eq!(check_gemm_role(row, &ks.gemm), Ok(()));
        }
        assert_eq!(
            FlowKernels::load(&lines, &tmp.0, GemmGateRow::Sq4096, &lookup).unwrap_err(),
            FlowError::UnsupportedRow
        );
    }

    #[test]
    fn entry_checks_fail_closed_without_leaking_values() {
        let tmp = TempDir::new("err");
        let lines = parse_manifest_lines(&write_fixture(&tmp.0)).expect("lines");
        let secret_name = "zz_unlisted_name";
        let long = "a".repeat(65);
        let cases: Vec<(Vec<(&str, &str)>, FlowError)> = vec![
            (vec![], FlowError::EntryMissing(Role::Add)),
            (vec![(ADD_ENV, "")], FlowError::EntryMissing(Role::Add)),
            (
                vec![(ADD_ENV, "1bad")],
                FlowError::EntryMalformed(Role::Add),
            ),
            (
                vec![(ADD_ENV, "bad-name")],
                FlowError::EntryMalformed(Role::Add),
            ),
            (
                vec![(ADD_ENV, long.as_str())],
                FlowError::EntryMalformed(Role::Add),
            ),
            (
                vec![(ADD_ENV, secret_name)],
                FlowError::EntryNotListed(Role::Add),
            ),
            (
                vec![(ADD_ENV, "k_spec")],
                FlowError::ModeMismatch(Role::Add),
            ),
        ];
        for (pairs, expected) in cases {
            let lookup = lookup_with(&pairs);
            let err = resolve_entry(&lines, Role::Add, &lookup).unwrap_err();
            assert_eq!(err, expected);
            let text = err.to_string();
            assert!(
                !text.contains(secret_name) && !text.contains("k_spec") && !text.contains("bad")
            );
        }
        let lookup = lookup_with(&[(SPECIALIZED_GEMM_ENV, "k_plain")]);
        assert_eq!(
            resolve_entry(&lines, Role::SpecializedGemm, &lookup).unwrap_err(),
            FlowError::ModeMismatch(Role::SpecializedGemm)
        );
    }

    #[test]
    fn an_entry_listed_twice_is_rejected() {
        let h = hex(&sha256_bytes(b"x"));
        let text = [line(&h, "msl", "dup_name"), line(&h, "msl", "dup_name")].join("\n");
        let lines = parse_manifest_lines(&text).expect("lines");
        let lookup = lookup_with(&[(RELU_ENV, "dup_name")]);
        let err = resolve_entry(&lines, Role::Relu, &lookup).unwrap_err();
        assert_eq!(err, FlowError::EntryListedTwice(Role::Relu));
        assert!(!err.to_string().contains("dup_name"));
    }

    #[test]
    fn source_verification_checks_digest_size_and_encoding() {
        let src = b"// synthetic\nkernel void k() {}\n";
        let want = sha256_bytes(src);
        assert_eq!(
            verify_source(src, &want).expect("ok"),
            String::from_utf8_lossy(src)
        );
        assert_eq!(
            verify_source(src, &sha256_bytes(b"other")),
            Err(SourceError::DigestMismatch)
        );
        let big = vec![b'a'; MAX_SOURCE_BYTES as usize + 1];
        assert_eq!(
            verify_source(&big, &sha256_bytes(&big)),
            Err(SourceError::TooLarge)
        );
        let bad = [0xffu8, 0xfe, 0x00];
        assert_eq!(
            verify_source(&bad, &sha256_bytes(&bad)),
            Err(SourceError::NotUtf8)
        );
    }

    #[test]
    fn unreadable_or_mismatched_sources_are_rejected_before_any_device() {
        let tmp = TempDir::new("src");
        let lines = parse_manifest_lines(&write_fixture(&tmp.0)).expect("lines");
        let lookup = lookup_with(&GOOD);
        std::fs::write(tmp.0.join("two.src"), "// tampered\n").expect("write");
        let err = FlowKernels::load(&lines, &tmp.0, GemmGateRow::Sq512, &lookup).unwrap_err();
        assert_eq!(err, FlowError::SourceUnreadable(Role::Add));
        assert!(!err.to_string().contains("k_add"));
        std::fs::remove_file(tmp.0.join("one.src")).expect("remove");
        let err = load_kernel(&lines, &tmp.0, Role::PlainGemm, &lookup).unwrap_err();
        assert_eq!(err, FlowError::SourceUnreadable(Role::PlainGemm));
        std::fs::write(
            tmp.0.join("one.src"),
            vec![b'a'; MAX_SOURCE_BYTES as usize + 1],
        )
        .expect("write");
        let err = load_kernel(&lines, &tmp.0, Role::PlainGemm, &lookup).unwrap_err();
        assert_eq!(err, FlowError::SourceUnreadable(Role::PlainGemm));
    }

    #[test]
    fn from_lookup_reads_the_manifest_and_directory() {
        let tmp = TempDir::new("env");
        let text = write_fixture(&tmp.0);
        let manifest_path = tmp.0.join("manifest.txt");
        std::fs::write(&manifest_path, format!("{text}\n")).expect("write");
        let mpath = manifest_path.to_str().expect("utf8").to_owned();
        let dpath = tmp.0.to_str().expect("utf8").to_owned();
        let lookup = |n: &str| -> Option<OsString> {
            match n {
                MANIFEST_ENV => Some(OsString::from(&mpath)),
                DIR_ENV => Some(OsString::from(&dpath)),
                other => GOOD
                    .iter()
                    .find(|(k, _)| *k == other)
                    .map(|(_, v)| OsString::from(v)),
            }
        };
        let ks = FlowKernels::from_lookup(GemmGateRow::Odd37x41x53, &lookup).expect("kernels");
        assert_eq!(ks.gemm.role, Role::PlainGemm);
        assert!(format!("{ks:?}").contains("FlowKernel"));
        assert!(!format!("{ks:?}").contains("synthetic"));

        let no_manifest = |n: &str| if n == MANIFEST_ENV { None } else { lookup(n) };
        assert_eq!(
            FlowKernels::from_lookup(GemmGateRow::Sq512, &no_manifest).unwrap_err(),
            FlowError::ManifestPathMissing
        );
        let no_dir = |n: &str| if n == DIR_ENV { None } else { lookup(n) };
        assert_eq!(
            FlowKernels::from_lookup(GemmGateRow::Sq512, &no_dir).unwrap_err(),
            FlowError::DirMissing
        );
        std::fs::remove_file(&manifest_path).expect("remove");
        assert_eq!(
            FlowKernels::from_lookup(GemmGateRow::Sq512, &lookup).unwrap_err(),
            FlowError::ManifestUnreadable
        );
        std::fs::write(&manifest_path, vec![b'a'; MAX_MANIFEST_BYTES as usize + 1]).expect("write");
        assert_eq!(
            FlowKernels::from_lookup(GemmGateRow::Sq512, &lookup).unwrap_err(),
            FlowError::ManifestTooLarge
        );
        std::fs::write(&manifest_path, [0xffu8, 0xfe]).expect("write");
        assert_eq!(
            FlowKernels::from_lookup(GemmGateRow::Sq512, &lookup).unwrap_err(),
            FlowError::ManifestNotUtf8
        );
    }

    #[test]
    fn gemm_input_lengths_and_roles_are_checked_against_the_row() {
        let odd = generate_f32(GemmGateRow::Odd37x41x53).expect("inputs");
        assert_eq!(
            check_gemm_input_lengths(GemmGateRow::Odd37x41x53, &odd),
            Ok(())
        );
        assert_eq!(
            check_gemm_input_lengths(GemmGateRow::Sq512, &odd),
            Err(FlowError::LengthMismatch)
        );

        let tmp = TempDir::new("role");
        let lines = parse_manifest_lines(&write_fixture(&tmp.0)).expect("lines");
        let lookup = lookup_with(&GOOD);
        let spec = load_kernel(&lines, &tmp.0, Role::SpecializedGemm, &lookup).expect("spec");
        let plain = load_kernel(&lines, &tmp.0, Role::PlainGemm, &lookup).expect("plain");
        let add = load_kernel(&lines, &tmp.0, Role::Add, &lookup).expect("add");
        assert_eq!(check_gemm_role(GemmGateRow::Sq512, &spec), Ok(()));
        assert_eq!(check_gemm_role(GemmGateRow::Odd37x41x53, &plain), Ok(()));
        assert_eq!(
            check_gemm_role(GemmGateRow::Sq512, &plain),
            Err(FlowError::RoleMismatch(Role::SpecializedGemm))
        );
        assert_eq!(
            check_gemm_role(GemmGateRow::Odd37x41x53, &spec),
            Err(FlowError::RoleMismatch(Role::PlainGemm))
        );
        assert_eq!(
            check_gemm_role(GemmGateRow::Sq4096, &spec),
            Err(FlowError::UnsupportedRow)
        );
        assert_eq!(check_role(&add, Role::Add), Ok(()));
        let err = check_role(&add, Role::Relu).unwrap_err();
        assert_eq!(err, FlowError::RoleMismatch(Role::Relu));
        assert!(!err.to_string().contains("k_add"));
    }
}
