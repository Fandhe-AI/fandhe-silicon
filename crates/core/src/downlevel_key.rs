//! 能力問合せの口へ渡す downlevel の鍵の静的な定数と、その一覧（TASK-1362）。
//!
//! 鍵は `FeatureId` の定数で、26 件ある。背後実装の型・規則は含まない。feature の条件なしで公開される。
//!
//! 鍵の文字列は `downlevel:` に続けて大文字の名前を並べる。

use crate::capability::FeatureId;

/// `downlevel:COMPUTE_SHADERS` の鍵。
pub const DOWNLEVEL_COMPUTE_SHADERS: FeatureId = FeatureId("downlevel:COMPUTE_SHADERS");

/// `downlevel:FRAGMENT_WRITABLE_STORAGE` の鍵。
pub const DOWNLEVEL_FRAGMENT_WRITABLE_STORAGE: FeatureId =
    FeatureId("downlevel:FRAGMENT_WRITABLE_STORAGE");

/// `downlevel:INDIRECT_EXECUTION` の鍵。
pub const DOWNLEVEL_INDIRECT_EXECUTION: FeatureId = FeatureId("downlevel:INDIRECT_EXECUTION");

/// `downlevel:BASE_VERTEX` の鍵。
pub const DOWNLEVEL_BASE_VERTEX: FeatureId = FeatureId("downlevel:BASE_VERTEX");

/// `downlevel:READ_ONLY_DEPTH_STENCIL` の鍵。
pub const DOWNLEVEL_READ_ONLY_DEPTH_STENCIL: FeatureId =
    FeatureId("downlevel:READ_ONLY_DEPTH_STENCIL");

/// `downlevel:NON_POWER_OF_TWO_MIPMAPPED_TEXTURES` の鍵。
pub const DOWNLEVEL_NON_POWER_OF_TWO_MIPMAPPED_TEXTURES: FeatureId =
    FeatureId("downlevel:NON_POWER_OF_TWO_MIPMAPPED_TEXTURES");

/// `downlevel:CUBE_ARRAY_TEXTURES` の鍵。
pub const DOWNLEVEL_CUBE_ARRAY_TEXTURES: FeatureId = FeatureId("downlevel:CUBE_ARRAY_TEXTURES");

/// `downlevel:COMPARISON_SAMPLERS` の鍵。
pub const DOWNLEVEL_COMPARISON_SAMPLERS: FeatureId = FeatureId("downlevel:COMPARISON_SAMPLERS");

/// `downlevel:INDEPENDENT_BLEND` の鍵。
pub const DOWNLEVEL_INDEPENDENT_BLEND: FeatureId = FeatureId("downlevel:INDEPENDENT_BLEND");

/// `downlevel:VERTEX_STORAGE` の鍵。
pub const DOWNLEVEL_VERTEX_STORAGE: FeatureId = FeatureId("downlevel:VERTEX_STORAGE");

/// `downlevel:ANISOTROPIC_FILTERING` の鍵。
pub const DOWNLEVEL_ANISOTROPIC_FILTERING: FeatureId = FeatureId("downlevel:ANISOTROPIC_FILTERING");

/// `downlevel:FRAGMENT_STORAGE` の鍵。
pub const DOWNLEVEL_FRAGMENT_STORAGE: FeatureId = FeatureId("downlevel:FRAGMENT_STORAGE");

/// `downlevel:MULTISAMPLED_SHADING` の鍵。
pub const DOWNLEVEL_MULTISAMPLED_SHADING: FeatureId = FeatureId("downlevel:MULTISAMPLED_SHADING");

/// `downlevel:DEPTH_TEXTURE_AND_BUFFER_COPIES` の鍵。
pub const DOWNLEVEL_DEPTH_TEXTURE_AND_BUFFER_COPIES: FeatureId =
    FeatureId("downlevel:DEPTH_TEXTURE_AND_BUFFER_COPIES");

/// `downlevel:WEBGPU_TEXTURE_FORMAT_SUPPORT` の鍵。
pub const DOWNLEVEL_WEBGPU_TEXTURE_FORMAT_SUPPORT: FeatureId =
    FeatureId("downlevel:WEBGPU_TEXTURE_FORMAT_SUPPORT");

/// `downlevel:BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` の鍵。
pub const DOWNLEVEL_BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED: FeatureId =
    FeatureId("downlevel:BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED");

/// `downlevel:UNRESTRICTED_INDEX_BUFFER` の鍵。
pub const DOWNLEVEL_UNRESTRICTED_INDEX_BUFFER: FeatureId =
    FeatureId("downlevel:UNRESTRICTED_INDEX_BUFFER");

/// `downlevel:FULL_DRAW_INDEX_UINT32` の鍵。
pub const DOWNLEVEL_FULL_DRAW_INDEX_UINT32: FeatureId =
    FeatureId("downlevel:FULL_DRAW_INDEX_UINT32");

/// `downlevel:DEPTH_BIAS_CLAMP` の鍵。
pub const DOWNLEVEL_DEPTH_BIAS_CLAMP: FeatureId = FeatureId("downlevel:DEPTH_BIAS_CLAMP");

/// `downlevel:VIEW_FORMATS` の鍵。
pub const DOWNLEVEL_VIEW_FORMATS: FeatureId = FeatureId("downlevel:VIEW_FORMATS");

/// `downlevel:UNRESTRICTED_EXTERNAL_TEXTURE_COPIES` の鍵。
pub const DOWNLEVEL_UNRESTRICTED_EXTERNAL_TEXTURE_COPIES: FeatureId =
    FeatureId("downlevel:UNRESTRICTED_EXTERNAL_TEXTURE_COPIES");

/// `downlevel:SURFACE_VIEW_FORMATS` の鍵。
pub const DOWNLEVEL_SURFACE_VIEW_FORMATS: FeatureId = FeatureId("downlevel:SURFACE_VIEW_FORMATS");

/// `downlevel:NONBLOCKING_QUERY_RESOLVE` の鍵。
pub const DOWNLEVEL_NONBLOCKING_QUERY_RESOLVE: FeatureId =
    FeatureId("downlevel:NONBLOCKING_QUERY_RESOLVE");

/// `downlevel:SHADER_F16_IN_F32` の鍵。
pub const DOWNLEVEL_SHADER_F16_IN_F32: FeatureId = FeatureId("downlevel:SHADER_F16_IN_F32");

/// `downlevel:MSL2_1` の鍵。
pub const DOWNLEVEL_MSL2_1: FeatureId = FeatureId("downlevel:MSL2_1");

/// `downlevel:TEXTURE_COMPRESSION` の鍵。
pub const DOWNLEVEL_TEXTURE_COMPRESSION: FeatureId = FeatureId("downlevel:TEXTURE_COMPRESSION");

/// downlevel の問合せの鍵の一覧。鍵は互いに異なる。
///
/// # Examples
///
/// ```
/// use fandhe_silicon_core::downlevel_key::{DOWNLEVEL_QUERY_KEYS, DOWNLEVEL_COMPUTE_SHADERS};
///
/// assert_eq!(DOWNLEVEL_QUERY_KEYS.len(), 26);
/// assert!(DOWNLEVEL_QUERY_KEYS.contains(&DOWNLEVEL_COMPUTE_SHADERS));
/// assert_eq!(DOWNLEVEL_COMPUTE_SHADERS.0, "downlevel:COMPUTE_SHADERS");
/// ```
pub const DOWNLEVEL_QUERY_KEYS: &[FeatureId] = &[
    DOWNLEVEL_COMPUTE_SHADERS,
    DOWNLEVEL_FRAGMENT_WRITABLE_STORAGE,
    DOWNLEVEL_INDIRECT_EXECUTION,
    DOWNLEVEL_BASE_VERTEX,
    DOWNLEVEL_READ_ONLY_DEPTH_STENCIL,
    DOWNLEVEL_NON_POWER_OF_TWO_MIPMAPPED_TEXTURES,
    DOWNLEVEL_CUBE_ARRAY_TEXTURES,
    DOWNLEVEL_COMPARISON_SAMPLERS,
    DOWNLEVEL_INDEPENDENT_BLEND,
    DOWNLEVEL_VERTEX_STORAGE,
    DOWNLEVEL_ANISOTROPIC_FILTERING,
    DOWNLEVEL_FRAGMENT_STORAGE,
    DOWNLEVEL_MULTISAMPLED_SHADING,
    DOWNLEVEL_DEPTH_TEXTURE_AND_BUFFER_COPIES,
    DOWNLEVEL_WEBGPU_TEXTURE_FORMAT_SUPPORT,
    DOWNLEVEL_BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED,
    DOWNLEVEL_UNRESTRICTED_INDEX_BUFFER,
    DOWNLEVEL_FULL_DRAW_INDEX_UINT32,
    DOWNLEVEL_DEPTH_BIAS_CLAMP,
    DOWNLEVEL_VIEW_FORMATS,
    DOWNLEVEL_UNRESTRICTED_EXTERNAL_TEXTURE_COPIES,
    DOWNLEVEL_SURFACE_VIEW_FORMATS,
    DOWNLEVEL_NONBLOCKING_QUERY_RESOLVE,
    DOWNLEVEL_SHADER_F16_IN_F32,
    DOWNLEVEL_MSL2_1,
    DOWNLEVEL_TEXTURE_COMPRESSION,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downlevel_query_keys_has_26_entries() {
        assert_eq!(DOWNLEVEL_QUERY_KEYS.len(), 26);
    }

    #[test]
    fn downlevel_query_keys_are_distinct() {
        for (i, a) in DOWNLEVEL_QUERY_KEYS.iter().enumerate() {
            for b in &DOWNLEVEL_QUERY_KEYS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn downlevel_query_keys_have_prefix_and_name() {
        for key in DOWNLEVEL_QUERY_KEYS {
            let rest = key.0.strip_prefix("downlevel:").expect("prefix");
            assert!(!rest.is_empty());
            assert!(
                rest.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            );
        }
    }

    #[test]
    fn constant_names_match_strings() {
        assert_eq!(DOWNLEVEL_QUERY_KEYS[0], DOWNLEVEL_COMPUTE_SHADERS);
        assert_eq!(
            DOWNLEVEL_QUERY_KEYS.last().copied(),
            Some(DOWNLEVEL_TEXTURE_COMPRESSION)
        );
        assert_eq!(DOWNLEVEL_MSL2_1.0, "downlevel:MSL2_1");
        assert_eq!(
            DOWNLEVEL_WEBGPU_TEXTURE_FORMAT_SUPPORT.0,
            "downlevel:WEBGPU_TEXTURE_FORMAT_SUPPORT"
        );
        assert_eq!(
            DOWNLEVEL_BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED.0,
            "downlevel:BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED"
        );
    }
}
