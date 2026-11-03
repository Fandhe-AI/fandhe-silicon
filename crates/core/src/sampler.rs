//! サンプラーの記述子の欄に入る値の型の置き場（TASK-373）。
//!
//! 共通の crate の `texture` と並ぶ module で、feature の条件なしで公開される。
//! 背後実装の API は呼ばず、値の列挙だけを持つ。

/// アドレスモード。テクスチャの範囲の外を引いたときの座標の扱いを指す。
///
/// 外の crate の `match` には `_` の腕が要る。
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::sampler::AddressMode;
/// fn name(m: AddressMode) -> &'static str {
///     match m {
///         AddressMode::ClampToEdge => "clamp",
///         AddressMode::Repeat => "repeat",
///         AddressMode::MirrorRepeat => "mirror",
///         AddressMode::ClampToBorder => "border",
///     }
/// }
/// ```
///
/// ```
/// use fandhe_silicon_core::sampler::AddressMode;
/// fn name(m: AddressMode) -> &'static str {
///     match m {
///         AddressMode::ClampToEdge => "clamp",
///         _ => "other",
///     }
/// }
/// assert_eq!(name(AddressMode::ClampToEdge), "clamp");
/// assert_eq!(name(AddressMode::Repeat), "other");
/// ```
///
/// 既定の値は `ClampToEdge`。
///
/// ```
/// use fandhe_silicon_core::sampler::AddressMode;
/// assert_eq!(AddressMode::default(), AddressMode::ClampToEdge);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AddressMode {
    /// 端の画素を延ばす（TEX-82）。
    #[default]
    ClampToEdge,
    /// 範囲を繰り返す（TEX-83）。
    Repeat,
    /// 範囲を折り返しながら繰り返す（TEX-84）。
    MirrorRepeat,
    /// 範囲の外を境界色にする（TEX-85）。
    ClampToBorder,
}

/// 拡大・縮小のフィルタ。既定の値は `Nearest`。
///
/// ```
/// use fandhe_silicon_core::sampler::FilterMode;
/// assert_eq!(FilterMode::default(), FilterMode::Nearest);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FilterMode {
    /// 最も近い画素を取る。
    #[default]
    Nearest,
    /// 近傍の画素を線形に混ぜる。
    Linear,
}

/// mip の段の間のフィルタ。既定の値は `Nearest`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MipmapFilterMode {
    /// 最も近い段を取る。
    #[default]
    Nearest,
    /// 隣り合う段を線形に混ぜる。
    Linear,
}

/// 境界色の指定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SamplerBorderColor {
    /// 透明な黒（TEX-86）。
    TransparentBlack,
    /// 不透明な黒（TEX-87）。
    OpaqueBlack,
    /// 不透明な白（TEX-88）。
    OpaqueWhite,
    /// 全チャネル 0 の境界色の指定（TEX-89）。
    Zero,
}

#[cfg(test)]
mod tests {
    use super::*;

    // variant が増えると `_` の無い match がコンパイルで落ちる。
    fn address_index(m: AddressMode) -> usize {
        match m {
            AddressMode::ClampToEdge => 0,
            AddressMode::Repeat => 1,
            AddressMode::MirrorRepeat => 2,
            AddressMode::ClampToBorder => 3,
        }
    }
    fn filter_index(m: FilterMode) -> usize {
        match m {
            FilterMode::Nearest => 0,
            FilterMode::Linear => 1,
        }
    }
    fn mipmap_index(m: MipmapFilterMode) -> usize {
        match m {
            MipmapFilterMode::Nearest => 0,
            MipmapFilterMode::Linear => 1,
        }
    }
    fn border_index(m: SamplerBorderColor) -> usize {
        match m {
            SamplerBorderColor::TransparentBlack => 0,
            SamplerBorderColor::OpaqueBlack => 1,
            SamplerBorderColor::OpaqueWhite => 2,
            SamplerBorderColor::Zero => 3,
        }
    }

    const ADDRESS: [AddressMode; 4] = [
        AddressMode::ClampToEdge,
        AddressMode::Repeat,
        AddressMode::MirrorRepeat,
        AddressMode::ClampToBorder,
    ];
    const FILTER: [FilterMode; 2] = [FilterMode::Nearest, FilterMode::Linear];
    const MIPMAP: [MipmapFilterMode; 2] = [MipmapFilterMode::Nearest, MipmapFilterMode::Linear];
    const BORDER: [SamplerBorderColor; 4] = [
        SamplerBorderColor::TransparentBlack,
        SamplerBorderColor::OpaqueBlack,
        SamplerBorderColor::OpaqueWhite,
        SamplerBorderColor::Zero,
    ];

    #[test]
    fn value_counts_and_indices() {
        for (i, v) in ADDRESS.iter().enumerate() {
            assert_eq!(address_index(*v), i);
        }
        for (i, v) in FILTER.iter().enumerate() {
            assert_eq!(filter_index(*v), i);
        }
        for (i, v) in MIPMAP.iter().enumerate() {
            assert_eq!(mipmap_index(*v), i);
        }
        for (i, v) in BORDER.iter().enumerate() {
            assert_eq!(border_index(*v), i);
        }
    }

    #[test]
    fn defaults() {
        assert_eq!(AddressMode::default(), AddressMode::ClampToEdge);
        assert_eq!(FilterMode::default(), FilterMode::Nearest);
        assert_eq!(MipmapFilterMode::default(), MipmapFilterMode::Nearest);
    }

    #[test]
    fn values_are_pairwise_distinct() {
        for (i, a) in ADDRESS.iter().enumerate() {
            for (j, b) in ADDRESS.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
        }
        for (i, a) in FILTER.iter().enumerate() {
            for (j, b) in FILTER.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
        }
        for (i, a) in MIPMAP.iter().enumerate() {
            for (j, b) in MIPMAP.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
        }
        for (i, a) in BORDER.iter().enumerate() {
            for (j, b) in BORDER.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
        }
    }
}
