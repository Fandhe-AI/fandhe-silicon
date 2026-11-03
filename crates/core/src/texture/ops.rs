//! テクスチャを作る操作の集まりの trait `TextureOps`。
//!
//! `texture` モジュールから再公開される。`Device::textures` が返す口の型で、
//! 記述子 `TextureDescriptor` を受けて handle `TextureHandle` を返す（TASK-150 のポインタ）。テクスチャと view を破棄する操作も持つ（TASK-372 のポインタ）。

use super::{TextureDescriptor, TextureHandle, TextureView};
use crate::error::ContractError;

/// device から届くテクスチャを作る操作の集まり。
///
/// `create_texture` に既定の本体は無く、持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// use fandhe_silicon_core::texture::TextureOps;
/// struct T;
/// impl TextureOps for T {}
/// ```
///
/// 持てば通る。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::texture_format::TextureFormat;
/// use fandhe_silicon_core::texture::{
///     TextureDescriptor, TextureDimension, TextureExtent, TextureHandle,
///     TextureOps, TextureUsage, TextureView,
/// };
/// struct T;
/// impl TextureOps for T {
///     fn create_texture(
///         &self,
///         desc: &TextureDescriptor<'_>,
///     ) -> Result<TextureHandle, ContractError> {
///         Ok(TextureHandle::__backend_new(1, 1, 1, desc))
///     }
///     fn destroy_texture(&self, _texture: TextureHandle) {}
///     fn destroy_view(&self, _view: TextureView<'_>) {}
/// }
/// let desc = TextureDescriptor::new(
///     TextureDimension::D2,
///     TextureExtent { width: 4, height: 4, depth_or_array_layers: 1 },
///     TextureFormat::Rgba8Unorm,
///     TextureUsage::TEXTURE_BINDING,
/// );
/// assert!(T.create_texture(&desc).is_ok());
/// ```
pub trait TextureOps {
    /// 記述子を受けてテクスチャを作る。検査と生成の中身は実装が決める。
    ///
    /// 対応する項目のポインタ: DEV-153。
    fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Result<TextureHandle, ContractError>;

    /// テクスチャを破棄する。handle を値で受けて消費し、`Result` は返さない。
    ///
    /// `dyn TextureOps` 越しに呼べる。
    ///
    /// ```
    /// use fandhe_silicon_core::texture::{TextureHandle, TextureOps};
    /// fn f(o: &dyn TextureOps, h: TextureHandle) {
    ///     o.destroy_texture(h);
    /// }
    /// ```
    ///
    /// 破棄の後に同じ handle を使うとコンパイルできない。
    ///
    /// ```compile_fail,E0382
    /// use fandhe_silicon_core::texture::{TextureHandle, TextureOps};
    /// fn f(o: &dyn TextureOps, h: TextureHandle) {
    ///     o.destroy_texture(h);
    ///     let _ = h.width();
    /// }
    /// ```
    ///
    /// 同じ handle を 2 回破棄するとコンパイルできない。
    ///
    /// ```compile_fail,E0382
    /// use fandhe_silicon_core::texture::{TextureHandle, TextureOps};
    /// fn f(o: &dyn TextureOps, h: TextureHandle) {
    ///     o.destroy_texture(h);
    ///     o.destroy_texture(h);
    /// }
    /// ```
    ///
    /// view が残る間に元の handle を破棄するとコンパイルできない。
    ///
    /// ```compile_fail,E0505
    /// use fandhe_silicon_core::texture::{TextureHandle, TextureOps, TextureView};
    /// fn f(o: &dyn TextureOps, h: TextureHandle) {
    ///     let v = TextureView::__backend_new(&h, 1, 2, 3);
    ///     o.destroy_texture(h);
    ///     let _ = v.texture();
    /// }
    /// ```
    ///
    /// 対応する項目のポインタ: TEX-19。
    fn destroy_texture(&self, texture: TextureHandle);

    /// view を破棄する。view を値で受けて消費し、`Result` は返さない。
    ///
    /// view の破棄の後に元の handle を破棄できる。
    ///
    /// ```
    /// use fandhe_silicon_core::texture::{TextureHandle, TextureOps, TextureView};
    /// fn f(o: &dyn TextureOps, h: TextureHandle) {
    ///     let v = TextureView::__backend_new(&h, 1, 2, 3);
    ///     o.destroy_view(v);
    ///     o.destroy_texture(h);
    /// }
    /// ```
    ///
    /// 破棄の後に同じ view を使うとコンパイルできない。
    ///
    /// ```compile_fail,E0382
    /// use fandhe_silicon_core::texture::{TextureHandle, TextureOps, TextureView};
    /// fn f(o: &dyn TextureOps, h: TextureHandle) {
    ///     let v = TextureView::__backend_new(&h, 1, 2, 3);
    ///     o.destroy_view(v);
    ///     let _ = v.texture();
    /// }
    /// ```
    ///
    /// 対応する項目のポインタ: TEX-19。
    fn destroy_view<'t>(&self, view: TextureView<'t>);
}
