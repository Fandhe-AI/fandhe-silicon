//! 結果の一致の試験の共通の入力: GEMM の 13 ケースと既存の 4 入力の計 17 ケース（spec TASK-977 の 2/4）。
//!
//! 各腕（背後実装ごと）の試験と、後段の CPU 参照の出力の作成から呼ばれる。入力は式から
//! その場で決定的に作り、ファイルには置かない。確保の失敗と次元の積の溢れは
//! [`ReferenceError`] で返し、panic しない。対象の項目は PARITY-4（ポインタのみ）。
//! 参照の出力はここでは作らない（呼び出し側が [`super::cpu_reference`] に通す）。

use super::cpu_reference::ReferenceError;

/// 17 ケースの識別子。順は G-01〜G-13、E-1〜E-4。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum CaseId {
    G01,
    G02,
    G03,
    G04,
    G05,
    G06,
    G07,
    G08,
    G09,
    G10,
    G11,
    G12,
    G13,
    E1,
    E2,
    E3,
    E4,
}

/// 出力の要素の設計上の区分。零は符号を区別しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputClass {
    /// NaN。
    Nan,
    /// 正の無限大。
    PosInf,
    /// 負の無限大。
    NegInf,
    /// 零（+0.0 と -0.0）。
    Zero,
    /// 零でない有限値。
    NonzeroFinite,
}

/// 値を [`OutputClass`] に分類する。
pub fn classify(x: f32) -> OutputClass {
    if x.is_nan() {
        OutputClass::Nan
    } else if x == f32::INFINITY {
        OutputClass::PosInf
    } else if x == f32::NEG_INFINITY {
        OutputClass::NegInf
    } else if x == 0.0 {
        OutputClass::Zero
    } else {
        OutputClass::NonzeroFinite
    }
}

/// GEMM のケース。A は m×k、B は k×n（いずれも行優先）。
#[derive(Debug, Clone, PartialEq)]
pub struct GemmCase {
    m: usize,
    k: usize,
    n: usize,
    a: Vec<f32>,
    b: Vec<f32>,
    design: Vec<OutputClass>,
}

impl GemmCase {
    /// A・C の行数。
    pub fn m(&self) -> usize {
        self.m
    }
    /// A の列数と B の行数。
    pub fn k(&self) -> usize {
        self.k
    }
    /// B・C の列数。
    pub fn n(&self) -> usize {
        self.n
    }
    /// A（m×k、行優先）。
    pub fn a(&self) -> &[f32] {
        &self.a
    }
    /// B（k×n、行優先）。
    pub fn b(&self) -> &[f32] {
        &self.b
    }
    /// 出力の要素ごとの設計の区分（m×n、行優先）。
    pub fn design(&self) -> &[OutputClass] {
        &self.design
    }
}

/// ケースの入力。
#[derive(Debug, Clone, PartialEq)]
pub enum CaseInput {
    /// GEMM。
    Gemm(GemmCase),
    /// 要素ごとの Relu の入力。
    Relu(Vec<f32>),
    /// Top-k のスコアと k。
    TopK {
        /// スコア。
        scores: Vec<f32>,
        /// 取り出す件数。
        k: usize,
    },
}

impl CaseInput {
    /// 処理する出力の要素数。GEMM は `m*n`、Relu は入力の長さ、Top-k は `min(k, 有限のスコアの件数)`。
    pub fn output_len(&self) -> Result<usize, ReferenceError> {
        match self {
            Self::Gemm(g) => g.m.checked_mul(g.n).ok_or(ReferenceError::SizeOverflow),
            Self::Relu(v) => Ok(v.len()),
            Self::TopK { scores, k } => {
                Ok((*k).min(scores.iter().filter(|x| x.is_finite()).count()))
            }
        }
    }
}

fn vec_with(len: usize) -> Result<Vec<f32>, ReferenceError> {
    let mut v = Vec::new();
    v.try_reserve_exact(len)?;
    Ok(v)
}

/// 37×53×41 の基の A（通し番号 i の式）。
fn base_a(i: u64) -> f32 {
    (i % 13) as f32 * 0.37 - 2.0
}

/// 37×53×41 の基の B。
fn base_b(i: u64) -> f32 {
    (i % 11) as f32 * 0.29 - 1.5
}

/// m=1・n=1 の小さなケース。設計の区分は出力 1 つ分。
fn tiny(a: &[f32], b: &[f32], class: OutputClass) -> Result<CaseInput, ReferenceError> {
    let mut av = vec_with(a.len())?;
    av.extend_from_slice(a);
    let mut bv = vec_with(b.len())?;
    bv.extend_from_slice(b);
    let mut design = Vec::new();
    design.try_reserve_exact(1)?;
    design.push(class);
    Ok(CaseInput::Gemm(GemmCase {
        m: 1,
        k: a.len(),
        n: 1,
        a: av,
        b: bv,
        design,
    }))
}

const INF: f32 = f32::INFINITY;
const NINF: f32 = f32::NEG_INFINITY;
const NAN: f32 = f32::NAN;

/// 37×53×41 の GEMM。`replaced` が真なら A の行 0〜4 を特殊値で置き換える（G-12）。
fn gemm_37(replaced: bool) -> Result<CaseInput, ReferenceError> {
    const M: usize = 37;
    const K: usize = 53;
    const N: usize = 41;
    let mut a = vec_with(M * K)?;
    a.extend((0..(M * K) as u64).map(base_a));
    let mut b = vec_with(K * N)?;
    b.extend((0..(K * N) as u64).map(base_b));
    let mut design = Vec::new();
    design.try_reserve_exact(M * N)?;
    design.resize(M * N, OutputClass::NonzeroFinite);
    if replaced {
        let set = |a: &mut [f32], row: usize, col: usize, v: f32| {
            if let Some(x) = a.get_mut(row * K + col) {
                *x = v;
            }
        };
        set(&mut a, 0, 0, NAN);
        set(&mut a, 1, 1, INF);
        set(&mut a, 2, 2, NINF);
        for col in 0..K {
            set(&mut a, 3, col, -0.0);
            set(&mut a, 4, col, 0.0);
        }
        for (idx, class) in design.iter_mut().enumerate() {
            let (row, col) = (idx / N, idx % N);
            *class = match row {
                0 => OutputClass::Nan,
                1 | 2 => {
                    // 行 1 は +Inf、行 2 は -Inf が A に入るので、B の符号で結果の無限大の向きが決まる。
                    let b_positive = b.get(row * N + col).is_some_and(|x| *x > 0.0);
                    if b_positive == (row == 1) {
                        OutputClass::PosInf
                    } else {
                        OutputClass::NegInf
                    }
                }
                3 | 4 => OutputClass::Zero,
                _ => OutputClass::NonzeroFinite,
            };
        }
    }
    Ok(CaseInput::Gemm(GemmCase {
        m: M,
        k: K,
        n: N,
        a,
        b,
        design,
    }))
}

impl CaseId {
    /// 17 ケースすべて（G-01〜G-13、E-1〜E-4 の順）。
    pub const ALL: [CaseId; 17] = [
        Self::G01,
        Self::G02,
        Self::G03,
        Self::G04,
        Self::G05,
        Self::G06,
        Self::G07,
        Self::G08,
        Self::G09,
        Self::G10,
        Self::G11,
        Self::G12,
        Self::G13,
        Self::E1,
        Self::E2,
        Self::E3,
        Self::E4,
    ];

    /// 小文字の識別名（例 `"g-01"`、`"e-1"`）。試験の枠の識別子に使える文字だけで作る。
    pub fn name(self) -> &'static str {
        match self {
            Self::G01 => "g-01",
            Self::G02 => "g-02",
            Self::G03 => "g-03",
            Self::G04 => "g-04",
            Self::G05 => "g-05",
            Self::G06 => "g-06",
            Self::G07 => "g-07",
            Self::G08 => "g-08",
            Self::G09 => "g-09",
            Self::G10 => "g-10",
            Self::G11 => "g-11",
            Self::G12 => "g-12",
            Self::G13 => "g-13",
            Self::E1 => "e-1",
            Self::E2 => "e-2",
            Self::E3 => "e-3",
            Self::E4 => "e-4",
        }
    }

    /// ケースの入力をその場で決定的に作る。
    pub fn input(self) -> Result<CaseInput, ReferenceError> {
        use OutputClass as C;
        match self {
            Self::G01 => tiny(&[NAN, 1.0, 2.0, 3.0], &[1.0; 4], C::Nan),
            Self::G02 => tiny(&[INF, 1.0, 2.0, 3.0], &[1.0; 4], C::PosInf),
            Self::G03 => tiny(&[NINF, 1.0, 2.0, 3.0], &[1.0; 4], C::NegInf),
            Self::G04 => tiny(&[INF, 1.0, 2.0, 3.0], &[-1.0, 1.0, 1.0, 1.0], C::NegInf),
            Self::G05 => tiny(&[INF, NINF, 1.0, 2.0], &[1.0; 4], C::Nan),
            Self::G06 => tiny(&[0.0, 1.0, 2.0, 3.0], &[INF, 1.0, 1.0, 1.0], C::Nan),
            Self::G07 => tiny(&[-0.0, 1.0, 2.0, 3.0], &[INF, 1.0, 1.0, 1.0], C::Nan),
            Self::G08 => tiny(&[0.0; 4], &[1.0, 2.0, 3.0, 4.0], C::Zero),
            Self::G09 => tiny(&[-0.0; 4], &[1.0, 2.0, 3.0, 4.0], C::Zero),
            Self::G10 => tiny(&[-0.0], &[1.0], C::Zero),
            Self::G11 => tiny(
                &[-0.0, 1.5, 0.0, -2.0],
                &[3.0, 2.0, 5.0, 1.0],
                C::NonzeroFinite,
            ),
            Self::G12 => gemm_37(true),
            Self::G13 => tiny(&[1.0, 2.0, 3.0, 4.0], &[NAN, 1.0, 1.0, 1.0], C::Nan),
            Self::E1 => gemm_37(false),
            Self::E2 => {
                let mut v = vec_with(37)?;
                v.extend((0..37usize).map(|i| match i % 6 {
                    0 => NAN,
                    1 => INF,
                    2 => NINF,
                    3 => -0.0,
                    4 => 0.0,
                    _ => (i as f32) * 0.5 - 10.0,
                }));
                Ok(CaseInput::Relu(v))
            }
            Self::E3 => {
                let mut scores = vec_with(37)?;
                scores.extend((0..37usize).map(|i| ((i * 17) % 37) as f32 - 37.0 / 2.0));
                Ok(CaseInput::TopK { scores, k: 5 })
            }
            Self::E4 => {
                let mut scores = vec_with(16)?;
                scores.resize(16, 1.0);
                for (slot, v) in scores.iter_mut().zip([NAN, INF, NINF, -0.0, 0.0]) {
                    *slot = v;
                }
                Ok(CaseInput::TopK { scores, k: 8 })
            }
        }
    }
}
