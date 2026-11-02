//! storage texture の束縛のアクセスの値の型の置き場（TASK-1944）。
//!
//! `texture` モジュールの一部で、公開のパスは `fandhe_silicon_core::texture::…`。
//! 値だけを扱う型で、背後実装の API は呼ばない。
//! 書く・読むの問いは [`StorageTextureAccess::writes`]・[`StorageTextureAccess::reads`] が返す。
//! 束縛の用途と、配置とシェーダーの形式・アクセスの照合を値だけで照らす純粋関数も置く。

use super::{TextureUsage, TextureUsageRole, validate_texture_usage_for_role};
use crate::error::ContractError;
use crate::texture_format::TextureFormat;

/// storage texture の束縛のアクセス。書く・読むは `writes`・`reads` で問う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StorageTextureAccess {
    /// 書き込むだけ（TEX-78）。
    WriteOnly,
    /// 読むだけ（TEX-79）。
    ReadOnly,
    /// 読みと書き込み（TEX-80）。
    ReadWrite,
    /// atomic の操作（TEX-81）。
    Atomic,
}

impl StorageTextureAccess {
    /// 4 値を宣言の順に並べた一覧。
    pub const ALL: &'static [Self] = &[
        Self::WriteOnly,
        Self::ReadOnly,
        Self::ReadWrite,
        Self::Atomic,
    ];

    /// 書き込みを含むなら真。
    ///
    /// ```
    /// use fandhe_silicon_core::texture::StorageTextureAccess;
    /// assert!(StorageTextureAccess::WriteOnly.writes());
    /// assert!(!StorageTextureAccess::ReadOnly.writes());
    /// ```
    pub const fn writes(self) -> bool {
        match self {
            Self::WriteOnly | Self::ReadWrite | Self::Atomic => true,
            Self::ReadOnly => false,
        }
    }

    /// 読みを含むなら真。
    ///
    /// ```
    /// use fandhe_silicon_core::texture::StorageTextureAccess;
    /// assert!(StorageTextureAccess::ReadOnly.reads());
    /// assert!(!StorageTextureAccess::WriteOnly.reads());
    /// ```
    pub const fn reads(self) -> bool {
        match self {
            Self::ReadOnly | Self::ReadWrite | Self::Atomic => true,
            Self::WriteOnly => false,
        }
    }
}

/// storage texture の束縛に要る用途が、申告した用途にそろっているかを判定する。
///
/// `declared` は作成の時に記録した用途を渡す。`Atomic` のときは atomic の用途も要る。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::texture::{StorageTextureAccess, TextureUsage, validate_storage_texture_usage};
///
/// let u = TextureUsage::STORAGE_BINDING;
/// assert!(validate_storage_texture_usage(u, StorageTextureAccess::ReadWrite).is_ok());
/// assert!(matches!(
///     validate_storage_texture_usage(u, StorageTextureAccess::Atomic),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// let both = TextureUsage::STORAGE_BINDING | TextureUsage::STORAGE_ATOMIC;
/// assert!(validate_storage_texture_usage(both, StorageTextureAccess::Atomic).is_ok());
/// ```
///
/// # Errors
///
/// 要る用途が無いとき `ContractError::InvalidArgument`。
pub fn validate_storage_texture_usage(
    declared: TextureUsage,
    access: StorageTextureAccess,
) -> Result<(), ContractError> {
    validate_texture_usage_for_role(declared, TextureUsageRole::StorageBinding)?;
    if access == StorageTextureAccess::Atomic {
        validate_texture_usage_for_role(declared, TextureUsageRole::StorageAtomic)?;
    }
    Ok(())
}

/// 配置側の形式・アクセスとシェーダー側の形式・アクセスを照合する。
///
/// 形式は完全一致だけを通す。アクセスは同じ値、または配置が `ReadWrite` でシェーダーが `WriteOnly` のときだけ通す。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::texture::{StorageTextureAccess as A, validate_storage_texture_shader_access};
/// use fandhe_silicon_core::texture_format::TextureFormat as F;
///
/// assert!(validate_storage_texture_shader_access(F::R32Float, A::ReadWrite, F::R32Float, A::WriteOnly).is_ok());
/// assert!(matches!(
///     validate_storage_texture_shader_access(F::R32Float, A::ReadOnly, F::R32Float, A::WriteOnly),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// assert!(matches!(
///     validate_storage_texture_shader_access(F::R32Float, A::WriteOnly, F::R32Uint, A::WriteOnly),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 形式の不一致、または許さないアクセスの組のとき `ContractError::InvalidArgument`。
pub fn validate_storage_texture_shader_access(
    layout_format: TextureFormat,
    layout_access: StorageTextureAccess,
    shader_format: TextureFormat,
    shader_access: StorageTextureAccess,
) -> Result<(), ContractError> {
    if layout_format != shader_format {
        return Err(ContractError::InvalidArgument(format!(
            "C2: shader format {shader_format:?} does not match layout format {layout_format:?}"
        )));
    }
    if layout_access == shader_access
        || matches!(
            (layout_access, shader_access),
            (
                StorageTextureAccess::ReadWrite,
                StorageTextureAccess::WriteOnly
            )
        )
    {
        return Ok(());
    }
    Err(ContractError::InvalidArgument(format!(
        "C2: shader access {shader_access:?} is not allowed for layout access {layout_access:?}"
    )))
}

#[cfg(test)]
mod tests {
    use super::StorageTextureAccess::{self, *};
    use super::*;

    fn is_invalid<T: std::fmt::Debug>(r: Result<T, ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn storage_access_usage_table() {
        let b = TextureUsage::STORAGE_BINDING;
        let a = TextureUsage::STORAGE_ATOMIC;
        let all = TextureUsage::COPY_SRC
            | TextureUsage::COPY_DST
            | TextureUsage::TEXTURE_BINDING
            | b
            | TextureUsage::RENDER_ATTACHMENT
            | TextureUsage::TRANSIENT_ATTACHMENT
            | a;
        let declared = [
            (b, true, false),
            (b | a, true, true),
            (a, false, false),
            (TextureUsage::empty(), false, false),
            (all, true, true),
        ];
        for (d, non_atomic_ok, atomic_ok) in declared {
            for acc in StorageTextureAccess::ALL {
                let want = if *acc == Atomic {
                    atomic_ok
                } else {
                    non_atomic_ok
                };
                let r = validate_storage_texture_usage(d, *acc);
                assert_eq!(r.is_ok(), want, "{d:?} {acc:?}");
                if !want {
                    assert!(is_invalid(r));
                }
            }
        }
    }

    #[test]
    fn storage_access_shader_access_table() {
        let f = TextureFormat::R32Float;
        for l in StorageTextureAccess::ALL {
            for s in StorageTextureAccess::ALL {
                let r = validate_storage_texture_shader_access(f, *l, f, *s);
                let ok = l == s || (*l == ReadWrite && *s == WriteOnly);
                assert_eq!(r.is_ok(), ok, "{l:?} {s:?}");
                if !ok {
                    assert!(is_invalid(r));
                }
            }
        }
    }

    #[test]
    fn storage_access_shader_format_mismatch() {
        for (x, y) in [
            (TextureFormat::R32Float, TextureFormat::R32Uint),
            (TextureFormat::Rgba8Unorm, TextureFormat::Rgba8UnormSrgb),
        ] {
            assert!(is_invalid(validate_storage_texture_shader_access(
                x, WriteOnly, y, WriteOnly
            )));
        }
    }

    #[test]
    fn storage_access_rejection_messages_start_with_c2() {
        let f = TextureFormat::R32Float;
        for r in [
            validate_storage_texture_shader_access(f, ReadOnly, f, WriteOnly),
            validate_storage_texture_shader_access(f, ReadOnly, TextureFormat::R32Uint, ReadOnly),
        ] {
            match r {
                Err(ContractError::InvalidArgument(m)) => assert!(m.starts_with("C2:"), "{m}"),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn storage_access_all_has_four_distinct_values() {
        let all = StorageTextureAccess::ALL;
        assert_eq!(all.len(), 4);
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j]);
            }
        }
        assert_eq!(all, &[WriteOnly, ReadOnly, ReadWrite, Atomic]);
    }

    #[test]
    fn storage_access_writes_reads_table() {
        let table = [
            (WriteOnly, true, false),
            (ReadOnly, false, true),
            (ReadWrite, true, true),
            (Atomic, true, true),
        ];
        for (a, w, r) in table {
            assert_eq!(a.writes(), w, "{a:?} writes");
            assert_eq!(a.reads(), r, "{a:?} reads");
        }
    }

    #[test]
    fn storage_access_every_value_reads_or_writes() {
        for a in StorageTextureAccess::ALL {
            assert!(a.writes() || a.reads(), "{a:?}");
        }
    }
}
