//! 形式ごとの能力の値の型と、形式の鍵ごとの値を、公開のパスだけで確かめる試験（TASK-2566）。
//!
//! 試験の中だけの `Capabilities` の実装を使う。feature の条件は要らない。
//! 項目のポインタ: COVT-131。

use fandhe_silicon_core::capability::{
    Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::feature_key::FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
use fandhe_silicon_core::texture::TextureUsage;
use fandhe_silicon_core::texture_format::TextureFormat;
use fandhe_silicon_core::texture_format_features::{TextureFormatFeature, TextureFormatFeatures};
use fandhe_silicon_core::texture_format_key::{TEXTURE_FORMAT_QUERY_KEYS, TEXTURE_FORMAT_ROWS};

use TextureFormatFeature as F;

static LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 0,
    max_dispatch_dims: [0, 0, 0],
    max_texture_size: 0,
};

const S2_RT: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::S2,
    unsupported_reason: None,
    evidence: Some(CapabilityEvidence::RuntimeApi),
};

const UNS_NS: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::Unsupported,
    unsupported_reason: Some(UnsupportedReason::NotSupported),
    evidence: None,
};

const UNS_NOAPI: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::Unsupported,
    unsupported_reason: Some(UnsupportedReason::NoApi),
    evidence: None,
};

const fn plain(level: CapabilityLevel) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: None,
        evidence: None,
    }
}

/// 一覧の全行の (鍵, 形式) の組を行の順に平らにした列。
fn pairs() -> Vec<(FeatureId, TextureFormat)> {
    TEXTURE_FORMAT_ROWS
        .iter()
        .flat_map(|row| row.keys.iter().map(|k| (k.key, k.format)))
        .collect()
}

fn key_of(format: TextureFormat) -> FeatureId {
    pairs()
        .into_iter()
        .find(|(_, f)| *f == format)
        .map(|(k, _)| k)
        .expect("format has a key")
}

/// 形式の鍵に `texformat` の値、機能の行の鍵に `adapter` の値を返す試験の中だけの実装。
/// `odd` の鍵だけは別の値を返す。`overridden` が真のとき、`Rgba8Unorm` の値を上書きする。
struct Impl {
    texformat: CapabilityQuery,
    adapter: CapabilityQuery,
    odd: Option<(FeatureId, CapabilityQuery)>,
    overridden: bool,
}

impl Impl {
    fn new(adapter: CapabilityQuery) -> Self {
        Self {
            texformat: S2_RT,
            adapter,
            odd: None,
            overridden: false,
        }
    }
}

impl Capabilities for Impl {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        self.query_ex(feature).level
    }

    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        if feature == FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES {
            return self.adapter;
        }
        if let Some((k, v)) = self.odd
            && k == feature
        {
            return v;
        }
        if TEXTURE_FORMAT_QUERY_KEYS.contains(&feature) {
            return self.texformat;
        }
        UNLISTED_FEATURE_QUERY
    }

    fn texture_format_features(&self, format: TextureFormat) -> TextureFormatFeatures {
        let unit = pairs()
            .into_iter()
            .find(|(_, f)| *f == format)
            .map_or(UNLISTED_FEATURE_QUERY, |(k, _)| self.query_ex(k));
        let elements: &[(TextureFormatFeature, CapabilityQuery)] = if self.overridden
            && format == TextureFormat::Rgba8Unorm
            && unit.level != CapabilityLevel::Unsupported
        {
            &[
                (F::Filterable, S2_RT),
                (F::Multisample4, S2_RT),
                (F::UsageTextureBinding, S2_RT),
                (F::UsageCopyDst, S2_RT),
                (F::Blendable, UNS_NS),
            ]
        } else {
            &[]
        };
        TextureFormatFeatures::new(unit, elements)
            .unwrap_or_else(|_| TextureFormatFeatures::capture_failed())
    }
}

fn is_invalid(r: Result<TextureFormatFeatures, ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

#[test]
fn feature_kinds_are_eighteen() {
    let all = TextureFormatFeature::ALL;
    assert_eq!(all.len(), 18);
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn builder_rules_reject_each_violation() {
    let s0_with_evidence = CapabilityQuery {
        evidence: Some(CapabilityEvidence::RuntimeApi),
        ..plain(CapabilityLevel::S0)
    };
    let unsupported_no_reason = plain(CapabilityLevel::Unsupported);
    let s1_with_reason = CapabilityQuery {
        unsupported_reason: Some(UnsupportedReason::NotSupported),
        ..plain(CapabilityLevel::S1)
    };
    let s3_no_evidence = plain(CapabilityLevel::S3);
    let s0 = plain(CapabilityLevel::S0);
    let s4_no_evidence = plain(CapabilityLevel::S4);
    #[allow(clippy::type_complexity)]
    let rejected: Vec<(CapabilityQuery, Vec<(F, CapabilityQuery)>)> = vec![
        // 形式の単位の値の規則。
        (s0_with_evidence, vec![]),
        (unsupported_no_reason, vec![]),
        (s1_with_reason, vec![]),
        (s3_no_evidence, vec![]),
        // 形式の単位が Unsupported のとき要素を持てない。
        (UNS_NS, vec![(F::Filterable, S2_RT)]),
        // 要素の値の規則と重複。
        (S2_RT, vec![(F::Filterable, s0)]),
        (S2_RT, vec![(F::Filterable, unsupported_no_reason)]),
        (S2_RT, vec![(F::Filterable, s4_no_evidence)]),
        (S2_RT, vec![(F::Filterable, S2_RT), (F::Filterable, S2_RT)]),
        // 用途と旗の組。
        (S2_RT, vec![(F::StorageReadOnly, S2_RT)]),
        (S2_RT, vec![(F::UsageStorageAtomic, S2_RT)]),
    ];
    for (unit, elements) in &rejected {
        let r1 = TextureFormatFeatures::new(*unit, elements);
        let r2 = TextureFormatFeatures::new(*unit, elements);
        assert_eq!(r1, r2);
        assert!(is_invalid(r1), "{unit:?} {elements:?}");
    }
    assert!(TextureFormatFeatures::new(S2_RT, &[]).is_ok());
}

#[test]
fn every_texture_format_key_returns_a_value() {
    let pairs = pairs();
    assert_eq!(pairs.len(), 118);
    assert_eq!(pairs.len(), TEXTURE_FORMAT_QUERY_KEYS.len());

    let dev = Impl::new(S2_RT);
    let dynamic: &dyn Capabilities = &dev;
    for (key, format) in &pairs {
        let v = dev.texture_format_features(*format);
        assert_eq!(v.format_query(), dev.query_ex(*key));
        for f in TextureFormatFeature::ALL {
            assert_eq!(v.feature(*f), UNLISTED_FEATURE_QUERY);
        }
        assert!(v.allowed_usages().is_empty());
        assert_eq!(v.supported_sample_counts(), vec![1]);
        assert_eq!(dynamic.texture_format_features(*format), v);
    }

    // 鍵を 1 つだけ非対応にすると、その形式だけが変わる。
    let target = key_of(TextureFormat::Rgba8Unorm);
    let mut odd = Impl::new(S2_RT);
    odd.odd = Some((target, UNS_NS));
    for (key, format) in &pairs {
        let v = odd.texture_format_features(*format);
        let is_target = *key == target;
        assert_eq!(v.format_query(), if is_target { UNS_NS } else { S2_RT });
        let want = if is_target {
            UNS_NS
        } else {
            UNLISTED_FEATURE_QUERY
        };
        for f in TextureFormatFeature::ALL {
            assert_eq!(v.feature(*f), want);
        }
    }
}

#[test]
fn format_values_do_not_depend_on_adapter_specific_feature_row() {
    let build = |adapter: CapabilityQuery, overridden: bool| {
        let mut d = Impl::new(adapter);
        d.overridden = overridden;
        d.odd = Some((key_of(TextureFormat::R8Unorm), UNS_NS));
        d
    };
    // 前提: 2 つの実装は、機能の行の鍵の値が違う。
    let (a, b) = (build(UNS_NOAPI, true), build(S2_RT, true));
    assert_ne!(
        a.query_ex(FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES),
        b.query_ex(FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
    );

    for overridden in [true, false] {
        let (a, b) = (build(UNS_NOAPI, overridden), build(S2_RT, overridden));
        let mut count = 0;
        for (key, format) in pairs() {
            let va = a.texture_format_features(format);
            let vb = b.texture_format_features(format);
            assert_eq!(va, vb);
            assert_eq!(va.format_query(), a.query_ex(key));
            count += 1;
        }
        assert_eq!(count, 118);
    }

    let v = build(UNS_NOAPI, true).texture_format_features(TextureFormat::Rgba8Unorm);
    assert_eq!(v.feature(F::Filterable), S2_RT);
    assert_eq!(v.feature(F::Multisample4), S2_RT);
    assert_eq!(v.feature(F::UsageTextureBinding), S2_RT);
    assert_eq!(v.feature(F::UsageCopyDst), S2_RT);
    assert_eq!(v.feature(F::Blendable), UNS_NS);
    assert!(!v.allowed_usages().contains(TextureUsage::RENDER_ATTACHMENT));
}

#[test]
fn overridden_values_follow_read_rules() {
    let mut d = Impl::new(S2_RT);
    d.overridden = true;
    let v = d.texture_format_features(TextureFormat::Rgba8Unorm);
    for (n, want) in [
        (1, true),
        (2, false),
        (4, true),
        (8, false),
        (16, false),
        (3, false),
        (32, false),
    ] {
        assert_eq!(v.sample_count_supported(n), want, "{n}");
    }
    assert_eq!(v.supported_sample_counts(), vec![1, 4]);
    assert_eq!(
        v.allowed_usages(),
        TextureUsage::TEXTURE_BINDING | TextureUsage::COPY_DST
    );

    // 形式の単位が非対応の形式は、全要素がその値になる。
    let mut odd = Impl::new(S2_RT);
    odd.overridden = true;
    odd.odd = Some((key_of(TextureFormat::Rgba8Unorm), UNS_NS));
    let u = odd.texture_format_features(TextureFormat::Rgba8Unorm);
    assert_eq!(u.format_query(), UNS_NS);
    for f in TextureFormatFeature::ALL {
        assert_eq!(u.feature(*f), UNS_NS);
    }
    assert!(u.allowed_usages().is_empty());
    assert_eq!(u.supported_sample_counts(), vec![1]);
}
