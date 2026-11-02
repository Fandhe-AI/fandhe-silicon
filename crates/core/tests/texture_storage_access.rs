//! storage texture のアクセスの型と、束縛の用途・シェーダーとの照合の検査を、別の crate から公開のパスだけで項目ごとに確かめる（TASK-1944 のポインタ）。
//!
//! feature なしで流れる。拒否は `ContractError::InvalidArgument` の種類と、照合の文言の先頭の `C2:` までを照らす。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::texture::{
    StorageTextureAccess, TextureUsage, validate_storage_texture_shader_access,
    validate_storage_texture_usage,
};
use fandhe_silicon_core::texture_format::TextureFormat;

use StorageTextureAccess::{Atomic, ReadOnly, ReadWrite, WriteOnly};

/// 拒否する用途の入力の組。
type UsageCase = (TextureUsage, StorageTextureAccess);
/// 拒否する照合の入力の 4 つ組。
type ShaderCase = (
    TextureFormat,
    StorageTextureAccess,
    TextureFormat,
    StorageTextureAccess,
);

/// 種類が `InvalidArgument` かを返す。
fn is_invalid(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

/// `InvalidArgument` で、文言が `C2:` で始まるかを返す。
fn is_c2(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with("C2:"))
}

/// 7 つの用途の和（過申告の入力）。
fn over_declared() -> TextureUsage {
    TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST
        | TextureUsage::TEXTURE_BINDING
        | TextureUsage::STORAGE_BINDING
        | TextureUsage::RENDER_ATTACHMENT
        | TextureUsage::TRANSIENT_ATTACHMENT
        | TextureUsage::STORAGE_ATOMIC
}

/// 形式 `R32Float` でそろえて照合を呼ぶ。
fn shader(layout: StorageTextureAccess, sh: StorageTextureAccess) -> Result<(), ContractError> {
    validate_storage_texture_shader_access(
        TextureFormat::R32Float,
        layout,
        TextureFormat::R32Float,
        sh,
    )
}

/// atomic 以外のアクセスの用途の入力を確かめる。
fn check_non_atomic_usage(access: StorageTextureAccess) {
    assert!(validate_storage_texture_usage(TextureUsage::STORAGE_BINDING, access).is_ok());
    assert!(validate_storage_texture_usage(over_declared(), access).is_ok());
    assert!(is_invalid(&validate_storage_texture_usage(
        TextureUsage::TEXTURE_BINDING,
        access
    )));
    assert!(is_invalid(&validate_storage_texture_usage(
        TextureUsage::empty(),
        access
    )));
}

/// 拒否する用途の入力の一覧。
fn rejected_usage_cases() -> Vec<UsageCase> {
    vec![
        (TextureUsage::TEXTURE_BINDING, WriteOnly),
        (TextureUsage::empty(), ReadOnly),
        (TextureUsage::empty(), ReadWrite),
        (TextureUsage::STORAGE_BINDING, Atomic),
        (TextureUsage::STORAGE_ATOMIC, Atomic),
        (TextureUsage::empty(), Atomic),
    ]
}

/// 拒否する照合の入力の一覧。
fn rejected_shader_cases() -> Vec<ShaderCase> {
    let f = TextureFormat::R32Float;
    vec![
        (f, ReadOnly, f, WriteOnly),
        (f, ReadOnly, f, ReadWrite),
        (f, ReadOnly, f, Atomic),
        (f, ReadWrite, f, ReadOnly),
        (f, ReadWrite, f, Atomic),
        (f, WriteOnly, f, ReadOnly),
        (f, WriteOnly, f, ReadWrite),
        (f, WriteOnly, f, Atomic),
        (f, Atomic, f, WriteOnly),
        (f, Atomic, f, ReadOnly),
        (f, Atomic, f, ReadWrite),
        (f, WriteOnly, TextureFormat::R32Uint, WriteOnly),
        (
            TextureFormat::Rgba8Unorm,
            ReadOnly,
            TextureFormat::Rgba8UnormSrgb,
            ReadOnly,
        ),
    ]
}

#[test]
fn access_values() {
    assert_eq!(
        StorageTextureAccess::ALL,
        &[WriteOnly, ReadOnly, ReadWrite, Atomic]
    );
    assert!(WriteOnly.writes() && !WriteOnly.reads());
    assert!(!ReadOnly.writes() && ReadOnly.reads());
    assert!(ReadWrite.writes() && ReadWrite.reads());
    assert!(Atomic.writes() && Atomic.reads());
}

// TEX-78
#[test]
fn write_only_binding_usage() {
    check_non_atomic_usage(WriteOnly);
}

// TEX-79
#[test]
fn read_only_binding_usage() {
    check_non_atomic_usage(ReadOnly);
}

// TEX-80
#[test]
fn read_write_binding_usage() {
    check_non_atomic_usage(ReadWrite);
}

// TEX-81
#[test]
fn atomic_binding_usage() {
    let both = TextureUsage::STORAGE_BINDING | TextureUsage::STORAGE_ATOMIC;
    assert!(validate_storage_texture_usage(both, Atomic).is_ok());
    assert!(validate_storage_texture_usage(over_declared(), Atomic).is_ok());
    for u in [
        TextureUsage::STORAGE_BINDING,
        TextureUsage::STORAGE_ATOMIC,
        TextureUsage::empty(),
    ] {
        assert!(is_invalid(&validate_storage_texture_usage(u, Atomic)));
    }
}

// TEX-79
#[test]
fn read_only_layout_rejects_writing_shader() {
    for s in [WriteOnly, ReadWrite, Atomic] {
        assert!(is_c2(&shader(ReadOnly, s)));
    }
    assert!(shader(ReadOnly, ReadOnly).is_ok());
}

// TEX-80
#[test]
fn read_write_layout_shader_access() {
    assert!(shader(ReadWrite, ReadWrite).is_ok());
    assert!(shader(ReadWrite, WriteOnly).is_ok());
    assert!(is_c2(&shader(ReadWrite, ReadOnly)));
    assert!(is_c2(&shader(ReadWrite, Atomic)));
}

// TEX-78
#[test]
fn write_only_layout_shader_access() {
    for &s in StorageTextureAccess::ALL {
        let r = shader(WriteOnly, s);
        if s == WriteOnly {
            assert!(r.is_ok());
        } else {
            assert!(is_c2(&r));
        }
    }
}

// TEX-81
#[test]
fn atomic_layout_shader_access() {
    for &s in StorageTextureAccess::ALL {
        let r = shader(Atomic, s);
        if s == Atomic {
            assert!(r.is_ok());
        } else {
            assert!(is_c2(&r));
        }
    }
}

#[test]
fn shader_format_must_match() {
    let pairs = [
        (TextureFormat::R32Float, TextureFormat::R32Uint),
        (TextureFormat::R32Uint, TextureFormat::R32Float),
        (TextureFormat::Rgba8Unorm, TextureFormat::Rgba8UnormSrgb),
        (TextureFormat::Rgba8UnormSrgb, TextureFormat::Rgba8Unorm),
    ];
    for &a in StorageTextureAccess::ALL {
        for (lf, sf) in pairs {
            assert!(is_c2(&validate_storage_texture_shader_access(lf, a, sf, a)));
        }
    }
}

#[test]
fn rejection_is_repeatable() {
    for (u, a) in rejected_usage_cases() {
        let first = validate_storage_texture_usage(u, a);
        assert!(first.is_err());
        assert_eq!(first, validate_storage_texture_usage(u, a));
    }
    for (lf, la, sf, sa) in rejected_shader_cases() {
        let first = validate_storage_texture_shader_access(lf, la, sf, sa);
        assert!(first.is_err());
        assert_eq!(
            first,
            validate_storage_texture_shader_access(lf, la, sf, sa)
        );
    }
    let ok = TextureUsage::STORAGE_BINDING;
    assert_eq!(validate_storage_texture_usage(ok, WriteOnly), Ok(()));
    assert_eq!(validate_storage_texture_usage(ok, WriteOnly), Ok(()));
    assert_eq!(shader(ReadWrite, WriteOnly), Ok(()));
    assert_eq!(shader(ReadWrite, WriteOnly), Ok(()));
}

#[test]
fn rejection_is_invalid_argument_only() {
    for (u, a) in rejected_usage_cases() {
        assert!(is_invalid(&validate_storage_texture_usage(u, a)));
    }
    for (lf, la, sf, sa) in rejected_shader_cases() {
        assert!(is_invalid(&validate_storage_texture_shader_access(
            lf, la, sf, sa
        )));
    }
}
