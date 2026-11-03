//! テクスチャの view を指す不透明な値の型の置き場（TASK-372）。
//!
//! `texture` モジュールの一部で、公開のパスは `fandhe_silicon_core::texture::…`。
//! 値だけを扱う型で、背後実装の API は呼ばない。

use super::TextureHandle;

/// テクスチャの view を指す不透明な値（TEX-18）。作るときに渡した元のテクスチャの handle を借用する。
///
/// 欄は非公開で、`Clone`・`Copy` は実装しない。`__backend_*` は背後実装だけが使う口で、
/// `__backend_new` は検査せず、safe で誰でも値を作れる。
///
/// ```
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage, TextureView,
/// };
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// let desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     TextureExtent { width: 8, height: 8, depth_or_array_layers: 1 },
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::empty(),
/// );
/// let h = TextureHandle::__backend_new(1, 2, 3, &desc);
/// let v = TextureView::__backend_new(&h, 4, 5, 6);
/// assert_eq!(v.__backend_generation(), 4);
/// assert_eq!(v.__backend_id(), 5);
/// assert_eq!(v.__backend_device_id(), 6);
/// assert!(core::ptr::eq(v.texture(), &h));
/// ```
///
/// view を落とした後も、`texture()` で得た借用は使える。
///
/// ```
/// # use fandhe_silicon_core::texture::{
/// #     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage, TextureView,
/// # };
/// # use fandhe_silicon_core::texture_format::TextureFormat;
/// # let desc = TextureDescriptor::new(
/// #     TextureDimension::D2,
/// #     TextureExtent { width: 8, height: 8, depth_or_array_layers: 1 },
/// #     TextureFormat::Rgba8Unorm,
/// #     TextureUsage::empty(),
/// # );
/// # let h = TextureHandle::__backend_new(1, 2, 3, &desc);
/// let t = {
///     let v = TextureView::__backend_new(&h, 1, 2, 3);
///     v.texture()
/// };
/// assert!(core::ptr::eq(t, &h));
/// ```
///
/// `Clone` は実装しない。
///
/// ```compile_fail,E0599
/// # use fandhe_silicon_core::texture::{
/// #     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage, TextureView,
/// # };
/// # use fandhe_silicon_core::texture_format::TextureFormat;
/// # let desc = TextureDescriptor::new(
/// #     TextureDimension::D2,
/// #     TextureExtent { width: 8, height: 8, depth_or_array_layers: 1 },
/// #     TextureFormat::Rgba8Unorm,
/// #     TextureUsage::empty(),
/// # );
/// # let h = TextureHandle::__backend_new(1, 2, 3, &desc);
/// let v = TextureView::__backend_new(&h, 1, 2, 3);
/// let _ = v.clone();
/// ```
///
/// 外の crate は構造体式で組めない。
///
/// ```compile_fail,E0451
/// # use fandhe_silicon_core::texture::{
/// #     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage, TextureView,
/// # };
/// # use fandhe_silicon_core::texture_format::TextureFormat;
/// # let desc = TextureDescriptor::new(
/// #     TextureDimension::D2,
/// #     TextureExtent { width: 8, height: 8, depth_or_array_layers: 1 },
/// #     TextureFormat::Rgba8Unorm,
/// #     TextureUsage::empty(),
/// # );
/// # let h = TextureHandle::__backend_new(1, 2, 3, &desc);
/// let _ = TextureView { texture: &h, generation: 1, id: 2, device_id: 3 };
/// ```
///
/// view が残る間は、元の handle を動かせない。
///
/// ```compile_fail,E0505
/// # use fandhe_silicon_core::texture::{
/// #     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle, TextureUsage, TextureView,
/// # };
/// # use fandhe_silicon_core::texture_format::TextureFormat;
/// # let desc = TextureDescriptor::new(
/// #     TextureDimension::D2,
/// #     TextureExtent { width: 8, height: 8, depth_or_array_layers: 1 },
/// #     TextureFormat::Rgba8Unorm,
/// #     TextureUsage::empty(),
/// # );
/// # let h = TextureHandle::__backend_new(1, 2, 3, &desc);
/// let v = TextureView::__backend_new(&h, 1, 2, 3);
/// let moved = h;
/// let _ = v.texture();
/// let _ = moved;
/// ```
#[derive(Debug)]
pub struct TextureView<'t> {
    texture: &'t TextureHandle,
    generation: u64,
    id: u64,
    device_id: u64,
}

impl<'t> TextureView<'t> {
    /// 背後実装向けの組み立ての口。検査はしない。
    #[doc(hidden)]
    pub fn __backend_new(
        texture: &'t TextureHandle,
        generation: u64,
        id: u64,
        device_id: u64,
    ) -> Self {
        Self {
            texture,
            generation,
            id,
            device_id,
        }
    }

    /// 組み立てで渡した世代を返す（背後実装向けの読み出しの口）。
    #[doc(hidden)]
    pub fn __backend_generation(&self) -> u64 {
        self.generation
    }

    /// 組み立てで渡した識別子を返す（背後実装向けの読み出しの口）。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// 組み立てで渡した device の識別子を返す（背後実装向けの読み出しの口）。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }

    /// view の元のテクスチャの handle を返す（TEX-30）。
    ///
    /// 組み立てで渡した借用をそのまま返す。`Result` を返さず、背後実装を呼ばず、
    /// 新しい handle も写しも作らない。返す借用の寿命は view ではなく元の handle に結びつく。
    pub fn texture(&self) -> &'t TextureHandle {
        self.texture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::{TextureDescriptor, TextureDimension, TextureExtent, TextureUsage};
    use crate::texture_format::TextureFormat;

    fn handle() -> TextureHandle {
        let desc = TextureDescriptor::new(
            TextureDimension::D2,
            TextureExtent {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            TextureFormat::Rgba8Unorm,
            TextureUsage::empty(),
        );
        TextureHandle::__backend_new(1, 2, 3, &desc)
    }

    #[test]
    fn round_trip_of_backend_values() {
        let h = handle();
        let v = TextureView::__backend_new(&h, 7, 8, 9);
        assert_eq!(
            (
                v.__backend_generation(),
                v.__backend_id(),
                v.__backend_device_id()
            ),
            (7, 8, 9)
        );
    }

    #[test]
    fn texture_returns_the_same_handle() {
        let h = handle();
        let v = TextureView::__backend_new(&h, 7, 8, 9);
        assert!(core::ptr::eq(v.texture(), &h));
    }

    #[test]
    fn two_views_of_one_handle_return_the_same_handle() {
        let h = handle();
        let a = TextureView::__backend_new(&h, 1, 1, 1);
        let b = TextureView::__backend_new(&h, 2, 2, 2);
        assert!(core::ptr::eq(a.texture(), b.texture()));
    }
}
