//! 操作の時点の用途と view の形式を照らす純粋関数（TASK-1942。TEX-50〜TEX-54・TEX-58・TEX-65 のポインタ）。
//!
//! `texture` モジュールの一部で、公開のパスは `fandhe_silicon_core::texture::…`。
//! 作成の記述子の検査とは別の関数で、作成の時に記録した用途と形式を引数で受けて、
//! 操作の時点の用途と view の形式を照らし、結果を `Result` で返す。
//! 背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。
//! バッファ側の対応する検査は `memory::validate_usage_for_role`（TASK-1796）。

use super::TextureUsage;
use crate::error::ContractError;
use crate::texture_format::TextureFormat;

/// 用途を要るテクスチャの操作の役割。要る用途は [`TextureUsageRole::required_usage`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureUsageRole {
    /// コピーの元。
    CopySource,
    /// コピーの先。
    CopyDestination,
    /// サンプリングの束縛。
    TextureBinding,
    /// storage の束縛。
    StorageBinding,
    /// storage の atomic。
    StorageAtomic,
    /// 描画先。
    RenderAttachment,
}

impl TextureUsageRole {
    /// 検査の対象の役割のすべて。
    pub const ALL: &'static [Self] = &[
        Self::CopySource,
        Self::CopyDestination,
        Self::TextureBinding,
        Self::StorageBinding,
        Self::StorageAtomic,
        Self::RenderAttachment,
    ];

    /// この役割に要る用途。常にちょうど 1 つのビットを持つ。
    pub const fn required_usage(self) -> TextureUsage {
        match self {
            Self::CopySource => TextureUsage::COPY_SRC,
            Self::CopyDestination => TextureUsage::COPY_DST,
            Self::TextureBinding => TextureUsage::TEXTURE_BINDING,
            Self::StorageBinding => TextureUsage::STORAGE_BINDING,
            Self::StorageAtomic => TextureUsage::STORAGE_ATOMIC,
            Self::RenderAttachment => TextureUsage::RENDER_ATTACHMENT,
        }
    }

    /// エラーの文字列に載せる役割の名前（英語）。
    const fn role_name(self) -> &'static str {
        match self {
            Self::CopySource => "copy-source",
            Self::CopyDestination => "copy-destination",
            Self::TextureBinding => "texture-binding",
            Self::StorageBinding => "storage-binding",
            Self::StorageAtomic => "storage-atomic",
            Self::RenderAttachment => "render-attachment",
        }
    }

    /// エラーの文字列に載せる、要る用途の値の名前（英語）。
    const fn usage_name(self) -> &'static str {
        match self {
            Self::CopySource => "COPY_SRC",
            Self::CopyDestination => "COPY_DST",
            Self::TextureBinding => "TEXTURE_BINDING",
            Self::StorageBinding => "STORAGE_BINDING",
            Self::StorageAtomic => "STORAGE_ATOMIC",
            Self::RenderAttachment => "RENDER_ATTACHMENT",
        }
    }
}

/// 作成の時に申告した用途 `declared` が `role` に要る用途を含むかを判定する。
///
/// 過申告（要る用途以外のビットも立っている）は通す。空の用途はどの役割でも拒否する。
/// 1 つの操作が複数の役割でテクスチャを使うときは、呼び手が役割ごとに呼ぶ。
/// `declared` は背後実装が作成の時に記録した値を渡すこと（handle の `usage()` を正にしない）。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::texture::{
///     validate_texture_usage_for_role, TextureUsage, TextureUsageRole,
/// };
///
/// let declared = TextureUsage::COPY_SRC | TextureUsage::TEXTURE_BINDING;
/// assert!(validate_texture_usage_for_role(declared, TextureUsageRole::CopySource).is_ok());
/// assert!(matches!(
///     validate_texture_usage_for_role(declared, TextureUsageRole::CopyDestination),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 要る用途が `declared` に無いとき `ContractError::InvalidArgument`。文字列に役割の名前と
/// 足りない用途の値の名前を載せる。
pub fn validate_texture_usage_for_role(
    declared: TextureUsage,
    role: TextureUsageRole,
) -> Result<(), ContractError> {
    if declared.contains(role.required_usage()) {
        return Ok(());
    }
    Err(ContractError::InvalidArgument(format!(
        "texture usage lacks {} required for {}",
        role.usage_name(),
        role.role_name()
    )))
}

/// view を作るときに求める形式 `requested` が、元の形式か作成の時の `view_formats` にあるかを判定する。
///
/// sRGB の対であることだけでは通さない。`texture_format` と `view_formats` は、
/// 背後実装が作成の時に記録した値を渡すこと。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::texture::validate_texture_view_format;
/// use fandhe_silicon_core::texture_format::TextureFormat;
///
/// assert!(validate_texture_view_format(
///     TextureFormat::Rgba8Unorm,
///     &[TextureFormat::Rgba8UnormSrgb],
///     TextureFormat::Rgba8UnormSrgb
/// )
/// .is_ok());
/// assert!(matches!(
///     validate_texture_view_format(
///         TextureFormat::Rgba8Unorm,
///         &[],
///         TextureFormat::Rgba8UnormSrgb
///     ),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 求める形式が元の形式とも `view_formats` のどれとも違うとき `ContractError::InvalidArgument`。
pub fn validate_texture_view_format(
    texture_format: TextureFormat,
    view_formats: &[TextureFormat],
    requested: TextureFormat,
) -> Result<(), ContractError> {
    if requested == texture_format || view_formats.contains(&requested) {
        return Ok(());
    }
    Err(ContractError::InvalidArgument(format!(
        "view format {requested:?} is neither the texture format {texture_format:?} nor in view_formats"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_usages() -> TextureUsage {
        TextureUsage::COPY_SRC
            | TextureUsage::COPY_DST
            | TextureUsage::TEXTURE_BINDING
            | TextureUsage::STORAGE_BINDING
            | TextureUsage::RENDER_ATTACHMENT
            | TextureUsage::TRANSIENT_ATTACHMENT
            | TextureUsage::STORAGE_ATOMIC
    }

    /// `role` に要る用途だけを除いた残り（過申告側の 6 ビット）。
    fn all_but(role: TextureUsageRole) -> TextureUsage {
        let mut rest = TextureUsage::TRANSIENT_ATTACHMENT;
        for &other in TextureUsageRole::ALL {
            if other != role {
                rest |= other.required_usage();
            }
        }
        rest
    }

    fn is_invalid(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn texture_usage_check_roles_accept_required_usage() {
        for &role in TextureUsageRole::ALL {
            assert!(validate_texture_usage_for_role(role.required_usage(), role).is_ok());
        }
    }

    #[test]
    fn texture_usage_check_roles_reject_missing_usage() {
        for &role in TextureUsageRole::ALL {
            let rest = all_but(role);
            assert_eq!(rest.bits().count_ones(), 6);
            assert!(is_invalid(validate_texture_usage_for_role(rest, role)));
        }
    }

    #[test]
    fn texture_usage_check_empty_usage_is_rejected() {
        for &role in TextureUsageRole::ALL {
            assert!(is_invalid(validate_texture_usage_for_role(
                TextureUsage::empty(),
                role
            )));
        }
    }

    #[test]
    fn texture_usage_check_all_usages_pass() {
        for &role in TextureUsageRole::ALL {
            assert!(validate_texture_usage_for_role(all_usages(), role).is_ok());
        }
    }

    #[test]
    fn texture_usage_check_storage_atomic_alone_is_not_storage_binding() {
        let atomic = TextureUsage::STORAGE_ATOMIC;
        assert!(is_invalid(validate_texture_usage_for_role(
            atomic,
            TextureUsageRole::StorageBinding
        )));
        assert!(validate_texture_usage_for_role(atomic, TextureUsageRole::StorageAtomic).is_ok());
        let both = TextureUsage::STORAGE_BINDING | TextureUsage::STORAGE_ATOMIC;
        assert!(validate_texture_usage_for_role(both, TextureUsageRole::StorageBinding).is_ok());
        assert!(validate_texture_usage_for_role(both, TextureUsageRole::StorageAtomic).is_ok());
    }

    #[test]
    fn texture_usage_check_roles_require_distinct_single_bits() {
        assert_eq!(TextureUsageRole::ALL.len(), 6);
        let mut seen = 0u32;
        for &role in TextureUsageRole::ALL {
            let bits = role.required_usage().bits();
            assert_eq!(bits.count_ones(), 1);
            assert_eq!(seen & bits, 0);
            seen |= bits;
        }
    }

    #[test]
    fn texture_usage_check_error_names_role_and_usage() {
        let Err(ContractError::InvalidArgument(msg)) = validate_texture_usage_for_role(
            TextureUsage::COPY_SRC,
            TextureUsageRole::RenderAttachment,
        ) else {
            panic!("expected InvalidArgument");
        };
        assert!(msg.contains("render-attachment"));
        assert!(msg.contains("RENDER_ATTACHMENT"));
    }

    #[test]
    fn texture_usage_check_view_format() {
        use TextureFormat::{Bgra8Unorm, Rgba8Unorm, Rgba8UnormSrgb};
        assert!(validate_texture_view_format(Rgba8Unorm, &[], Rgba8Unorm).is_ok());
        assert!(
            validate_texture_view_format(Rgba8Unorm, &[Rgba8UnormSrgb], Rgba8UnormSrgb).is_ok()
        );
        assert!(is_invalid(validate_texture_view_format(
            Rgba8Unorm,
            &[Rgba8UnormSrgb],
            Bgra8Unorm
        )));
        assert!(is_invalid(validate_texture_view_format(
            Rgba8Unorm,
            &[],
            Rgba8UnormSrgb
        )));
        assert!(
            validate_texture_view_format(Rgba8Unorm, &[Bgra8Unorm, Rgba8UnormSrgb], Rgba8UnormSrgb)
                .is_ok()
        );
    }
}
