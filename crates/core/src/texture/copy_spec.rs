//! テクスチャをコピーの転送元・転送先として指す指定の値の型の置き場（TASK-372）。
//!
//! `texture` モジュールの一部で、公開のパスは `fandhe_silicon_core::texture::…`。
//! 値だけを扱う型で、背後実装の API は呼ばない。
//! 指定を受ける記録の操作や検査は、この module には置かない。

/// コピーの原点。`z` は 1 次元・2 次元のテクスチャでは始まりの層の番号、
/// 3 次元のテクスチャでは奥行きの位置（TEX-20）。
///
/// ```
/// use fandhe_silicon_core::texture::TextureOrigin;
///
/// let o = TextureOrigin { x: 1, y: 2, z: 3 };
/// assert_eq!((o.x, o.y, o.z), (1, 2, 3));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureOrigin {
    /// 横方向の位置。
    pub x: u32,
    /// 縦方向の位置。
    pub y: u32,
    /// 層の番号、または奥行きの位置。
    pub z: u32,
}

/// テクスチャをコピーの転送元・転送先として指す指定。`texture` の型は型引数で受ける（TEX-20）。
///
/// 組み立ての口 [`TextureCopySpec::whole`]・[`TextureCopySpec::new`] は値を持たせるだけで、
/// 検査せず、失敗せず、panic しない。
///
/// ```
/// use fandhe_silicon_core::texture::{
///     TextureCopySpec, TextureDescriptor, TextureDimension, TextureExtent, TextureHandle,
///     TextureOrigin, TextureUsage,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     TextureExtent { width: 8, height: 8, depth_or_array_layers: 1 },
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::empty(),
/// );
/// let h = TextureHandle::__backend_new(1, 1, 1, &desc);
///
/// let whole = TextureCopySpec::whole(&h);
/// assert_eq!(whole.mip_level, 0);
/// assert_eq!(whole.origin, TextureOrigin { x: 0, y: 0, z: 0 });
/// assert!(core::ptr::eq(whole.texture, &h));
///
/// let spec = TextureCopySpec::new(&h, 3, TextureOrigin { x: 1, y: 2, z: 5 });
/// assert_eq!(spec.mip_level, 3);
/// assert_eq!(spec.origin, TextureOrigin { x: 1, y: 2, z: 5 });
///
/// let max = TextureCopySpec::new(
///     &h,
///     u32::MAX,
///     TextureOrigin { x: u32::MAX, y: u32::MAX, z: u32::MAX },
/// );
/// assert_eq!(max.mip_level, u32::MAX);
/// assert_eq!(max.origin.z, u32::MAX);
/// ```
///
/// 外の crate は構造体式で組めない（`#[non_exhaustive]`）。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::texture::{TextureCopySpec, TextureOrigin};
///
/// let _ = TextureCopySpec {
///     texture: 0u32,
///     mip_level: 0,
///     origin: TextureOrigin { x: 0, y: 0, z: 0 },
/// };
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct TextureCopySpec<T> {
    /// 指すテクスチャ。
    pub texture: T,
    /// mip の段。
    pub mip_level: u32,
    /// コピーの原点。
    pub origin: TextureOrigin,
}

impl<T> TextureCopySpec<T> {
    /// mip の段 0・原点 `(0, 0, 0)` で指す。
    #[must_use]
    pub fn whole(texture: T) -> Self {
        Self::new(texture, 0, TextureOrigin { x: 0, y: 0, z: 0 })
    }

    /// 渡した値をそのまま持つ指定を作る。範囲の検査はしない。
    #[must_use]
    pub fn new(texture: T, mip_level: u32, origin: TextureOrigin) -> Self {
        Self {
            texture,
            mip_level,
            origin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::{
        TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage,
    };
    use crate::texture_format::TextureFormat;

    #[test]
    fn whole_is_level_zero_at_origin() {
        let s = TextureCopySpec::whole(7u32);
        assert_eq!(s.texture, 7);
        assert_eq!(s.mip_level, 0);
        assert_eq!(s.origin, TextureOrigin { x: 0, y: 0, z: 0 });
    }

    #[test]
    fn new_keeps_values_as_given() {
        let o = TextureOrigin {
            x: u32::MAX,
            y: 2,
            z: 3,
        };
        let s = TextureCopySpec::new("t", u32::MAX, o);
        assert_eq!(s.texture, "t");
        assert_eq!(s.mip_level, u32::MAX);
        assert_eq!(s.origin, o);
    }

    #[test]
    fn handle_reference_spec_is_copy() {
        let desc = TextureDescriptor::new(
            TextureDimension::D2,
            TextureExtent {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            TextureFormat::Rgba8Unorm,
            TextureUsage::empty(),
        );
        let h = TextureHandle::__backend_new(1, 1, 1, &desc);
        let a = TextureCopySpec::whole(&h);
        let b = a;
        assert!(core::ptr::eq(a.texture, b.texture));
    }
}
