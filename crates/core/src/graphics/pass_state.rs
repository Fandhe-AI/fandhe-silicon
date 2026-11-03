//! 描画パスの状態を設定する操作が受ける値の型の置き場（TASK-838）。
//!
//! `graphics` モジュールから再公開される（公開のパスは `fandhe_silicon_core::graphics::…`）。
//! 背後実装の API は呼ばず、状態を持たない。

/// ビューポートの値（項目 `REND-57`）。欄は公開で、外の crate から構造体式で作れる。
///
/// ```
/// use fandhe_silicon_core::graphics::Viewport;
/// let v = Viewport { x: 0.0, y: 0.0, width: 640.0, height: 480.0, min_depth: 0.0, max_depth: 1.0 };
/// let a = v;
/// let b = v;
/// assert_eq!(a, b);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// 左端の位置。
    pub x: f32,
    /// 上端の位置。
    pub y: f32,
    /// 幅。
    pub width: f32,
    /// 高さ。
    pub height: f32,
    /// 深度の範囲の下端。
    pub min_depth: f32,
    /// 深度の範囲の上端。
    pub max_depth: f32,
}

/// シザー矩形の値（項目 `REND-56`）。欄は公開で、外の crate から構造体式で作れる。
///
/// ```
/// use fandhe_silicon_core::graphics::ScissorRect;
/// let r = ScissorRect { x: 1, y: 2, width: 3, height: 4 };
/// let a = r;
/// let b = r;
/// assert_eq!(a, b);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScissorRect {
    /// 左端の位置。
    pub x: u32,
    /// 上端の位置。
    pub y: u32,
    /// 幅。
    pub width: u32,
    /// 高さ。
    pub height: u32,
}

/// インデックスの形式（項目 `REND-63`）。既定は `Uint32`。
/// 外の crate からは、ワイルドカードの腕の無い `match` は書けない。
///
/// ```
/// use fandhe_silicon_core::graphics::IndexFormat;
/// assert_eq!(IndexFormat::default(), IndexFormat::Uint32);
/// ```
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::graphics::IndexFormat;
/// fn f(i: IndexFormat) -> u8 {
///     match i {
///         IndexFormat::Uint16 => 0,
///         IndexFormat::Uint32 => 1,
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IndexFormat {
    /// 16 bit の符号なし整数のインデックス。
    Uint16,
    /// 32 bit の符号なし整数のインデックス。
    #[default]
    Uint32,
}

/// インデックス 1 個のバイト数を返す。`Uint16` は 2、`Uint32` は 4。
///
/// ```
/// use fandhe_silicon_core::graphics::{IndexFormat, index_format_bytes};
/// assert_eq!(index_format_bytes(IndexFormat::Uint16), 2);
/// assert_eq!(index_format_bytes(IndexFormat::Uint32), 4);
/// ```
#[must_use]
pub const fn index_format_bytes(format: IndexFormat) -> u32 {
    match format {
        IndexFormat::Uint16 => 2,
        IndexFormat::Uint32 => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARIANTS: usize = 2;
    const FORMATS: [IndexFormat; VARIANTS] = [IndexFormat::Uint16, IndexFormat::Uint32];

    /// `_` の腕を持たない `match`。値を足すとコンパイルが落ちて番号を足させる。
    const fn ordinal(f: IndexFormat) -> usize {
        match f {
            IndexFormat::Uint16 => 0,
            IndexFormat::Uint32 => 1,
        }
    }

    #[test]
    fn index_format_has_two_values() {
        assert_eq!(FORMATS.len(), VARIANTS);
        let mut seen = [0usize; VARIANTS];
        for f in FORMATS {
            seen[ordinal(f)] += 1;
        }
        assert_eq!(seen, [1; VARIANTS]);
    }

    #[test]
    fn index_format_bytes_values() {
        assert_eq!(index_format_bytes(IndexFormat::Uint16), 2);
        assert_eq!(index_format_bytes(IndexFormat::Uint32), 4);
    }

    #[test]
    fn index_format_default_is_uint32() {
        assert_eq!(IndexFormat::default(), IndexFormat::Uint32);
    }
}
