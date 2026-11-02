//! 能力の問合せの口へテクスチャの形式の問合せの鍵を流し、戻り値を検査する試験用の関数（`test-support` の裏）。
//!
//! `Capabilities` を実装する値（`&dyn Device` を含む）を受け、鍵ごとに `query` と `query_ex` を呼んで
//! 共通の規則（`capability_rule::check_query`。頁は `docs/capability-query-rules.md`）と
//! 段階の一致を確かめる（TASK-1314 のポインタ）。鍵の一覧は `texture_format_key` モジュール
//! （TASK-1313 のポインタ）にあり、呼ぶ側が渡す。
//! 製品の経路からは使わない。`crate::fault` は参照しない（`test-support` だけの組でもビルドされるため）。

use crate::capability::{Capabilities, CapabilityLevel, CapabilityQuery, FeatureId};
use crate::capability_rule::{CapabilityRuleViolation, check_query};
use crate::capability_table::UNLISTED_FEATURE_QUERY;
use crate::error::UnsupportedReason;
use crate::texture_format::TextureFormat;
use crate::texture_format_key::TextureFormatRow;

/// 形式の問合せの戻り値が検査に外れた 1 件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextureFormatQueryViolation {
    /// `capability_rule::check_query` が返した違反をそのまま包んだもの。
    Rule(CapabilityRuleViolation),
    /// `query` の段階と `query_ex` の段階が違う。
    LevelMismatch {
        /// 検査した鍵。
        feature: FeatureId,
        /// `query` が返した段階。
        level: CapabilityLevel,
        /// `query_ex` が返した値。
        query: CapabilityQuery,
    },
    /// 行の無い鍵で、`query_ex` が行の無い値でない、または `query` が `Unsupported` でない。
    RowlessKeyMismatch {
        /// 検査した鍵。
        feature: FeatureId,
        /// `query` が返した段階。
        level: CapabilityLevel,
        /// `query_ex` が返した値。
        query: CapabilityQuery,
    },
    /// `texture_format_features` の形式の単位が、その形式の鍵の `query_ex` の値と違う。
    FeaturesFormatQueryMismatch {
        /// 検査した鍵。
        feature: FeatureId,
        /// 鍵に対応する形式。
        format: TextureFormat,
        /// `texture_format_features` の形式の単位の値。
        features_query: CapabilityQuery,
        /// `query_ex` が返した値。
        query: CapabilityQuery,
    },
}

/// `rows` の各鍵で `texture_format_features(format).format_query()` と `query_ex(key)` を 1 回ずつ呼び、
/// 食い違いを鍵の順に全部集める。要素ごとの値の規則は見ない。`rows` が空なら `Ok`。
///
/// ```
/// use fandhe_silicon_core::capability::{
///     Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
/// };
/// use fandhe_silicon_core::capability_rule::CapabilityTable;
/// use fandhe_silicon_core::error::UnsupportedReason;
/// use fandhe_silicon_core::texture_format::TextureFormat;
/// use fandhe_silicon_core::texture_format_features::TextureFormatFeatures;
/// use fandhe_silicon_core::texture_format_key::{TEXTURE_FORMAT_QUERY_KEYS, TEXTURE_FORMAT_ROWS};
/// use fandhe_silicon_core::texture_format_query_check::{
///     check_texture_format_features, TextureFormatQueryViolation,
/// };
///
/// static LIMITS: LimitsTable = LimitsTable {
///     max_buffer_bytes: 0,
///     max_dispatch_dims: [0, 0, 0],
///     max_texture_size: 0,
/// };
/// let row = CapabilityQuery {
///     level: CapabilityLevel::Unsupported,
///     unsupported_reason: Some(UnsupportedReason::NotSupported),
///     evidence: None,
/// };
/// let mut table = CapabilityTable::new();
/// for key in TEXTURE_FORMAT_QUERY_KEYS {
///     table = table.with(*key, row);
/// }
/// assert_eq!(check_texture_format_features(&table, TEXTURE_FORMAT_ROWS), Ok(()));
///
/// struct Skewed(CapabilityTable, CapabilityQuery);
/// impl Capabilities for Skewed {
///     fn query(&self, f: FeatureId) -> CapabilityLevel {
///         self.0.query(f)
///     }
///     fn limits(&self) -> &LimitsTable {
///         &LIMITS
///     }
///     fn query_ex(&self, f: FeatureId) -> CapabilityQuery {
///         self.0.query_ex(f)
///     }
///     fn texture_format_features(&self, format: TextureFormat) -> TextureFormatFeatures {
///         if format == TextureFormat::Rgba8Unorm {
///             TextureFormatFeatures::capture_failed()
///         } else {
///             TextureFormatFeatures::new(self.1, &[]).unwrap()
///         }
///     }
/// }
/// let err = check_texture_format_features(&Skewed(table, row), TEXTURE_FORMAT_ROWS).unwrap_err();
/// assert_eq!(err.len(), 1);
/// assert!(matches!(
///     err[0],
///     TextureFormatQueryViolation::FeaturesFormatQueryMismatch { .. }
/// ));
/// ```
pub fn check_texture_format_features<D: Capabilities + ?Sized>(
    device: &D,
    rows: &[TextureFormatRow],
) -> Result<(), Vec<TextureFormatQueryViolation>> {
    let mut violations = Vec::new();
    for row in rows {
        for k in row.keys {
            let features_query = device.texture_format_features(k.format).format_query();
            let query = device.query_ex(k.key);
            if features_query != query {
                violations.push(TextureFormatQueryViolation::FeaturesFormatQueryMismatch {
                    feature: k.key,
                    format: k.format,
                    features_query,
                    query,
                });
            }
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// 行の無い鍵の試しに使う鍵。一覧に無い綴りで、互いに異なる。
const ROWLESS_PROBE_KEYS: &[FeatureId] = &[
    FeatureId("texformat:Astc"),
    FeatureId("texformat:"),
    FeatureId("texformat:r8unorm"),
    FeatureId("texformat:Astc.B4x4"),
    FeatureId("texformat:Astc.B4x4.Unorm.X"),
];

/// `keys` の各鍵で `query` と `query_ex` を 1 回ずつ呼び、違反を鍵の順に全部集める。
///
/// 鍵ごとの違反の順は、`check_query` の違反（`Rule`）→ 段階の食い違い（`LevelMismatch`）。
/// 行の無い値（`UnknownFeature`）は違反にしない。`keys` が空なら `Ok`。
///
/// ```
/// use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
/// use fandhe_silicon_core::capability_rule::CapabilityTable;
/// use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
/// use fandhe_silicon_core::error::UnsupportedReason;
/// use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;
/// use fandhe_silicon_core::texture_format_query_check::{
///     check_texture_format_queries, TextureFormatQueryViolation,
/// };
/// use fandhe_silicon_core::capability_rule::CapabilityRuleViolation;
///
/// let row = CapabilityQuery {
///     level: CapabilityLevel::Unsupported,
///     unsupported_reason: Some(UnsupportedReason::NotSupported),
///     evidence: None,
/// };
/// let mut table = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
/// for key in TEXTURE_FORMAT_QUERY_KEYS {
///     table = table.with(*key, row);
/// }
/// assert_eq!(check_texture_format_queries(&table, TEXTURE_FORMAT_QUERY_KEYS), Ok(()));
///
/// let broken = CapabilityQuery { unsupported_reason: None, ..row };
/// let table = table.with(TEXTURE_FORMAT_QUERY_KEYS[0], broken);
/// let err = check_texture_format_queries(&table, TEXTURE_FORMAT_QUERY_KEYS).unwrap_err();
/// assert_eq!(err.len(), 1);
/// assert!(matches!(
///     err[0],
///     TextureFormatQueryViolation::Rule(CapabilityRuleViolation::MissingUnsupportedReason { .. })
/// ));
/// ```
pub fn check_texture_format_queries<D: Capabilities + ?Sized>(
    device: &D,
    keys: &[FeatureId],
) -> Result<(), Vec<TextureFormatQueryViolation>> {
    let mut violations = Vec::new();
    for &feature in keys {
        let level = device.query(feature);
        let query = device.query_ex(feature);
        if let Err(rules) = check_query(feature, query) {
            violations.extend(rules.into_iter().map(TextureFormatQueryViolation::Rule));
        }
        if level != query.level {
            violations.push(TextureFormatQueryViolation::LevelMismatch {
                feature,
                level,
                query,
            });
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// 行の無い鍵の試しの鍵それぞれで、`query_ex` が `UNLISTED_FEATURE_QUERY`、`query` が `Unsupported` であることを確かめる。
///
/// 外れた鍵は 1 件ずつ `RowlessKeyMismatch` にして、試しの鍵の順に全部集める。
///
/// ```
/// use fandhe_silicon_core::capability::CapabilityLevel;
/// use fandhe_silicon_core::capability_rule::CapabilityTable;
/// use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
/// use fandhe_silicon_core::texture_format_query_check::check_rowless_texture_format_keys;
///
/// let ok = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
/// assert_eq!(check_rowless_texture_format_keys(&ok), Ok(()));
/// let bad = CapabilityTable::new();
/// assert!(check_rowless_texture_format_keys(&bad).is_err());
/// ```
pub fn check_rowless_texture_format_keys<D: Capabilities + ?Sized>(
    device: &D,
) -> Result<(), Vec<TextureFormatQueryViolation>> {
    let mut violations = Vec::new();
    for &feature in ROWLESS_PROBE_KEYS {
        let level = device.query(feature);
        let query = device.query_ex(feature);
        if query != UNLISTED_FEATURE_QUERY || level != CapabilityLevel::Unsupported {
            violations.push(TextureFormatQueryViolation::RowlessKeyMismatch {
                feature,
                level,
                query,
            });
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// `keys` のうち、`query_ex` の理由が `UnknownFeature` の鍵を `keys` の順で返す。
///
/// ```
/// use fandhe_silicon_core::capability_rule::CapabilityTable;
/// use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
/// use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;
/// use fandhe_silicon_core::texture_format_query_check::texture_format_keys_without_rows;
///
/// let table = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
/// let rest = texture_format_keys_without_rows(&table, TEXTURE_FORMAT_QUERY_KEYS);
/// assert_eq!(rest.len(), TEXTURE_FORMAT_QUERY_KEYS.len());
/// ```
#[must_use]
pub fn texture_format_keys_without_rows<D: Capabilities + ?Sized>(
    device: &D,
    keys: &[FeatureId],
) -> Vec<FeatureId> {
    keys.iter()
        .copied()
        .filter(|&key| {
            device.query_ex(key).unsupported_reason == Some(UnsupportedReason::UnknownFeature)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::LimitsTable;
    use crate::capability_rule::CapabilityTable;
    use crate::texture_format_key::{TEXTURE_FORMAT_QUERY_KEYS, TEXTURE_FORMAT_ROWS};

    static LIMITS: LimitsTable = LimitsTable {
        max_buffer_bytes: 0,
        max_dispatch_dims: [0, 0, 0],
        max_texture_size: 0,
    };

    const ROW: CapabilityQuery = CapabilityQuery {
        level: CapabilityLevel::Unsupported,
        unsupported_reason: Some(UnsupportedReason::NotSupported),
        evidence: None,
    };

    /// `texture_format_features` を上書きして、指定した形式だけ外した値を返す試験用の実装。
    struct Skewed {
        table: CapabilityTable,
        bad: &'static [TextureFormat],
    }

    impl Capabilities for Skewed {
        fn query(&self, f: FeatureId) -> CapabilityLevel {
            self.table.query(f)
        }
        fn limits(&self) -> &LimitsTable {
            &LIMITS
        }
        fn query_ex(&self, f: FeatureId) -> CapabilityQuery {
            self.table.query_ex(f)
        }
        fn texture_format_features(
            &self,
            format: TextureFormat,
        ) -> crate::texture_format_features::TextureFormatFeatures {
            use crate::texture_format_features::TextureFormatFeatures;
            if self.bad.contains(&format) {
                TextureFormatFeatures::capture_failed()
            } else {
                TextureFormatFeatures::new(ROW, &[]).unwrap()
            }
        }
    }

    #[test]
    fn features_check_accepts_empty_rows() {
        let dev = Skewed {
            table: passing_table(),
            bad: &[TextureFormat::R8Unorm],
        };
        assert_eq!(check_texture_format_features(&dev, &[]), Ok(()));
    }

    #[test]
    fn features_check_collects_all_mismatches_in_key_order() {
        let dev = Skewed {
            table: passing_table(),
            bad: &[TextureFormat::Rgba8Unorm, TextureFormat::R8Unorm],
        };
        let err = check_texture_format_features(&dev, TEXTURE_FORMAT_ROWS).unwrap_err();
        let formats: Vec<TextureFormat> = err
            .iter()
            .map(|v| match v {
                TextureFormatQueryViolation::FeaturesFormatQueryMismatch { format, .. } => *format,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(formats, [TextureFormat::R8Unorm, TextureFormat::Rgba8Unorm]);
    }

    fn passing_table() -> CapabilityTable {
        let mut t = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
        for key in TEXTURE_FORMAT_QUERY_KEYS {
            t = t.with(*key, ROW);
        }
        t
    }

    /// `query` と `query_ex` の段階が食い違う試験の中だけの実装。
    struct Mismatched;

    impl Capabilities for Mismatched {
        fn query(&self, _feature: FeatureId) -> CapabilityLevel {
            CapabilityLevel::S1
        }
        fn limits(&self) -> &LimitsTable {
            &LIMITS
        }
        fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
            ROW
        }
    }

    #[test]
    fn passing_table_passes_all_three() {
        let t = passing_table();
        assert_eq!(
            check_texture_format_queries(&t, TEXTURE_FORMAT_QUERY_KEYS),
            Ok(())
        );
        assert_eq!(check_rowless_texture_format_keys(&t), Ok(()));
        assert!(texture_format_keys_without_rows(&t, TEXTURE_FORMAT_QUERY_KEYS).is_empty());
    }

    #[test]
    fn broken_rows_are_reported_as_rule_violations() {
        let k = TEXTURE_FORMAT_QUERY_KEYS;
        let no_reason = CapabilityQuery {
            unsupported_reason: None,
            ..ROW
        };
        let s0_reason = CapabilityQuery {
            level: CapabilityLevel::S0,
            ..ROW
        };
        let s3_no_evidence = CapabilityQuery {
            level: CapabilityLevel::S3,
            unsupported_reason: None,
            evidence: None,
        };
        let singles = [(no_reason, 0), (s0_reason, 1), (s3_no_evidence, 2)];
        for (value, idx) in singles {
            let t = passing_table().with(k[idx], value);
            let err = check_texture_format_queries(&t, k).unwrap_err();
            assert_eq!(err.len(), 1);
            assert!(matches!(err[0], TextureFormatQueryViolation::Rule(_)));
        }
        let t = passing_table()
            .with(k[0], no_reason)
            .with(k[1], s0_reason)
            .with(k[2], s3_no_evidence);
        let err = check_texture_format_queries(&t, k).unwrap_err();
        assert_eq!(err.len(), 3);
        assert!(matches!(
            err[0],
            TextureFormatQueryViolation::Rule(
                CapabilityRuleViolation::MissingUnsupportedReason { feature, .. }
            ) if feature == k[0]
        ));
        assert!(matches!(
            err[1],
            TextureFormatQueryViolation::Rule(
                CapabilityRuleViolation::UnexpectedUnsupportedReason { feature, .. }
            ) if feature == k[1]
        ));
        assert!(matches!(
            err[2],
            TextureFormatQueryViolation::Rule(
                CapabilityRuleViolation::MissingEvidence { feature, .. }
            ) if feature == k[2]
        ));
    }

    #[test]
    fn level_mismatch_is_reported() {
        let keys = &TEXTURE_FORMAT_QUERY_KEYS[..3];
        let err = check_texture_format_queries(&Mismatched, keys).unwrap_err();
        assert_eq!(err.len(), 3);
        for (e, key) in err.iter().zip(keys) {
            assert_eq!(
                *e,
                TextureFormatQueryViolation::LevelMismatch {
                    feature: *key,
                    level: CapabilityLevel::S1,
                    query: ROW,
                }
            );
        }
        // 鍵ごとの順は Rule -> LevelMismatch。
        struct NoReason;
        impl Capabilities for NoReason {
            fn query(&self, _feature: FeatureId) -> CapabilityLevel {
                CapabilityLevel::S1
            }
            fn limits(&self) -> &LimitsTable {
                &LIMITS
            }
            fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
                CapabilityQuery {
                    unsupported_reason: None,
                    ..ROW
                }
            }
        }
        let err = check_texture_format_queries(&NoReason, &keys[..1]).unwrap_err();
        assert_eq!(err.len(), 2);
        assert!(matches!(err[0], TextureFormatQueryViolation::Rule(_)));
        assert!(matches!(
            err[1],
            TextureFormatQueryViolation::LevelMismatch { .. }
        ));
    }

    #[test]
    fn rowless_check_flags_every_probe_on_s0_default() {
        let err = check_rowless_texture_format_keys(&CapabilityTable::new()).unwrap_err();
        assert_eq!(err.len(), ROWLESS_PROBE_KEYS.len());
        for (e, key) in err.iter().zip(ROWLESS_PROBE_KEYS) {
            assert!(matches!(
                e,
                TextureFormatQueryViolation::RowlessKeyMismatch { feature, .. } if feature == key
            ));
        }
    }

    #[test]
    fn keys_without_rows_returns_remaining_in_order() {
        let k = TEXTURE_FORMAT_QUERY_KEYS;
        let mut t = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
        for key in &k[..10] {
            t = t.with(*key, ROW);
        }
        assert_eq!(texture_format_keys_without_rows(&t, k), k[10..].to_vec());
        assert!(texture_format_keys_without_rows(&t, &[]).is_empty());
        assert_eq!(check_texture_format_queries(&t, &[]), Ok(()));
    }

    #[test]
    fn probe_keys_are_outside_list_and_anchor_on_astc_row() {
        for (i, a) in ROWLESS_PROBE_KEYS.iter().enumerate() {
            assert!(!TEXTURE_FORMAT_QUERY_KEYS.contains(a), "{a:?}");
            for b in &ROWLESS_PROBE_KEYS[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let multi: Vec<_> = TEXTURE_FORMAT_ROWS
            .iter()
            .filter(|r| r.keys.len() >= 2)
            .collect();
        assert_eq!(multi.len(), 1);
        assert_eq!(
            format!("texformat:{}", multi[0].row),
            ROWLESS_PROBE_KEYS[0].0
        );
    }
}
