//! 機能の鍵の一覧・能力の表の行の型と引き方・表と台帳の検査の道具を、外の crate から公開のパスだけで確かめる公開テスト（TASK-1114）。

use std::cell::Cell;

use fandhe_silicon_core::capability::{
    BackendKind, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::capability_rule::CapabilityRuleViolation;
use fandhe_silicon_core::capability_table::{FeatureRow, UNLISTED_FEATURE_QUERY, query_from_table};
use fandhe_silicon_core::capability_table_check::{
    BaselineDiff, BaselineDiffEntry, KeyTableViolation, check_key_list, check_ledger_keys,
    check_table_keys, check_table_queries,
};
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::feature_key::{
    FEATURE_MAPPABLE_PRIMARY_BUFFERS, FEATURE_PIPELINE_STATISTICS_QUERY, FEATURE_QUERY_KEYS,
    FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_TEXTURE_COMPRESSION_ASTC_HDR,
    FEATURE_TEXTURE_FORMAT_16BIT_NORM, FEATURE_TIMESTAMP_QUERY_INSIDE_ENCODERS,
};
use fandhe_silicon_core::texture_format::TextureFormat;

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

fn full_table() -> Vec<FeatureRow<()>> {
    FEATURE_QUERY_KEYS
        .iter()
        .map(|k| FeatureRow {
            feature: *k,
            rule: (),
            source: "test",
        })
        .collect()
}

#[test]
fn feature_query_keys_are_complete_list() {
    assert_eq!(FEATURE_QUERY_KEYS.len(), 79);
    assert_eq!(check_key_list(FEATURE_QUERY_KEYS), Ok(()));
    assert!(
        FEATURE_QUERY_KEYS
            .iter()
            .all(|k| k.0.starts_with("feature:"))
    );
}

#[test]
fn missing_or_extra_key_is_detected() {
    let full = full_table();
    assert_eq!(check_table_keys(&full, FEATURE_QUERY_KEYS), Ok(()));

    let removed = FEATURE_QUERY_KEYS[10];
    let reduced: Vec<FeatureId> = FEATURE_QUERY_KEYS
        .iter()
        .copied()
        .filter(|k| *k != removed)
        .collect();
    assert_eq!(reduced.len(), 78);
    assert_eq!(
        check_table_keys(&full, &reduced),
        Err(vec![KeyTableViolation::RowKeyNotInList { key: removed }])
    );

    let ghost = FeatureId("feature:NOT_IN_LIST");
    let mut extra = full_table();
    extra.push(FeatureRow {
        feature: ghost,
        rule: (),
        source: "test",
    });
    assert_eq!(
        check_table_keys(&extra, FEATURE_QUERY_KEYS),
        Err(vec![KeyTableViolation::RowKeyNotInList { key: ghost }])
    );

    let dup_key = FEATURE_QUERY_KEYS[0];
    let mut dup = full_table();
    dup.push(FeatureRow {
        feature: dup_key,
        rule: (),
        source: "test",
    });
    assert_eq!(
        check_table_keys(&dup, FEATURE_QUERY_KEYS),
        Err(vec![KeyTableViolation::DuplicateRowKey { key: dup_key }])
    );

    let mut doubled: Vec<FeatureId> = FEATURE_QUERY_KEYS.to_vec();
    doubled.push(dup_key);
    assert_eq!(
        check_key_list(&doubled),
        Err(vec![KeyTableViolation::DuplicateKey { key: dup_key }])
    );
}

#[test]
fn unlisted_key_is_unknown_feature_not_no_api() {
    static EMPTY: [FeatureRow<CapabilityQuery>; 0] = [];
    let calls = Cell::new(0u32);
    for k in FEATURE_QUERY_KEYS {
        let got = query_from_table(&EMPTY, *k, |r| {
            calls.set(calls.get() + 1);
            *r
        });
        assert_eq!(got, UNLISTED_FEATURE_QUERY);
        assert_eq!(got.level, CapabilityLevel::Unsupported);
        assert_eq!(
            got.unsupported_reason,
            Some(UnsupportedReason::UnknownFeature)
        );
        assert_eq!(got.evidence, None);
        assert_ne!(got.unsupported_reason, Some(UnsupportedReason::NoApi));
    }
    assert_eq!(calls.get(), 0);

    let a = q(
        CapabilityLevel::Unsupported,
        Some(UnsupportedReason::NoApi),
        None,
    );
    let b = q(
        CapabilityLevel::Unsupported,
        Some(UnsupportedReason::NotSupported),
        Some(CapabilityEvidence::RuntimeApi),
    );
    let c = q(
        CapabilityLevel::S2,
        None,
        Some(CapabilityEvidence::RuntimeApi),
    );
    static TABLE: [FeatureRow<CapabilityQuery>; 3] = [
        row(
            FEATURE_SHADER_FLOAT32_ATOMIC,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NoApi),
                None,
            ),
        ),
        row(
            FEATURE_PIPELINE_STATISTICS_QUERY,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NotSupported),
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
        row(
            FEATURE_MAPPABLE_PRIMARY_BUFFERS,
            q(
                CapabilityLevel::S2,
                None,
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
    ];
    let expected = [
        (FEATURE_SHADER_FLOAT32_ATOMIC, a),
        (FEATURE_PIPELINE_STATISTICS_QUERY, b),
        (FEATURE_MAPPABLE_PRIMARY_BUFFERS, c),
    ];
    for (k, want) in expected {
        assert_eq!(query_from_table(&TABLE, k, |r| *r), want);
    }
    let rest: Vec<FeatureId> = FEATURE_QUERY_KEYS
        .iter()
        .copied()
        .filter(|k| expected.iter().all(|(e, _)| e != k))
        .collect();
    assert_eq!(rest.len(), 79 - 3);
    for k in rest {
        let got = query_from_table(&TABLE, k, |r| *r);
        assert_eq!(got, UNLISTED_FEATURE_QUERY);
        assert_ne!(got.unsupported_reason, Some(UnsupportedReason::NoApi));
    }
}

#[test]
fn s3_and_above_carry_evidence() {
    static OK_TABLE: [FeatureRow<CapabilityQuery>; 5] = [
        row(
            FEATURE_SHADER_FLOAT32_ATOMIC,
            q(
                CapabilityLevel::S1,
                None,
                Some(CapabilityEvidence::StaticVendorTable),
            ),
        ),
        row(
            FEATURE_PIPELINE_STATISTICS_QUERY,
            q(
                CapabilityLevel::S2,
                None,
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
        row(
            FEATURE_MAPPABLE_PRIMARY_BUFFERS,
            q(
                CapabilityLevel::S3,
                None,
                Some(CapabilityEvidence::ProbeCompile),
            ),
        ),
        row(
            FEATURE_TEXTURE_FORMAT_16BIT_NORM,
            q(
                CapabilityLevel::S4,
                None,
                Some(CapabilityEvidence::ImplementationLedger),
            ),
        ),
        row(
            FEATURE_TEXTURE_COMPRESSION_ASTC_HDR,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NotSupported),
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
    ];
    assert_eq!(
        check_table_queries(&OK_TABLE, FEATURE_QUERY_KEYS, |r| *r),
        Ok(())
    );

    static BAD_TABLE: [FeatureRow<CapabilityQuery>; 6] = [
        row(
            FEATURE_SHADER_FLOAT32_ATOMIC,
            q(
                CapabilityLevel::S1,
                None,
                Some(CapabilityEvidence::StaticVendorTable),
            ),
        ),
        row(
            FEATURE_PIPELINE_STATISTICS_QUERY,
            q(
                CapabilityLevel::S2,
                None,
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
        row(
            FEATURE_MAPPABLE_PRIMARY_BUFFERS,
            q(
                CapabilityLevel::S3,
                None,
                Some(CapabilityEvidence::ProbeCompile),
            ),
        ),
        row(
            FEATURE_TEXTURE_FORMAT_16BIT_NORM,
            q(
                CapabilityLevel::S4,
                None,
                Some(CapabilityEvidence::ImplementationLedger),
            ),
        ),
        row(
            FEATURE_TEXTURE_COMPRESSION_ASTC_HDR,
            q(
                CapabilityLevel::Unsupported,
                Some(UnsupportedReason::NotSupported),
                Some(CapabilityEvidence::RuntimeApi),
            ),
        ),
        row(
            FEATURE_TIMESTAMP_QUERY_INSIDE_ENCODERS,
            q(CapabilityLevel::S3, None, None),
        ),
    ];
    let err = check_table_queries(&BAD_TABLE, FEATURE_QUERY_KEYS, |r| *r)
        .expect_err("S3 without evidence must be rejected");
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        CapabilityRuleViolation::MissingEvidence { feature, .. }
            if feature == FEATURE_TIMESTAMP_QUERY_INSIDE_ENCODERS
    ));
}

#[test]
fn texture_format_required_features_are_listed() {
    let mut some_count = 0usize;
    for f in TextureFormat::all() {
        if let Some(k) = f.required_feature() {
            some_count += 1;
            assert!(
                FEATURE_QUERY_KEYS.contains(&k),
                "required_feature of {f:?} is not in the key list: {k:?}"
            );
        }
    }
    assert!(some_count > 0);
}

#[test]
fn ledger_entries_follow_key_list() {
    let k0 = FEATURE_QUERY_KEYS[0];
    let k1 = FEATURE_QUERY_KEYS[1];
    let entry = |feature, backend, result, test| BaselineDiffEntry {
        feature,
        backend,
        result,
        test,
    };

    assert_eq!(check_ledger_keys(&[], FEATURE_QUERY_KEYS), Ok(()));
    let ok = [
        entry(k0, BackendKind::Vulkan, BaselineDiff::Differs, "t_a"),
        entry(k1, BackendKind::Vulkan, BaselineDiff::Same, "t_b"),
        entry(k0, BackendKind::Metal, BaselineDiff::Same, "t_c"),
    ];
    assert_eq!(check_ledger_keys(&ok, FEATURE_QUERY_KEYS), Ok(()));

    let ghost = FeatureId("feature:NOT_IN_LIST");
    let outside = [entry(ghost, BackendKind::Vulkan, BaselineDiff::Same, "t")];
    assert_eq!(
        check_ledger_keys(&outside, FEATURE_QUERY_KEYS),
        Err(vec![KeyTableViolation::LedgerKeyNotInList { key: ghost }])
    );

    let dup = [
        entry(k0, BackendKind::Vulkan, BaselineDiff::Same, "t1"),
        entry(k0, BackendKind::Vulkan, BaselineDiff::Differs, "t2"),
    ];
    assert_eq!(
        check_ledger_keys(&dup, FEATURE_QUERY_KEYS),
        Err(vec![KeyTableViolation::DuplicateLedgerEntry {
            key: k0,
            backend: BackendKind::Vulkan
        }])
    );

    let empty = [entry(k0, BackendKind::Cuda, BaselineDiff::Same, "")];
    assert_eq!(
        check_ledger_keys(&empty, FEATURE_QUERY_KEYS),
        Err(vec![KeyTableViolation::EmptyTestName {
            key: k0,
            backend: BackendKind::Cuda
        }])
    );
}
