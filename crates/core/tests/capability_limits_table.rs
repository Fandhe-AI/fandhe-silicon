//! `LimitsTable` の公開フィールドの集合と型、値としての振る舞い、`Capabilities::limits()` の返し方を
//! 別の crate から固定する公開テスト（TASK-157。DEV-110・DEV-114・DEV-115 のポインタのみ）。
//!
//! 仕組み: 分解は `..` なしで束縛し、フィールドが増えるとコンパイルが落ちる。型注釈で各フィールドの型を固定する。

use fandhe_silicon_core::capability::{Capabilities, CapabilityLevel, FeatureId, LimitsTable};

#[test]
fn limits_table_has_exactly_three_fields_with_fixed_types() {
    let t = LimitsTable {
        max_buffer_bytes: 11,
        max_dispatch_dims: [2, 3, 4],
        max_texture_size: 5,
    };
    let LimitsTable {
        max_buffer_bytes,
        max_dispatch_dims,
        max_texture_size,
    } = t;
    let _: u64 = max_buffer_bytes;
    let _: [u32; 3] = max_dispatch_dims;
    let _: u32 = max_texture_size;
    assert_eq!(max_buffer_bytes, 11);
    assert_eq!(max_dispatch_dims, [2, 3, 4]);
    assert_eq!(max_texture_size, 5);
}

#[test]
fn limits_table_is_copy_eq_and_debug() {
    let a = LimitsTable {
        max_buffer_bytes: 1,
        max_dispatch_dims: [1, 2, 3],
        max_texture_size: 4,
    };
    let b = a;
    assert_eq!(a, b);
    let c = LimitsTable {
        max_texture_size: 5,
        ..a
    };
    assert_ne!(a, c);
    let d = LimitsTable {
        max_dispatch_dims: [1, 2, 4],
        ..a
    };
    assert_ne!(a, d);
    let e = LimitsTable {
        max_buffer_bytes: 2,
        ..a
    };
    assert_ne!(a, e);
    let text = format!("{a:?}");
    assert!(text.contains("LimitsTable"), "{text}");
    assert!(text.contains("max_buffer_bytes"), "{text}");
}

/// 試験用の実装 1。実装 2 とは別の表を返す。
struct AdapterLike(LimitsTable);
/// 試験用の実装 2。
struct DeviceLike(LimitsTable);

impl Capabilities for AdapterLike {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S2
    }

    fn limits(&self) -> &LimitsTable {
        &self.0
    }
}

impl Capabilities for DeviceLike {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S2
    }

    fn limits(&self) -> &LimitsTable {
        &self.0
    }
}

const ADAPTER_TABLE: LimitsTable = LimitsTable {
    max_buffer_bytes: 100,
    max_dispatch_dims: [10, 20, 30],
    max_texture_size: 40,
};
const DEVICE_TABLE: LimitsTable = LimitsTable {
    max_buffer_bytes: 50,
    max_dispatch_dims: [5, 6, 7],
    max_texture_size: 8,
};

#[test]
fn two_implementations_each_return_their_own_table() {
    assert_eq!(*AdapterLike(ADAPTER_TABLE).limits(), ADAPTER_TABLE);
    assert_eq!(*DeviceLike(DEVICE_TABLE).limits(), DEVICE_TABLE);
}

#[test]
fn limits_is_callable_through_dyn_capabilities() {
    let d = DeviceLike(DEVICE_TABLE);
    let r: &dyn Capabilities = &d;
    assert_eq!(*r.limits(), DEVICE_TABLE);

    let b: Box<dyn Capabilities + Send + Sync> = Box::new(AdapterLike(ADAPTER_TABLE));
    assert_eq!(*b.limits(), ADAPTER_TABLE);
}
