//! 問合せの結果の配置（フラグの組の数・stride・位置）と問合せの組の個数の上限・定数を公開のパスから確かめる（TASK-1877・CMD-127〜CMD-130 のポインタ）。
//!
//! 期待は試験の中で独立に数え、実装の計算を写さない。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    PipelineStatisticsFlags, QUERY_RESOLVE_BUFFER_ALIGNMENT, QUERY_SET_MAX_QUERIES, QUERY_SIZE,
    validate_query_resolve, validate_query_set_count,
};

/// 宣言の順の 5 つのフラグ。
const ALL: [PipelineStatisticsFlags; 5] = [
    PipelineStatisticsFlags::VERTEX_SHADER_INVOCATIONS,
    PipelineStatisticsFlags::CLIPPER_INVOCATIONS,
    PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT,
    PipelineStatisticsFlags::FRAGMENT_SHADER_INVOCATIONS,
    PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS,
];

/// 空でない 31 通りの組すべてで、数・stride・位置が独立に数えた値と一致すること（CMD-127）。
#[test]
fn layout_all_31_combinations() {
    for mask in 1u8..32 {
        let mut flags = PipelineStatisticsFlags::empty();
        for (j, f) in ALL.iter().enumerate() {
            if mask & (1 << j) != 0 {
                flags |= *f;
            }
        }
        let n = mask.count_ones();
        assert_eq!(flags.count(), n, "mask {mask}");
        assert_eq!(flags.stride(), u64::from(n) * 8, "mask {mask}");
        let mut seen = Vec::new();
        for (j, f) in ALL.iter().enumerate() {
            if mask & (1 << j) != 0 {
                let expected = (0..j).filter(|k| mask & (1 << k) != 0).count();
                let pos = flags.position(*f);
                assert_eq!(pos, Some(expected as u32), "mask {mask} flag {j}");
                seen.push(pos.unwrap());
            } else {
                assert_eq!(flags.position(*f), None, "mask {mask} flag {j}");
            }
        }
        let want: Vec<u32> = (0..n).collect();
        assert_eq!(seen, want, "mask {mask}");
    }
}

/// 2 個の組の例で stride が 16、位置が 0・1、バイトの位置が 0・8 であること（CMD-127）。
#[test]
fn layout_example_two_flags() {
    let a = PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT;
    let b = PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS;
    let flags = a | b;
    assert_eq!(flags.stride(), 16);
    assert_eq!(flags.position(a), Some(0));
    assert_eq!(flags.position(b), Some(1));
    assert_eq!(u64::from(flags.position(a).unwrap()) * QUERY_SIZE, 0);
    assert_eq!(u64::from(flags.position(b).unwrap()) * QUERY_SIZE, 8);
}

/// i 番目の問合せの先頭が `offset + i × stride` で、最後の終端が書き込みの終端と一致すること（CMD-127）。
#[test]
fn layout_query_i_starts_at_offset_plus_i_stride() {
    let flags = PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT
        | PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS;
    let stride = flags.stride();
    let offset = 256u64;
    assert_eq!(
        validate_query_resolve(4096, stride, 0, 3, 304, offset),
        Ok(48)
    );
    let starts: Vec<u64> = (0..3u64).map(|i| offset + i * stride).collect();
    assert_eq!(starts, [256, 272, 288]);
    assert_eq!(starts[2] + stride, offset + 48);
    assert_eq!(offset + 48, 304);
}

/// 空・2 ビットの和・含まないフラグの `position` が `None` であること。
#[test]
fn position_rejects_non_single_flag() {
    let flags = PipelineStatisticsFlags::CLIPPER_INVOCATIONS
        | PipelineStatisticsFlags::FRAGMENT_SHADER_INVOCATIONS;
    assert_eq!(flags.position(PipelineStatisticsFlags::empty()), None);
    assert_eq!(
        flags.position(
            PipelineStatisticsFlags::CLIPPER_INVOCATIONS
                | PipelineStatisticsFlags::FRAGMENT_SHADER_INVOCATIONS
        ),
        None
    );
    assert_eq!(
        flags.position(PipelineStatisticsFlags::VERTEX_SHADER_INVOCATIONS),
        None
    );
}

/// 結果 1 つの大きさ・個数の上限・解決先の整列の定数の値（CMD-127〜CMD-130）。
#[test]
fn query_size_and_constants() {
    assert_eq!(QUERY_SIZE, 8);
    assert_eq!(QUERY_SET_MAX_QUERIES, 4096);
    assert_eq!(QUERY_RESOLVE_BUFFER_ALIGNMENT, 256);
}

/// 個数は 1〜4096 が受理され、0 と 4096 超が拒否されること（CMD-128）。
#[test]
fn set_count_bounds() {
    for c in 1..=4096u32 {
        assert_eq!(validate_query_set_count(c), Ok(()), "count {c}");
    }
    for c in [0u32, 4097, u32::MAX] {
        assert!(matches!(
            validate_query_set_count(c),
            Err(ContractError::InvalidArgument(_))
        ));
    }
    for c in (4097u32..=u32::MAX).step_by(1_000_003) {
        assert!(
            matches!(
                validate_query_set_count(c),
                Err(ContractError::InvalidArgument(_))
            ),
            "count {c}"
        );
    }
}

/// 空の組の `count`・`stride` が 0 で、その stride は解決で拒否されること。
#[test]
fn empty_flags_have_zero_stride() {
    let e = PipelineStatisticsFlags::empty();
    assert_eq!(e.count(), 0);
    assert_eq!(e.stride(), 0);
    assert!(e.is_empty());
    match validate_query_resolve(4096, e.stride(), 0, 1, 512, 256) {
        Err(ContractError::InvalidArgument(m)) => assert!(m.starts_with("query resolve stride")),
        other => panic!("unexpected: {other:?}"),
    }
}

/// 個数と空の stride の拒否が `InvalidArgument` だけであること。
#[test]
fn rejection_is_invalid_argument_only() {
    for c in [0u32, 4097, u32::MAX] {
        let r = validate_query_set_count(c);
        assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
    }
    let r = validate_query_resolve(
        4096,
        PipelineStatisticsFlags::empty().stride(),
        0,
        1,
        512,
        256,
    );
    assert!(matches!(r, Err(ContractError::InvalidArgument(_))), "{r:?}");
}
