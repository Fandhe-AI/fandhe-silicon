//! 代わりの実行の試験の入力（spec TASK-929 の 2/6・3/6）。
//!
//! `f16-fma` の 4 種（`E1`・`E2`・`S`・`R`）、`subgroup-sum` の u32 と f32、`atomic-add` の
//! 入力、長さ n の表、出力の数 m の表を持つ。
//! 各経路の試験から呼ばれる。入力は固定 seed の
//! [`super::gemm_inputs::SplitMix64`] からその場で決定的に作り、ファイルには置かない。
//! `E2`・`S` は全数の列挙で乱数を使わない。長さ 0 の拒否は下-2 の入口の受け持ちで、
//! ここでは空の列を返す。f32 の生成は libm を使わず基本演算だけで作る
//! （実行する環境によらず同じ列になる）。

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

/// `subgroup-sum` の f32 の `FS` で型を割り当てるブロックの大きさ（要素数）。
pub const SUBGROUP_SUM_BLOCK: usize = 256;

/// `atomic-add` の出力の要素数 m の表。
pub const ATOMIC_ADD_OUTPUT_COUNTS: [usize; 3] = [1, 64, 4096];

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
    /// `subgroup-sum` の u32 の一様な乱数。
    SubgroupSumU32U,
    /// `subgroup-sum` の f32 の整数値（−256〜256）。
    SubgroupSumF32Fx,
    /// `subgroup-sum` の f32 の対数一様な大きさの有限値。
    SubgroupSumF32Fg,
    /// `subgroup-sum` の f32 の、ブロックごとに 8 種の型を置く入力。
    SubgroupSumF32Fs,
    /// `atomic-add` の整数値（−256〜256）。
    AtomicAddAx,
    /// `atomic-add` の対数一様な大きさの有限値。
    AtomicAddAg,
    /// `atomic-add` の、出力の要素ごとに 8 種の型を置く入力。
    AtomicAddAs,
}

impl FallbackInputKind {
    /// 表示用の名前。
    pub fn name(self) -> &'static str {
        match self {
            Self::F16FmaE1 => "f16-fma-e1",
            Self::F16FmaE2 => "f16-fma-e2",
            Self::F16FmaS => "f16-fma-s",
            Self::F16FmaR => "f16-fma-r",
            Self::SubgroupSumU32U => "subgroup-sum-u32-u",
            Self::SubgroupSumF32Fx => "subgroup-sum-f32-fx",
            Self::SubgroupSumF32Fg => "subgroup-sum-f32-fg",
            Self::SubgroupSumF32Fs => "subgroup-sum-f32-fs",
            Self::AtomicAddAx => "atomic-add-ax",
            Self::AtomicAddAg => "atomic-add-ag",
            Self::AtomicAddAs => "atomic-add-as",
        }
    }

    /// seed に足す種類ごとの番号。
    fn number(self) -> u64 {
        match self {
            Self::F16FmaE1 => 1,
            Self::F16FmaE2 => 2,
            Self::F16FmaS => 3,
            Self::F16FmaR => 4,
            Self::SubgroupSumU32U => 5,
            Self::SubgroupSumF32Fx => 6,
            Self::SubgroupSumF32Fg => 7,
            Self::SubgroupSumF32Fs => 8,
            Self::AtomicAddAx => 9,
            Self::AtomicAddAg => 10,
            Self::AtomicAddAs => 11,
        }
    }

    /// 種類と長さ `n` から決まる seed。
    ///
    /// 種類か長さが違えば値が違うことは、[`FALLBACK_LENGTHS`] の長さの範囲でだけ保証する。
    /// 任意の `usize` では 64 bit 環境で衝突しうる（検証はしない）。
    pub fn seed_for(self, n: usize) -> u64 {
        FALLBACK_INPUT_SEED
            .wrapping_add(self.number() << 32)
            .wrapping_add(n as u64)
    }

    /// 出力の要素数 m にも依る入力（`AS`）の seed。[`Self::seed_for`] に m を上位の桁で足す。
    ///
    /// 一意は [`FALLBACK_LENGTHS`] と [`ATOMIC_ADD_OUTPUT_COUNTS`] の範囲でだけ保証する。
    pub fn seed_for_outputs(self, n: usize, m: usize) -> u64 {
        self.seed_for(n).wrapping_add((m as u64) << 40)
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

fn alloc<T>(n: usize) -> Result<Vec<T>, ReferenceError> {
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
/// `AX` で使える (n, m) の組か。出力の要素ごとの和の上限 256 × ⌈n ÷ m⌉ が 2^24 未満なら真。
/// m = 0 と、計算が桁あふれする組は偽。
pub fn ax_pair_allowed(n: usize, m: usize) -> bool {
    if m == 0 {
        return false;
    }
    n.div_ceil(m)
        .checked_mul(256)
        .is_some_and(|bound| bound < (1 << 24))
}

/// `[-limit, limit]` の整数を 1 つ引き、f32 にする。
fn draw_int_f32(rng: &mut SplitMix64, limit: u64) -> f32 {
    let span = limit * 2 + 1;
    ((rng.next_u64() % span) as i64 - limit as i64) as f32
}

/// 2^f（f は [0, 1)）を、ln2 の定数と固定項数のテイラー展開で求める。
/// 加算と乗算だけを使うので、libm に依らず実行する環境によらず同じ値になる。
fn exp2_unit(f: f64) -> f64 {
    const LN2: f64 = std::f64::consts::LN_2;
    let x = f * LN2;
    let mut term = 1.0_f64;
    let mut sum = 1.0_f64;
    for k in 1..=25 {
        term = term * x / k as f64;
        sum += term;
    }
    sum
}

/// 符号は一様、大きさは 2^k（k は [−20, 20) の一様な実数）の有限の f32 を 1 つ引く。
/// 引く順は、k 用の 1 回、符号用の 1 回。
fn draw_log_uniform_f32(rng: &mut SplitMix64) -> f32 {
    let u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    let k = -20.0 + 40.0 * u;
    let e = k.floor();
    let frac = k - e;
    let scale = f64::from_bits(((e as i64 + 1023) as u64) << 52);
    let mag = (exp2_unit(frac) * scale) as f32;
    if rng.next_u64() >> 63 == 1 { -mag } else { mag }
}

/// 符号 1 bit・指数部 0・仮数 1〜2^23−1 の一様な非正規化数を 1 つ引く。
fn draw_subnormal_f32(rng: &mut SplitMix64) -> f32 {
    let r = rng.next_u64();
    let mantissa = 1 + ((r >> 32) as u32 % ((1 << 23) - 1));
    f32::from_bits((((r >> 63) as u32) << 31) | mantissa)
}

/// `U`: u32 の全範囲の一様な乱数を n 個。
pub fn generate_subgroup_sum_u32_u(n: usize) -> Result<Vec<u32>, ReferenceError> {
    let mut out = alloc(n)?;
    let mut rng = SplitMix64::new(FallbackInputKind::SubgroupSumU32U.seed_for(n));
    for _ in 0..n {
        out.push((rng.next_u64() >> 32) as u32);
    }
    Ok(out)
}

fn generate_int_f32(kind: FallbackInputKind, n: usize) -> Result<Vec<f32>, ReferenceError> {
    let mut out = alloc(n)?;
    let mut rng = SplitMix64::new(kind.seed_for(n));
    for _ in 0..n {
        out.push(draw_int_f32(&mut rng, 256));
    }
    Ok(out)
}

fn generate_log_uniform_f32(kind: FallbackInputKind, n: usize) -> Result<Vec<f32>, ReferenceError> {
    let mut out = alloc(n)?;
    let mut rng = SplitMix64::new(kind.seed_for(n));
    for _ in 0..n {
        out.push(draw_log_uniform_f32(&mut rng));
    }
    Ok(out)
}

/// `FX`: −256〜256 の整数値の f32 を n 個。
pub fn generate_subgroup_sum_f32_fx(n: usize) -> Result<Vec<f32>, ReferenceError> {
    generate_int_f32(FallbackInputKind::SubgroupSumF32Fx, n)
}

/// `FG`: 符号は一様、大きさは 2^k（k は [−20, 20) の一様な実数）の f32 を n 個。|x| は 2^20 以下。
pub fn generate_subgroup_sum_f32_fg(n: usize) -> Result<Vec<f32>, ReferenceError> {
    generate_log_uniform_f32(FallbackInputKind::SubgroupSumF32Fg, n)
}

/// `AX`: `FX` と同じ分布（−256〜256 の整数値）。x は m によらず n だけで決まる。
/// 使える組は [`ax_pair_allowed`] で選ぶ。
pub fn generate_atomic_add_ax(n: usize) -> Result<Vec<f32>, ReferenceError> {
    generate_int_f32(FallbackInputKind::AtomicAddAx, n)
}

/// `AG`: `FG` と同じ分布（seed は別）。x は m によらず n だけで決まる。
pub fn generate_atomic_add_ag(n: usize) -> Result<Vec<f32>, ReferenceError> {
    generate_log_uniform_f32(FallbackInputKind::AtomicAddAg, n)
}

/// 要素をクラスに分ける割り付け。クラスごとに 8 種の型のうち `クラス番号 % 8` を置く。
#[derive(Clone, Copy)]
enum ClassLayout {
    /// 連続する [`SUBGROUP_SUM_BLOCK`] 個を 1 クラスとする。
    Block,
    /// `i % m` が同じ要素を 1 クラスとする。
    Strided(usize),
}

impl ClassLayout {
    /// 長さ n に対するクラスの数。
    fn class_count(self, n: usize) -> usize {
        match self {
            Self::Block => n.div_ceil(SUBGROUP_SUM_BLOCK),
            Self::Strided(m) => n.min(m),
        }
    }

    /// 要素 `i` の (クラス番号, クラス内の順番)。
    fn locate(self, i: usize) -> (usize, usize) {
        match self {
            Self::Block => (i / SUBGROUP_SUM_BLOCK, i % SUBGROUP_SUM_BLOCK),
            Self::Strided(m) => (i % m, i / m),
        }
    }

    /// クラス `j` の順番 `r` の要素の添字。
    fn position(self, j: usize, r: usize) -> usize {
        match self {
            Self::Block => j * SUBGROUP_SUM_BLOCK + r,
            Self::Strided(m) => j + r * m,
        }
    }

    /// クラス `j`（`j < class_count(n)`）の要素数。
    fn class_len(self, n: usize, j: usize) -> usize {
        match self {
            Self::Block => SUBGROUP_SUM_BLOCK.min(n - j * SUBGROUP_SUM_BLOCK),
            Self::Strided(m) => (n - j).div_ceil(m),
        }
    }
}

/// `FS` と `AS` の共通の生成。引く順は、まず添字 0..n の各要素の値、その後クラス 0.. の特殊値の位置。
///
/// 型は `クラス番号 % 8`: (0) NaN を 1 つ、(1) +Inf を 1 つ、(2) −Inf を 1 つ、
/// (3) +Inf と −Inf を 1 つずつ（クラスの長さが 1 なら +Inf だけ）、(4) すべて −0.0、
/// (5) +0.0 と −0.0 の混在、(6) すべて非正規化数、(7) 順番が偶数の要素が非正規化数で奇数が正規化数。
/// 特殊値以外の要素は `FG` と同じ作り方の有限値。
fn generate_class_patterns(
    seed: u64,
    n: usize,
    layout: ClassLayout,
) -> Result<Vec<f32>, ReferenceError> {
    let mut out = alloc(n)?;
    let mut rng = SplitMix64::new(seed);
    for i in 0..n {
        let (j, r) = layout.locate(i);
        let v = match j % 8 {
            0..=3 => draw_log_uniform_f32(&mut rng),
            4 => -0.0,
            5 => {
                if rng.next_u64() >> 63 == 1 {
                    -0.0
                } else {
                    0.0
                }
            }
            6 => draw_subnormal_f32(&mut rng),
            _ => {
                if r % 2 == 0 {
                    draw_subnormal_f32(&mut rng)
                } else {
                    draw_log_uniform_f32(&mut rng).abs()
                }
            }
        };
        out.push(v);
    }
    for j in 0..layout.class_count(n) {
        let len = layout.class_len(n, j);
        let special = match j % 8 {
            0 => Some(f32::from_bits(0x7FC0_0000)),
            1 | 3 => Some(f32::INFINITY),
            2 => Some(f32::NEG_INFINITY),
            _ => None,
        };
        let Some(value) = special else { continue };
        let r = (rng.next_u64() % len as u64) as usize;
        out[layout.position(j, r)] = value;
        if j % 8 == 3 && len >= 2 {
            let s = (rng.next_u64() % (len as u64 - 1)) as usize;
            out[layout.position(j, (r + 1 + s) % len)] = f32::NEG_INFINITY;
        }
    }
    Ok(out)
}

/// `FS`: [`SUBGROUP_SUM_BLOCK`] 個のブロックごとに、ブロック番号 % 8 の型を置く
/// （型と引く順は内部の共通の生成の規則）。
pub fn generate_subgroup_sum_f32_fs(n: usize) -> Result<Vec<f32>, ReferenceError> {
    let kind = FallbackInputKind::SubgroupSumF32Fs;
    generate_class_patterns(kind.seed_for(n), n, ClassLayout::Block)
}

/// `AS`: 出力の要素ごと（`i % m` が同じ要素の組ごと）に、出力の要素番号 % 8 の型を置く。
/// n 未満の添字を受けない出力の要素には何も置かない。m = 0 は空の列を返す。
pub fn generate_atomic_add_as(n: usize, m: usize) -> Result<Vec<f32>, ReferenceError> {
    if m == 0 {
        return Ok(Vec::new());
    }
    let kind = FallbackInputKind::AtomicAddAs;
    generate_class_patterns(kind.seed_for_outputs(n, m), n, ClassLayout::Strided(m))
}
