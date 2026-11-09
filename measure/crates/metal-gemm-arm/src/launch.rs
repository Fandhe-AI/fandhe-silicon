//! 門の 1 セルを Metal へ 1 回投入するときの起動の表と、入力の詰め物・出力の切り出し（TASK-1008 のポインタ）。
//!
//! どの OS でもコンパイルされる純粋な表で、`unsafe` を持たない。[`launch_for`] が橋の module
//! （macOS 向けの `metal`）に、入口・function constant・スレッド数・threadgroup 数・threadgroup のメモリ長・
//! 実効次元・バッファの割り当てを渡す。詰め物と切り出しは、門の枠が詰め物の無い M×K・K×N の入力を渡し
//! M×N の出力を期待するため、腕の中で行う。

use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

use crate::ArmError;

/// 呼び出すカーネルの入口の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// 入出力とも f32 の入口。
    F32,
    /// 入出力が binary16 で累積が f32 の入口。
    F16Acc,
}

impl Entry {
    /// カーネルの入口名。
    pub const fn name(self) -> &'static str {
        match self {
            Entry::F32 => "gemm_simdgroup_tiled",
            Entry::F16Acc => "gemm_simdgroup_tiled_f16",
        }
    }
}

/// function constant の値。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstantValue {
    /// 4 バイトの符号なし整数。
    UInt(u32),
    /// 1 バイトの真偽。
    Bool(bool),
}

/// function constant の個数。index は 0 から `CONSTANT_COUNT - 1` まで。
pub const CONSTANT_COUNT: usize = 18;

/// バッファと `setBytes` の割り当ての種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferLayout {
    /// バッファ 3 つと `setBytes` 4 つ（index 0〜6 を全部設定する）。
    F32Seven,
    /// バッファ 3 つと `setBytes` 1 つ（index 0〜3 を設定する）。
    F16Four,
}

/// 出力の要素の型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    /// f32。
    F32,
    /// binary16 のビット列（`u16`）。
    Binary16,
}

/// tile の形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// 出力の行方向の tile 幅。
    pub bm: u32,
    /// 出力の列方向の tile 幅。
    pub bn: u32,
    /// 内積方向の tile 幅。
    pub bk: u32,
    /// 行方向の simdgroup 数。
    pub wm: u32,
    /// 列方向の simdgroup 数。
    pub wn: u32,
    /// threadgroup のメモリに tile を段にして置くか。
    pub staged: bool,
}

/// 1 セルの起動の表の 1 行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Launch {
    /// 入口の種類。
    pub entry: Entry,
    /// function constant の (index, 値)。
    pub constants: [(u32, ConstantValue); CONSTANT_COUNT],
    /// tile の形。
    pub tile: Tile,
    /// threadgroup あたりのスレッド数（x 方向。y・z は 1）。
    pub threads: usize,
    /// threadgroup 数の幅。
    pub groups_width: usize,
    /// threadgroup 数の高さ。
    pub groups_height: usize,
    /// threadgroup のメモリ長（index 0）。
    pub threadgroup_memory: usize,
    /// 詰めた後の M。
    pub eff_m: usize,
    /// 詰めた後の N。
    pub eff_n: usize,
    /// 詰めた後の K。
    pub eff_k: usize,
    /// バッファの割り当て。
    pub buffers: BufferLayout,
    /// 出力の型。
    pub output: OutputKind,
}

/// 行ごとの値。f32 と f16 で tile・スレッド数・threadgroup 数は同じで、メモリ長だけが違う。
struct RowSpec {
    eff: (usize, usize, usize),
    tile: Tile,
    threads: usize,
    groups: (usize, usize),
    mem_f32: usize,
    mem_f16: usize,
}

const fn tile(bm: u32, bn: u32, bk: u32, wm: u32, wn: u32, staged: bool) -> Tile {
    Tile {
        bm,
        bn,
        bk,
        wm,
        wn,
        staged,
    }
}

fn row_spec(row: GemmGateRow) -> RowSpec {
    match row {
        GemmGateRow::Sq512 => RowSpec {
            eff: (512, 512, 512),
            tile: tile(64, 32, 32, 2, 2, true),
            threads: 128,
            groups: (16, 8),
            mem_f32: 13824,
            mem_f16: 15104,
        },
        GemmGateRow::Sq2048 => RowSpec {
            eff: (2048, 2048, 2048),
            tile: tile(64, 32, 16, 2, 2, true),
            threads: 128,
            groups: (64, 32),
            mem_f32: 7424,
            mem_f16: 11904,
        },
        GemmGateRow::Sq4096 => RowSpec {
            eff: (4096, 4096, 4096),
            tile: tile(32, 64, 16, 2, 2, true),
            threads: 128,
            groups: (64, 128),
            mem_f32: 6912,
            mem_f16: 11648,
        },
        GemmGateRow::Rect4096x1024x4096 => RowSpec {
            eff: (4096, 1024, 4096),
            tile: tile(64, 32, 16, 2, 2, true),
            threads: 128,
            groups: (32, 64),
            mem_f32: 7424,
            mem_f16: 11904,
        },
        GemmGateRow::Odd37x41x53 => RowSpec {
            eff: (40, 48, 56),
            tile: tile(8, 8, 8, 1, 1, false),
            threads: 32,
            groups: (6, 5),
            mem_f32: 16,
            mem_f16: 256,
        },
    }
}

fn constants(t: Tile) -> [(u32, ConstantValue); CONSTANT_COUNT] {
    use ConstantValue::{Bool, UInt};
    [
        (0, UInt(t.bm)),
        (1, UInt(t.bn)),
        (2, UInt(t.bk)),
        (3, UInt(t.wm)),
        (4, UInt(t.wn)),
        (5, Bool(t.staged)),
        (6, UInt(if t.staged { 4 } else { 0 })),
        (7, Bool(false)),
        (8, Bool(false)),
        (9, Bool(false)),
        (10, Bool(false)),
        (11, Bool(false)),
        (12, Bool(false)),
        (13, UInt(1)),
        (14, UInt(0)),
        (15, UInt(0)),
        (16, Bool(false)),
        (17, UInt(0)),
    ]
}

/// セルの起動の表の行を返す。腕が `G6` のセルだけが `Some`、ほかの腕のセルは `None`。
pub fn launch_for(cell: GemmGateCell) -> Option<Launch> {
    if cell.arm() != GemmGateArm::G6 {
        return None;
    }
    let spec = row_spec(cell.row());
    let (entry, buffers, output, memory) = match cell.dtype() {
        GemmInputDtype::F32 => (
            Entry::F32,
            BufferLayout::F32Seven,
            OutputKind::F32,
            spec.mem_f32,
        ),
        GemmInputDtype::F16Acc => (
            Entry::F16Acc,
            BufferLayout::F16Four,
            OutputKind::Binary16,
            spec.mem_f16,
        ),
    };
    Some(Launch {
        entry,
        constants: constants(spec.tile),
        tile: spec.tile,
        threads: spec.threads,
        groups_width: spec.groups.0,
        groups_height: spec.groups.1,
        threadgroup_memory: memory,
        eff_m: spec.eff.0,
        eff_n: spec.eff.1,
        eff_k: spec.eff.2,
        buffers,
        output,
    })
}

/// 次元を 8 の倍数へ切り上げる。あふれたら [`ArmError::Overflow`]。
pub fn pad8_dim(x: usize) -> Result<usize, ArmError> {
    let up = x.checked_add(7).ok_or(ArmError::Overflow)?;
    Ok(up / 8 * 8)
}

/// 行優先の `rows × cols` を、左上に写した `eff_rows × eff_cols` の行列にする。余りは `zero` で埋める。
///
/// `src.len()` が `rows × cols` と違うとき、または実効の次元が元より小さいときは
/// [`ArmError::LengthMismatch`]。積があふれたら [`ArmError::Overflow`]。
pub fn pad_matrix<T: Copy>(
    src: &[T],
    rows: usize,
    cols: usize,
    eff_rows: usize,
    eff_cols: usize,
    zero: T,
) -> Result<Vec<T>, ArmError> {
    let expected = rows.checked_mul(cols).ok_or(ArmError::Overflow)?;
    let total = eff_rows.checked_mul(eff_cols).ok_or(ArmError::Overflow)?;
    if src.len() != expected || eff_rows < rows || eff_cols < cols {
        return Err(ArmError::LengthMismatch);
    }
    if eff_rows == rows && eff_cols == cols {
        return Ok(src.to_vec());
    }
    let mut out = vec![zero; total];
    if cols == 0 {
        return Ok(out);
    }
    for (r, line) in src.chunks_exact(cols).enumerate() {
        let start = r * eff_cols;
        out[start..start + cols].copy_from_slice(line);
    }
    Ok(out)
}

/// 行優先の `eff_rows × eff_cols` から、先頭の `rows` 行の先頭の `cols` 要素を取り出す。
///
/// `src.len()` が `eff_rows × eff_cols` と違うとき、または切り出す次元が実効より大きいときは
/// [`ArmError::LengthMismatch`]。積があふれたら [`ArmError::Overflow`]。
pub fn crop_matrix<T: Copy>(
    src: &[T],
    eff_rows: usize,
    eff_cols: usize,
    rows: usize,
    cols: usize,
) -> Result<Vec<T>, ArmError> {
    let expected = eff_rows.checked_mul(eff_cols).ok_or(ArmError::Overflow)?;
    let total = rows.checked_mul(cols).ok_or(ArmError::Overflow)?;
    if src.len() != expected || rows > eff_rows || cols > eff_cols {
        return Err(ArmError::LengthMismatch);
    }
    let mut out = Vec::with_capacity(total);
    for r in 0..rows {
        let start = r * eff_cols;
        out.extend_from_slice(&src[start..start + cols]);
    }
    Ok(out)
}
