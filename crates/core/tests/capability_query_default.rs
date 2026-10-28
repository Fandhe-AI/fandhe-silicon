//! `Capabilities` の機能 ID を鍵にした段階の返りと、`query_ex` の既定の実装の値を固定する公開テスト（TASK-136 の 3/6）。
//!
//! 試験用のスタブは必須の `query`・`limits` だけを実装し `query_ex` を上書きしない。これがそのままコンパイルできることが、
//! 既定の実装つきであることの確かめになる（DEV-86。ポインタのみ）。
//! `query` は機能 ID ごとに別の段階を返し、値がそのまま渡ることを見る（DEV-82。ポインタのみ）。

use fandhe_silicon_core::capability::{Capabilities, CapabilityLevel, CapabilityQuery, FeatureId};

struct Table;

/// `Capabilities::limits` の試験用の表。値は試験用の小さな数で、意味は持たない。
static TEST_LIMITS: fandhe_silicon_core::capability::LimitsTable =
    fandhe_silicon_core::capability::LimitsTable {
        max_buffer_bytes: 1,
        max_dispatch_dims: [1, 1, 1],
        max_texture_size: 1,
    };

impl Capabilities for Table {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        match feature.0 {
            "feature:SUBGROUP" => CapabilityLevel::S3,
            "feature:ATOMIC_ADD" => CapabilityLevel::S1,
            "feature:F16_FMA" => CapabilityLevel::S6,
            _ => CapabilityLevel::Unsupported,
        }
    }
}

const EXPECTED: [(&str, CapabilityLevel); 4] = [
    ("feature:SUBGROUP", CapabilityLevel::S3),
    ("feature:ATOMIC_ADD", CapabilityLevel::S1),
    ("feature:F16_FMA", CapabilityLevel::S6),
    ("feature:UNKNOWN", CapabilityLevel::Unsupported),
];

#[test]
fn query_returns_level_keyed_by_feature_id() {
    for (name, level) in EXPECTED {
        let id = FeatureId(name);
        let FeatureId(s) = id;
        assert_eq!(s, name);
        assert_eq!(Table.query(id), level, "{name}");
    }
    assert_eq!(FeatureId("feature:SUBGROUP"), FeatureId("feature:SUBGROUP"));
    assert_ne!(
        FeatureId("feature:SUBGROUP"),
        FeatureId("feature:ATOMIC_ADD")
    );
}

#[test]
fn query_ex_default_passes_query_level_through() {
    for (name, level) in EXPECTED {
        let CapabilityQuery {
            level: got,
            unsupported_reason,
            evidence,
        } = Table.query_ex(FeatureId(name));
        assert_eq!(got, level, "{name}");
        assert_eq!(got, Table.query(FeatureId(name)), "{name}");
        assert_eq!(unsupported_reason, None, "{name}");
        assert_eq!(evidence, None, "{name}");
    }
}

#[test]
fn query_ex_default_works_through_dyn() {
    let c: &dyn Capabilities = &Table;
    for (name, level) in EXPECTED {
        let q = c.query_ex(FeatureId(name));
        assert_eq!(q.level, level, "{name}");
        assert_eq!(q.unsupported_reason, None, "{name}");
        assert_eq!(q.evidence, None, "{name}");
    }
}
