//! デコレータの上限の差し替えの口の確認（spec TASK-271 の 6/8 の (8)。上限の型は本ファイルの中だけの偽物）。

use std::cell::Cell;

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, FeatureId, LimitsTable,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeLimits {
    max_buffer_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OtherLimits;

#[derive(Default)]
struct Fake {
    calls: Cell<u32>,
}

fn real(f: &Fake) -> FakeLimits {
    f.calls.set(f.calls.get() + 1);
    FakeLimits {
        max_buffer_bytes: 1 << 30,
    }
}

#[test]
fn without_override_calls_inner_once() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough);
    assert_eq!(
        d.limits_with(real),
        Ok(FakeLimits {
            max_buffer_bytes: 1 << 30
        })
    );
    assert_eq!(d.inner().calls.get(), 1);
}

#[test]
fn override_replaces_without_calling_inner() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough).with_limits_override(
        FakeLimits {
            max_buffer_bytes: 16,
        },
    );
    for _ in 0..3 {
        assert_eq!(
            d.limits_with(real),
            Ok(FakeLimits {
                max_buffer_bytes: 16
            })
        );
    }
    assert_eq!(d.inner().calls.get(), 0);
}

#[test]
fn override_of_other_type_fails_closed() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::PassThrough)
        .with_limits_override(OtherLimits);
    let r = d.limits_with(real);
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
    assert_eq!(d.inner().calls.get(), 0);
}

#[test]
fn override_is_independent_of_script() {
    let d = FaultInjecting::new(Fake::default(), FaultScript::AllocationFails)
        .with_limits_override(FakeLimits {
            max_buffer_bytes: 8,
        });
    assert_eq!(
        d.limits_with(real),
        Ok(FakeLimits {
            max_buffer_bytes: 8
        })
    );
    let r: Result<u32, _> = d.allocate_with(5, |_, _| Ok(1));
    assert_eq!(
        r,
        Err(ContractError::AllocationFailed { requested_bytes: 5 })
    );
}

#[test]
fn decorator_stays_send_sync_and_debug() {
    fn check<T: Send + Sync + std::fmt::Debug>() {}
    check::<FaultInjecting<u8>>();
}

/// `Capabilities::limits()` を持つ偽の内側。表は試験用の小さな値。
struct LimitsInner(LimitsTable);

impl Capabilities for LimitsInner {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S2
    }

    fn limits(&self) -> &LimitsTable {
        &self.0
    }
}

const INNER_TABLE: LimitsTable = LimitsTable {
    max_buffer_bytes: 3,
    max_dispatch_dims: [3, 3, 3],
    max_texture_size: 3,
};

const OVERRIDE_TABLE: LimitsTable = LimitsTable {
    max_buffer_bytes: 7,
    max_dispatch_dims: [7, 8, 9],
    max_texture_size: 7,
};

#[test]
fn limits_without_override_is_inner_table() {
    let d = FaultInjecting::new(LimitsInner(INNER_TABLE), FaultScript::PassThrough);
    assert_eq!(*d.limits(), INNER_TABLE);
}

#[test]
fn limits_table_override_applies_to_limits_for_every_script() {
    // `FaultScript` の全 6 種類を並べる（バリアントを足したらここも足す）。
    let scripts = [
        FaultScript::PassThrough,
        FaultScript::CompletionUnknown,
        FaultScript::CapabilityInsufficient {
            feature: FeatureId("feature:X"),
            required: CapabilityLevel::S2,
        },
        FaultScript::CapabilityQueryFails,
        FaultScript::AllocationFails,
        FaultScript::RuntimeError {
            backend: BackendKind::Vulkan,
            detail: "x".into(),
        },
    ];
    for script in scripts {
        let d = FaultInjecting::new(LimitsInner(INNER_TABLE), script)
            .with_limits_override(OVERRIDE_TABLE);
        assert_eq!(*d.limits(), OVERRIDE_TABLE);
        assert_eq!(
            d.limits_with::<LimitsTable, _>(|i| *i.limits()),
            Ok(OVERRIDE_TABLE)
        );
    }
}

#[test]
fn other_type_override_leaves_limits_on_inner_table() {
    let d = FaultInjecting::new(LimitsInner(INNER_TABLE), FaultScript::PassThrough)
        .with_limits_override(OtherLimits);
    assert_eq!(*d.limits(), INNER_TABLE);
    let r = d.limits_with::<LimitsTable, _>(|i| *i.limits());
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
}
