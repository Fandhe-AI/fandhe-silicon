//! テクスチャの値の型（次元・大きさ・用途）。
//!
//! 共通の crate が feature の条件なしで公開する。背後実装の型は含まない（TASK-371 のポインタ）。

use core::ops::{BitOr, BitOrAssign};

/// テクスチャの次元（TEX-27 のポインタ）。
///
/// 3 値を持ち、`#[non_exhaustive]` なので外の crate の `match` には `_` の腕が要る。
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::texture::TextureDimension;
///
/// fn name(d: TextureDimension) -> &'static str {
///     match d {
///         TextureDimension::D1 => "1d",
///         TextureDimension::D2 => "2d",
///         TextureDimension::D3 => "3d",
///     }
/// }
/// ```
///
/// ```
/// use fandhe_silicon_core::texture::TextureDimension;
///
/// fn name(d: TextureDimension) -> &'static str {
///     match d {
///         TextureDimension::D1 => "1d",
///         TextureDimension::D2 => "2d",
///         TextureDimension::D3 => "3d",
///         _ => "other",
///     }
/// }
/// assert_eq!(name(TextureDimension::D2), "2d");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureDimension {
    /// 1 次元。
    D1,
    /// 2 次元。
    D2,
    /// 3 次元。
    D3,
}

/// テクスチャの大きさ。幅・高さ・層数または奥行き（TEX-21・TEX-24 のポインタ）。
///
/// ```
/// use fandhe_silicon_core::texture::TextureExtent;
///
/// let a = TextureExtent { width: 4, height: 3, depth_or_array_layers: 2 };
/// let b = a;
/// assert_eq!(a, b);
/// assert_eq!(a.width, 4);
/// assert_eq!(b.depth_or_array_layers, 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureExtent {
    /// 幅。
    pub width: u32,
    /// 高さ。
    pub height: u32,
    /// 1 次元・2 次元では層数、3 次元では奥行き。
    pub depth_or_array_layers: u32,
}

/// テクスチャの用途のビットの集まり（TEX-29 のポインタ）。
///
/// 7 つの定数を `|` で組み合わせる。ビットから作る口は無い。
///
/// ```
/// use fandhe_silicon_core::texture::TextureUsage;
///
/// assert_eq!(TextureUsage::COPY_SRC.bits(), 1);
/// assert_eq!(TextureUsage::COPY_DST.bits(), 2);
/// assert_eq!(TextureUsage::TEXTURE_BINDING.bits(), 4);
/// assert_eq!(TextureUsage::STORAGE_BINDING.bits(), 8);
/// assert_eq!(TextureUsage::RENDER_ATTACHMENT.bits(), 16);
/// assert_eq!(TextureUsage::TRANSIENT_ATTACHMENT.bits(), 32);
/// assert_eq!(TextureUsage::STORAGE_ATOMIC.bits(), 64);
///
/// let both = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
/// assert!(both.contains(TextureUsage::COPY_SRC));
/// assert!(both.contains(TextureUsage::COPY_DST));
/// assert!(!both.contains(TextureUsage::TEXTURE_BINDING));
/// assert!(TextureUsage::empty().is_empty());
///
/// let mut u = TextureUsage::empty();
/// u |= TextureUsage::STORAGE_BINDING;
/// assert!(u.contains(TextureUsage::STORAGE_BINDING));
/// assert!(!u.is_empty());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureUsage(u32);

impl TextureUsage {
    /// コピーの転送元にする。
    pub const COPY_SRC: Self = Self(1 << 0);
    /// コピーの転送先にする。
    pub const COPY_DST: Self = Self(1 << 1);
    /// 標本化する束縛として使う。
    pub const TEXTURE_BINDING: Self = Self(1 << 2);
    /// storage の束縛として使う。
    pub const STORAGE_BINDING: Self = Self(1 << 3);
    /// 描画先として使う。
    pub const RENDER_ATTACHMENT: Self = Self(1 << 4);
    /// 一時的な描画先として使う。
    pub const TRANSIENT_ATTACHMENT: Self = Self(1 << 5);
    /// storage の atomic 操作の対象にする。
    pub const STORAGE_ATOMIC: Self = Self(1 << 6);

    /// どの用途も含まない値。
    pub const fn empty() -> Self {
        Self(0)
    }

    /// どの用途も含まないとき `true`。
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// ビット値を返す。
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// `other` のビットをすべて含むとき `true`。
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for TextureUsage {
    type Output = Self;

    /// 2 つの用途の和集合を返す。
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for TextureUsage {
    /// 用途を足す。
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
