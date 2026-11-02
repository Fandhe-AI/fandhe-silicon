//! 上限と downlevel の鍵の一覧を能力の表の枠に載せ、網羅・行の無い鍵の値・根拠の規則を一覧ごとに、外の crate から公開のパスだけで確かめる公開テスト（TASK-1362）。

use std::cell::Cell;
use std::collections::HashSet;

use fandhe_silicon_core::capability::{
    CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::capability_rule::CapabilityRuleViolation;
use fandhe_silicon_core::capability_table::{FeatureRow, UNLISTED_FEATURE_QUERY, query_from_table};
use fandhe_silicon_core::capability_table_check::{
    KeyTableViolation, check_key_list, check_table_keys, check_table_queries,
};
use fandhe_silicon_core::downlevel_key::{
    DOWNLEVEL_BASE_VERTEX, DOWNLEVEL_COMPUTE_SHADERS, DOWNLEVEL_FULL_DRAW_INDEX_UINT32,
    DOWNLEVEL_INDIRECT_EXECUTION, DOWNLEVEL_MSL2_1, DOWNLEVEL_QUERY_KEYS,
    DOWNLEVEL_TEXTURE_COMPRESSION,
};
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::feature_key::FEATURE_QUERY_KEYS;
use fandhe_silicon_core::limit_key::{
    LIMIT_MAX_BIND_GROUPS, LIMIT_MAX_BUFFER_SIZE, LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_X,
    LIMIT_MAX_RAY_RECURSION_DEPTH, LIMIT_MAX_TEXTURE_DIMENSION_1D,
    LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT, LIMIT_QUERY_KEYS,
};
use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;
use fandhe_silicon_core::value_key;

const fn q(
    level: CapabilityLevel,
    reason: Option<UnsupportedReason>,
    evidence: Option<CapabilityEvidence>,
) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: reason,
        evidence,
    }
}

const fn row(feature: FeatureId, rule: CapabilityQuery) -> FeatureRow<CapabilityQuery> {
    FeatureRow {
        feature,
        rule,
        source: "test",
    }
}

fn unit_row(feature: FeatureId) -> FeatureRow<()> {
    FeatureRow {
        feature,
        rule: (),
        source: "test",
    }
}

fn full_table(keys: &[FeatureId]) -> Vec<FeatureRow<()>> {
    keys.iter().map(|k| unit_row(*k)).collect()
}

/// 一覧の名前・一覧・鍵の接頭辞・一覧に入らない架空の鍵・もう一方の一覧の鍵の組。
type ListCase = (
    &'static str,
    &'static [FeatureId],
    &'static str,
    FeatureId,
    FeatureId,
);

fn lists() -> [ListCase; 2] {
    [
        (
            "limit",
            LIMIT_QUERY_KEYS,
            "limit:",
            FeatureId("limit:not_in_list"),
            DOWNLEVEL_COMPUTE_SHADERS,
        ),
        (
            "downlevel",
            DOWNLEVEL_QUERY_KEYS,
            "downlevel:",
            FeatureId("downlevel:NOT_IN_LIST"),
            LIMIT_MAX_BUFFER_SIZE,
        ),
    ]
}

#[test]
fn limit_and_downlevel_key_lists_are_complete() {
    assert_eq!(LIMIT_QUERY_KEYS.len(), 57);
    assert_eq!(DOWNLEVEL_QUERY_KEYS.len(), 26);
    for (name, keys, prefix, _, _) in lists() {
        assert_eq!(check_key_list(keys), Ok(()), "{name}");
        assert!(keys.iter().all(|k| k.0.starts_with(prefix)), "{name}");
    }
}

#[test]
fn missing_or_extra_key_is_detected() {
    for (name, keys, _, ghost, other_key) in lists() {
        let full = full_table(keys);
        assert_eq!(check_table_keys(&full, keys), Ok(()), "{name}");

        let removed = keys[0];
        let reduced: Vec<FeatureId> = keys.iter().copied().filter(|k| *k != removed).collect();
        assert_eq!(reduced.len(), keys.len() - 1, "{name}");
        assert_eq!(
            check_table_keys(&full, &reduced),
            Err(vec![KeyTableViolation::RowKeyNotInList { key: removed }]),
            "{name}"
        );

        for extra_key in [ghost, other_key] {
            let mut extra = full_table(keys);
            extra.push(unit_row(extra_key));
            assert_eq!(
                check_table_keys(&extra, keys),
                Err(vec![KeyTableViolation::RowKeyNotInList { key: extra_key }]),
                "{name}"
            );
        }

        let dup_key = keys[0];
        let mut dup = full_table(keys);
        dup.push(unit_row(dup_key));
        assert_eq!(
            check_table_keys(&dup, keys),
            Err(vec![KeyTableViolation::DuplicateRowKey { key: dup_key }]),
            "{name}"
        );

        let mut doubled: Vec<FeatureId> = keys.to_vec();
        doubled.push(dup_key);
        assert_eq!(
            check_key_list(&doubled),
            Err(vec![KeyTableViolation::DuplicateKey { key: dup_key }]),
            "{name}"
        );
    }
}

/// 行のある鍵は行の値を、行の無い鍵は `UNLISTED_FEATURE_QUERY`（`NoApi` ではない）を返すことを確かめる。
fn check_unlisted(name: &str, keys: &[FeatureId], table: &'static [FeatureRow<CapabilityQuery>]) {
    for r in table {
        assert_eq!(query_from_table(table, r.feature, |x| *x), r.rule, "{name}");
    }
    let rest: Vec<FeatureId> = keys
        .iter()
        .copied()
        .filter(|k| table.iter().all(|r| r.feature != *k))
        .collect();
    assert_eq!(rest.len(), keys.len() - table.len(), "{name}");
    for k in rest {
        let got = query_from_table(table, k, |x| *x);
        assert_eq!(got, UNLISTED_FEATURE_QUERY, "{name}");
        assert_ne!(
            got.unsupported_reason,
            Some(UnsupportedReason::NoApi),
            "{name}"
        );
    }
}

#[test]
fn unlisted_key_is_unknown_feature_not_no_api() {
    static EMPTY: [FeatureRow<CapabilityQuery>; 0] = [];
    let calls = Cell::new(0u32);
    for (name, keys, _, _, _) in lists() {
        for k in keys {
            let got = query_from_table(&EMPTY, *k, |r| {
                calls.set(calls.get() + 1);
                *r
            });
            assert_eq!(got, UNLISTED_FEATURE_QUERY, "{name}");
            assert_eq!(got.level, CapabilityLevel::Unsupported, "{name}");
            assert_eq!(
                got.unsupported_reason,
                Some(UnsupportedReason::UnknownFeature),
                "{name}"
            );
            assert_eq!(got.evidence, None, "{name}");
            assert_ne!(
                got.unsupported_reason,
                Some(UnsupportedReason::NoApi),
                "{name}"
            );
        }
    }
    assert_eq!(calls.get(), 0);

    static LIMIT_TABLE: [FeatureRow<CapabilityQuery>; 4] = [
        row(
            LIMIT_MAX_TEXTURE_DIMENSION_1D,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NoApi),
                None,
            ),
        ),
        row(
            LIMIT_MAX_BUFFER_SIZE,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NotSupported),
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
        row(
            LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT,
            q(CapabilityLevel::S0, None, None),
        ),
        row(
            LIMIT_MAX_RAY_RECURSION_DEPTH,
            q(
                CapabilityLevel::S2,
                None,
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
    ];
    static DOWNLEVEL_TABLE: [FeatureRow<CapabilityQuery>; 4] = [
        row(
            DOWNLEVEL_COMPUTE_SHADERS,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NoApi),
                None,
            ),
        ),
        row(
            DOWNLEVEL_INDIRECT_EXECUTION,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NotSupported),
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
        row(DOWNLEVEL_MSL2_1, q(CapabilityLevel::S0, None, None)),
        row(
            DOWNLEVEL_TEXTURE_COMPRESSION,
            q(
                CapabilityLevel::S2,
                None,
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
    ];
    check_unlisted("limit", LIMIT_QUERY_KEYS, &LIMIT_TABLE);
    check_unlisted("downlevel", DOWNLEVEL_QUERY_KEYS, &DOWNLEVEL_TABLE);
}

/// 根拠の規則に合う 5 つの段階の組み合わせ。鍵だけを差し替えて表の行にする。
const fn evidence_rules() -> [CapabilityQuery; 5] {
    [
        q(
            CapabilityLevel::S1,
            None,
            Some(CapabilityEvidence::StaticVendorTable),
        ),
        q(
            CapabilityLevel::S2,
            None,
            Some(CapabilityEvidence::RuntimeApi),
        ),
        q(
            CapabilityLevel::S3,
            None,
            Some(CapabilityEvidence::ProbeCompile),
        ),
        q(
            CapabilityLevel::S4,
            None,
            Some(CapabilityEvidence::ImplementationLedger),
        ),
        q(
            CapabilityLevel::Unsupported,
            Some(UnsupportedReason::NotSupported),
            Some(CapabilityEvidence::RuntimeApi),
        ),
    ]
}

static LIMIT_OK: [FeatureRow<CapabilityQuery>; 5] = {
    let r = evidence_rules();
    [
        row(LIMIT_MAX_TEXTURE_DIMENSION_1D, r[0]),
        row(LIMIT_MAX_BUFFER_SIZE, r[1]),
        row(LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT, r[2]),
        row(LIMIT_MAX_RAY_RECURSION_DEPTH, r[3]),
        row(LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_X, r[4]),
    ]
};

static LIMIT_BAD: [FeatureRow<CapabilityQuery>; 6] = {
    let r = evidence_rules();
    [
        row(LIMIT_MAX_TEXTURE_DIMENSION_1D, r[0]),
        row(LIMIT_MAX_BUFFER_SIZE, r[1]),
        row(LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT, r[2]),
        row(LIMIT_MAX_RAY_RECURSION_DEPTH, r[3]),
        row(LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_X, r[4]),
        row(LIMIT_MAX_BIND_GROUPS, q(CapabilityLevel::S3, None, None)),
    ]
};

static DOWNLEVEL_OK: [FeatureRow<CapabilityQuery>; 5] = {
    let r = evidence_rules();
    [
        row(DOWNLEVEL_COMPUTE_SHADERS, r[0]),
        row(DOWNLEVEL_INDIRECT_EXECUTION, r[1]),
        row(DOWNLEVEL_MSL2_1, r[2]),
        row(DOWNLEVEL_TEXTURE_COMPRESSION, r[3]),
        row(DOWNLEVEL_FULL_DRAW_INDEX_UINT32, r[4]),
    ]
};

static DOWNLEVEL_BAD: [FeatureRow<CapabilityQuery>; 6] = {
    let r = evidence_rules();
    [
        row(DOWNLEVEL_COMPUTE_SHADERS, r[0]),
        row(DOWNLEVEL_INDIRECT_EXECUTION, r[1]),
        row(DOWNLEVEL_MSL2_1, r[2]),
        row(DOWNLEVEL_TEXTURE_COMPRESSION, r[3]),
        row(DOWNLEVEL_FULL_DRAW_INDEX_UINT32, r[4]),
        row(DOWNLEVEL_BASE_VERTEX, q(CapabilityLevel::S3, None, None)),
    ]
};

#[test]
fn s3_and_above_carry_evidence() {
    assert_eq!(
        check_table_queries(&LIMIT_OK, LIMIT_QUERY_KEYS, |x| *x),
        Ok(())
    );
    let err = check_table_queries(&LIMIT_BAD, LIMIT_QUERY_KEYS, |x| *x)
        .expect_err("S3 without evidence must be rejected");
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        CapabilityRuleViolation::MissingEvidence { feature, .. }
            if feature == LIMIT_MAX_BIND_GROUPS
    ));

    assert_eq!(
        check_table_queries(&DOWNLEVEL_OK, DOWNLEVEL_QUERY_KEYS, |x| *x),
        Ok(())
    );
    let err = check_table_queries(&DOWNLEVEL_BAD, DOWNLEVEL_QUERY_KEYS, |x| *x)
        .expect_err("S3 without evidence must be rejected");
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        CapabilityRuleViolation::MissingEvidence { feature, .. }
            if feature == DOWNLEVEL_BASE_VERTEX
    ));
}

#[test]
fn value_keys_follow_limit_keys() {
    assert_eq!(value_key::LIMIT_VALUE_KEYS.len(), LIMIT_QUERY_KEYS.len());
    for (i, (v, l)) in value_key::LIMIT_VALUE_KEYS
        .iter()
        .zip(LIMIT_QUERY_KEYS)
        .enumerate()
    {
        assert_eq!(v.0, l.0, "position {i}");
    }
    let distinct: HashSet<_> = value_key::LIMIT_VALUE_KEYS.iter().collect();
    assert_eq!(distinct.len(), 57);
    assert_eq!(value_key::LIMIT_MAX_BUFFER_SIZE.0, LIMIT_MAX_BUFFER_SIZE.0);
}

#[test]
fn key_lists_are_disjoint() {
    let all: [(&str, &[FeatureId]); 4] = [
        ("limit", LIMIT_QUERY_KEYS),
        ("downlevel", DOWNLEVEL_QUERY_KEYS),
        ("feature", FEATURE_QUERY_KEYS),
        ("texture_format", TEXTURE_FORMAT_QUERY_KEYS),
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            let a: HashSet<FeatureId> = all[i].1.iter().copied().collect();
            let b: HashSet<FeatureId> = all[j].1.iter().copied().collect();
            assert!(
                a.is_disjoint(&b),
                "{} and {} share a key",
                all[i].0,
                all[j].0
            );
        }
    }
}
