//! GEMM の門の run に渡す入力と、比べる相手の CPU 参照（spec TASK-1005 の 2/6）。
//!
//! 門の run の枠（後段）と、各腕の試験から呼ばれる。入力は固定 seed の SplitMix64 から
//! その場で決定的に作り、ファイルには置かない。参照は [`super::cpu_reference`] の
//! `reference_gemm_f32` に通すだけで、累積の手順をここでは新しく書かない。
//! 判定は [`super::tolerance`] の `compare_f32` にそのまま渡せる f32 の列で返す。
//! 累積の順を `k` の降順にした対照の参照は `reference_for_descending_k`（TASK-1005 の 4/6。判定には使わない）。

use super::binary16::{round_slice_to_binary16, widen_binary16_slice};
use super::cpu_reference::{
    ReferenceError, reference_gemm_f16acc, reference_gemm_f32, reference_gemm_f32_descending_k,
};

/// SplitMix64 の加算の定数（状態を 1 回進める幅）。
const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// 入力の生成に使う固定の seed。
pub const GEMM_INPUT_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// SplitMix64 の乱数生成器。入力の生成だけが使う。
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// `seed` から生成器を作る。
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// 次の 64 bit を返す。
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `n` 回 [`Self::next_u64`] を呼んだのと同じ状態へ、一定の手数で進める。
    pub fn skip(&mut self, n: u64) {
        self.state = self.state.wrapping_add(n.wrapping_mul(GAMMA));
    }

    /// 上位 24 bit から `[-1, 1)` の f32 を 1 つ作る（値は 2^-23 の倍数）。
    ///
    /// 演算の並び（24 bit を f32 にして 2^24 で割り、2 倍して 1 を引く）は値を決めるので変えない。
    pub fn next_signed_unit_f32(&mut self) -> f32 {
        let bits = (self.next_u64() >> 40) as u32;
        let unit = bits as f32 / (1u32 << 24) as f32;
        unit * 2.0 - 1.0
    }
}

/// 門の run の行。表記は M×N×K で、A は M×K、B は K×N、C は M×N（いずれも行優先）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GemmGateRow {
    /// 512×512×512。
    Sq512,
    /// 2048×2048×2048。
    Sq2048,
    /// 4096×4096×4096。
    Sq4096,
    /// 4096×1024×4096（M×N×K）。
    Rect4096x1024x4096,
    /// 37×41×53。
    Odd37x41x53,
}

/// 行ごとの定数（M, N, K, A の位置, B の位置）。位置は生成の並びの中で、その入力の先頭の値を
/// 取る前に消費済みの `next_u64` の回数（並びの定義は spec TASK-1005 のポインタ）。
struct RowSpec {
    name: &'static str,
    m: usize,
    n: usize,
    k: usize,
    a_offset: u64,
    b_offset: u64,
}

impl GemmGateRow {
    /// 5 つの行すべて。
    pub const ALL: [GemmGateRow; 5] = [
        Self::Sq512,
        Self::Sq2048,
        Self::Sq4096,
        Self::Rect4096x1024x4096,
        Self::Odd37x41x53,
    ];

    fn spec(self) -> RowSpec {
        match self {
            Self::Sq512 => RowSpec {
                name: "sq512",
                m: 512,
                n: 512,
                k: 512,
                a_offset: 3_703_729,
                b_offset: 3_965_873,
            },
            Self::Sq2048 => RowSpec {
                name: "sq2048",
                m: 2048,
                n: 2048,
                k: 2048,
                a_offset: 4_228_017,
                b_offset: 8_422_321,
            },
            Self::Sq4096 => RowSpec {
                name: "sq4096",
                m: 4096,
                n: 4096,
                k: 4096,
                a_offset: 32_520_606,
                b_offset: 49_297_822,
            },
            Self::Rect4096x1024x4096 => RowSpec {
                name: "rect4096x1024x4096",
                m: 4096,
                n: 1024,
                k: 4096,
                a_offset: 66_075_038,
                b_offset: 82_852_254,
            },
            Self::Odd37x41x53 => RowSpec {
                name: "odd37x41x53",
                m: 37,
                n: 41,
                k: 53,
                a_offset: 139_264,
                b_offset: 141_225,
            },
        }
    }

    /// 行の識別名（例 `"sq512"`）。
    pub fn name(self) -> &'static str {
        self.spec().name
    }

    /// A・C の行数。
    pub fn m(self) -> usize {
        self.spec().m
    }

    /// B・C の列数。
    pub fn n(self) -> usize {
        self.spec().n
    }

    /// 内積の長さ（A の列数・B の行数）。
    pub fn k(self) -> usize {
        self.spec().k
    }

    /// A の要素数（M×K）。溢れたら [`ReferenceError::SizeOverflow`]。
    pub fn a_len(self) -> Result<usize, ReferenceError> {
        let s = self.spec();
        s.m.checked_mul(s.k).ok_or(ReferenceError::SizeOverflow)
    }

    /// B の要素数（K×N）。溢れたら [`ReferenceError::SizeOverflow`]。
    pub fn b_len(self) -> Result<usize, ReferenceError> {
        let s = self.spec();
        s.k.checked_mul(s.n).ok_or(ReferenceError::SizeOverflow)
    }

    /// A の先頭の値を取る前に消費済みの `next_u64` の回数。
    pub fn a_offset(self) -> u64 {
        self.spec().a_offset
    }

    /// B の先頭の値を取る前に消費済みの `next_u64` の回数。
    pub fn b_offset(self) -> u64 {
        self.spec().b_offset
    }
}

/// 入力の dtype。`F32` は A・B とも f32、`F16acc` は A・B を binary16 へ 1 回丸めた値（累積は f32）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GemmInputDtype {
    /// A・B とも f32。
    F32,
    /// A・B を f32 の入力から binary16 へ 1 回だけ丸めた値。累積は f32。
    F16Acc,
}

impl GemmInputDtype {
    /// dtype の識別名（`"f32"`・`"f16acc"`）。
    pub fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F16Acc => "f16acc",
        }
    }
}

/// 位置 `offset` から `len` 個の値を、固定 seed の生成器で作る。
fn generate_values(offset: u64, len: usize) -> Result<Vec<f32>, ReferenceError> {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    rng.skip(offset);
    let mut out = Vec::new();
    out.try_reserve_exact(len)?;
    for _ in 0..len {
        out.push(rng.next_signed_unit_f32());
    }
    Ok(out)
}

/// f32 の入力（A・B）。
#[derive(Debug, Clone, PartialEq)]
pub struct GemmInputsF32 {
    a: Vec<f32>,
    b: Vec<f32>,
}

impl GemmInputsF32 {
    /// A（M×K、行優先）。
    pub fn a(&self) -> &[f32] {
        &self.a
    }

    /// B（K×N、行優先）。
    pub fn b(&self) -> &[f32] {
        &self.b
    }

    /// f32 の入力から binary16 へ 1 回だけ丸めた入力を作る（二重の丸めを経ない）。
    pub fn to_binary16(&self) -> Result<GemmInputsBinary16, ReferenceError> {
        Ok(GemmInputsBinary16 {
            a: round_slice_to_binary16(&self.a)?,
            b: round_slice_to_binary16(&self.b)?,
        })
    }
}

/// binary16 のビット列の入力（A・B）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GemmInputsBinary16 {
    a: Vec<u16>,
    b: Vec<u16>,
}

impl GemmInputsBinary16 {
    /// A（M×K、行優先）のビット列。
    pub fn a(&self) -> &[u16] {
        &self.a
    }

    /// B（K×N、行優先）のビット列。
    pub fn b(&self) -> &[u16] {
        &self.b
    }
}

/// `row` の f32 の入力を作る。確保に失敗したら [`ReferenceError::AllocationFailed`]。
pub fn generate_f32(row: GemmGateRow) -> Result<GemmInputsF32, ReferenceError> {
    Ok(GemmInputsF32 {
        a: generate_values(row.a_offset(), row.a_len()?)?,
        b: generate_values(row.b_offset(), row.b_len()?)?,
    })
}

/// 行の CPU 参照（M×N、行優先、f32）。
///
/// 「行ごとに 1 回作る」は呼び出し側が結果を持ち回る形で表す。内部ではキャッシュしない。
#[derive(Debug, Clone, PartialEq)]
pub struct GemmGateReference {
    values: Vec<f32>,
}

impl GemmGateReference {
    /// 参照の値（`compare_f32` の参照側にそのまま渡せる）。
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// 出力が binary16 の腕のために、参照を binary16 へ 1 回だけ丸めたビット列を返す。
    pub fn rounded_to_binary16(&self) -> Result<Vec<u16>, ReferenceError> {
        round_slice_to_binary16(&self.values)
    }

    /// [`Self::rounded_to_binary16`] を f32 へ広げた列（広げた列どうしで比べる用）。
    pub fn rounded_to_binary16_widened(&self) -> Result<Vec<f32>, ReferenceError> {
        widen_binary16_slice(&self.rounded_to_binary16()?)
    }
}

/// 行・dtype・入力から CPU 参照を作る。
///
/// `F32` は f32 の入力をそのまま、`F16Acc` は binary16 の入力を広げて f32 で累積する。
/// 入力は `inputs` の f32 から作る（`F16Acc` では内部で 1 回だけ丸める）。
pub fn reference_for(
    row: GemmGateRow,
    dtype: GemmInputDtype,
    inputs: &GemmInputsF32,
) -> Result<GemmGateReference, ReferenceError> {
    let values = match dtype {
        GemmInputDtype::F32 => {
            reference_gemm_f32(inputs.a(), inputs.b(), row.m(), row.k(), row.n())?
        }
        GemmInputDtype::F16Acc => {
            let half = inputs.to_binary16()?;
            reference_gemm_f16acc(half.a(), half.b(), row.m(), row.k(), row.n())?
        }
    };
    Ok(GemmGateReference { values })
}

/// [`reference_for`] と同じ形で、累積の順だけを `k` の降順にした対照の参照を作る（TASK-1005 の 4/6）。
///
/// 判定には使わない。`F16Acc` の入力の丸め（f32 から binary16 へ 1 回）は [`reference_for`] と同じ。
/// 返す [`GemmGateReference`] は `compare_f32` にそのまま渡せる。
pub fn reference_for_descending_k(
    row: GemmGateRow,
    dtype: GemmInputDtype,
    inputs: &GemmInputsF32,
) -> Result<GemmGateReference, ReferenceError> {
    let values = match dtype {
        GemmInputDtype::F32 => {
            reference_gemm_f32_descending_k(inputs.a(), inputs.b(), row.m(), row.k(), row.n())?
        }
        GemmInputDtype::F16Acc => {
            let half = inputs.to_binary16()?;
            let a = widen_binary16_slice(half.a())?;
            let b = widen_binary16_slice(half.b())?;
            reference_gemm_f32_descending_k(&a, &b, row.m(), row.k(), row.n())?
        }
    };
    Ok(GemmGateReference { values })
}
