//! 間接実行の引数の 3 つの型について、大きさ・整列・欄ごとのバイトの並びと往復を公開のパスから確かめる（TASK-1878 のポインタ）。
//!
//! 期待のバイト列は試験の中に手で書いた配列で持ち、実装の変換を写さない。

use fandhe_silicon_core::execution::{
    DispatchIndirectArgs, DrawIndexedIndirectArgs, DrawIndirectArgs,
};

const U32_EDGES: [u32; 3] = [0, 1, u32::MAX];
const I32_EDGES: [i32; 4] = [i32::MIN, -1, 0, i32::MAX];

/// `size_of` が `SIZE` と一致し、値も 16・20・12 であること。
#[test]
fn sizes_match_size_constants() {
    assert_eq!(size_of::<DrawIndirectArgs>() as u64, DrawIndirectArgs::SIZE);
    assert_eq!(DrawIndirectArgs::SIZE, 16);
    assert_eq!(
        size_of::<DrawIndexedIndirectArgs>() as u64,
        DrawIndexedIndirectArgs::SIZE
    );
    assert_eq!(DrawIndexedIndirectArgs::SIZE, 20);
    assert_eq!(
        size_of::<DispatchIndirectArgs>() as u64,
        DispatchIndirectArgs::SIZE
    );
    assert_eq!(DispatchIndirectArgs::SIZE, 12);
}

/// 3 型とも整列が 4 であること。
#[test]
fn alignment_is_four() {
    assert_eq!(align_of::<DrawIndirectArgs>(), 4);
    assert_eq!(align_of::<DrawIndexedIndirectArgs>(), 4);
    assert_eq!(align_of::<DispatchIndirectArgs>(), 4);
}

const DRAW_VALUE: DrawIndirectArgs = DrawIndirectArgs {
    vertex_count: 0x0403_0201,
    instance_count: 0x0807_0605,
    first_vertex: 0x0C0B_0A09,
    first_instance: 0x100F_0E0D,
};
const DRAW_BYTES: [u8; 16] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
];

const INDEXED_VALUE: DrawIndexedIndirectArgs = DrawIndexedIndirectArgs {
    index_count: 0x0403_0201,
    instance_count: 0x0807_0605,
    first_index: 0x0C0B_0A09,
    base_vertex: 0x100F_0E0D,
    first_instance: 0x1413_1211,
};
const INDEXED_BYTES: [u8; 20] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
    0x11, 0x12, 0x13, 0x14,
];

const DISPATCH_VALUE: DispatchIndirectArgs = DispatchIndirectArgs {
    x: 0x0403_0201,
    y: 0x0807_0605,
    z: 0x0C0B_0A09,
};
const DISPATCH_BYTES: [u8; 12] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
];

/// Draw の欄が宣言順・リトルエンディアンで並ぶこと。
#[test]
fn draw_bytes_follow_field_order() {
    assert_eq!(DRAW_VALUE.to_bytes(), DRAW_BYTES);
}

/// DrawIndexed の欄が宣言順・リトルエンディアンで並ぶこと。
#[test]
fn draw_indexed_bytes_follow_field_order() {
    assert_eq!(INDEXED_VALUE.to_bytes(), INDEXED_BYTES);
}

/// Dispatch の欄が宣言順・リトルエンディアンで並ぶこと。
#[test]
fn dispatch_bytes_follow_field_order() {
    assert_eq!(DISPATCH_VALUE.to_bytes(), DISPATCH_BYTES);
}

/// `base_vertex` が 2 の補数のリトルエンディアンで 12..16 バイト目に入り、ほかの欄は動かないこと。
#[test]
fn base_vertex_is_twos_complement_le() {
    let cases: [(i32, [u8; 4]); 3] = [
        (-1, [0xFF, 0xFF, 0xFF, 0xFF]),
        (i32::MIN, [0x00, 0x00, 0x00, 0x80]),
        (i32::MAX, [0xFF, 0xFF, 0xFF, 0x7F]),
    ];
    for (base_vertex, expected) in cases {
        let args = DrawIndexedIndirectArgs {
            base_vertex,
            ..INDEXED_VALUE
        };
        let bytes = args.to_bytes();
        assert_eq!(bytes[12..16], expected);
        assert_eq!(bytes[..12], INDEXED_BYTES[..12]);
        assert_eq!(bytes[16..], INDEXED_BYTES[16..]);
    }
}

/// 境界値の直積で `to_bytes` と `from_bytes` が往復すること。
#[test]
fn round_trip_over_boundary_values() {
    let mut draw = 0;
    for a in U32_EDGES {
        for b in U32_EDGES {
            for c in U32_EDGES {
                for d in U32_EDGES {
                    let x = DrawIndirectArgs {
                        vertex_count: a,
                        instance_count: b,
                        first_vertex: c,
                        first_instance: d,
                    };
                    assert_eq!(DrawIndirectArgs::from_bytes(&x.to_bytes()), x);
                    draw += 1;
                    for v in I32_EDGES {
                        let y = DrawIndexedIndirectArgs {
                            index_count: a,
                            instance_count: b,
                            first_index: c,
                            base_vertex: v,
                            first_instance: d,
                        };
                        assert_eq!(DrawIndexedIndirectArgs::from_bytes(&y.to_bytes()), y);
                    }
                }
            }
            for c in U32_EDGES {
                let z = DispatchIndirectArgs { x: a, y: b, z: c };
                assert_eq!(DispatchIndirectArgs::from_bytes(&z.to_bytes()), z);
            }
        }
    }
    assert_eq!(draw, 81);
}

/// 手で書いた配列から値を作り、再び配列に戻しても同じであること。
#[test]
fn bytes_round_trip_from_expected_arrays() {
    assert_eq!(DrawIndirectArgs::from_bytes(&DRAW_BYTES), DRAW_VALUE);
    assert_eq!(
        DrawIndirectArgs::from_bytes(&DRAW_BYTES).to_bytes(),
        DRAW_BYTES
    );
    assert_eq!(
        DrawIndexedIndirectArgs::from_bytes(&INDEXED_BYTES),
        INDEXED_VALUE
    );
    assert_eq!(
        DrawIndexedIndirectArgs::from_bytes(&INDEXED_BYTES).to_bytes(),
        INDEXED_BYTES
    );
    assert_eq!(
        DispatchIndirectArgs::from_bytes(&DISPATCH_BYTES),
        DISPATCH_VALUE
    );
    assert_eq!(
        DispatchIndirectArgs::from_bytes(&DISPATCH_BYTES).to_bytes(),
        DISPATCH_BYTES
    );
}

/// 既定値の全欄が 0 で、バイト列も全部 0 であること。
#[test]
fn default_is_all_zero() {
    assert_eq!(
        DrawIndirectArgs::default(),
        DrawIndirectArgs {
            vertex_count: 0,
            instance_count: 0,
            first_vertex: 0,
            first_instance: 0,
        }
    );
    assert_eq!(DrawIndirectArgs::default().to_bytes(), [0u8; 16]);
    assert_eq!(
        DrawIndexedIndirectArgs::default(),
        DrawIndexedIndirectArgs {
            index_count: 0,
            instance_count: 0,
            first_index: 0,
            base_vertex: 0,
            first_instance: 0,
        }
    );
    assert_eq!(DrawIndexedIndirectArgs::default().to_bytes(), [0u8; 20]);
    assert_eq!(
        DispatchIndirectArgs::default(),
        DispatchIndirectArgs { x: 0, y: 0, z: 0 }
    );
    assert_eq!(DispatchIndirectArgs::default().to_bytes(), [0u8; 12]);
}
