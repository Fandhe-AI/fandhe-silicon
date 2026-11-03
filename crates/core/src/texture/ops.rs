//! テクスチャを作る操作の集まりの trait `TextureOps`。
//!
//! `texture` モジュールから再公開される。`Device::textures` が返す口の型で、
//! 記述子 `TextureDescriptor` を受けて handle `TextureHandle` を返す（TASK-150 のポインタ）。

use super::{TextureDescriptor, TextureHandle};
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
///     TextureOps, TextureUsage,
/// };
/// struct T;
/// impl TextureOps for T {
///     fn create_texture(
///         &self,
///         desc: &TextureDescriptor<'_>,
///     ) -> Result<TextureHandle, ContractError> {
///         Ok(TextureHandle::__backend_new(1, 1, 1, desc))
///     }
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
}
