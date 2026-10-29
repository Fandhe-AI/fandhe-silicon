//! デコレータの上限の差し替えを行ごとに確かめる試験（spec TASK-2935）。
//!
//! `FaultInjecting::with_limits_override` が `Capabilities::limits()` に効く行は `LimitsTable` の 3 項目だけで、
//! 表の外の行の型は `limits()` では差し替えられない。
//! 値は試験用の小さな数で、実際の確保や機材は使わない。

use fandhe_silicon_core::capability::{Capabilities, CapabilityLevel, FeatureId, LimitsTable};
use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};
use fandhe_silicon_core::memory::validate_allocation_size;

/// 大きな値の表を持つ偽の内側。差し替えが効いたかを値の違いで見分ける。
struct Inner(LimitsTable);

impl Capabilities for Inner {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S2
    }

    fn limits(&self) -> &LimitsTable {
        &self.0
    }
}

const BIG: LimitsTable = LimitsTable {
    max_buffer_bytes: 1 << 40,
    max_dispatch_dims: [65_535, 65_535, 65_535],
    max_texture_size: 16_384,
};

fn decorated(table: LimitsTable) -> FaultInjecting<Inner> {
    FaultInjecting::new(Inner(BIG), FaultScript::PassThrough).with_limits_override(table)
}

/// `max_buffer_bytes` を小さくすると、上限を 1 だけ超える大きさの入力が確保の検査で拒否される。
/// 内側の大きな上限なら同じ大きさが通る。確保は行わず、数だけで作れる。
#[test]
fn small_max_buffer_bytes_makes_over_limit_input_without_allocating() {
    let small = LimitsTable {
        max_buffer_bytes: 16,
        ..BIG
    };
    let d = decorated(small);
    let limit = d.limits().max_buffer_bytes;
    assert_eq!(limit, 16);
    assert!(validate_allocation_size(limit, limit).is_ok());
    assert!(validate_allocation_size(limit + 1, limit).is_err());
    // 差し替えなしの内側の上限では、同じ大きさが通る。
    assert!(validate_allocation_size(limit + 1, Inner(BIG).limits().max_buffer_bytes).is_ok());
}

/// `LimitsTable` の 3 項目は、1 項目ずつ差し替えられ、ほかの項目は置き換えた表の値になる。
#[test]
fn each_of_the_three_rows_is_replaceable_through_limits() {
    let rows: [(&str, LimitsTable); 3] = [
        (
            "max_buffer_bytes",
            LimitsTable {
                max_buffer_bytes: 1,
                ..BIG
            },
        ),
        (
            "max_dispatch_dims",
            LimitsTable {
                max_dispatch_dims: [1, 2, 3],
                ..BIG
            },
        ),
        (
            "max_texture_size",
            LimitsTable {
                max_texture_size: 1,
                ..BIG
            },
        ),
    ];
    for (name, table) in rows {
        let d = decorated(table);
        assert_eq!(*d.limits(), table, "{name}");
        assert_ne!(*d.limits(), BIG, "{name}");
        let l = d.limits();
        match name {
            "max_buffer_bytes" => assert_eq!(l.max_buffer_bytes, 1),
            "max_dispatch_dims" => assert_eq!(l.max_dispatch_dims, [1, 2, 3]),
            _ => assert_eq!(l.max_texture_size, 1),
        }
    }
}

/// 試験の中で定義した任意の行の型。`LimitsTable` の外の型なので、表としては取り出せない。
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowOverride {
    value: u32,
}

/// `LimitsTable` の外の行の型で置き換えても `limits()` は内側の表のまま変わらず（差し替えられない）、
/// `limits_with::<LimitsTable>` は `Err` になる。行の型では `limits_with` から取り出せる。
#[test]
fn row_type_outside_the_table_is_not_replaceable_through_limits() {
    let d = FaultInjecting::new(Inner(BIG), FaultScript::PassThrough)
        .with_limits_override(RowOverride { value: 1 });
    assert_eq!(*d.limits(), BIG);
    assert!(d.limits_with::<LimitsTable, _>(|i| *i.limits()).is_err());
    assert_eq!(
        d.limits_with::<RowOverride, _>(|_| unreachable!("inner is not called")),
        Ok(RowOverride { value: 1 })
    );
}

/// 表の項目の数は 3 のまま。表に行を足す場所がないことを分解で固定する
/// （項目が増えるとこの試験がコンパイルで落ち、行の一覧と照らし直す）。
#[test]
fn table_has_only_the_three_rows() {
    let LimitsTable {
        max_buffer_bytes: _,
        max_dispatch_dims: _,
        max_texture_size: _,
    } = BIG;
}
