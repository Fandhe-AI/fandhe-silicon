//! 間接実行の引数の 3 つの型について、公開のパスから大きさとバイト列の往復を確かめる最小の試験（TASK-1878 のポインタ）。
//!
//! 欄ごとの配置の本格的な試験は別のテストが受け持つ。

use fandhe_silicon_core::execution::{
    DispatchIndirectArgs, DrawIndexedIndirectArgs, DrawIndirectArgs,
};

#[test]
fn draw_indirect_args_size_matches_layout() {
    assert_eq!(size_of::<DrawIndirectArgs>() as u64, DrawIndirectArgs::SIZE);
    assert_eq!(DrawIndirectArgs::SIZE, 16);
}

#[test]
fn draw_indexed_indirect_args_size_matches_layout() {
    assert_eq!(
        size_of::<DrawIndexedIndirectArgs>() as u64,
        DrawIndexedIndirectArgs::SIZE
    );
    assert_eq!(DrawIndexedIndirectArgs::SIZE, 20);
}

#[test]
fn dispatch_indirect_args_size_matches_layout() {
    assert_eq!(
        size_of::<DispatchIndirectArgs>() as u64,
        DispatchIndirectArgs::SIZE
    );
    assert_eq!(DispatchIndirectArgs::SIZE, 12);
}

#[test]
fn bytes_round_trip() {
    let a = DrawIndirectArgs {
        vertex_count: 1,
        instance_count: 2,
        first_vertex: 3,
        first_instance: 4,
    };
    assert_eq!(DrawIndirectArgs::from_bytes(&a.to_bytes()), a);
    let b = DrawIndexedIndirectArgs {
        index_count: 5,
        instance_count: 6,
        first_index: 7,
        base_vertex: -8,
        first_instance: 9,
    };
    assert_eq!(DrawIndexedIndirectArgs::from_bytes(&b.to_bytes()), b);
    let c = DispatchIndirectArgs {
        x: 10,
        y: 11,
        z: 12,
    };
    assert_eq!(DispatchIndirectArgs::from_bytes(&c.to_bytes()), c);
}
