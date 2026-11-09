//! 比較基準の MPS の腕（TASK-1011）の、どの OS でもコンパイルされる純粋な部分。
//!
//! 門の run のセルから計画を作る関数、行の間隔の照らし、密な列と行の間隔つきのバイト列の詰め替えを持つ。
//! 門の run の枠は `fandhe_silicon_core::parity::gemm_gate` が持ち、この crate はそれを使う側にあたる
//! （製品の crate からは参照されない）。長さ・バイト数の計算は `checked_*` で行い、あふれ・長さの不一致・
//! 確保の失敗は `Err` で返す（panic しない）。
//!
//! macOS 向けだけの橋の module `mps` が MPS の行列積を 1 回投入し、外へ出すのは safe な `run_matmul` だけ。
//! この crate で `unsafe` を持つのはその module だけで、失敗は固定の [`MpsError`] で返す。

use fandhe_silicon_core::parity::gemm_inputs::GemmInputDtype;
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

/// MPS の行列積の呼び出しの失敗。表示は固定の英語で、OS のメッセージ・アドレス・入力の値を含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MpsError {
    /// 入力の種類が計画の入力の型と合わない（device には触れない）。
    InputMismatch,
    /// 既定の Metal の device が得られない。
    NoDevice,
    /// device・クラス・初期化のどれかが MPS の行列積に対応しない（別の経路には回さない）。
    Unsupported,
    /// 行の間隔の照らし・詰め替え・戻しが失敗した。
    Plan(PlanError),
    /// バッファ・コマンドキュー・コマンドバッファが作れない、またはバッファの長さが 0。
    AllocationFailed,
    /// 完了の状態が完了ではない、または出力のバッファが短い（出力は返さない）。
    Execution,
}

impl std::fmt::Display for MpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputMismatch => f.write_str("inputs do not match the plan input type"),
            Self::NoDevice => f.write_str("no default Metal device"),
            Self::Unsupported => f.write_str("the device does not support this MPS operation"),
            Self::Plan(e) => write!(f, "layout failed: {e}"),
            Self::AllocationFailed => f.write_str("allocation failed"),
            Self::Execution => f.write_str("command buffer did not complete"),
        }
    }
}

impl std::error::Error for MpsError {}

/// MPS の行列積を呼ぶ橋。この crate で `unsafe` を持つのはこの module だけ（macOS 向けだけ）。
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod mps;

#[cfg(target_os = "macos")]
pub use mps::run_matmul;

/// 出力の要素の型の選び方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    /// 入力と同じ型（`F32` のセルは f32、`F16Acc` のセルは binary16）。
    SameAsInput,
    /// f32。`F16Acc` のセルだけが受け付ける。
    F32,
}

/// 要素の型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// f32（4 バイト）。
    F32,
    /// binary16（2 バイト）。
    F16,
}

impl ElementType {
    /// 要素 1 つのバイト数。
    pub fn size(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
        }
    }
}

/// 計画の失敗。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanError {
    /// セルの腕が `GemmGateArm::Mps` ではない。
    NotMpsCell,
    /// セルの dtype がその出力の型を受け付けない。
    OutputNotForCell,
    /// 行の間隔が 1 行の最小のバイト数より小さい。
    RowBytesTooSmall,
    /// 行の間隔が要素のバイト数の倍数ではない。
    RowBytesNotAligned,
    /// 要素数またはバイト数の計算があふれる。
    SizeOverflow,
    /// 渡された列の長さが、大きさから求めた長さと合わない。
    LengthMismatch,
    /// 列の確保に失敗した。
    AllocationFailed,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::NotMpsCell => "cell does not belong to the Mps arm",
            Self::OutputNotForCell => "output kind is not accepted for this cell",
            Self::RowBytesTooSmall => "row bytes is smaller than one packed row",
            Self::RowBytesNotAligned => "row bytes is not a multiple of the element size",
            Self::SizeOverflow => "size computation overflows usize",
            Self::LengthMismatch => "slice length does not match the layout",
            Self::AllocationFailed => "allocation failed",
        };
        f.write_str(text)
    }
}

impl std::error::Error for PlanError {}

/// 1 つのセルから作った行列積の計画。大きさと入出力の要素の型を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MpsPlan {
    m: usize,
    n: usize,
    k: usize,
    input: ElementType,
    output: ElementType,
}

impl MpsPlan {
    /// 行列 A・C の行数。
    pub fn m(&self) -> usize {
        self.m
    }

    /// 行列 B・C の列数。
    pub fn n(&self) -> usize {
        self.n
    }

    /// 行列 A の列数・B の行数。
    pub fn k(&self) -> usize {
        self.k
    }

    /// A・B の要素の型。
    pub fn input(&self) -> ElementType {
        self.input
    }

    /// C の要素の型。
    pub fn output(&self) -> ElementType {
        self.output
    }
}

/// 門の run のセルから計画を作る。
///
/// 腕が `Mps` でないセルは `NotMpsCell`。`F32` のセルに `OutputKind::F32` は `OutputNotForCell`。
pub fn plan(cell: GemmGateCell, output: OutputKind) -> Result<MpsPlan, PlanError> {
    if cell.arm() != GemmGateArm::Mps {
        return Err(PlanError::NotMpsCell);
    }
    let (input, output) = match (cell.dtype(), output) {
        (GemmInputDtype::F32, OutputKind::SameAsInput) => (ElementType::F32, ElementType::F32),
        (GemmInputDtype::F32, OutputKind::F32) => return Err(PlanError::OutputNotForCell),
        (GemmInputDtype::F16Acc, OutputKind::SameAsInput) => (ElementType::F16, ElementType::F16),
        (GemmInputDtype::F16Acc, OutputKind::F32) => (ElementType::F16, ElementType::F32),
    };
    let row = cell.row();
    Ok(MpsPlan {
        m: row.m(),
        n: row.n(),
        k: row.k(),
        input,
        output,
    })
}

/// 行の間隔を照らし、`rows` 行ぶんのバッファの長さ（バイト）を返す。
///
/// `row_bytes` が 1 行の最小（`columns × size`）より小さい・`size` の倍数でない・あふれるとき `Err`。
pub fn check_row_bytes(
    columns: usize,
    element: ElementType,
    row_bytes: usize,
    rows: usize,
) -> Result<usize, PlanError> {
    let min = columns
        .checked_mul(element.size())
        .ok_or(PlanError::SizeOverflow)?;
    if row_bytes < min {
        return Err(PlanError::RowBytesTooSmall);
    }
    if !row_bytes.is_multiple_of(element.size()) {
        return Err(PlanError::RowBytesNotAligned);
    }
    rows.checked_mul(row_bytes).ok_or(PlanError::SizeOverflow)
}

/// 行優先の密な列を、行の間隔つきの little-endian のバイト列にする。行の末尾の余りは 0。
fn pack<T: Copy, const N: usize>(
    dense: &[T],
    to_bytes: fn(T) -> [u8; N],
    element: ElementType,
    rows: usize,
    columns: usize,
    row_bytes: usize,
) -> Result<Vec<u8>, PlanError> {
    let count = rows.checked_mul(columns).ok_or(PlanError::SizeOverflow)?;
    if dense.len() != count {
        return Err(PlanError::LengthMismatch);
    }
    let total = check_row_bytes(columns, element, row_bytes, rows)?;
    let mut out = Vec::new();
    out.try_reserve_exact(total)
        .map_err(|_| PlanError::AllocationFailed)?;
    if columns == 0 {
        out.resize(total, 0);
        return Ok(out);
    }
    for row in dense.chunks_exact(columns) {
        for &value in row {
            out.extend_from_slice(&to_bytes(value));
        }
        // 行の末尾の余り（check_row_bytes で row_bytes >= columns * N は確かめ済み）。
        out.resize(out.len() + (row_bytes - columns * N), 0);
    }
    Ok(out)
}

/// 行の間隔つきのバイト列から、行の先頭の `columns × N` バイトだけを読んで密な列にする。余りは読まない。
fn unpack<T, const N: usize>(
    bytes: &[u8],
    from_bytes: fn([u8; N]) -> T,
    element: ElementType,
    rows: usize,
    columns: usize,
    row_bytes: usize,
) -> Result<Vec<T>, PlanError> {
    let total = check_row_bytes(columns, element, row_bytes, rows)?;
    if bytes.len() != total {
        return Err(PlanError::LengthMismatch);
    }
    let count = rows.checked_mul(columns).ok_or(PlanError::SizeOverflow)?;
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| PlanError::AllocationFailed)?;
    if row_bytes == 0 {
        return Ok(out);
    }
    for row in bytes.chunks_exact(row_bytes) {
        let (values, _) = row[..columns * N].as_chunks::<N>();
        out.extend(values.iter().map(|raw| from_bytes(*raw)));
    }
    Ok(out)
}

/// f32 の密な列（`rows × columns`）を行の間隔つきのバイト列にする。
pub fn pack_f32(
    dense: &[f32],
    rows: usize,
    columns: usize,
    row_bytes: usize,
) -> Result<Vec<u8>, PlanError> {
    pack(
        dense,
        f32::to_le_bytes,
        ElementType::F32,
        rows,
        columns,
        row_bytes,
    )
}

/// binary16 のビット列の密な列（`rows × columns`）を行の間隔つきのバイト列にする。
pub fn pack_f16(
    dense: &[u16],
    rows: usize,
    columns: usize,
    row_bytes: usize,
) -> Result<Vec<u8>, PlanError> {
    pack(
        dense,
        u16::to_le_bytes,
        ElementType::F16,
        rows,
        columns,
        row_bytes,
    )
}

/// 行の間隔つきのバイト列から f32 の密な列（`rows × columns`）を取り出す。
pub fn unpack_f32(
    bytes: &[u8],
    rows: usize,
    columns: usize,
    row_bytes: usize,
) -> Result<Vec<f32>, PlanError> {
    unpack(
        bytes,
        f32::from_le_bytes,
        ElementType::F32,
        rows,
        columns,
        row_bytes,
    )
}

/// 行の間隔つきのバイト列から binary16 のビット列の密な列（`rows × columns`）を取り出す。
pub fn unpack_f16(
    bytes: &[u8],
    rows: usize,
    columns: usize,
    row_bytes: usize,
) -> Result<Vec<u16>, PlanError> {
    unpack(
        bytes,
        u16::from_le_bytes,
        ElementType::F16,
        rows,
        columns,
        row_bytes,
    )
}
