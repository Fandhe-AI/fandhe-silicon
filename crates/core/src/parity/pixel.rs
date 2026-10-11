//! 画素の一致を確かめる道具の、画像の入力の受け口と比べる関数・閾値の設定（spec TASK-26 の 3/6・4/6）。
//!
//! 背後実装の出力と参照を画素で比べる側（検証用のテスト・測定）から呼ばれる。
//! feature `test-support` の裏にだけ公開される。標準ライブラリだけで書き、確保はしない。
//! 置き場・feature 名・入力の形は未承認の仮置き（`docs/pixel-parity-tool.md` の Q1〜Q8・Q10）。
//! spec との対応は `docs/spec/05-tasks.md` の TASK-26、`docs/spec/02-poc-plan.md` の T-3 のポインタのみ。

use std::fmt;

/// 1 画素あたりの要素数。要素型は `u8`（未承認の仮置き。`docs/pixel-parity-tool.md` の Q10）。
pub const CHANNELS_PER_PIXEL: usize = 4;

/// 画像の入力の検査で返るエラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PixelInputError {
    /// 幅か高さが 0。
    ZeroSize {
        /// 渡された幅。
        width: usize,
        /// 渡された高さ。
        height: usize,
    },
    /// 幅 × 高さ × [`CHANNELS_PER_PIXEL`] が `usize` に収まらない。
    SizeOverflow,
    /// 列の長さが幅・高さから求めた要素数と違う。
    LengthMismatch {
        /// 幅・高さから求めた要素数。
        expected: usize,
        /// 渡された列の長さ。
        actual: usize,
    },
}

impl fmt::Display for PixelInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSize { width, height } => {
                write!(
                    f,
                    "image size must be non-zero (width={width}, height={height})"
                )
            }
            Self::SizeOverflow => write!(f, "image size overflows usize"),
            Self::LengthMismatch { expected, actual } => {
                write!(
                    f,
                    "pixel data length mismatch (expected={expected}, actual={actual})"
                )
            }
        }
    }
}

impl std::error::Error for PixelInputError {}

/// 画素の列の借用と幅・高さを束ねた入力。隙間なく行優先で並んだ列だけを受ける（行の詰め物は受けない）。
///
/// [`PixelView::new`] だけが作る口で、生成時に大きさと長さを検査する。
/// `PartialEq` は持たない（`==` が比べの判定と紛れないようにする）。
///
/// ```
/// use fandhe_silicon_core::parity::pixel::{CHANNELS_PER_PIXEL, PixelView};
/// let data = vec![0u8; 2 * 3 * CHANNELS_PER_PIXEL];
/// let v = PixelView::new(&data, 2, 3).unwrap();
/// assert_eq!((v.width(), v.height(), v.data().len()), (2, 3, data.len()));
/// ```
///
/// ```compile_fail
/// use fandhe_silicon_core::parity::pixel::PixelView;
/// let _ = PixelView { data: &[], width: 0, height: 0 };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PixelView<'a> {
    data: &'a [u8],
    width: usize,
    height: usize,
}

impl<'a> PixelView<'a> {
    /// 入力を検査して作る。0 の大きさ、積の溢れ、長さ違いはエラーで返す。
    pub fn new(data: &'a [u8], width: usize, height: usize) -> Result<Self, PixelInputError> {
        if width == 0 || height == 0 {
            return Err(PixelInputError::ZeroSize { width, height });
        }
        let expected = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(CHANNELS_PER_PIXEL))
            .ok_or(PixelInputError::SizeOverflow)?;
        if data.len() != expected {
            return Err(PixelInputError::LengthMismatch {
                expected,
                actual: data.len(),
            });
        }
        Ok(Self {
            data,
            width,
            height,
        })
    }

    /// 渡した幅。
    pub const fn width(&self) -> usize {
        self.width
    }

    /// 渡した高さ。
    pub const fn height(&self) -> usize {
        self.height
    }

    /// 渡した画素の列。
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }
}
/// 背後実装の間で画素を比べるときの閾値の組。値は整数だけで持つ（浮動小数点の境界のずれを避ける）。
///
/// 外からは [`T3`] だけが手に入り、フィールドを直接書いて作ることはできない
/// （次の `compile_fail` の例が確かめる。stable の rustdoc は失敗の理由を照合しないので、
/// 通る例で import が正しいことを担保している）。
///
/// ```
/// use fandhe_silicon_core::parity::pixel::{PixelTolerance, T3};
/// let t: PixelTolerance = T3;
/// assert!(t.min_near_denominator() > 0);
/// ```
///
/// ```compile_fail
/// use fandhe_silicon_core::parity::pixel::PixelTolerance;
/// let _ = PixelTolerance { near_channel_diff: 1, max_channel_diff: 2, min_near_numerator: 1, min_near_denominator: 1 };
/// ```
///
/// ```compile_fail
/// use fandhe_silicon_core::parity::pixel::{PixelTolerance, T3};
/// let _ = PixelTolerance { near_channel_diff: 1, ..T3 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelTolerance {
    near_channel_diff: u8,
    max_channel_diff: u8,
    min_near_numerator: u32,
    min_near_denominator: u32,
}

impl PixelTolerance {
    /// 1 画素を「内」と数える、チャネル差の上限（この値以下なら内）。
    pub const fn near_channel_diff(&self) -> u8 {
        self.near_channel_diff
    }

    /// 全画素・全チャネルを通したチャネル差の上限（この値以下なら満たす）。
    pub const fn max_channel_diff(&self) -> u8 {
        self.max_channel_diff
    }

    /// 「内」の画素の割合の下限の分子（割合がこの値以上なら満たす）。
    pub const fn min_near_numerator(&self) -> u32 {
        self.min_near_numerator
    }

    /// 「内」の画素の割合の下限の分母（0 にならない）。
    pub const fn min_near_denominator(&self) -> u32 {
        self.min_near_denominator
    }
}

/// T-3 の閾値の設定（`docs/spec/02-poc-plan.md` の T-3 のポインタ）。
/// 任意の値を作る口は持たない。
pub const T3: PixelTolerance = PixelTolerance {
    near_channel_diff: 1,
    max_channel_diff: 2,
    min_near_numerator: 999,
    min_near_denominator: 1000,
};

/// 画素の比べで返るエラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PixelCompareError {
    /// 幅か高さが違う（要素数が同じでも、2×3 と 3×2 のように形が違えば比べない）。
    SizeMismatch {
        /// 参照の（幅, 高さ）。
        reference: (usize, usize),
        /// 実際の（幅, 高さ）。
        actual: (usize, usize),
    },
}

impl fmt::Display for PixelCompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeMismatch { reference, actual } => write!(
                f,
                "image size mismatch (reference={}x{}, actual={}x{})",
                reference.0, reference.1, actual.0, actual.1
            ),
        }
    }
}

impl std::error::Error for PixelCompareError {}

/// 「同じ成果物・同じ背後実装」の比べ（完全一致）の集計。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactPixelComparison {
    compared: usize,
    mismatched: usize,
    first_mismatch: Option<usize>,
}

impl ExactPixelComparison {
    /// 比べた画素数。
    pub const fn compared(&self) -> usize {
        self.compared
    }

    /// 1 要素でも違った画素の数（複数チャネルが違っても 1 画素と数える）。
    pub const fn mismatched(&self) -> usize {
        self.mismatched
    }

    /// 最初に違った画素の添字（行優先）。無ければ `None`。
    pub const fn first_mismatch(&self) -> Option<usize> {
        self.first_mismatch
    }

    /// 全画素が完全に一致したか。
    pub const fn is_match(&self) -> bool {
        self.mismatched == 0
    }
}

/// 「背後実装の間」の比べ（閾値つき）の集計。判定は渡した閾値で関数の中で 1 度だけ求める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelComparison {
    compared: usize,
    near: usize,
    max_channel_diff: u8,
    first_beyond_near: Option<usize>,
    matched: bool,
}

impl PixelComparison {
    /// 比べた画素数。
    pub const fn compared(&self) -> usize {
        self.compared
    }

    /// 「内」の画素の数（画素の差が内側の上限以下）。
    pub const fn near(&self) -> usize {
        self.near
    }

    /// 全画素・全チャネルを通したチャネル差の最大。
    pub const fn max_channel_diff(&self) -> u8 {
        self.max_channel_diff
    }

    /// 最初に「内」を外れた画素の添字（行優先）。無ければ `None`。
    pub const fn first_beyond_near(&self) -> Option<usize> {
        self.first_beyond_near
    }

    /// 渡した閾値を満たしたか（割合と全体の上限の両方）。
    pub const fn is_match(&self) -> bool {
        self.matched
    }
}

/// 2 つの入力の大きさが同じか確かめる。違えば走査の前にエラーで返す。
fn check_same_size(
    reference: PixelView<'_>,
    actual: PixelView<'_>,
) -> Result<(), PixelCompareError> {
    if reference.width != actual.width || reference.height != actual.height {
        return Err(PixelCompareError::SizeMismatch {
            reference: (reference.width, reference.height),
            actual: (actual.width, actual.height),
        });
    }
    Ok(())
}

/// 「同じ成果物・同じ背後実装」の比べ。閾値は取らず、画素の 4 要素がすべて等しいときだけ一致とする。
/// 大きさが違えば [`PixelCompareError::SizeMismatch`]。
pub fn compare_pixels_exact(
    reference: PixelView<'_>,
    actual: PixelView<'_>,
) -> Result<ExactPixelComparison, PixelCompareError> {
    check_same_size(reference, actual)?;
    let mut compared = 0usize;
    let mut mismatched = 0usize;
    let mut first_mismatch = None;
    for (i, (r, a)) in reference
        .data
        .as_chunks::<CHANNELS_PER_PIXEL>()
        .0
        .iter()
        .zip(actual.data.as_chunks::<CHANNELS_PER_PIXEL>().0.iter())
        .enumerate()
    {
        compared += 1;
        if r != a {
            mismatched += 1;
            first_mismatch.get_or_insert(i);
        }
    }
    Ok(ExactPixelComparison {
        compared,
        mismatched,
        first_mismatch,
    })
}

/// 「背後実装の間」の比べ。閾値は呼び出し側が渡す（道具の内側に既定値を置かない）。
///
/// 画素の差は 4 要素（アルファを含む）のチャネル差の最大。向きは対称。
/// 「内」の画素の割合が下限以上、かつ全体のチャネル差の最大が上限以下のときに一致とする。
/// 割合は整数の掛け合わせ（`u128`）で比べ、溢れさせない。
/// 大きさが違えば [`PixelCompareError::SizeMismatch`]。
pub fn compare_pixels(
    reference: PixelView<'_>,
    actual: PixelView<'_>,
    tolerance: PixelTolerance,
) -> Result<PixelComparison, PixelCompareError> {
    check_same_size(reference, actual)?;
    let mut compared = 0usize;
    let mut near = 0usize;
    let mut max_channel_diff = 0u8;
    let mut first_beyond_near = None;
    for (i, (r, a)) in reference
        .data
        .as_chunks::<CHANNELS_PER_PIXEL>()
        .0
        .iter()
        .zip(actual.data.as_chunks::<CHANNELS_PER_PIXEL>().0.iter())
        .enumerate()
    {
        compared += 1;
        let pixel_diff = r
            .iter()
            .zip(a)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        max_channel_diff = max_channel_diff.max(pixel_diff);
        if pixel_diff <= tolerance.near_channel_diff {
            near += 1;
        } else {
            first_beyond_near.get_or_insert(i);
        }
    }
    let ratio_ok = (near as u128) * u128::from(tolerance.min_near_denominator)
        >= (compared as u128) * u128::from(tolerance.min_near_numerator);
    let matched = ratio_ok && max_channel_diff <= tolerance.max_channel_diff;
    Ok(PixelComparison {
        compared,
        near,
        max_channel_diff,
        first_beyond_near,
        matched,
    })
}
