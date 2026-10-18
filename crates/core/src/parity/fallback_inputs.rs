//! 代わりの実行の試験の入力のうち、`f16-fma` の 4 種（`E1`・`E2`・`S`・`R`）と長さ n の表（spec TASK-929 の 2/6）。
//!
//! 各経路の試験と、後段の CPU の参照（TASK-929 の 4/6）から呼ばれる。入力は固定 seed の
//! [`super::gemm_inputs::SplitMix64`] からその場で決定的に作り、ファイルには置かない。
//! `E2`・`S` は全数の列挙で乱数を使わない。長さ 0 の拒否は下-2 の入口の受け持ちで、
//! ここでは空の列を返す。`subgroup-sum`・`atomic-add` の入力と m の表は後段（TASK-929 の 3/6）。

use super::binary16::f32_to_binary16_bits;
use super::cpu_reference::ReferenceError;
use super::gemm_inputs::{GEMM_INPUT_SEED, SplitMix64};

/// 長さ n の表（13 通り）。
pub const FALLBACK_LENGTHS: [usize; 13] = [
    1,
    31,
    32,
    33,
    255,
    256,
    257,
    1000,
    65_539,
    1 << 12,
    1 << 16,
    1 << 18,
    1 << 22,
];

/// 入力の生成に使う基の seed。種類ごとの番号と長さを足して使う。
pub const FALLBACK_INPUT_SEED: u64 = GEMM_INPUT_SEED;

/// `E2` の (b, c) の組の数。
pub const E2_PAIR_COUNT: usize = 16;

/// `E2` の (b, c) の 16 組（binary16 のビット列）。
pub const E2_PAIRS: [(u16, u16); E2_PAIR_COUNT] = [
    (0x3C00, 0x0000), // (1, +0)
    (0x3C00, 0x8000), // (1, -0)
    (0xBC00, 0x0000), // (-1, +0)
    (0x3800, 0x0000), // (0.5, +0)
    (0x4000, 0x0000), // (2, +0)
    (0x4200, 0x0000), // (3, +0)
    (0x3C01, 0x0000), // (1 + 2^-10, +0)
    (0x3BFF, 0x0000), // (1 - 2^-11, +0)
    (0x3C00, 0x0001), // (1, 2^-24)
    (0x3C00, 0x8001), // (1, -2^-24)
    (0x3C00, 0x3C00), // (1, 1)
    (0x3C00, 0xBC00), // (1, -1)
    (0x6400, 0x0000), // (1024, +0)
    (0x0400, 0x0000), // (2^-14, +0)
    (0x7BFF, 0x0000), // (65504, +0)
    (0x3C00, 0x7BFF), // (1, 65504)
];

/// `E2` の要素数（65,536 × 16）。
pub const E2_LEN: usize = 65_536 * E2_PAIR_COUNT;

/// `S` の特殊値の数。
pub const S_VALUE_COUNT: usize = 15;

/// `S` の特殊値 15 値（binary16 のビット列）。
pub const S_VALUES: [u16; S_VALUE_COUNT] = [
    0x0000, // +0
    0x8000, // -0
    0x0001, // 最小の非正規化数
    0x8001, // 同 負
    0x03FF, // 最大の非正規化数
    0x83FF, // 同 負
    0x0400, // 最小の正規化数
    0x8400, // 同 負
    0x3C00, // +1
    0xBC00, // -1
    0x7BFF, // 最大の有限値
    0xFBFF, // 同 負
    0x7C00, // +Inf
    0xFC00, // -Inf
    0x7E00, // NaN
];

/// `S` の要素数（15 の 3 乗）。
pub const S_LEN: usize = S_VALUE_COUNT * S_VALUE_COUNT * S_VALUE_COUNT;

/// 入力の種類。seed の番号を種類ごとに分ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackInputKind {
    /// `f16-fma` の整数値の入力。
    F16FmaE1,
    /// `f16-fma` の全ビット列の入力。
    F16FmaE2,
    /// `f16-fma` の特殊値の全組。
    F16FmaS,
    /// `f16-fma` の有限値を一様に選んだ入力。
    F16FmaR,
}

impl FallbackInputKind {
    /// 表示用の名前。
    pub fn name(self) -> &'static str {
        match self {
            Self::F16FmaE1 => "f16-fma-e1",
            Self::F16FmaE2 => "f16-fma-e2",
            Self::F16FmaS => "f16-fma-s",
            Self::F16FmaR => "f16-fma-r",
        }
    }

    /// seed に足す種類ごとの番号。
    fn number(self) -> u64 {
        match self {
            Self::F16FmaE1 => 1,
            Self::F16FmaE2 => 2,
            Self::F16FmaS => 3,
            Self::F16FmaR => 4,
        }
    }

    /// 種類と長さ `n` から決まる seed。種類か長さが違えば値が違う。
    pub fn seed_for(self, n: usize) -> u64 {
        FALLBACK_INPUT_SEED
            .wrapping_add(self.number() << 32)
            .wrapping_add(n as u64)
    }
}

/// `f16-fma` の入力 a・b・c（binary16 のビット列。長さは揃う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct F16FmaInputs {
    /// 第 1 項。
    pub a: Vec<u16>,
    /// 第 2 項。
    pub b: Vec<u16>,
    /// 加数。
    pub c: Vec<u16>,
}

impl F16FmaInputs {
    /// 要素数。
    pub fn len(&self) -> usize {
        self.a.len()
    }

    /// 空かどうか。
    pub fn is_empty(&self) -> bool {
        self.a.is_empty()
    }
}

fn alloc(n: usize) -> Result<Vec<u16>, ReferenceError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| ReferenceError::AllocationFailed)?;
    Ok(v)
}

fn alloc3(n: usize) -> Result<F16FmaInputs, ReferenceError> {
    Ok(F16FmaInputs {
        a: alloc(n)?,
        b: alloc(n)?,
        c: alloc(n)?,
    })
}

/// `[-limit, limit]` の整数を 1 つ引き、binary16 のビット列にする（|値| は 2048 以下で正確に表せる）。
fn draw_int_bits(rng: &mut SplitMix64, limit: u64) -> u16 {
    let span = limit * 2 + 1;
    let v = (rng.next_u64() % span) as i64 - limit as i64;
    f32_to_binary16_bits(v as f32)
}

/// `E1`: a・b は -32〜32、c は -1024〜1024 の整数。a・b・c の順に n 個ずつ引く。
pub fn generate_f16_fma_e1(n: usize) -> Result<F16FmaInputs, ReferenceError> {
    let mut out = alloc3(n)?;
    let mut rng = SplitMix64::new(FallbackInputKind::F16FmaE1.seed_for(n));
    for _ in 0..n {
        out.a.push(draw_int_bits(&mut rng, 32));
    }
    for _ in 0..n {
        out.b.push(draw_int_bits(&mut rng, 32));
    }
    for _ in 0..n {
        out.c.push(draw_int_bits(&mut rng, 1024));
    }
    Ok(out)
}

/// `R`: NaN・Inf を除く有限の binary16 のビット列を一様に選ぶ（指数部が全 1 は引き直す）。
pub fn generate_f16_fma_r(n: usize) -> Result<F16FmaInputs, ReferenceError> {
    let mut out = alloc3(n)?;
    let mut rng = SplitMix64::new(FallbackInputKind::F16FmaR.seed_for(n));
    for dst in [&mut out.a, &mut out.b, &mut out.c] {
        for _ in 0..n {
            loop {
                let bits = (rng.next_u64() >> 48) as u16;
                if bits & 0x7C00 != 0x7C00 {
                    dst.push(bits);
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// `E2`: a は全 65,536 のビット列、(b, c) は [`E2_PAIRS`]。a が外側、組が内側の列挙。
pub fn generate_f16_fma_e2() -> Result<F16FmaInputs, ReferenceError> {
    let mut out = alloc3(E2_LEN)?;
    for a in 0..=u16::MAX {
        for &(b, c) in &E2_PAIRS {
            out.a.push(a);
            out.b.push(b);
            out.c.push(c);
        }
    }
    Ok(out)
}

/// `S`: [`S_VALUES`] から a・b・c の全組（添字 i に対し a = i / 225、b = (i / 15) % 15、c = i % 15）。
pub fn generate_f16_fma_s() -> Result<F16FmaInputs, ReferenceError> {
    let mut out = alloc3(S_LEN)?;
    for &a in &S_VALUES {
        for &b in &S_VALUES {
            for &c in &S_VALUES {
                out.a.push(a);
                out.b.push(b);
                out.c.push(c);
            }
        }
    }
    Ok(out)
}
