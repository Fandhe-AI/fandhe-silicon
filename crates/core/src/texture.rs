//! テクスチャの値の型（次元・大きさ・用途）と、作るときに渡す記述子と、テクスチャを指す handle と handle の問い合わせ。
//!
//! 共通の crate が feature の条件なしで公開する。背後実装の型は含まない（TASK-371 のポインタ）。

use core::ops::{BitOr, BitOrAssign};

use crate::texture_format::TextureFormat;

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

/// テクスチャを作るときに渡す記述子（TEX-21〜TEX-29 のポインタ）。
///
/// `#[non_exhaustive]` なので、外の crate は構造体式で作れない。`new` で作ってから欄を書き換える。
///
/// ```
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let size = TextureExtent { width: 64, height: 32, depth_or_array_layers: 1 };
/// let desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     size,
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::TEXTURE_BINDING,
/// );
/// assert_eq!(desc.label, None);
/// assert_eq!(desc.dimension, TextureDimension::D2);
/// assert_eq!(desc.size, size);
/// assert_eq!(desc.mip_level_count, 1);
/// assert_eq!(desc.sample_count, 1);
/// assert_eq!(desc.format, TextureFormat::Rgba8Unorm);
/// assert_eq!(desc.usage, TextureUsage::TEXTURE_BINDING);
/// assert!(desc.view_formats.is_empty());
/// ```
///
/// 欄は作った後に書き換えられる。
///
/// ```
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let size = TextureExtent { width: 8, height: 8, depth_or_array_layers: 1 };
/// let mut desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     size,
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::COPY_DST,
/// );
/// desc.label = Some("name");
/// desc.mip_level_count = 4;
/// desc.view_formats = &[TextureFormat::Rgba8UnormSrgb];
/// assert_eq!(desc.label, Some("name"));
/// assert_eq!(desc.mip_level_count, 4);
/// assert_eq!(desc.view_formats, &[TextureFormat::Rgba8UnormSrgb]);
/// ```
///
/// 0 の大きさも `new` は検査せずそのまま保持する。
///
/// ```
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let size = TextureExtent { width: 0, height: 0, depth_or_array_layers: 0 };
/// let desc = TextureDescriptor::new(
///     TextureDimension::D3,
///     size,
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::empty(),
/// );
/// assert_eq!(desc.size, size);
/// assert!(desc.usage.is_empty());
/// ```
///
/// 外の crate の構造体式は通らない。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let _ = TextureDescriptor {
///     label: None,
///     dimension: TextureDimension::D2,
///     size: TextureExtent { width: 1, height: 1, depth_or_array_layers: 1 },
///     mip_level_count: 1,
///     sample_count: 1,
///     format: TextureFormat::Rgba8Unorm,
///     usage: TextureUsage::empty(),
///     view_formats: &[],
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextureDescriptor<'a> {
    /// テクスチャの名前。
    pub label: Option<&'a str>,
    /// テクスチャの次元。
    pub dimension: TextureDimension,
    /// テクスチャの大きさ。
    pub size: TextureExtent,
    /// mip の段数。
    pub mip_level_count: u32,
    /// サンプル数。
    pub sample_count: u32,
    /// テクスチャの形式。
    pub format: TextureFormat,
    /// テクスチャの用途。
    pub usage: TextureUsage,
    /// view で使う形式の列。
    pub view_formats: &'a [TextureFormat],
}

impl TextureDescriptor<'_> {
    /// 次元・大きさ・形式・用途から記述子を作る。
    ///
    /// `label` は `None`、`mip_level_count` と `sample_count` は 1、`view_formats` は空になる。
    /// 値は検査せず、そのまま保持する。
    #[must_use]
    pub fn new(
        dimension: TextureDimension,
        size: TextureExtent,
        format: TextureFormat,
        usage: TextureUsage,
    ) -> Self {
        Self {
            label: None,
            dimension,
            size,
            mip_level_count: 1,
            sample_count: 1,
            format,
            usage,
            view_formats: &[],
        }
    }
}

/// テクスチャを指す不透明な handle（TASK-371 のポインタ）。
///
/// 欄（世代・識別子・device の識別子と、作るときに渡した記述子の値）は非公開で、
/// `Clone`・`Copy` を実装しない。`__backend_*` は背後実装だけが使う口で、
/// `__backend_new` は safe で誰でも値を作れる。
///
/// 往復の例。
///
/// ```
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let mut desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     TextureExtent { width: 64, height: 32, depth_or_array_layers: 6 },
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::TEXTURE_BINDING,
/// );
/// desc.mip_level_count = 4;
/// desc.sample_count = 1;
/// let h = TextureHandle::__backend_new(5, 3, 1, &desc);
/// assert_eq!(h.__backend_generation(), 5);
/// assert_eq!(h.__backend_id(), 3);
/// assert_eq!(h.__backend_device_id(), 1);
/// assert_eq!(h.size(), desc.size);
/// assert_eq!(h.width(), desc.size.width);
/// assert_eq!(h.height(), desc.size.height);
/// assert_eq!(h.depth_or_array_layers(), desc.size.depth_or_array_layers);
/// assert_eq!(h.mip_level_count(), desc.mip_level_count);
/// assert_eq!(h.sample_count(), desc.sample_count);
/// assert_eq!(h.dimension(), desc.dimension);
/// assert_eq!(h.format(), desc.format);
/// assert_eq!(h.usage(), desc.usage);
/// ```
///
/// 外の crate からは構造体式で作れない。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::texture::{
///     TextureDimension, TextureExtent, TextureHandle, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
/// let _ = TextureHandle {
///     generation: 0,
///     id: 0,
///     device_id: 0,
///     dimension: TextureDimension::D2,
///     size: TextureExtent { width: 1, height: 1, depth_or_array_layers: 1 },
///     mip_level_count: 1,
///     sample_count: 1,
///     format: TextureFormat::Rgba8Unorm,
///     usage: TextureUsage::empty(),
/// };
/// ```
///
/// `clone` は呼べない。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
/// let desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     TextureExtent { width: 1, height: 1, depth_or_array_layers: 1 },
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::empty(),
/// );
/// let h = TextureHandle::__backend_new(5, 3, 1, &desc);
/// let _ = h.clone();
/// ```
#[derive(Debug)]
pub struct TextureHandle {
    generation: u64,
    id: u64,
    device_id: u64,
    dimension: TextureDimension,
    size: TextureExtent,
    mip_level_count: u32,
    sample_count: u32,
    format: TextureFormat,
    usage: TextureUsage,
}

impl TextureHandle {
    /// 背後実装向けの組み立ての口。記述子の値を写して持つ。検査はしない。
    #[doc(hidden)]
    pub fn __backend_new(
        generation: u64,
        id: u64,
        device_id: u64,
        desc: &TextureDescriptor<'_>,
    ) -> Self {
        Self {
            generation,
            id,
            device_id,
            dimension: desc.dimension,
            size: desc.size,
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
            format: desc.format,
            usage: desc.usage,
        }
    }

    /// 背後実装向けの読み出しの口（世代）。
    #[doc(hidden)]
    pub fn __backend_generation(&self) -> u64 {
        self.generation
    }

    /// 背後実装向けの読み出しの口（識別子）。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// 背後実装向けの読み出しの口（所属する device の識別子）。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }

    /// 大きさを返す（TEX-21 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn size(&self) -> TextureExtent {
        self.size
    }

    /// 幅を返す（TEX-22 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn width(&self) -> u32 {
        self.size.width
    }

    /// 高さを返す（TEX-23 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn height(&self) -> u32 {
        self.size.height
    }

    /// 奥行きまたは配列の層数を返す（TEX-24 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn depth_or_array_layers(&self) -> u32 {
        self.size.depth_or_array_layers
    }

    /// mip の段数を返す（TEX-25 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn mip_level_count(&self) -> u32 {
        self.mip_level_count
    }

    /// サンプル数を返す（TEX-26 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// 次元を返す（TEX-27 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn dimension(&self) -> TextureDimension {
        self.dimension
    }

    /// テクスチャの形式を返す（TEX-28 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn format(&self) -> TextureFormat {
        self.format
    }

    /// テクスチャの用途を返す（TEX-29 のポインタ）。
    ///
    /// `__backend_new` に渡した記述子の値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn usage(&self) -> TextureUsage {
        self.usage
    }
}

mod descriptor_check;
pub use descriptor_check::{max_mip_level_count, validate_texture_descriptor};
