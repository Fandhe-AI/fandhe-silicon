//! テクスチャの形式ごとの能力の値の型と、その値を作る組み立ての口と読み出し（TASK-2566。ポインタ）。
//!
//! 形式の単位の能力の値 1 つと、18 の要素（[`TextureFormatFeature`]）ごとの能力の値を持つ
//! [`TextureFormatFeatures`] を、feature の条件なしで公開する。値は純粋なデータで、背後実装の型を含まない。
//! 組み立ての口 [`TextureFormatFeatures::new`] は共通の規則に外れる入力を `Err` で拒むので、
//! 外から規則に外れる値は作れない。項目は COVT-131。

use crate::capability::{CapabilityLevel, CapabilityQuery};
use crate::capability_table::UNLISTED_FEATURE_QUERY;
use crate::error::ContractError;
use crate::texture::TextureUsage;

/// 形式ごとの能力の要素の数。
const FEATURE_COUNT: usize = 18;

/// 形式ごとの能力の要素。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureFormatFeature {
    /// フィルタ可。
    Filterable,
    /// ブレンド可。
    Blendable,
    /// サンプル数 2 のマルチサンプル。
    Multisample2,
    /// サンプル数 4 のマルチサンプル。
    Multisample4,
    /// サンプル数 8 のマルチサンプル。
    Multisample8,
    /// サンプル数 16 のマルチサンプル。
    Multisample16,
    /// マルチサンプルの解決。
    MultisampleResolve,
    /// storage の読み取り専用。
    StorageReadOnly,
    /// storage の書き込み専用。
    StorageWriteOnly,
    /// storage の読み書き。
    StorageReadWrite,
    /// storage の atomic。
    StorageAtomic,
    /// 用途 `COPY_SRC`。
    UsageCopySrc,
    /// 用途 `COPY_DST`。
    UsageCopyDst,
    /// 用途 `TEXTURE_BINDING`。
    UsageTextureBinding,
    /// 用途 `STORAGE_BINDING`。
    UsageStorageBinding,
    /// 用途 `RENDER_ATTACHMENT`。
    UsageRenderAttachment,
    /// 用途 `TRANSIENT_ATTACHMENT`。
    UsageTransientAttachment,
    /// 用途 `STORAGE_ATOMIC`。
    UsageStorageAtomic,
}

impl TextureFormatFeature {
    /// 全 18 要素。宣言の順。
    pub const ALL: &'static [Self] = &[
        Self::Filterable,
        Self::Blendable,
        Self::Multisample2,
        Self::Multisample4,
        Self::Multisample8,
        Self::Multisample16,
        Self::MultisampleResolve,
        Self::StorageReadOnly,
        Self::StorageWriteOnly,
        Self::StorageReadWrite,
        Self::StorageAtomic,
        Self::UsageCopySrc,
        Self::UsageCopyDst,
        Self::UsageTextureBinding,
        Self::UsageStorageBinding,
        Self::UsageRenderAttachment,
        Self::UsageTransientAttachment,
        Self::UsageStorageAtomic,
    ];

    /// 欄の添字。網羅の `match` で、要素を足すとコンパイルが落ちる。
    const fn index(self) -> usize {
        match self {
            Self::Filterable => 0,
            Self::Blendable => 1,
            Self::Multisample2 => 2,
            Self::Multisample4 => 3,
            Self::Multisample8 => 4,
            Self::Multisample16 => 5,
            Self::MultisampleResolve => 6,
            Self::StorageReadOnly => 7,
            Self::StorageWriteOnly => 8,
            Self::StorageReadWrite => 9,
            Self::StorageAtomic => 10,
            Self::UsageCopySrc => 11,
            Self::UsageCopyDst => 12,
            Self::UsageTextureBinding => 13,
            Self::UsageStorageBinding => 14,
            Self::UsageRenderAttachment => 15,
            Self::UsageTransientAttachment => 16,
            Self::UsageStorageAtomic => 17,
        }
    }

    /// 用途の要素に対応する用途のビット。用途でない要素は `None`。
    const fn usage(self) -> Option<TextureUsage> {
        match self {
            Self::UsageCopySrc => Some(TextureUsage::COPY_SRC),
            Self::UsageCopyDst => Some(TextureUsage::COPY_DST),
            Self::UsageTextureBinding => Some(TextureUsage::TEXTURE_BINDING),
            Self::UsageStorageBinding => Some(TextureUsage::STORAGE_BINDING),
            Self::UsageRenderAttachment => Some(TextureUsage::RENDER_ATTACHMENT),
            Self::UsageTransientAttachment => Some(TextureUsage::TRANSIENT_ATTACHMENT),
            Self::UsageStorageAtomic => Some(TextureUsage::STORAGE_ATOMIC),
            Self::Filterable
            | Self::Blendable
            | Self::Multisample2
            | Self::Multisample4
            | Self::Multisample8
            | Self::Multisample16
            | Self::MultisampleResolve
            | Self::StorageReadOnly
            | Self::StorageWriteOnly
            | Self::StorageReadWrite
            | Self::StorageAtomic => None,
        }
    }
}

/// 段階が `S1`〜`S6`（使える段階）のとき `true`。
const fn is_usable(level: CapabilityLevel) -> bool {
    match level {
        CapabilityLevel::S1
        | CapabilityLevel::S2
        | CapabilityLevel::S3
        | CapabilityLevel::S4
        | CapabilityLevel::S5
        | CapabilityLevel::S6 => true,
        CapabilityLevel::S0 | CapabilityLevel::Unsupported => false,
    }
}

/// 根拠が要る段階（`S3`〜`S6`）のとき `true`。
const fn requires_evidence(level: CapabilityLevel) -> bool {
    matches!(
        level,
        CapabilityLevel::S3 | CapabilityLevel::S4 | CapabilityLevel::S5 | CapabilityLevel::S6
    )
}

/// 段階が `S0` か `Unsupported`（要素を持てない形式の単位）のとき `true`。
const fn is_unit_only(level: CapabilityLevel) -> bool {
    matches!(level, CapabilityLevel::S0 | CapabilityLevel::Unsupported)
}

fn invalid(msg: String) -> ContractError {
    ContractError::InvalidArgument(msg)
}

/// 形式の単位の値を共通の規則に照らす。
fn check_format_query(q: &CapabilityQuery) -> Result<(), ContractError> {
    match q.level {
        CapabilityLevel::S0 => {
            if q.unsupported_reason.is_some() || q.evidence.is_some() {
                return Err(invalid(
                    "format query: S0 must not carry a reason or evidence".to_string(),
                ));
            }
        }
        CapabilityLevel::Unsupported => {
            if q.unsupported_reason.is_none() {
                return Err(invalid(
                    "format query: Unsupported must carry a reason".to_string(),
                ));
            }
        }
        _ => {
            if q.unsupported_reason.is_some() {
                return Err(invalid(
                    "format query: a supported level must not carry a reason".to_string(),
                ));
            }
            if requires_evidence(q.level) && q.evidence.is_none() {
                return Err(invalid(
                    "format query: S3..S6 must carry evidence".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// 要素の値を共通の規則に照らす。
fn check_feature_query(f: TextureFormatFeature, q: &CapabilityQuery) -> Result<(), ContractError> {
    if q.level == CapabilityLevel::S0 {
        return Err(invalid(format!("feature {f:?}: S0 is not allowed")));
    }
    let has_reason = q.unsupported_reason.is_some();
    if q.level == CapabilityLevel::Unsupported {
        if !has_reason {
            return Err(invalid(format!(
                "feature {f:?}: Unsupported must carry a reason"
            )));
        }
    } else if has_reason {
        return Err(invalid(format!(
            "feature {f:?}: a supported level must not carry a reason"
        )));
    }
    if requires_evidence(q.level) && q.evidence.is_none() {
        return Err(invalid(format!(
            "feature {f:?}: S3..S6 must carry evidence"
        )));
    }
    Ok(())
}

/// 形式ごとの能力の値。形式の単位の値 1 つと、要素ごとの値（持たない要素もある）から成る。
///
/// 値は [`TextureFormatFeatures::new`] か [`TextureFormatFeatures::capture_failed`] でだけ作れ、
/// 欄は非公開。
///
/// ```
/// use fandhe_silicon_core::capability::{CapabilityEvidence, CapabilityLevel, CapabilityQuery};
/// use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
/// use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
/// use fandhe_silicon_core::texture::TextureUsage;
/// use fandhe_silicon_core::texture_format_features::{
///     TextureFormatFeature as F, TextureFormatFeatures,
/// };
///
/// let s2 = CapabilityQuery {
///     level: CapabilityLevel::S2,
///     unsupported_reason: None,
///     evidence: Some(CapabilityEvidence::RuntimeApi),
/// };
/// let v = TextureFormatFeatures::new(s2, &[(F::Filterable, s2), (F::UsageTextureBinding, s2)])
///     .unwrap();
/// assert_eq!(v.feature(F::Filterable), s2);
/// assert_eq!(v.feature(F::Blendable), UNLISTED_FEATURE_QUERY);
/// assert_eq!(v.allowed_usages(), TextureUsage::TEXTURE_BINDING);
/// assert_eq!(v.supported_sample_counts(), vec![1]);
///
/// let unsupported = CapabilityQuery {
///     level: CapabilityLevel::Unsupported,
///     unsupported_reason: Some(UnsupportedReason::NotSupported),
///     evidence: None,
/// };
/// let r = TextureFormatFeatures::new(unsupported, &[(F::Filterable, s2)]);
/// assert!(matches!(r, Err(ContractError::InvalidArgument(_))));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureFormatFeatures {
    format_query: CapabilityQuery,
    features: [Option<CapabilityQuery>; FEATURE_COUNT],
}

impl TextureFormatFeatures {
    /// 形式の単位の値と要素ごとの値から組み立てる。
    ///
    /// # Errors
    ///
    /// 次を順に照らし、最初に当たった 1 件を `ContractError::InvalidArgument` で返す。
    ///
    /// 1. 形式の単位の値: `S0` は理由も根拠も持たない。`Unsupported` は理由を持つ。
    ///    `S1`〜`S6` は理由を持たず、`S3`〜`S6` は根拠を持つ。
    /// 2. 形式の単位が `S0` か `Unsupported` で、`features` が空でない。
    /// 3. `features` の各要素を入力の順に: 段階が `S0`、`Unsupported` で理由が無い、
    ///    `Unsupported` でないのに理由がある、`S3`〜`S6` で根拠が無い、同じ要素が 2 回目に出る。
    /// 4. 用途と旗の組: `UsageStorageBinding` が `S1`〜`S6` でないのに storage の 4 つの旗
    ///    （`StorageReadOnly`・`StorageWriteOnly`・`StorageReadWrite`・`StorageAtomic`）のどれかが
    ///    `S1`〜`S6`。`UsageStorageAtomic` が `S1`〜`S6` なのに `StorageAtomic` が `S1`〜`S6` でない。
    pub fn new(
        format_query: CapabilityQuery,
        features: &[(TextureFormatFeature, CapabilityQuery)],
    ) -> Result<Self, ContractError> {
        check_format_query(&format_query)?;
        if is_unit_only(format_query.level) && !features.is_empty() {
            return Err(invalid(format!(
                "features: a {:?} format must not carry feature entries",
                format_query.level
            )));
        }
        let mut slots: [Option<CapabilityQuery>; FEATURE_COUNT] = [None; FEATURE_COUNT];
        for (f, q) in features {
            check_feature_query(*f, q)?;
            let slot = &mut slots[f.index()];
            if slot.is_some() {
                return Err(invalid(format!("feature {f:?}: given more than once")));
            }
            *slot = Some(*q);
        }
        let usable = |f: TextureFormatFeature| slots[f.index()].is_some_and(|q| is_usable(q.level));
        if !usable(TextureFormatFeature::UsageStorageBinding) {
            for f in [
                TextureFormatFeature::StorageReadOnly,
                TextureFormatFeature::StorageWriteOnly,
                TextureFormatFeature::StorageReadWrite,
                TextureFormatFeature::StorageAtomic,
            ] {
                if usable(f) {
                    return Err(invalid(format!(
                        "feature {f:?}: usable without UsageStorageBinding"
                    )));
                }
            }
        }
        if usable(TextureFormatFeature::UsageStorageAtomic)
            && !usable(TextureFormatFeature::StorageAtomic)
        {
            return Err(invalid(
                "feature UsageStorageAtomic: usable without StorageAtomic".to_string(),
            ));
        }
        Ok(Self {
            format_query,
            features: slots,
        })
    }

    /// 形式の単位が `S0`（理由・根拠なし）で、要素を持たない値。失敗しない。
    pub const fn capture_failed() -> Self {
        Self {
            format_query: CapabilityQuery {
                level: CapabilityLevel::S0,
                unsupported_reason: None,
                evidence: None,
            },
            features: [None; FEATURE_COUNT],
        }
    }

    /// 形式の単位の値。
    #[must_use]
    pub fn format_query(&self) -> CapabilityQuery {
        self.format_query
    }

    /// 要素の値。形式の単位が `S0` か `Unsupported` なら形式の単位の値をそのまま返す。
    /// そうでなく値があればその値、無ければ [`UNLISTED_FEATURE_QUERY`]。
    #[must_use]
    pub fn feature(&self, f: TextureFormatFeature) -> CapabilityQuery {
        if is_unit_only(self.format_query.level) {
            return self.format_query;
        }
        self.features[f.index()].unwrap_or(UNLISTED_FEATURE_QUERY)
    }

    /// 使える段階（`S1`〜`S6`）の用途の要素に対応する用途の和。
    #[must_use]
    pub fn allowed_usages(&self) -> TextureUsage {
        let mut usages = TextureUsage::empty();
        for f in TextureFormatFeature::ALL {
            if let Some(u) = f.usage()
                && is_usable(self.feature(*f).level)
            {
                usages |= u;
            }
        }
        usages
    }

    /// サンプル数 `count` が使えるとき `true`。1 は常に `true`。
    /// 2・4・8・16 は対応する要素が使える段階のとき `true`。ほかの数は `false`。
    #[must_use]
    pub fn sample_count_supported(&self, count: u32) -> bool {
        let f = match count {
            1 => return true,
            2 => TextureFormatFeature::Multisample2,
            4 => TextureFormatFeature::Multisample4,
            8 => TextureFormatFeature::Multisample8,
            16 => TextureFormatFeature::Multisample16,
            _ => return false,
        };
        is_usable(self.feature(f).level)
    }

    /// 使えるサンプル数を昇順で返す。1 を含む。
    #[must_use]
    pub fn supported_sample_counts(&self) -> Vec<u32> {
        [1, 2, 4, 8, 16]
            .into_iter()
            .filter(|c| self.sample_count_supported(*c))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::TextureFormatFeature as F;
    use super::*;
    use crate::capability::CapabilityEvidence;
    use crate::error::UnsupportedReason;

    fn q(level: CapabilityLevel) -> CapabilityQuery {
        CapabilityQuery {
            level,
            unsupported_reason: None,
            evidence: None,
        }
    }
    fn s(level: CapabilityLevel) -> CapabilityQuery {
        CapabilityQuery {
            evidence: Some(CapabilityEvidence::RuntimeApi),
            ..q(level)
        }
    }
    fn uns() -> CapabilityQuery {
        CapabilityQuery {
            level: CapabilityLevel::Unsupported,
            unsupported_reason: Some(UnsupportedReason::NotSupported),
            evidence: None,
        }
    }
    fn s2() -> CapabilityQuery {
        s(CapabilityLevel::S2)
    }
    fn rejected(r: Result<TextureFormatFeatures, ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }
    fn with(entries: &[(F, CapabilityQuery)]) -> TextureFormatFeatures {
        TextureFormatFeatures::new(s2(), entries).unwrap()
    }

    #[test]
    fn all_has_eighteen_distinct() {
        assert_eq!(F::ALL.len(), 18);
        for (i, a) in F::ALL.iter().enumerate() {
            for b in &F::ALL[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn index_is_bijection() {
        let mut seen = [false; 18];
        for f in F::ALL {
            assert!(!seen[f.index()]);
            seen[f.index()] = true;
        }
        assert!(seen.iter().all(|b| *b));
        for (i, f) in F::ALL.iter().enumerate() {
            assert_eq!(f.index(), i);
        }
    }

    #[test]
    fn format_query_rules() {
        let mut s0_reason = q(CapabilityLevel::S0);
        s0_reason.unsupported_reason = Some(UnsupportedReason::NoApi);
        let mut s0_evidence = q(CapabilityLevel::S0);
        s0_evidence.evidence = Some(CapabilityEvidence::RuntimeApi);
        let mut s1_reason = q(CapabilityLevel::S1);
        s1_reason.unsupported_reason = Some(UnsupportedReason::NoApi);
        let mut unsupported_no_reason = uns();
        unsupported_no_reason.unsupported_reason = None;
        assert!(rejected(TextureFormatFeatures::new(s0_reason, &[])));
        assert!(rejected(TextureFormatFeatures::new(s0_evidence, &[])));
        assert!(TextureFormatFeatures::new(q(CapabilityLevel::S0), &[]).is_ok());
        assert!(rejected(TextureFormatFeatures::new(
            unsupported_no_reason,
            &[]
        )));
        assert!(rejected(TextureFormatFeatures::new(s1_reason, &[])));
        assert!(rejected(TextureFormatFeatures::new(
            q(CapabilityLevel::S3),
            &[]
        )));
        assert!(TextureFormatFeatures::new(q(CapabilityLevel::S1), &[]).is_ok());
        assert!(TextureFormatFeatures::new(q(CapabilityLevel::S2), &[]).is_ok());
        assert!(TextureFormatFeatures::new(s(CapabilityLevel::S3), &[]).is_ok());
    }

    #[test]
    fn unit_without_value_rejects_features() {
        assert!(rejected(TextureFormatFeatures::new(
            q(CapabilityLevel::S0),
            &[(F::Filterable, s2())]
        )));
        assert!(rejected(TextureFormatFeatures::new(
            uns(),
            &[(F::Filterable, s2())]
        )));
        assert!(TextureFormatFeatures::new(uns(), &[]).is_ok());
    }

    #[test]
    fn feature_entry_rules() {
        let mut uns_no_reason = uns();
        uns_no_reason.unsupported_reason = None;
        let mut s2_reason = s2();
        s2_reason.unsupported_reason = Some(UnsupportedReason::NoApi);
        let n = TextureFormatFeatures::new;
        assert!(rejected(n(
            s2(),
            &[(F::Filterable, q(CapabilityLevel::S0))]
        )));
        assert!(rejected(n(s2(), &[(F::Filterable, uns_no_reason)])));
        assert!(rejected(n(s2(), &[(F::Filterable, s2_reason)])));
        assert!(rejected(n(
            s2(),
            &[(F::Filterable, q(CapabilityLevel::S4))]
        )));
        assert!(rejected(n(
            s2(),
            &[(F::Filterable, s2()), (F::Filterable, s2())]
        )));
        assert!(n(s2(), &[(F::Filterable, uns())]).is_ok());
        assert!(n(s2(), &[(F::Filterable, s(CapabilityLevel::S4))]).is_ok());
    }

    #[test]
    fn usage_flag_pairs() {
        // COVT-131: 用途と旗の組。
        for flag in [
            F::StorageReadOnly,
            F::StorageWriteOnly,
            F::StorageReadWrite,
            F::StorageAtomic,
        ] {
            assert!(rejected(TextureFormatFeatures::new(s2(), &[(flag, s2())])));
            assert!(
                TextureFormatFeatures::new(s2(), &[(flag, s2()), (F::UsageStorageBinding, s2())])
                    .is_ok()
            );
        }
        assert!(rejected(TextureFormatFeatures::new(
            s2(),
            &[(F::UsageStorageAtomic, s2())]
        )));
        assert!(rejected(TextureFormatFeatures::new(
            s2(),
            &[
                (F::UsageStorageBinding, s2()),
                (F::UsageStorageAtomic, s2()),
                (F::StorageAtomic, uns())
            ]
        )));
        assert!(
            TextureFormatFeatures::new(
                s2(),
                &[
                    (F::UsageStorageBinding, uns()),
                    (F::StorageReadOnly, uns()),
                    (F::StorageAtomic, uns()),
                ]
            )
            .is_ok()
        );
        assert!(
            TextureFormatFeatures::new(
                s2(),
                &[
                    (F::UsageStorageBinding, s2()),
                    (F::StorageAtomic, s2()),
                    (F::UsageStorageAtomic, s2()),
                ]
            )
            .is_ok()
        );
    }

    #[test]
    fn capture_failed_value() {
        let v = TextureFormatFeatures::capture_failed();
        assert_eq!(v.format_query(), q(CapabilityLevel::S0));
        for f in F::ALL {
            assert_eq!(v.feature(*f), q(CapabilityLevel::S0));
        }
    }

    #[test]
    fn feature_reads() {
        let s0 = TextureFormatFeatures::new(q(CapabilityLevel::S0), &[]).unwrap();
        let un = TextureFormatFeatures::new(uns(), &[]).unwrap();
        for f in F::ALL {
            assert_eq!(s0.feature(*f), q(CapabilityLevel::S0));
            assert_eq!(un.feature(*f), uns());
        }
        let v = with(&[(F::Filterable, s(CapabilityLevel::S4))]);
        assert_eq!(v.feature(F::Filterable), s(CapabilityLevel::S4));
        assert_eq!(v.feature(F::Blendable), UNLISTED_FEATURE_QUERY);
    }

    #[test]
    fn allowed_usages_map_each_bit() {
        let pairs = [
            (F::UsageCopySrc, TextureUsage::COPY_SRC),
            (F::UsageCopyDst, TextureUsage::COPY_DST),
            (F::UsageTextureBinding, TextureUsage::TEXTURE_BINDING),
            (F::UsageStorageBinding, TextureUsage::STORAGE_BINDING),
            (F::UsageRenderAttachment, TextureUsage::RENDER_ATTACHMENT),
            (
                F::UsageTransientAttachment,
                TextureUsage::TRANSIENT_ATTACHMENT,
            ),
        ];
        for (f, u) in pairs {
            assert_eq!(with(&[(f, s2())]).allowed_usages(), u);
        }
        let atomic = with(&[
            (F::UsageStorageBinding, s2()),
            (F::StorageAtomic, s2()),
            (F::UsageStorageAtomic, s2()),
        ]);
        assert_eq!(
            atomic.allowed_usages(),
            TextureUsage::STORAGE_BINDING | TextureUsage::STORAGE_ATOMIC
        );
        assert!(
            with(&[(F::UsageCopySrc, uns())])
                .allowed_usages()
                .is_empty()
        );
        let un = TextureFormatFeatures::new(uns(), &[]).unwrap();
        assert!(un.allowed_usages().is_empty());
    }

    #[test]
    fn sample_counts() {
        let v = with(&[(F::Multisample4, s2()), (F::Multisample16, s2())]);
        for (c, want) in [
            (0, false),
            (1, true),
            (2, false),
            (3, false),
            (4, true),
            (8, false),
            (16, true),
            (32, false),
        ] {
            assert_eq!(v.sample_count_supported(c), want, "count {c}");
        }
        assert_eq!(v.supported_sample_counts(), vec![1, 4, 16]);
        let un = TextureFormatFeatures::new(uns(), &[]).unwrap();
        assert!(un.sample_count_supported(1));
        assert!(!un.sample_count_supported(4));
        assert_eq!(un.supported_sample_counts(), vec![1]);
    }
}
