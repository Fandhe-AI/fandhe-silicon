//! 検査関数の契約テストの組み合わせ（spec TASK-273 の 5/6。ERR-51・ERR-52 のポインタ）。
//!
//! 4/6 の検査関数（`check_query`・`check_query_ex`）と 2/6 の部品（`CapabilityTable`・`StepLog`）を、
//! デコレータ（`FaultInjecting`）・`dyn Capabilities` の入れ物・複数の機能を順に流す形と組み合わせて確かめる。

use std::sync::Arc;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
};
use fandhe_silicon_core::capability_rule::{
    CapabilityRuleViolation, CapabilityTable, Step, StepLog, check_query_ex,
};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

const A: FeatureId = FeatureId("feature:A");
const B: FeatureId = FeatureId("feature:B");
const C: FeatureId = FeatureId("feature:C");
const UNLISTED: FeatureId = FeatureId("feature:unlisted");
const ALL: [FeatureId; 4] = [A, B, C, UNLISTED];

type Checked = Result<CapabilityQuery, Vec<CapabilityRuleViolation>>;

fn q(
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

fn unsupported() -> CapabilityQuery {
    q(
        CapabilityLevel::Unsupported,
        Some(UnsupportedReason::NotSupported),
        None,
    )
}

fn s0() -> CapabilityQuery {
    q(CapabilityLevel::S0, None, None)
}

fn s2() -> CapabilityQuery {
    q(
        CapabilityLevel::S2,
        None,
        Some(CapabilityEvidence::RuntimeApi),
    )
}

/// 規則に反する値（S4 なのに根拠が無い）。
fn bad() -> CapabilityQuery {
    q(CapabilityLevel::S4, None, None)
}

/// A は確認済みの非対応、B は S2、C は規則に反する値を返す表。
fn table(log: &Arc<StepLog>) -> CapabilityTable {
    CapabilityTable::new()
        .with(A, unsupported())
        .with(B, s2())
        .with(C, bad())
        .with_log(log.clone())
}

/// 内側の表と記録を作り、`script` で包んで返す。
fn decorated(script: FaultScript) -> (FaultInjecting<CapabilityTable>, Arc<StepLog>) {
    let log = Arc::new(StepLog::new());
    (FaultInjecting::new(table(&log), script), log)
}

fn missing_evidence_c() -> Vec<CapabilityRuleViolation> {
    vec![CapabilityRuleViolation::MissingEvidence {
        feature: C,
        query: bad(),
    }]
}

/// `query` だけを実装し、既定の `query_ex` を使うスタブ。
struct QueryOnly(CapabilityLevel);

/// `Capabilities::limits` の試験用の表。値は試験用の小さな数で、意味は持たない。
static TEST_LIMITS: fandhe_silicon_core::capability::LimitsTable =
    fandhe_silicon_core::capability::LimitsTable {
        max_buffer_bytes: 1,
        max_dispatch_dims: [1, 1, 1],
        max_texture_size: 1,
    };

impl Capabilities for QueryOnly {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        &TEST_LIMITS
    }

    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        self.0
    }
}

/// 製品側に包括の実装は無いので、`dyn Capabilities` を持つ試験用の入れ物で検査に渡す。
struct DynDevice(Box<dyn Capabilities + Send + Sync>);

impl Capabilities for DynDevice {
    fn limits(&self) -> &fandhe_silicon_core::capability::LimitsTable {
        self.0.limits()
    }

    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        self.0.query(feature)
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        self.0.query_ex(feature)
    }
}

#[test]
fn query_fails_masks_inner_and_passes_check() {
    let (dev, log) = decorated(FaultScript::CapabilityQueryFails);
    for f in ALL {
        assert_eq!(check_query_ex(&dev, f), Ok(s0()), "{f:?}");
    }
    assert_eq!(log.read_count(), 0);

    let plain_log = Arc::new(StepLog::new());
    let plain = check_query_ex(&table(&plain_log), A);
    assert_eq!(plain, Ok(unsupported()));
    assert_ne!(check_query_ex(&dev, A), plain);
}

#[test]
fn capability_insufficient_target_passes_and_others_pass_through() {
    let (dev, log) = decorated(FaultScript::CapabilityInsufficient {
        feature: A,
        required: CapabilityLevel::S2,
    });
    assert_eq!(check_query_ex(&dev, A), Ok(unsupported()));
    assert_eq!(log.read_count(), 0);

    assert_eq!(check_query_ex(&dev, B), Ok(s2()));
    assert_eq!(log.steps(), vec![Step::ReadFeature(B)]);

    assert_eq!(check_query_ex(&dev, C), Err(missing_evidence_c()));
    assert_eq!(log.read_count(), 2);
}

#[test]
fn non_capability_scripts_match_bare_table() {
    let scripts = vec![
        FaultScript::PassThrough,
        FaultScript::CompletionUnknown,
        FaultScript::AllocationFails,
        FaultScript::RuntimeError {
            backend: BackendKind::Metal,
            detail: "test".to_string(),
        },
    ];
    let bare_log = Arc::new(StepLog::new());
    let bare = table(&bare_log);
    for script in scripts {
        let (dev, _log) = decorated(script);
        for f in ALL {
            let wrapped: Checked = check_query_ex(&dev, f);
            assert_eq!(wrapped, check_query_ex(&bare, f), "{f:?}");
        }
    }
}

#[test]
fn default_query_ex_violation_survives_decorator() {
    let no_reason = q(CapabilityLevel::Unsupported, None, None);
    let pass = FaultInjecting::new(
        QueryOnly(CapabilityLevel::Unsupported),
        FaultScript::PassThrough,
    );
    assert_eq!(
        check_query_ex(&pass, A),
        Err(vec![CapabilityRuleViolation::MissingUnsupportedReason {
            feature: A,
            query: no_reason,
        }])
    );

    let dev = FaultInjecting::new(
        QueryOnly(CapabilityLevel::Unsupported),
        FaultScript::CapabilityInsufficient {
            feature: A,
            required: CapabilityLevel::S2,
        },
    );
    assert_eq!(check_query_ex(&dev, A), Ok(unsupported()));
    assert_eq!(
        check_query_ex(&dev, B),
        Err(vec![CapabilityRuleViolation::MissingUnsupportedReason {
            feature: B,
            query: no_reason,
        }])
    );
}

fn dyn_scripts() -> Vec<FaultScript> {
    vec![
        FaultScript::PassThrough,
        FaultScript::CapabilityQueryFails,
        FaultScript::CapabilityInsufficient {
            feature: A,
            required: CapabilityLevel::S2,
        },
    ]
}

#[test]
fn dyn_wrappers_give_same_results() {
    for script in dyn_scripts() {
        let (direct, _l1) = decorated(script.clone());
        let (for_ref, _l2) = decorated(script.clone());
        let (for_box, _l3) = decorated(script);
        let by_ref: &dyn Capabilities = &for_ref;
        let boxed = DynDevice(Box::new(for_box));
        for f in ALL {
            let expected = check_query_ex(&direct, f);
            assert_eq!(check_query_ex(by_ref, f), expected, "{f:?}");
            assert_eq!(check_query_ex(&boxed, f), expected, "{f:?}");
        }
    }

    let (failed, _l1) = decorated(FaultScript::CapabilityQueryFails);
    let (plain, _l2) = decorated(FaultScript::PassThrough);
    let failed = DynDevice(Box::new(failed));
    let plain = DynDevice(Box::new(plain));
    assert_eq!(check_query_ex(&failed, A), Ok(s0()));
    assert_eq!(check_query_ex(&plain, A), Ok(unsupported()));
}

#[test]
fn sweep_reports_violations_per_feature_in_order() {
    let (dev, log) = decorated(FaultScript::PassThrough);
    let results: Vec<Checked> = ALL.iter().map(|f| check_query_ex(&dev, *f)).collect();
    assert_eq!(results[0], Ok(unsupported()));
    assert_eq!(results[1], Ok(s2()));
    assert_eq!(results[2], Err(missing_evidence_c()));
    assert_eq!(results[3], Ok(s0()));
    assert_eq!(
        log.steps(),
        ALL.iter()
            .map(|f| Step::ReadFeature(*f))
            .collect::<Vec<_>>()
    );
}

fn wrap(t: CapabilityTable) -> (FaultInjecting<CapabilityTable>, DynDevice) {
    (
        FaultInjecting::new(t.clone(), FaultScript::PassThrough),
        DynDevice(Box::new(FaultInjecting::new(t, FaultScript::PassThrough))),
    )
}

#[test]
fn levels_without_runtime_data_through_wrappers() {
    let evidences = [
        CapabilityEvidence::RuntimeApi,
        CapabilityEvidence::StaticVendorTable,
        CapabilityEvidence::ProbeCompile,
        CapabilityEvidence::ImplementationLedger,
        CapabilityEvidence::FieldRecord,
        CapabilityEvidence::PerformanceComparison,
    ];
    let high = [
        CapabilityLevel::S3,
        CapabilityLevel::S4,
        CapabilityLevel::S5,
        CapabilityLevel::S6,
    ];

    let s1 = q(CapabilityLevel::S1, None, None);
    let (a, b) = wrap(CapabilityTable::new().with(A, s1));
    assert_eq!(check_query_ex(&a, A), Ok(s1));
    assert_eq!(check_query_ex(&b, A), Ok(s1));

    for level in high {
        let none = q(level, None, None);
        let (a, b) = wrap(CapabilityTable::new().with(A, none));
        let expected = Err(vec![CapabilityRuleViolation::MissingEvidence {
            feature: A,
            query: none,
        }]);
        assert_eq!(check_query_ex(&a, A), expected, "{level:?}");
        assert_eq!(check_query_ex(&b, A), expected, "{level:?}");
        for e in evidences {
            let v = q(level, None, Some(e));
            let (a, b) = wrap(CapabilityTable::new().with(A, v));
            assert_eq!(check_query_ex(&a, A), Ok(v), "{level:?} {e:?}");
            assert_eq!(check_query_ex(&b, A), Ok(v), "{level:?} {e:?}");
        }
    }
}

/// 列挙 → 取得 → 検査の順に試験の中で手で組む。失敗した段より後は呼ばない。
fn acquire_then_check<D: Capabilities>(
    log: &StepLog,
    enumerate: Result<(), ContractError>,
    acquire: Result<D, ContractError>,
    feature: FeatureId,
) -> Option<Checked> {
    log.push(Step::Enumerate);
    enumerate.ok()?;
    log.push(Step::RequestDevice);
    let dev = acquire.ok()?;
    Some(check_query_ex(&dev, feature))
}

#[test]
fn acquire_then_check_in_order() {
    let log = Arc::new(StepLog::new());
    let r = acquire_then_check(&log, Err(ContractError::Timeout), Ok(table(&log)), A);
    assert_eq!(r, None);
    assert_eq!(log.read_count(), 0);
    assert_eq!(log.steps(), vec![Step::Enumerate]);

    let log = Arc::new(StepLog::new());
    let r = acquire_then_check::<CapabilityTable>(&log, Ok(()), Err(ContractError::Timeout), A);
    assert_eq!(r, None);
    assert_eq!(log.read_count(), 0);
    assert_eq!(log.steps(), vec![Step::Enumerate, Step::RequestDevice]);

    let log = Arc::new(StepLog::new());
    let r = acquire_then_check(&log, Ok(()), Ok(table(&log)), C);
    assert_eq!(r, Some(Err(missing_evidence_c())));
    assert_eq!(
        log.steps(),
        vec![Step::Enumerate, Step::RequestDevice, Step::ReadFeature(C)]
    );
}
