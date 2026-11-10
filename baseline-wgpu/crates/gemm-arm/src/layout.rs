//! 行列の大きさを 8 の倍数へ詰める・詰めた C から元の大きさを切り出す・起動の引数を作る純粋関数。
//!
//! device には触れない。腕の起動側（device の経路）が、WGSL に渡す入力の列・workgroup の数・
//! 定数の束縛のバイト列をここから受け取る。長さ・次元の計算は `checked_*` で行い、あふれは `Err` で返す（panic しない）。

use fandhe_silicon_core::parity::gemm_inputs::GemmGateRow;

use crate::TILE;

/// 詰め物・切り出しの失敗。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayoutError {
    /// 次元が `u32` に収まらない、または詰めた値が `u32` をあふれる。
    DimensionOverflow,
    /// 要素数またはバイト数の計算があふれる。
    SizeOverflow,
    /// 渡された列の長さが、大きさから求めた要素数と合わない。
    LengthMismatch,
    /// 列の確保に失敗した。
    AllocationFailed,
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::DimensionOverflow => "dimension does not fit in u32",
            Self::SizeOverflow => "element count overflows usize",
            Self::LengthMismatch => "slice length does not match the layout",
            Self::AllocationFailed => "allocation failed",
        };
        f.write_str(text)
    }
}

impl std::error::Error for LayoutError {}

/// `x` を 8 の倍数へ切り上げる。`u32` をあふれるとき `None`。
pub fn padded_dim(x: u32) -> Option<u32> {
    x.checked_add(TILE - 1).map(|v| v & !(TILE - 1))
}

/// 元の `(m, n, k)` と、8 の倍数へ詰めた `(mp, np, kp)` の組。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GemmLayout {
    m: u32,
    n: u32,
    k: u32,
    mp: u32,
    np: u32,
    kp: u32,
}

impl GemmLayout {
    /// 元の大きさから作る。次元が `u32` に収まらない・詰めてあふれる・要素数があふれるとき `Err`。
    pub fn new(m: usize, n: usize, k: usize) -> Result<Self, LayoutError> {
        let to_u32 = |x: usize| u32::try_from(x).map_err(|_| LayoutError::DimensionOverflow);
        let (m, n, k) = (to_u32(m)?, to_u32(n)?, to_u32(k)?);
        let pad = |x: u32| padded_dim(x).ok_or(LayoutError::DimensionOverflow);
        let layout = Self {
            m,
            n,
            k,
            mp: pad(m)?,
            np: pad(n)?,
            kp: pad(k)?,
        };
        // 3 つの行列の要素数が usize に収まることをここで確かめる。
        layout.a_padded_len()?;
        layout.b_padded_len()?;
        layout.c_padded_len()?;
        Ok(layout)
    }

    /// 門の行から作る。
    pub fn for_row(row: GemmGateRow) -> Result<Self, LayoutError> {
        Self::new(row.m(), row.n(), row.k())
    }

    /// 元の行数。
    pub fn m(&self) -> u32 {
        self.m
    }

    /// 元の列数。
    pub fn n(&self) -> u32 {
        self.n
    }

    /// 元の内積の長さ。
    pub fn k(&self) -> u32 {
        self.k
    }

    /// 詰めた後の行数。
    pub fn mp(&self) -> u32 {
        self.mp
    }

    /// 詰めた後の列数。
    pub fn np(&self) -> u32 {
        self.np
    }

    /// 詰めた後の内積の長さ。
    pub fn kp(&self) -> u32 {
        self.kp
    }

    fn mul(a: u32, b: u32) -> Result<usize, LayoutError> {
        usize::try_from(a)
            .ok()
            .and_then(|a| usize::try_from(b).ok().and_then(|b| a.checked_mul(b)))
            .ok_or(LayoutError::SizeOverflow)
    }

    /// 詰めた後の A（Mp x Kp）の要素数。
    pub fn a_padded_len(&self) -> Result<usize, LayoutError> {
        Self::mul(self.mp, self.kp)
    }

    /// 詰めた後の B（Kp x Np）の要素数。
    pub fn b_padded_len(&self) -> Result<usize, LayoutError> {
        Self::mul(self.kp, self.np)
    }

    /// 詰めた後の C（Mp x Np）の要素数。
    pub fn c_padded_len(&self) -> Result<usize, LayoutError> {
        Self::mul(self.mp, self.np)
    }
}

/// `src`（rows x cols、行優先）を `zero` で埋めて prows x pcols にする。
fn pad_matrix<T: Copy>(
    src: &[T],
    rows: u32,
    cols: u32,
    prows: u32,
    pcols: u32,
    zero: T,
) -> Result<Vec<T>, LayoutError> {
    let expected = GemmLayout::mul(rows, cols)?;
    if src.len() != expected {
        return Err(LayoutError::LengthMismatch);
    }
    let total = GemmLayout::mul(prows, pcols)?;
    let mut out = Vec::new();
    out.try_reserve_exact(total)
        .map_err(|_| LayoutError::AllocationFailed)?;
    out.resize(total, zero);
    let (cols, pcols) = (cols as usize, pcols as usize);
    if cols > 0 {
        for (r, line) in src.chunks_exact(cols).enumerate() {
            let start = r * pcols;
            out[start..start + cols].copy_from_slice(line);
        }
    }
    Ok(out)
}

/// A（M x K の f32）を Mp x Kp へ `0.0` で詰める。長さが `m * k` と違うとき `Err`。
pub fn pad_a_f32(layout: &GemmLayout, a: &[f32]) -> Result<Vec<f32>, LayoutError> {
    pad_matrix(a, layout.m, layout.k, layout.mp, layout.kp, 0.0)
}

/// B（K x N の f32）を Kp x Np へ `0.0` で詰める。長さが `k * n` と違うとき `Err`。
pub fn pad_b_f32(layout: &GemmLayout, b: &[f32]) -> Result<Vec<f32>, LayoutError> {
    pad_matrix(b, layout.k, layout.n, layout.kp, layout.np, 0.0)
}

/// A（M x K の binary16 のビット列）を Mp x Kp へ `0x0000` で詰める。長さが `m * k` と違うとき `Err`。
pub fn pad_a_binary16(layout: &GemmLayout, a: &[u16]) -> Result<Vec<u16>, LayoutError> {
    pad_matrix(a, layout.m, layout.k, layout.mp, layout.kp, 0)
}

/// B（K x N の binary16 のビット列）を Kp x Np へ `0x0000` で詰める。長さが `k * n` と違うとき `Err`。
pub fn pad_b_binary16(layout: &GemmLayout, b: &[u16]) -> Result<Vec<u16>, LayoutError> {
    pad_matrix(b, layout.k, layout.n, layout.kp, layout.np, 0)
}

/// 詰めた後の C（Mp x Np）から元の M x N を切り出す。長さが `mp * np` と違うとき `Err`。
pub fn crop_c(layout: &GemmLayout, c_padded: &[f32]) -> Result<Vec<f32>, LayoutError> {
    if c_padded.len() != layout.c_padded_len()? {
        return Err(LayoutError::LengthMismatch);
    }
    let total = GemmLayout::mul(layout.m, layout.n)?;
    let mut out = Vec::new();
    out.try_reserve_exact(total)
        .map_err(|_| LayoutError::AllocationFailed)?;
    let (n, np) = (layout.n as usize, layout.np as usize);
    for r in 0..layout.m as usize {
        let start = r * np;
        out.extend_from_slice(&c_padded[start..start + n]);
    }
    Ok(out)
}

/// 起動する workgroup の数 `[np / 8, mp / 8, 1]`（x が列、y が行）。
pub fn workgroup_count(layout: &GemmLayout) -> [u32; 3] {
    [layout.np / TILE, layout.mp / TILE, 1]
}

/// 定数の束縛のバイト列。詰めた後の `m`・`k`・`n` をこの順に little-endian の `u32` で並べ、後ろ 4 バイトは 0。
pub fn constants_bytes(layout: &GemmLayout) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&layout.mp.to_le_bytes());
    out[4..8].copy_from_slice(&layout.kp.to_le_bytes());
    out[8..12].copy_from_slice(&layout.np.to_le_bytes());
    out
}

/// 実行時に受けた WGSL を起動する workgroup の数 `[ceil(n / 16), ceil(m / 16), 1]`（x が列、y が行）。
///
/// 詰め物はしない。`m`・`n` が `u32` に収まらないとき `Err`。
pub fn tiled_workgroup_count(m: usize, n: usize) -> Result<[u32; 3], LayoutError> {
    let to_u32 = |x: usize| u32::try_from(x).map_err(|_| LayoutError::DimensionOverflow);
    let (m, n) = (to_u32(m)?, to_u32(n)?);
    let [tx, ty, _] = crate::external::TILED_WORKGROUP_SIZE;
    Ok([n.div_ceil(tx), m.div_ceil(ty), 1])
}

/// 実行時に受けた WGSL の定数の束縛のバイト列。元の `m`・`n`・`k` をこの順に little-endian の `u32` で並べ、後ろ 4 バイトは 0。
///
/// 詰め物はしない。いずれかが `u32` に収まらないとき `Err`。
pub fn tiled_constants_bytes(m: usize, n: usize, k: usize) -> Result<[u8; 16], LayoutError> {
    let to_u32 = |x: usize| u32::try_from(x).map_err(|_| LayoutError::DimensionOverflow);
    let (m, n, k) = (to_u32(m)?, to_u32(n)?, to_u32(k)?);
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&m.to_le_bytes());
    out[4..8].copy_from_slice(&n.to_le_bytes());
    out[8..12].copy_from_slice(&k.to_le_bytes());
    Ok(out)
}
