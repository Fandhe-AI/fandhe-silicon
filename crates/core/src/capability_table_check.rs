//! 能力の表・機能の鍵の一覧・差分の台帳の行を、試験で確かめる道具（TASK-1114 のポインタ）。
//!
//! feature `test-support` の裏にあり、製品の経路からは使わない。どの関数も違反を全部集め、
//! 入力の順に並べて返す。重複は 2 回目以降の出現 1 つにつき 1 件として数える。

use std::collections::HashSet;

use crate::capability::{BackendKind, CapabilityQuery, FeatureId};
use crate::capability_rule::{CapabilityRuleViolation, check_query};
use crate::capability_table::{FeatureRow, query_from_table};

/// 鍵の一覧・表の行・台帳の行の形の違反 1 件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyTableViolation {
    /// 鍵の文字列が空、または最初の `:` の後が空。
    EmptyKey {
        /// 一覧の中の添字。
        index: usize,
    },
    /// 一覧の中で鍵が重なっている。
    DuplicateKey {
        /// 重なった鍵。
        key: FeatureId,
    },
    /// 表の行の鍵が一覧に無い。
    RowKeyNotInList {
        /// 一覧に無い鍵。
        key: FeatureId,
    },
    /// 表の中で行の鍵が重なっている。
    DuplicateRowKey {
        /// 重なった鍵。
        key: FeatureId,
    },
    /// 台帳の行の鍵が一覧に無い。
    LedgerKeyNotInList {
        /// 一覧に無い鍵。
        key: FeatureId,
    },
    /// 台帳の中で（鍵, 背後実装）の組が重なっている。
    DuplicateLedgerEntry {
        /// 重なった組の鍵。
        key: FeatureId,
        /// 重なった組の背後実装。
        backend: BackendKind,
    },
    /// 台帳の行の試験の名前が空。
    EmptyTestName {
        /// 行の鍵。
        key: FeatureId,
        /// 行の背後実装。
        backend: BackendKind,
    },
}

/// 比較基準との結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BaselineDiff {
    /// 比較基準と一致する。
    Same,
    /// 比較基準と違う。
    Differs,
}

/// 差分の台帳の 1 行。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BaselineDiffEntry {
    /// 行の鍵。
    pub feature: FeatureId,
    /// 行の背後実装。
    pub backend: BackendKind,
    /// 比較基準と一致するか違うか。
    pub result: BaselineDiff,
    /// 結果を確かめる公開テストの名前。
    pub test: &'static str,
}

fn finish(v: Vec<KeyTableViolation>) -> Result<(), Vec<KeyTableViolation>> {
    if v.is_empty() { Ok(()) } else { Err(v) }
}

fn is_empty_key(key: &FeatureId) -> bool {
    key.0.is_empty()
        || key
            .0
            .split_once(':')
            .is_some_and(|(_, rest)| rest.is_empty())
}

/// 鍵の一覧を確かめる。鍵の文字列が空（または最初の `:` の後が空）なら `EmptyKey`、重なれば `DuplicateKey` を返す。
///
/// ```
/// use fandhe_silicon_core::capability::FeatureId;
/// use fandhe_silicon_core::capability_table_check::{check_key_list, KeyTableViolation};
///
/// assert!(check_key_list(&[FeatureId("feature:A"), FeatureId("feature:B")]).is_ok());
/// let dup = [FeatureId("feature:A"), FeatureId("feature:A")];
/// assert_eq!(
///     check_key_list(&dup),
///     Err(vec![KeyTableViolation::DuplicateKey { key: FeatureId("feature:A") }])
/// );
/// ```
pub fn check_key_list(keys: &[FeatureId]) -> Result<(), Vec<KeyTableViolation>> {
    let mut seen = HashSet::new();
    let mut v = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        if is_empty_key(key) {
            v.push(KeyTableViolation::EmptyKey { index });
        }
        if !seen.insert(*key) {
            v.push(KeyTableViolation::DuplicateKey { key: *key });
        }
    }
    finish(v)
}

/// 表の各行の鍵が一覧にあり、表の中で重ならないことを確かめる。一覧の鍵が表に無くてもよい。
///
/// ```
/// use fandhe_silicon_core::capability::FeatureId;
/// use fandhe_silicon_core::capability_table::FeatureRow;
/// use fandhe_silicon_core::capability_table_check::{check_table_keys, KeyTableViolation};
///
/// let keys = [FeatureId("feature:A")];
/// let ok = [FeatureRow { feature: FeatureId("feature:A"), rule: (), source: "s" }];
/// assert!(check_table_keys(&ok, &keys).is_ok());
/// let bad = [FeatureRow { feature: FeatureId("feature:Z"), rule: (), source: "s" }];
/// assert_eq!(
///     check_table_keys(&bad, &keys),
///     Err(vec![KeyTableViolation::RowKeyNotInList { key: FeatureId("feature:Z") }])
/// );
/// ```
pub fn check_table_keys<R: 'static>(
    table: &[FeatureRow<R>],
    keys: &[FeatureId],
) -> Result<(), Vec<KeyTableViolation>> {
    let listed: HashSet<FeatureId> = keys.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut v = Vec::new();
    for row in table {
        if !listed.contains(&row.feature) {
            v.push(KeyTableViolation::RowKeyNotInList { key: row.feature });
        }
        if !seen.insert(row.feature) {
            v.push(KeyTableViolation::DuplicateRowKey { key: row.feature });
        }
    }
    finish(v)
}

/// 一覧の各鍵について表から引いた値を `check_query` に通し、違反を全部集める。行の無い鍵は合格する値になる。
///
/// ```
/// use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery, FeatureId};
/// use fandhe_silicon_core::capability_rule::CapabilityRuleViolation;
/// use fandhe_silicon_core::capability_table::FeatureRow;
/// use fandhe_silicon_core::capability_table_check::check_table_queries;
///
/// const K: FeatureId = FeatureId("feature:A");
/// static TABLE: [FeatureRow<CapabilityQuery>; 1] = [FeatureRow {
///     feature: K,
///     rule: CapabilityQuery {
///         level: CapabilityLevel::S3,
///         unsupported_reason: None,
///         evidence: None,
///     },
///     source: "s",
/// }];
/// let err = check_table_queries(&TABLE, &[K], |q| *q).unwrap_err();
/// assert!(matches!(err[0], CapabilityRuleViolation::MissingEvidence { .. }));
/// assert!(check_table_queries(&TABLE, &[FeatureId("feature:B")], |q| *q).is_ok());
/// ```
pub fn check_table_queries<R: 'static>(
    table: &'static [FeatureRow<R>],
    keys: &[FeatureId],
    eval: impl Fn(&R) -> CapabilityQuery,
) -> Result<(), Vec<CapabilityRuleViolation>> {
    let mut v = Vec::new();
    for key in keys {
        let q = query_from_table(table, *key, &eval);
        if let Err(found) = check_query(*key, q) {
            v.extend(found);
        }
    }
    if v.is_empty() { Ok(()) } else { Err(v) }
}

/// 台帳の各行の鍵が一覧にあり、（鍵, 背後実装）の組が重ならず、試験の名前が空でないことを確かめる。
///
/// ```
/// use fandhe_silicon_core::capability::{BackendKind, FeatureId};
/// use fandhe_silicon_core::capability_table_check::{
///     check_ledger_keys, BaselineDiff, BaselineDiffEntry, KeyTableViolation,
/// };
///
/// let keys = [FeatureId("feature:A")];
/// let mut e = BaselineDiffEntry {
///     feature: FeatureId("feature:A"),
///     backend: BackendKind::Vulkan,
///     result: BaselineDiff::Same,
///     test: "t",
/// };
/// assert!(check_ledger_keys(&[e], &keys).is_ok());
/// e.test = "";
/// assert_eq!(
///     check_ledger_keys(&[e], &keys),
///     Err(vec![KeyTableViolation::EmptyTestName {
///         key: FeatureId("feature:A"),
///         backend: BackendKind::Vulkan,
///     }])
/// );
/// ```
pub fn check_ledger_keys(
    ledger: &[BaselineDiffEntry],
    keys: &[FeatureId],
) -> Result<(), Vec<KeyTableViolation>> {
    let listed: HashSet<FeatureId> = keys.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut v = Vec::new();
    for e in ledger {
        if !listed.contains(&e.feature) {
            v.push(KeyTableViolation::LedgerKeyNotInList { key: e.feature });
        }
        if !seen.insert((e.feature, e.backend)) {
            v.push(KeyTableViolation::DuplicateLedgerEntry {
                key: e.feature,
                backend: e.backend,
            });
        }
        if e.test.is_empty() {
            v.push(KeyTableViolation::EmptyTestName {
                key: e.feature,
                backend: e.backend,
            });
        }
    }
    finish(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::CapabilityLevel;
    use crate::feature_key::FEATURE_QUERY_KEYS;
    use std::cell::Cell;

    const A: FeatureId = FeatureId("feature:A");
    const B: FeatureId = FeatureId("feature:B");
    const Z: FeatureId = FeatureId("feature:Z");

    fn row(feature: FeatureId) -> FeatureRow<()> {
        FeatureRow {
            feature,
            rule: (),
            source: "s",
        }
    }

    fn entry(feature: FeatureId, backend: BackendKind, test: &'static str) -> BaselineDiffEntry {
        BaselineDiffEntry {
            feature,
            backend,
            result: BaselineDiff::Differs,
            test,
        }
    }

    const fn query_at(level: CapabilityLevel) -> CapabilityQuery {
        CapabilityQuery {
            level,
            unsupported_reason: None,
            evidence: None,
        }
    }

    #[test]
    fn key_list_passes_for_real_list_and_empty() {
        assert!(check_key_list(FEATURE_QUERY_KEYS).is_ok());
        assert!(check_key_list(&[]).is_ok());
    }

    #[test]
    fn key_list_reports_empty_and_duplicate_in_order() {
        let keys = [A, FeatureId(""), FeatureId("feature:"), A];
        assert_eq!(
            check_key_list(&keys),
            Err(vec![
                KeyTableViolation::EmptyKey { index: 1 },
                KeyTableViolation::EmptyKey { index: 2 },
                KeyTableViolation::DuplicateKey { key: A },
            ])
        );
    }

    #[test]
    fn table_keys_pass_when_sparse_and_empty() {
        assert!(check_table_keys(&[row(A)], &[A, B]).is_ok());
        assert!(check_table_keys::<()>(&[], &[A]).is_ok());
    }

    #[test]
    fn table_keys_collect_all_violations() {
        let table = [row(Z), row(Z), row(A), row(A)];
        assert_eq!(
            check_table_keys(&table, &[A]),
            Err(vec![
                KeyTableViolation::RowKeyNotInList { key: Z },
                KeyTableViolation::RowKeyNotInList { key: Z },
                KeyTableViolation::DuplicateRowKey { key: Z },
                KeyTableViolation::DuplicateRowKey { key: A },
            ])
        );
    }

    #[test]
    fn table_queries_pass_for_sparse_table() {
        static T: [FeatureRow<CapabilityQuery>; 1] = [FeatureRow {
            feature: A,
            rule: query_at(CapabilityLevel::S1),
            source: "s",
        }];
        assert!(check_table_queries(&T, &[A, B], |q| *q).is_ok());
    }

    #[test]
    fn table_queries_flag_missing_evidence() {
        static T: [FeatureRow<CapabilityQuery>; 1] = [FeatureRow {
            feature: A,
            rule: query_at(CapabilityLevel::S3),
            source: "s",
        }];
        let err = check_table_queries(&T, &[A, B], |q| *q).unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(
            matches!(err[0], CapabilityRuleViolation::MissingEvidence { feature, .. } if feature == A)
        );
    }

    #[test]
    fn table_queries_do_not_evaluate_rows_outside_list() {
        static T: [FeatureRow<CapabilityQuery>; 1] = [FeatureRow {
            feature: Z,
            rule: query_at(CapabilityLevel::S3),
            source: "s",
        }];
        let calls = Cell::new(0u32);
        let r = check_table_queries(&T, &[A], |q| {
            calls.set(calls.get() + 1);
            *q
        });
        assert!(r.is_ok());
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn ledger_passes_and_collects_each_violation() {
        assert!(check_ledger_keys(&[], &[A]).is_ok());
        let ok = [
            entry(A, BackendKind::Vulkan, "t"),
            entry(A, BackendKind::Metal, "t"),
        ];
        assert!(check_ledger_keys(&ok, &[A]).is_ok());
        let bad = [
            entry(Z, BackendKind::Vulkan, "t"),
            entry(A, BackendKind::Vulkan, "t"),
            entry(A, BackendKind::Vulkan, ""),
        ];
        assert_eq!(
            check_ledger_keys(&bad, &[A]),
            Err(vec![
                KeyTableViolation::LedgerKeyNotInList { key: Z },
                KeyTableViolation::DuplicateLedgerEntry {
                    key: A,
                    backend: BackendKind::Vulkan
                },
                KeyTableViolation::EmptyTestName {
                    key: A,
                    backend: BackendKind::Vulkan
                },
            ])
        );
    }

    #[test]
    fn baseline_diff_variants_are_distinct() {
        assert_ne!(BaselineDiff::Same, BaselineDiff::Differs);
    }
}
