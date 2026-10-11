//! 画素の一致を確かめる道具に渡す、画像の入力の受け口（spec TASK-26 の 3/6）。
//!
//! 背後実装の出力と参照を画素で比べる側（検証用のテスト・測定）から呼ばれる。
//! feature `test-support` の裏にだけ公開される。標準ライブラリだけで書き、確保はしない。
//! 置き場・feature 名・入力の形は未承認の仮置き（`docs/pixel-parity-tool.md` の Q1〜Q4・Q7・Q8・Q10）。
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
