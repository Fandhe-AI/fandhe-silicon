//! feature `graphics` の描画パスの状態の値の型・範囲の型と検査・状態を設定する操作の型を、
//! 公開のパスだけで外の crate から確かめる統合テスト（TASK-838）。
//!
//! 検査の純粋関数は戻り値だけを見る。`InvalidArgument` の中の文字列は比べない。

// 引数と戻り値の型を `fn` ポインタの型として書き下すこと自体が試験の目的なので、複雑さの警告は許す。
#![allow(clippy::type_complexity)]

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::PipelineHandle;
use fandhe_silicon_core::graphics::{
    DrawRange, GraphicsOps, IndexFormat, IndexedDrawRange, RenderEncoder, ScissorRect, Viewport,
    index_format_bytes, validate_draw_range, validate_indexed_draw_range,
};
use fandhe_silicon_core::memory::BufferHandle;

fn is_invalid(r: &Result<(), ContractError>) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(_)))
}

fn dr(first_vertex: u32, vertex_count: u32, first_instance: u32, instance_count: u32) -> DrawRange {
    DrawRange {
        first_vertex,
        vertex_count,
        first_instance,
        instance_count,
    }
}

fn idr(
    first_index: u32,
    index_count: u32,
    base_vertex: i32,
    first_instance: u32,
    instance_count: u32,
) -> IndexedDrawRange {
    IndexedDrawRange {
        first_index,
        index_count,
        base_vertex,
        first_instance,
        instance_count,
    }
}

#[test]
fn op_signatures_have_listed_types() {
    let _: fn(&dyn GraphicsOps, &mut RenderEncoder, &PipelineHandle) -> Result<(), ContractError> =
        |g, e, p| g.set_pipeline(e, p);
    let _: fn(
        &dyn GraphicsOps,
        &mut RenderEncoder,
        u32,
        &BufferHandle,
        u64,
        Option<u64>,
    ) -> Result<(), ContractError> = |g, e, s, b, o, z| g.set_vertex_buffer(e, s, b, o, z);
    let _: fn(
        &dyn GraphicsOps,
        &mut RenderEncoder,
        &BufferHandle,
        IndexFormat,
        u64,
        Option<u64>,
    ) -> Result<(), ContractError> = |g, e, b, f, o, z| g.set_index_buffer(e, b, f, o, z);
    let _: fn(&dyn GraphicsOps, &mut RenderEncoder, u32, &[u8]) -> Result<(), ContractError> =
        |g, e, o, d| g.set_immediates(e, o, d);
    let _: fn(&dyn GraphicsOps, &mut RenderEncoder, Viewport) -> Result<(), ContractError> =
        |g, e, v| g.set_viewport(e, v);
    let _: fn(&dyn GraphicsOps, &mut RenderEncoder, ScissorRect) -> Result<(), ContractError> =
        |g, e, r| g.set_scissor_rect(e, r);
    let _: fn(&dyn GraphicsOps, &mut RenderEncoder, [f32; 4]) -> Result<(), ContractError> =
        |g, e, c| g.set_blend_constant(e, c);
    let _: fn(&dyn GraphicsOps, &mut RenderEncoder, u32) -> Result<(), ContractError> =
        |g, e, r| g.set_stencil_reference(e, r);
}

// REND-57
#[test]
fn viewport_is_built_from_outside() {
    let v = Viewport {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    let a = v;
    let b = v;
    assert_eq!(a, b);
    assert_ne!(
        a,
        Viewport {
            max_depth: 0.5,
            ..v
        }
    );
}

// REND-56
#[test]
fn scissor_rect_is_built_from_outside() {
    let r = ScissorRect {
        x: 1,
        y: 2,
        width: 3,
        height: 4,
    };
    let a = r;
    let b = r;
    assert_eq!(a, b);
    assert_ne!(a, ScissorRect { height: 5, ..r });
}

// REND-63
#[test]
fn index_format_has_default_and_bytes() {
    assert_eq!(IndexFormat::default(), IndexFormat::Uint32);
    assert_eq!(index_format_bytes(IndexFormat::Uint16), 2);
    assert_eq!(index_format_bytes(IndexFormat::Uint32), 4);
    let name = match IndexFormat::default() {
        IndexFormat::Uint16 => "u16",
        IndexFormat::Uint32 => "u32",
        _ => "other",
    };
    assert_eq!(name, "u32");
}

#[test]
fn draw_ranges_are_built_from_outside() {
    // REND-62
    let a = dr(1, 2, 3, 4);
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, dr(1, 2, 3, 5));
    // REND-63
    let c = idr(1, 2, -3, 4, 5);
    let d = c;
    assert_eq!(c, d);
    assert_eq!(c.base_vertex, -3);
    assert_ne!(c, idr(1, 2, 3, 4, 5));
}

// REND-62
#[test]
fn draw_range_sum_overflow_is_rejected() {
    assert_eq!(validate_draw_range(&dr(u32::MAX - 1, 1, 0, 1)), Ok(()));
    assert!(is_invalid(&validate_draw_range(&dr(u32::MAX, 1, 0, 1))));
    assert_eq!(validate_draw_range(&dr(0, 1, u32::MAX - 1, 1)), Ok(()));
    assert!(is_invalid(&validate_draw_range(&dr(0, 1, u32::MAX, 1))));
}

// REND-63
#[test]
fn indexed_draw_range_bounds() {
    // (a) 終わりちょうどは通し、1 超えは拒む。
    let r = idr(0, 4, 0, 0, 1);
    assert_eq!(
        validate_indexed_draw_range(&r, IndexFormat::Uint16, 8, 16),
        Ok(())
    );
    assert!(is_invalid(&validate_indexed_draw_range(
        &r,
        IndexFormat::Uint16,
        8,
        15
    )));
    // (b) u32 の和のあふれ。
    assert!(is_invalid(&validate_indexed_draw_range(
        &idr(u32::MAX, 1, 0, 0, 1),
        IndexFormat::Uint16,
        0,
        u64::MAX
    )));
    assert!(is_invalid(&validate_indexed_draw_range(
        &idr(0, 1, 0, u32::MAX, 1),
        IndexFormat::Uint16,
        0,
        u64::MAX
    )));
    // (c) u64 の足し算のあふれ。
    let one = idr(0, 1, 0, 0, 1);
    assert!(is_invalid(&validate_indexed_draw_range(
        &one,
        IndexFormat::Uint32,
        u64::MAX - 3,
        u64::MAX
    )));
    assert_eq!(
        validate_indexed_draw_range(&one, IndexFormat::Uint32, u64::MAX - 4, u64::MAX),
        Ok(())
    );
    // (d) 形式で終わりが 2 倍違う。
    let n = idr(0, 3, 0, 0, 1);
    assert_eq!(
        validate_indexed_draw_range(&n, IndexFormat::Uint16, 10, 16),
        Ok(())
    );
    assert!(is_invalid(&validate_indexed_draw_range(
        &n,
        IndexFormat::Uint16,
        10,
        15
    )));
    assert_eq!(
        validate_indexed_draw_range(&n, IndexFormat::Uint32, 10, 22),
        Ok(())
    );
    assert!(is_invalid(&validate_indexed_draw_range(
        &n,
        IndexFormat::Uint32,
        10,
        21
    )));
    // (e) base_vertex は結果を変えない。
    for bv in [i32::MIN, 0, i32::MAX] {
        let r = idr(0, 4, bv, 0, 1);
        assert_eq!(
            validate_indexed_draw_range(&r, IndexFormat::Uint16, 8, 16),
            Ok(())
        );
        assert!(is_invalid(&validate_indexed_draw_range(
            &r,
            IndexFormat::Uint16,
            8,
            15
        )));
    }
}

// REND-62
#[test]
fn rejection_is_repeatable() {
    let bad = dr(u32::MAX, 1, 0, 1);
    assert_eq!(validate_draw_range(&bad), validate_draw_range(&bad));
    let bad = idr(0, 4, 0, 0, 1);
    assert_eq!(
        validate_indexed_draw_range(&bad, IndexFormat::Uint16, 8, 15),
        validate_indexed_draw_range(&bad, IndexFormat::Uint16, 8, 15)
    );
}

// REND-63
#[test]
fn rejection_is_invalid_argument_only() {
    let results = [
        validate_draw_range(&dr(u32::MAX, 1, 0, 1)),
        validate_draw_range(&dr(0, 1, u32::MAX, 1)),
        validate_indexed_draw_range(&idr(u32::MAX, 1, 0, 0, 1), IndexFormat::Uint16, 0, u64::MAX),
        validate_indexed_draw_range(&idr(0, 1, 0, u32::MAX, 1), IndexFormat::Uint16, 0, u64::MAX),
        validate_indexed_draw_range(
            &idr(0, 1, 0, 0, 1),
            IndexFormat::Uint32,
            u64::MAX - 3,
            u64::MAX,
        ),
        validate_indexed_draw_range(&idr(0, 4, 0, 0, 1), IndexFormat::Uint16, 8, 15),
    ];
    for r in &results {
        assert!(is_invalid(r), "{r:?}");
    }
}
