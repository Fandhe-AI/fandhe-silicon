//! 形式ごとの能力の値を、試験用の device・故障の差し込みの包み・検査の関数に流す契約テスト（TASK-2566）。
//!
//! 公開のパスだけを使う。形式単位の能力の機能の鍵が非対応の device でも、全形式で値が返ることを確かめる。

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
    LimitsTable,
};
use fandhe_silicon_core::capability_rule::CapabilityTable;
use fandhe_silicon_core::capability_table::UNLISTED_FEATURE_QUERY;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::UnsupportedReason;
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::feature_key::FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
use fandhe_silicon_core::stub_device::{StubDevice, StubDeviceProvider};
use fandhe_silicon_core::texture_format::TextureFormat;
use fandhe_silicon_core::texture_format_features::{TextureFormatFeature, TextureFormatFeatures};
use fandhe_silicon_core::texture_format_key::{TEXTURE_FORMAT_QUERY_KEYS, TEXTURE_FORMAT_ROWS};
use fandhe_silicon_core::texture_format_query_check::{
    TextureFormatQueryViolation, check_texture_format_features,
};

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

const S0_PLAIN: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::S0,
    unsupported_reason: None,
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

fn pairs() -> Vec<(FeatureId, TextureFormat)> {
    TEXTURE_FORMAT_ROWS
        .iter()
        .flat_map(|row| row.keys.iter().map(|k| (k.key, k.format)))
        .collect()
}

/// 全形式の鍵に `value` を持たせた表。ほかの鍵には `UNLISTED_FEATURE_QUERY` を返す。
fn table(value: CapabilityQuery) -> CapabilityTable {
    let mut t = CapabilityTable::new().with_default(UNLISTED_FEATURE_QUERY);
    for key in TEXTURE_FORMAT_QUERY_KEYS {
        t = t.with(*key, value);
    }
    t
}

/// `broken` の形式だけ `capture_failed()` を返す、表を包んだ試験の中だけの実装。
struct Broken {
    inner: CapabilityTable,
    broken: TextureFormat,
}

impl Capabilities for Broken {
    fn query(&self, feature: FeatureId) -> CapabilityLevel {
        self.inner.query(feature)
    }

    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }

    fn query_ex(&self, feature: FeatureId) -> CapabilityQuery {
        self.inner.query_ex(feature)
    }

    fn texture_format_features(&self, format: TextureFormat) -> TextureFormatFeatures {
        if format == self.broken {
            TextureFormatFeatures::capture_failed()
        } else {
            self.inner.texture_format_features(format)
        }
    }
}

/// 全形式で形式の単位が `unit`、18 要素が `elements`、用途が空、サンプル数が `[1]` であること。
fn assert_all_formats(d: &dyn Capabilities, unit: CapabilityQuery, elements: CapabilityQuery) {
    let mut count = 0;
    for (_, format) in pairs() {
        let v = d.texture_format_features(format);
        assert_eq!(v.format_query(), unit, "{format:?}");
        for f in TextureFormatFeature::ALL {
            assert_eq!(v.feature(*f), elements, "{format:?} {f:?}");
        }
        assert!(v.allowed_usages().is_empty());
        assert_eq!(v.supported_sample_counts(), vec![1]);
        count += 1;
    }
    assert_eq!(count, 118);
}

#[test]
fn stub_device_returns_value_for_every_key() {
    let mut combos = 0;
    for backend in backends() {
        for profile in profiles() {
            let device = StubDeviceProvider::request(request(backend, profile)).unwrap();
            let d: &dyn Device = &*device;
            assert_eq!(
                check_texture_format_features(d, TEXTURE_FORMAT_ROWS),
                Ok(())
            );
            assert_all_formats(d, UNS_NS, UNS_NS);
            // 形式単位の能力の機能の鍵は行の無い値で、それでも全形式で値が返る。
            let row = d.query_ex(FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
            assert_eq!(row, UNLISTED_FEATURE_QUERY);
            assert_eq!(row.level, CapabilityLevel::Unsupported);
            combos += 1;
        }
    }
    assert_eq!(combos, 12);
}

#[test]
fn checker_reports_mismatch() {
    let target = TextureFormat::Rgba8Unorm;
    let d = Broken {
        inner: table(UNS_NS),
        broken: target,
    };
    let err = check_texture_format_features(&d, TEXTURE_FORMAT_ROWS).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(matches!(
        &err[0],
        TextureFormatQueryViolation::FeaturesFormatQueryMismatch { format, .. } if *format == target
    ));
}

#[test]
fn decorator_keeps_format_query_consistent() {
    // 何も差し込まない包みは、包まない値と同じ。
    let pass = FaultInjecting::new(stub(), FaultScript::PassThrough);
    assert_eq!(
        check_texture_format_features(&pass, TEXTURE_FORMAT_ROWS),
        Ok(())
    );
    let plain = stub();
    for (_, format) in pairs() {
        assert_eq!(
            pass.texture_format_features(format),
            plain.texture_format_features(format)
        );
    }

    // 能力の取得の失敗では、全形式で形式の単位も要素も S0。
    let fails = FaultInjecting::new(stub(), FaultScript::CapabilityQueryFails);
    assert_eq!(
        check_texture_format_features(&fails, TEXTURE_FORMAT_ROWS),
        Ok(())
    );
    assert_all_formats(&fails, S0_PLAIN, S0_PLAIN);

    // 指定した形式の鍵だけが非対応になる。試験用の device は全形式が非対応なので、
    // 全形式が対応の表も包んで区別する。
    let (target_key, target_format) = pairs()[0];
    let script = FaultScript::CapabilityInsufficient {
        feature: target_key,
        required: CapabilityLevel::S2,
    };
    let on_stub = FaultInjecting::new(stub(), script.clone());
    assert_eq!(
        check_texture_format_features(&on_stub, TEXTURE_FORMAT_ROWS),
        Ok(())
    );
    assert_eq!(
        on_stub
            .texture_format_features(target_format)
            .format_query(),
        UNS_NS
    );

    let on_table = FaultInjecting::new(table(S2_RT), script);
    assert_eq!(
        check_texture_format_features(&on_table, TEXTURE_FORMAT_ROWS),
        Ok(())
    );
    let mut others = 0;
    for (key, format) in pairs() {
        let want = if key == target_key { UNS_NS } else { S2_RT };
        assert_eq!(
            on_table.texture_format_features(format).format_query(),
            want
        );
        if key != target_key {
            others += 1;
        }
    }
    assert_eq!(others, 117);
}
