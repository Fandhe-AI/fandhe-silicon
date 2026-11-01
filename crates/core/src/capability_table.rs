//! 機能の鍵ごとの行を持つ静的な表の行の型と、鍵で行を引く関数と、行の無い鍵に返す値。
//!
//! 背後実装の型を含まず、feature の条件なしで公開される（TASK-1114 のポインタ）。

use crate::capability::{CapabilityLevel, CapabilityQuery, FeatureId};
use crate::error::UnsupportedReason;

/// 能力の表の 1 行。規則の型 `R` は利用側が決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureRow<R: 'static> {
    /// 行の鍵。
    pub feature: FeatureId,
    /// 行の規則の値。
    pub rule: R,
    /// 一次資料の出どころの文字列。
    pub source: &'static str,
}

/// 表に行の無い鍵に返す値。段階は `Unsupported`、理由は `UnknownFeature`、根拠は無し。
pub const UNLISTED_FEATURE_QUERY: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::Unsupported,
    unsupported_reason: Some(UnsupportedReason::UnknownFeature),
    evidence: None,
};

/// 表を先頭から探し、鍵が等しい最初の行を返す。無ければ `None`。
///
/// ```
/// use fandhe_silicon_core::capability::FeatureId;
/// use fandhe_silicon_core::capability_table::{find_row, FeatureRow};
/// use fandhe_silicon_core::feature_key::FEATURE_SHADER_F16;
///
/// static TABLE: [FeatureRow<&str>; 1] = [FeatureRow {
///     feature: FEATURE_SHADER_F16,
///     rule: "a",
///     source: "s",
/// }];
/// assert_eq!(find_row(&TABLE, FEATURE_SHADER_F16).map(|r| r.rule), Some("a"));
/// assert!(find_row(&TABLE, FeatureId("feature:none")).is_none());
/// ```
#[must_use]
pub fn find_row<R: 'static>(
    table: &'static [FeatureRow<R>],
    feature: FeatureId,
) -> Option<&'static FeatureRow<R>> {
    table.iter().find(|row| row.feature == feature)
}

/// 行があれば `eval` に規則を渡した値を、無ければ `UNLISTED_FEATURE_QUERY` を返す（このとき `eval` は呼ばない）。
///
/// ```
/// use fandhe_silicon_core::capability::{CapabilityLevel, FeatureId};
/// use fandhe_silicon_core::capability_table::{
///     query_from_table, FeatureRow, UNLISTED_FEATURE_QUERY,
/// };
/// use fandhe_silicon_core::feature_key::FEATURE_SHADER_F16;
///
/// static TABLE: [FeatureRow<CapabilityLevel>; 1] = [FeatureRow {
///     feature: FEATURE_SHADER_F16,
///     rule: CapabilityLevel::S1,
///     source: "s",
/// }];
/// let mut hit_query = UNLISTED_FEATURE_QUERY;
/// hit_query.level = CapabilityLevel::S1;
/// let hit = query_from_table(&TABLE, FEATURE_SHADER_F16, |_| hit_query);
/// assert_eq!(hit.level, CapabilityLevel::S1);
/// let miss = query_from_table(&TABLE, FeatureId("feature:none"), |_| panic!("not called"));
/// assert_eq!(miss, UNLISTED_FEATURE_QUERY);
/// ```
#[must_use]
pub fn query_from_table<R: 'static>(
    table: &'static [FeatureRow<R>],
    feature: FeatureId,
    eval: impl FnOnce(&R) -> CapabilityQuery,
) -> CapabilityQuery {
    match find_row(table, feature) {
        Some(row) => eval(&row.rule),
        None => UNLISTED_FEATURE_QUERY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature_key::{FEATURE_QUERY_KEYS, FEATURE_SHADER_F16};
    use std::cell::Cell;

    const OTHER_KEY: FeatureId = FeatureId("feature:test-other");

    #[test]
    fn empty_table_returns_unlisted_for_all_keys_without_eval() {
        let t: &'static [FeatureRow<()>] = &[];
        let calls = Cell::new(0u32);
        for key in FEATURE_QUERY_KEYS {
            assert!(find_row(t, *key).is_none());
            let q = query_from_table(t, *key, |_| {
                calls.set(calls.get() + 1);
                UNLISTED_FEATURE_QUERY
            });
            assert_eq!(q, UNLISTED_FEATURE_QUERY);
        }
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn duplicate_keys_return_first_row() {
        static T: [FeatureRow<u8>; 2] = [
            FeatureRow {
                feature: FEATURE_SHADER_F16,
                rule: 1,
                source: "first",
            },
            FeatureRow {
                feature: FEATURE_SHADER_F16,
                rule: 2,
                source: "second",
            },
        ];
        let row = find_row(&T, FEATURE_SHADER_F16).unwrap();
        assert_eq!((row.rule, row.source), (1, "first"));
        let seen = Cell::new(0u8);
        let _ = query_from_table(&T, FEATURE_SHADER_F16, |r| {
            seen.set(*r);
            UNLISTED_FEATURE_QUERY
        });
        assert_eq!(seen.get(), 1);
    }

    #[test]
    fn rows_return_their_value_and_missing_keys_unlisted() {
        static T: [FeatureRow<CapabilityQuery>; 2] = [
            FeatureRow {
                feature: OTHER_KEY,
                rule: CapabilityQuery {
                    level: CapabilityLevel::Unsupported,
                    unsupported_reason: Some(UnsupportedReason::NoApi),
                    evidence: None,
                },
                source: "a",
            },
            FeatureRow {
                feature: FEATURE_SHADER_F16,
                rule: CapabilityQuery {
                    level: CapabilityLevel::Unsupported,
                    unsupported_reason: Some(UnsupportedReason::NotSupported),
                    evidence: None,
                },
                source: "b",
            },
        ];
        assert_eq!(query_from_table(&T, FEATURE_SHADER_F16, |q| *q), T[1].rule);
        assert_eq!(query_from_table(&T, OTHER_KEY, |q| *q), T[0].rule);
        assert_eq!(
            query_from_table(&T, FeatureId("feature:none"), |q| *q),
            UNLISTED_FEATURE_QUERY
        );
    }
}
