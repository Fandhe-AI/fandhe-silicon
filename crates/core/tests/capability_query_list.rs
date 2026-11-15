//! `Capabilities::query_list` の既定の実装の公開テスト（TASK-179。ポインタのみ）。

use fandhe_silicon_core::capability::{
    Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId, KeyList,
    LimitsTable,
};
use fandhe_silicon_core::downlevel_key::DOWNLEVEL_QUERY_KEYS;
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::feature_key::{FEATURE_QUERY_KEYS, FEATURE_SHADER_F16, FEATURE_SUBGROUP};
use fandhe_silicon_core::limit_key::LIMIT_QUERY_KEYS;
use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;
use std::cell::Cell;

static LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1,
    max_dispatch_dims: [1, 1, 1],
    max_texture_size: 1,
};

/// 鍵ごとに別の値を返し、`query_ex` の呼び出しを数える試験用の値。
struct Dev {
    calls: Cell<usize>,
}

fn value_of(f: FeatureId) -> CapabilityQuery {
    if f == FEATURE_SHADER_F16 {
        CapabilityQuery {
            level: CapabilityLevel::S2,
            unsupported_reason: None,
            evidence: Some(CapabilityEvidence::RuntimeApi),
        }
    } else if f == FEATURE_SUBGROUP {
        CapabilityQuery {
            level: CapabilityLevel::S0,
            unsupported_reason: None,
            evidence: None,
        }
    } else {
        CapabilityQuery {
            level: CapabilityLevel::Unsupported,
            unsupported_reason: Some(UnsupportedReason::UnknownFeature),
            evidence: None,
        }
    }
}

impl Capabilities for Dev {
    fn query(&self, f: FeatureId) -> CapabilityLevel {
        value_of(f).level
    }
    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }
    fn query_ex(&self, f: FeatureId) -> CapabilityQuery {
        self.calls.set(self.calls.get() + 1);
        value_of(f)
    }
}

const LISTS: [(KeyList, &[FeatureId]); 4] = [
    (KeyList::Features, FEATURE_QUERY_KEYS),
    (KeyList::Downlevel, DOWNLEVEL_QUERY_KEYS),
    (KeyList::Limits, LIMIT_QUERY_KEYS),
    (KeyList::TextureFormats, TEXTURE_FORMAT_QUERY_KEYS),
];

// DEV-103
#[test]
fn key_list_points_at_the_core_lists() {
    for (list, keys) in LISTS {
        assert_eq!(list.keys(), keys, "{list:?}");
    }
}

// DEV-100
#[test]
fn query_list_returns_every_key_in_order_with_its_query_ex_value() {
    for (list, keys) in LISTS {
        let dev = Dev {
            calls: Cell::new(0),
        };
        let got = dev.query_list(list);
        assert_eq!(dev.calls.get(), keys.len(), "{list:?}");
        assert_eq!(got.len(), keys.len(), "{list:?}");
        for (&(k, q), &key) in got.iter().zip(keys) {
            assert_eq!(k, key);
            assert_eq!(q, value_of(key));
        }
    }
}

#[test]
fn query_list_keeps_s0_and_unsupported_keys() {
    let got = Dev {
        calls: Cell::new(0),
    }
    .query_list(KeyList::Features);
    assert!(
        got.iter()
            .any(|(k, q)| *k == FEATURE_SUBGROUP && q.level == CapabilityLevel::S0)
    );
    assert!(
        got.iter()
            .any(|(_, q)| q.level == CapabilityLevel::Unsupported)
    );
}

#[test]
fn query_list_works_through_dyn() {
    let dev: &dyn Capabilities = &Dev {
        calls: Cell::new(0),
    };
    assert_eq!(
        dev.query_list(KeyList::Downlevel).len(),
        DOWNLEVEL_QUERY_KEYS.len()
    );
}

#[cfg(feature = "fault-injection")]
#[test]
fn query_list_through_fault_injecting_follows_its_query_ex() {
    use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
    let failing = FaultInjecting::new(
        Dev {
            calls: Cell::new(0),
        },
        FaultScript::CapabilityQueryFails,
    );
    let got = failing.query_list(KeyList::Features);
    assert_eq!(got.len(), FEATURE_QUERY_KEYS.len());
    assert!(got.iter().all(|(_, q)| q.level == CapabilityLevel::S0));
    assert_eq!(failing.inner().calls.get(), 0);
    let over = CapabilityQuery {
        level: CapabilityLevel::Unsupported,
        unsupported_reason: Some(UnsupportedReason::NotSupported),
        evidence: None,
    };
    let overridden = FaultInjecting::new(
        Dev {
            calls: Cell::new(0),
        },
        FaultScript::CapabilityOverride {
            feature: FEATURE_SHADER_F16,
            query: over,
        },
    );
    for (k, q) in overridden.query_list(KeyList::Features) {
        assert_eq!(q, overridden.query_ex(k), "{k:?}");
    }
}
