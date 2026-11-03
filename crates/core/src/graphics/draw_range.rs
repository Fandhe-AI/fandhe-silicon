//! 範囲つきの描画の範囲の値の型と、範囲を照らす純粋関数の置き場（TASK-838）。
//!
//! `graphics` モジュールから再公開される（公開のパスは `fandhe_silicon_core::graphics::…`）。
//! 背後実装の API は呼ばず、状態を持たない。

use super::{IndexFormat, index_format_bytes};
use crate::error::ContractError;

/// インデックスを使わない描画の範囲（項目 `REND-62`）。欄は公開で、外の crate から構造体式で作れる。
///
/// ```
/// use fandhe_silicon_core::graphics::DrawRange;
/// let r = DrawRange { first_vertex: 0, vertex_count: 3, first_instance: 0, instance_count: 1 };
/// assert_eq!(r, r);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawRange {
    /// 最初の頂点。
    pub first_vertex: u32,
    /// 頂点の数。
    pub vertex_count: u32,
    /// 最初のインスタンス。
    pub first_instance: u32,
    /// インスタンスの数。
    pub instance_count: u32,
}

/// インデックスを使う描画の範囲（項目 `REND-63`）。欄は公開で、外の crate から構造体式で作れる。
///
/// ```
/// use fandhe_silicon_core::graphics::IndexedDrawRange;
/// let r = IndexedDrawRange {
///     first_index: 0,
///     index_count: 6,
///     base_vertex: -1,
///     first_instance: 0,
///     instance_count: 1,
/// };
/// assert_eq!(r, r);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedDrawRange {
    /// 最初のインデックス。
    pub first_index: u32,
    /// インデックスの数。
    pub index_count: u32,
    /// 頂点に足す値。
    pub base_vertex: i32,
    /// 最初のインスタンス。
    pub first_instance: u32,
    /// インスタンスの数。
    pub instance_count: u32,
}

/// `first_vertex + vertex_count` または `first_instance + instance_count` が `u32` であふれるときに拒む。
/// 数が 0 でも同じ規則で照らす。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::graphics::{DrawRange, validate_draw_range};
/// let ok = DrawRange { first_vertex: 0, vertex_count: 3, first_instance: 0, instance_count: 1 };
/// assert_eq!(validate_draw_range(&ok), Ok(()));
/// let bad = DrawRange { first_vertex: u32::MAX, vertex_count: 1, first_instance: 0, instance_count: 1 };
/// assert!(matches!(validate_draw_range(&bad), Err(ContractError::InvalidArgument(_))));
/// ```
///
/// # Errors
///
/// 頂点の側かインスタンスの側の和があふれるとき `ContractError::InvalidArgument`。
pub fn validate_draw_range(range: &DrawRange) -> Result<(), ContractError> {
    if range.first_vertex.checked_add(range.vertex_count).is_none() {
        return Err(ContractError::InvalidArgument(format!(
            "draw range: first_vertex {} + vertex_count {} overflows",
            range.first_vertex, range.vertex_count
        )));
    }
    if range
        .first_instance
        .checked_add(range.instance_count)
        .is_none()
    {
        return Err(ContractError::InvalidArgument(format!(
            "draw range: first_instance {} + instance_count {} overflows",
            range.first_instance, range.instance_count
        )));
    }
    Ok(())
}

/// インデックスの側とインスタンスの側の和のあふれを拒み、
/// `offset + (first_index + index_count) * index_format_bytes(format)` が `bound_end` を超えるときも拒む。
/// 終わりが `bound_end` ちょうどなら通す。`base_vertex` と `offset` の倍数は照らさない。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::graphics::{IndexFormat, IndexedDrawRange, validate_indexed_draw_range};
/// let r = IndexedDrawRange { first_index: 0, index_count: 4, base_vertex: 0, first_instance: 0, instance_count: 1 };
/// assert_eq!(validate_indexed_draw_range(&r, IndexFormat::Uint16, 8, 16), Ok(()));
/// assert!(matches!(
///     validate_indexed_draw_range(&r, IndexFormat::Uint16, 8, 15),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
///
/// # Errors
///
/// 和があふれるとき、または終わりが `bound_end` を超えるとき `ContractError::InvalidArgument`。
pub fn validate_indexed_draw_range(
    range: &IndexedDrawRange,
    format: IndexFormat,
    offset: u64,
    bound_end: u64,
) -> Result<(), ContractError> {
    let Some(index_end) = range.first_index.checked_add(range.index_count) else {
        return Err(ContractError::InvalidArgument(format!(
            "indexed draw range: first_index {} + index_count {} overflows",
            range.first_index, range.index_count
        )));
    };
    if range
        .first_instance
        .checked_add(range.instance_count)
        .is_none()
    {
        return Err(ContractError::InvalidArgument(format!(
            "indexed draw range: first_instance {} + instance_count {} overflows",
            range.first_instance, range.instance_count
        )));
    }
    let end = u64::from(index_end)
        .checked_mul(u64::from(index_format_bytes(format)))
        .and_then(|bytes| bytes.checked_add(offset));
    match end {
        Some(end) if end <= bound_end => Ok(()),
        Some(end) => Err(ContractError::InvalidArgument(format!(
            "indexed draw range: end {end} exceeds bound {bound_end}"
        ))),
        None => Err(ContractError::InvalidArgument(
            "indexed draw range: end overflows".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dr(fv: u32, vc: u32, fi: u32, ic: u32) -> DrawRange {
        DrawRange {
            first_vertex: fv,
            vertex_count: vc,
            first_instance: fi,
            instance_count: ic,
        }
    }

    fn idr(fi: u32, ic: u32, bv: i32, fin: u32, inc: u32) -> IndexedDrawRange {
        IndexedDrawRange {
            first_index: fi,
            index_count: ic,
            base_vertex: bv,
            first_instance: fin,
            instance_count: inc,
        }
    }

    fn is_invalid(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn draw_range_sum_boundary() {
        assert_eq!(validate_draw_range(&dr(u32::MAX - 1, 1, 0, 1)), Ok(()));
        assert!(is_invalid(validate_draw_range(&dr(u32::MAX, 1, 0, 1))));
        assert_eq!(validate_draw_range(&dr(0, 1, u32::MAX - 1, 1)), Ok(()));
        assert!(is_invalid(validate_draw_range(&dr(0, 1, u32::MAX, 1))));
    }

    #[test]
    fn draw_range_zero_counts_follow_sum_rule() {
        assert_eq!(validate_draw_range(&dr(u32::MAX, 0, u32::MAX, 0)), Ok(()));
        assert_eq!(validate_draw_range(&dr(0, 0, 0, 0)), Ok(()));
    }

    #[test]
    fn indexed_end_equals_bound() {
        let r = idr(2, 4, 0, 0, 1);
        assert_eq!(
            validate_indexed_draw_range(&r, IndexFormat::Uint16, 8, 20),
            Ok(())
        );
        assert!(is_invalid(validate_indexed_draw_range(
            &r,
            IndexFormat::Uint16,
            8,
            19
        )));
    }

    #[test]
    fn indexed_sum_overflows() {
        assert!(is_invalid(validate_indexed_draw_range(
            &idr(u32::MAX, 1, 0, 0, 1),
            IndexFormat::Uint16,
            0,
            u64::MAX
        )));
        assert!(is_invalid(validate_indexed_draw_range(
            &idr(0, 1, 0, u32::MAX, 1),
            IndexFormat::Uint16,
            0,
            u64::MAX
        )));
    }

    #[test]
    fn indexed_offset_addition_overflows() {
        let r = idr(0, 1, 0, 0, 1);
        assert!(is_invalid(validate_indexed_draw_range(
            &r,
            IndexFormat::Uint32,
            u64::MAX - 3,
            u64::MAX
        )));
        assert_eq!(
            validate_indexed_draw_range(&r, IndexFormat::Uint32, u64::MAX - 4, u64::MAX),
            Ok(())
        );
    }

    #[test]
    fn indexed_format_changes_end_by_factor_two() {
        let n = 5u32;
        let r = idr(0, n, 0, 0, 1);
        let off = 10u64;
        let n64 = u64::from(n);
        assert_eq!(
            validate_indexed_draw_range(&r, IndexFormat::Uint16, off, off + 2 * n64),
            Ok(())
        );
        assert!(is_invalid(validate_indexed_draw_range(
            &r,
            IndexFormat::Uint16,
            off,
            off + 2 * n64 - 1
        )));
        assert_eq!(
            validate_indexed_draw_range(&r, IndexFormat::Uint32, off, off + 4 * n64),
            Ok(())
        );
        assert!(is_invalid(validate_indexed_draw_range(
            &r,
            IndexFormat::Uint32,
            off,
            off + 4 * n64 - 1
        )));
    }

    #[test]
    fn indexed_base_vertex_does_not_matter() {
        for bv in [i32::MIN, 0, i32::MAX] {
            let r = idr(0, 4, bv, 0, 1);
            assert_eq!(
                validate_indexed_draw_range(&r, IndexFormat::Uint16, 0, 8),
                Ok(())
            );
            assert!(is_invalid(validate_indexed_draw_range(
                &r,
                IndexFormat::Uint16,
                0,
                7
            )));
        }
    }
}
