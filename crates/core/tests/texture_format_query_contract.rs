//! 形式の問合せの鍵を、試験用の device と試験の中だけの実装に流す契約テスト（TASK-1314）。
//!
//! 公開のパスだけを使う。プロセスの環境は読まず、設定は `RunSetting::from_lookup` へ渡す。

use std::cell::Cell;
use std::ffi::OsString;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, FeatureId, LimitsTable,
};
use fandhe_silicon_core::capability_rule::{CapabilityRuleViolation, CapabilityTable};
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::stub_device::{StubDevice, StubDeviceProvider};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Declaration, Label, MachineCombination, NotRunReason, RunResult, RunSetting,
    Subject, Verdict, judge,
};
use fandhe_silicon_core::texture_format_key::TEXTURE_FORMAT_QUERY_KEYS;
use fandhe_silicon_core::texture_format_query_check::{
    TextureFormatQueryViolation, check_rowless_texture_format_keys, check_texture_format_queries,
    texture_format_keys_without_rows,
};

const ASTC: FeatureId = FeatureId("texformat:Astc");

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

fn backends() -> [BackendKind; 4] {
    [
        BackendKind::Metal,
        BackendKind::Vulkan,
        BackendKind::Cuda,
        BackendKind::Dx12Compute,
    ]
}

fn profiles() -> [Profile; 3] {
    [Profile::CpuOnly, Profile::ComputeOnly, Profile::Graphics]
}

fn request(backend: BackendKind, profile: Profile) -> DeviceRequest {
    DeviceRequest {
        backend,
        profile,
        ownership: DeviceOwnership::Shared,
    }
}

fn stub() -> StubDevice {
    StubDevice::new(request(BackendKind::Metal, Profile::ComputeOnly))
}

/// 一覧の全鍵に `ROW` を持たせ、それ以外は `UNLISTED_FEATURE_QUERY` を返す表。
fn passing_table() -> CapabilityTable {
    let mut t = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
    for key in TEXTURE_FORMAT_QUERY_KEYS {
        t = t.with(*key, ROW);
    }
    t
}

/// `query` が `S1`、`query_ex` が別の段階を返す試験の中だけの実装。
struct LevelSplit;

impl Capabilities for LevelSplit {
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

/// 指定の鍵の問合せで panic する試験の中だけの実装。
struct PanicsOn(FeatureId);

impl Capabilities for PanicsOn {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        if feature == self.0 {
            panic!("query panicked on purpose");
        }
        ROW.level
    }
    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }
    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        if feature == self.0 {
            panic!("query_ex panicked on purpose");
        }
        ROW
    }
}

#[test]
fn stub_passes_texture_format_query_checks() {
    let mut count = 0;
    for backend in backends() {
        for profile in profiles() {
            let device = StubDeviceProvider::request(request(backend, profile)).unwrap();
            let d: &dyn Device = &*device;
            assert_eq!(
                check_texture_format_queries(d, TEXTURE_FORMAT_QUERY_KEYS),
                Ok(())
            );
            assert_eq!(check_rowless_texture_format_keys(d), Ok(()));
            assert!(texture_format_keys_without_rows(d, TEXTURE_FORMAT_QUERY_KEYS).is_empty());
            count += 1;
        }
    }
    assert_eq!(count, 12);
}

#[test]
fn stub_returns_row_values_for_query_keys() {
    let d = stub();
    for key in TEXTURE_FORMAT_QUERY_KEYS {
        assert_eq!(d.query(*key), CapabilityLevel::Unsupported);
        let q = d.query_ex(*key);
        assert_eq!(q, ROW);
        assert_ne!(q, UNLISTED_FEATURE_QUERY);
    }
}

#[test]
fn astc_row_key_returns_unlisted_value() {
    let d = stub();
    assert!(!TEXTURE_FORMAT_QUERY_KEYS.contains(&ASTC));
    assert_eq!(d.query_ex(ASTC), UNLISTED_FEATURE_QUERY);
    assert_eq!(d.query(ASTC), CapabilityLevel::Unsupported);
}

#[test]
fn failed_acquisition_s0_differs_from_stub_unsupported() {
    let faulty = FaultInjecting::new(stub(), FaultScript::CapabilityQueryFails);
    let plain = stub();
    for key in TEXTURE_FORMAT_QUERY_KEYS {
        assert_eq!(faulty.query(*key), CapabilityLevel::S0);
        let q = faulty.query_ex(*key);
        assert_eq!(q.level, CapabilityLevel::S0);
        assert_eq!(q.unsupported_reason, None);
        assert_eq!(q.evidence, None);
        assert_ne!(faulty.query(*key), plain.query(*key));
    }
    assert_eq!(
        check_texture_format_queries(&faulty, TEXTURE_FORMAT_QUERY_KEYS),
        Ok(())
    );
}

#[test]
fn checks_accept_key_subsets() {
    let subset = &TEXTURE_FORMAT_QUERY_KEYS[..15];
    let mut t = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
    for key in subset {
        t = t.with(*key, ROW);
    }
    assert_eq!(check_texture_format_queries(&t, subset), Ok(()));
    assert!(texture_format_keys_without_rows(&t, subset).is_empty());
    let rest = texture_format_keys_without_rows(&t, TEXTURE_FORMAT_QUERY_KEYS);
    assert_eq!(rest, TEXTURE_FORMAT_QUERY_KEYS[15..].to_vec());
    assert_eq!(rest.len(), TEXTURE_FORMAT_QUERY_KEYS.len() - 15);
    assert_eq!(
        check_texture_format_queries(&t, TEXTURE_FORMAT_QUERY_KEYS),
        Ok(())
    );

    let device =
        StubDeviceProvider::request(request(BackendKind::Vulkan, Profile::Graphics)).unwrap();
    let d: &dyn Device = &*device;
    assert_eq!(check_texture_format_queries(d, subset), Ok(()));
}

#[test]
fn broken_implementations_are_reported() {
    let k = |i: usize| TEXTURE_FORMAT_QUERY_KEYS[i];
    let missing_reason = CapabilityQuery {
        unsupported_reason: None,
        ..ROW
    };
    let unexpected_reason = CapabilityQuery {
        level: CapabilityLevel::S1,
        ..ROW
    };
    let missing_evidence = CapabilityQuery {
        level: CapabilityLevel::S3,
        unsupported_reason: None,
        evidence: None,
    };

    // (a) 理由の無い Unsupported
    let t = passing_table().with(k(0), missing_reason);
    let err = check_texture_format_queries(&t, TEXTURE_FORMAT_QUERY_KEYS).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        TextureFormatQueryViolation::Rule(CapabilityRuleViolation::MissingUnsupportedReason {
            feature,
            ..
        }) if feature == k(0)
    ));

    // (b) 理由のある S1
    let t = passing_table().with(k(1), unexpected_reason);
    let err = check_texture_format_queries(&t, TEXTURE_FORMAT_QUERY_KEYS).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        TextureFormatQueryViolation::Rule(CapabilityRuleViolation::UnexpectedUnsupportedReason {
            feature,
            ..
        }) if feature == k(1)
    ));

    // (c) 根拠の無い S3
    let t = passing_table().with(k(2), missing_evidence);
    let err = check_texture_format_queries(&t, TEXTURE_FORMAT_QUERY_KEYS).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        TextureFormatQueryViolation::Rule(CapabilityRuleViolation::MissingEvidence {
            feature,
            ..
        }) if feature == k(2)
    ));

    // (d) 段階の食い違い
    let err = check_texture_format_queries(&LevelSplit, &[k(0)]).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        TextureFormatQueryViolation::LevelMismatch {
            feature,
            level: CapabilityLevel::S1,
            ..
        } if feature == k(0)
    ));

    // (e) 行の無い鍵に行の値
    let t = passing_table().with(ASTC, ROW);
    let err = check_rowless_texture_format_keys(&t).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(matches!(
        err[0],
        TextureFormatQueryViolation::RowlessKeyMismatch { feature, .. } if feature == ASTC
    ));

    // (f) 1 件の行を外す
    let mut t = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
    for key in TEXTURE_FORMAT_QUERY_KEYS.iter().filter(|key| **key != k(3)) {
        t = t.with(*key, ROW);
    }
    assert_eq!(
        texture_format_keys_without_rows(&t, TEXTURE_FORMAT_QUERY_KEYS),
        vec![k(3)]
    );

    // (g) 複数の違反が鍵の順に全部集まる
    let t = passing_table()
        .with(k(0), missing_reason)
        .with(k(1), unexpected_reason)
        .with(k(2), missing_evidence);
    let err = check_texture_format_queries(&t, TEXTURE_FORMAT_QUERY_KEYS).unwrap_err();
    assert_eq!(err.len(), 3);
    assert!(matches!(
        err[0],
        TextureFormatQueryViolation::Rule(CapabilityRuleViolation::MissingUnsupportedReason { .. })
    ));
    assert!(matches!(
        err[1],
        TextureFormatQueryViolation::Rule(
            CapabilityRuleViolation::UnexpectedUnsupportedReason { .. }
        )
    ));
    assert!(matches!(
        err[2],
        TextureFormatQueryViolation::Rule(CapabilityRuleViolation::MissingEvidence { .. })
    ));
}

#[test]
#[should_panic(expected = "query panicked on purpose")]
fn panic_in_query_reaches_caller() {
    let device = PanicsOn(TEXTURE_FORMAT_QUERY_KEYS[5]);
    let _ = check_texture_format_queries(&device, TEXTURE_FORMAT_QUERY_KEYS);
}

fn setting(configured: bool) -> RunSetting {
    RunSetting::from_lookup(move |n| {
        (configured && n == COMBINATION_ENV)
            .then(|| OsString::from(MachineCombination::MetalMacMini.name()))
    })
}

/// 3 つの関数をかけて、全部通れば `Match`、外れれば `Mismatch` を返す。
fn verdict_for<D: Capabilities + ?Sized>(device: &D) -> RunResult {
    let ok = check_texture_format_queries(device, TEXTURE_FORMAT_QUERY_KEYS).is_ok()
        && check_rowless_texture_format_keys(device).is_ok()
        && texture_format_keys_without_rows(device, TEXTURE_FORMAT_QUERY_KEYS).is_empty();
    RunResult::Verdict(if ok {
        Verdict::Match
    } else {
        Verdict::Mismatch
    })
}

#[test]
fn checks_compose_with_test_frame_judge() {
    let subject = Subject::from_parts("texture_format_query_contract", &["COVT-1"], &[]).unwrap();
    let declared = Declaration::from_parts(&[MachineCombination::MetalMacMini], &[]).unwrap();
    let calls = Cell::new(0u32);

    // 本体の device は試験用の device で代える。
    let o = judge(&subject, &declared, &setting(true), |_| {
        calls.set(calls.get() + 1);
        verdict_for(&stub())
    });
    assert_eq!(*o.label(), Label::Ok);
    assert_eq!(calls.get(), 1);

    let broken = passing_table().with(
        TEXTURE_FORMAT_QUERY_KEYS[0],
        CapabilityQuery {
            unsupported_reason: None,
            ..ROW
        },
    );
    let o = judge(&subject, &declared, &setting(true), |_| {
        calls.set(calls.get() + 1);
        verdict_for(&broken)
    });
    assert_eq!(*o.label(), Label::Ng);
    assert_eq!(calls.get(), 2);

    let o = judge(&subject, &declared, &setting(false), |_| {
        calls.set(calls.get() + 1);
        verdict_for(&stub())
    });
    assert_eq!(*o.label(), Label::NotRun(NotRunReason::NotConfigured));
    assert_eq!(calls.get(), 2);
}
