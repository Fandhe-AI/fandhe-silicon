//! サンプラーの記述子の欄に入る値の型と、記述子・handle の置き場（TASK-373）。
//!
//! 共通の crate の `texture` と並ぶ module で、feature の条件なしで公開される。
//! 背後実装の API は呼ばず、値の列挙と、記述子と、サンプラーを指す不透明な handle を持つ。

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

/// サンプラーの比較関数の値。
///
/// `Default` を実装しない。外の crate の `match` には `_` の腕が要る。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::sampler::CompareFunction;
/// let _ = CompareFunction::default();
/// ```
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::sampler::CompareFunction;
/// fn name(c: CompareFunction) -> &'static str {
///     match c {
///         CompareFunction::Never => "never",
///         CompareFunction::Less => "less",
///         CompareFunction::Equal => "equal",
///         CompareFunction::LessEqual => "less-equal",
///         CompareFunction::Greater => "greater",
///         CompareFunction::NotEqual => "not-equal",
///         CompareFunction::GreaterEqual => "greater-equal",
///         CompareFunction::Always => "always",
///     }
/// }
/// ```
///
/// ```
/// use fandhe_silicon_core::sampler::CompareFunction;
/// fn name(c: CompareFunction) -> &'static str {
///     match c {
///         CompareFunction::Never => "never",
///         CompareFunction::Always => "always",
///         _ => "other",
///     }
/// }
/// assert_eq!(name(CompareFunction::Never), "never");
/// assert_eq!(name(CompareFunction::Less), "other");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompareFunction {
    /// 常に偽（TEX-96）。
    Never,
    /// より小さい（TEX-97）。
    Less,
    /// 等しい（TEX-98）。
    Equal,
    /// 以下（TEX-99）。
    LessEqual,
    /// より大きい（TEX-100）。
    Greater,
    /// 等しくない（TEX-101）。
    NotEqual,
    /// 以上（TEX-102）。
    GreaterEqual,
    /// 常に真（TEX-103）。
    Always,
}

/// サンプラーを作るときに渡す指定の組（TEX-39 のポインタ）。
///
/// 値は検査せず、どの欄もどの値でも保持する。`default()` の後に欄を書き換えて作る。
///
/// ```
/// use fandhe_silicon_core::sampler::{FilterMode, SamplerDescriptor};
/// let mut d = SamplerDescriptor::default();
/// d.mag_filter = FilterMode::Linear;
/// d.anisotropy_clamp = 16;
/// assert_eq!(d.mag_filter, FilterMode::Linear);
/// assert_eq!(d.anisotropy_clamp, 16);
/// assert_eq!(d.min_filter, FilterMode::Nearest);
/// ```
///
/// 外の crate からの構造体式は拒まれる。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::sampler::{FilterMode, SamplerDescriptor};
/// let _ = SamplerDescriptor {
///     mag_filter: FilterMode::Linear,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SamplerDescriptor<'a> {
    /// 名前。
    pub label: Option<&'a str>,
    /// u 軸のアドレスモード。
    pub address_mode_u: AddressMode,
    /// v 軸のアドレスモード。
    pub address_mode_v: AddressMode,
    /// w 軸のアドレスモード。
    pub address_mode_w: AddressMode,
    /// 拡大側のフィルタ（TEX-90）。
    pub mag_filter: FilterMode,
    /// 縮小側のフィルタ（TEX-91）。
    pub min_filter: FilterMode,
    /// mip の段の間のフィルタ（TEX-92）。
    pub mipmap_filter: MipmapFilterMode,
    /// LOD の下限（TEX-93）。
    pub lod_min_clamp: f32,
    /// LOD の上限（TEX-94）。
    pub lod_max_clamp: f32,
    /// 比較関数。
    pub compare: Option<CompareFunction>,
    /// 異方性の上限（TEX-95）。
    pub anisotropy_clamp: u16,
    /// `ClampToBorder` の軸の境界色。
    pub border_color: Option<SamplerBorderColor>,
}

impl Default for SamplerDescriptor<'_> {
    /// 名前なし・3 軸 `ClampToEdge`・両フィルタ `Nearest`・mip `Nearest`・LOD 0.0〜32.0・比較関数なし・異方性 1・境界色なし。
    fn default() -> Self {
        Self {
            label: None,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }
}

/// サンプラーを指す不透明な handle（TEX-39 のポインタ）。
///
/// 欄（世代・識別子・device の識別子）は非公開で、`Clone`・`Copy` を実装しない。
/// `__backend_*` は背後実装だけが使う口。`__backend_new` は safe で、検査しない。
///
/// ```
/// use fandhe_silicon_core::sampler::SamplerHandle;
/// let h = SamplerHandle::__backend_new(5, 3, 1);
/// assert_eq!(h.__backend_generation(), 5);
/// assert_eq!(h.__backend_id(), 3);
/// assert_eq!(h.__backend_device_id(), 1);
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::sampler::SamplerHandle;
/// let _ = SamplerHandle { generation: 5, id: 3, device_id: 1 };
/// ```
///
/// `clone` は呼べない。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::sampler::SamplerHandle;
/// let h = SamplerHandle::__backend_new(5, 3, 1);
/// let _ = h.clone();
/// ```
#[derive(Debug)]
pub struct SamplerHandle {
    generation: u64,
    id: u64,
    device_id: u64,
}

impl SamplerHandle {
    /// 背後実装向けの組み立ての口。検査はしない。
    #[doc(hidden)]
    pub fn __backend_new(generation: u64, id: u64, device_id: u64) -> Self {
        Self {
            generation,
            id,
            device_id,
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
    fn descriptor_default_values() {
        let d = SamplerDescriptor::default();
        assert_eq!(d.label, None);
        assert_eq!(d.address_mode_u, AddressMode::ClampToEdge);
        assert_eq!(d.address_mode_v, AddressMode::ClampToEdge);
        assert_eq!(d.address_mode_w, AddressMode::ClampToEdge);
        assert_eq!(d.mag_filter, FilterMode::Nearest);
        assert_eq!(d.min_filter, FilterMode::Nearest);
        assert_eq!(d.mipmap_filter, MipmapFilterMode::Nearest);
        assert_eq!(d.lod_min_clamp, 0.0);
        assert_eq!(d.lod_max_clamp, 32.0);
        assert_eq!(d.anisotropy_clamp, 1);
        assert_eq!(d.border_color, None);
    }

    #[test]
    fn descriptor_clone_eq_and_modified_ne() {
        let base = SamplerDescriptor::default();
        assert_eq!(base.clone(), base);
        let variants = [
            SamplerDescriptor {
                label: Some("s"),
                ..SamplerDescriptor::default()
            },
            SamplerDescriptor {
                address_mode_u: AddressMode::Repeat,
                ..SamplerDescriptor::default()
            },
            SamplerDescriptor {
                mag_filter: FilterMode::Linear,
                ..SamplerDescriptor::default()
            },
            SamplerDescriptor {
                mipmap_filter: MipmapFilterMode::Linear,
                ..SamplerDescriptor::default()
            },
            SamplerDescriptor {
                lod_max_clamp: 1.0,
                ..SamplerDescriptor::default()
            },
            SamplerDescriptor {
                anisotropy_clamp: 16,
                ..SamplerDescriptor::default()
            },
            SamplerDescriptor {
                border_color: Some(SamplerBorderColor::Zero),
                ..SamplerDescriptor::default()
            },
        ];
        for v in &variants {
            assert_ne!(*v, base);
            assert_eq!(v.clone(), *v);
        }
    }

    #[test]
    fn handle_round_trip() {
        for (g, i, d) in [(0, 0, 0), (1, 2, 3), (u64::MAX, u64::MAX, u64::MAX)] {
            let h = SamplerHandle::__backend_new(g, i, d);
            assert_eq!(h.__backend_generation(), g);
            assert_eq!(h.__backend_id(), i);
            assert_eq!(h.__backend_device_id(), d);
        }
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

    fn compare_index(c: CompareFunction) -> usize {
        match c {
            CompareFunction::Never => 0,
            CompareFunction::Less => 1,
            CompareFunction::Equal => 2,
            CompareFunction::LessEqual => 3,
            CompareFunction::Greater => 4,
            CompareFunction::NotEqual => 5,
            CompareFunction::GreaterEqual => 6,
            CompareFunction::Always => 7,
        }
    }

    const COMPARE: [CompareFunction; 8] = [
        CompareFunction::Never,
        CompareFunction::Less,
        CompareFunction::Equal,
        CompareFunction::LessEqual,
        CompareFunction::Greater,
        CompareFunction::NotEqual,
        CompareFunction::GreaterEqual,
        CompareFunction::Always,
    ];

    #[test]
    fn compare_value_count_and_indices() {
        assert_eq!(COMPARE.len(), 8);
        for (i, v) in COMPARE.iter().enumerate() {
            assert_eq!(compare_index(*v), i);
        }
    }

    #[test]
    fn compare_values_are_pairwise_distinct() {
        for (i, a) in COMPARE.iter().enumerate() {
            for (j, b) in COMPARE.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
        }
    }

    #[test]
    fn descriptor_default_compare_is_none() {
        assert_eq!(SamplerDescriptor::default().compare, None);
    }

    #[test]
    fn descriptor_compare_set_differs_from_default() {
        let base = SamplerDescriptor::default();
        for c in COMPARE {
            let d = SamplerDescriptor {
                compare: Some(c),
                ..SamplerDescriptor::default()
            };
            assert_ne!(d, base);
            assert_eq!(d.clone(), d);
        }
    }
}
