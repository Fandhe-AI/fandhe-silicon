//! 引数の宣言の件数と次元数を、パイプラインを作る前に照らす純粋関数の置き場（TASK-420 のポインタ）。
//!
//! `execution` から再公開される。上や下-2 が組み立てた宣言を、下-1 の背後実装へ渡す前に
//! 共通の層で拒否するために置く。C1（宣言の件数と `BindingLayout.slots` の件数）と
//! C3（宣言ごとの `shape` と `stride` の次元数）を判定する。
//! 違反は契約のエラー型 `ContractError` の `InvalidArgument` で返す。文言は英語で、先頭は `C1:`・`C3:` のいずれか。
//! 背後実装・ファイル・時刻には触れず、確保が起きるのはエラーの文字列だけ。

use super::{BindingLayout, OperandSpec};
use crate::error::ContractError;

/// 引数の宣言の件数と次元数を照らす。
///
/// 次の順に判定し、最初に当たったものを返す。
///
/// 1. C1（BIND-33 のポインタ）: `declared` の件数が `layout.slots` の件数と違えば拒否する
/// 2. C3（BIND-34 のポインタ）: `declared` を先頭から見て、`shape` と `stride` の次元数が違う最初の宣言で拒否する。`stride` が空でも比べる
///
/// `slot` の値・重複、`dtype`、`alignment`、`shape`・`stride` の値は見ない。
///
/// # Errors
///
/// 違反は `ContractError::InvalidArgument`。文言の先頭は `C1:` か `C3:`。
pub fn validate_declared_operands(
    declared: &[OperandSpec<'_>],
    layout: &BindingLayout,
) -> Result<(), ContractError> {
    let d = declared.len();
    let s = layout.slots.len();
    if d != s {
        return Err(ContractError::InvalidArgument(format!(
            "C1: declared count ({d}) differs from BindingLayout.slots count ({s})"
        )));
    }
    for spec in declared {
        let n = spec.shape.len();
        let m = spec.stride.len();
        if n != m {
            let slot = spec.slot;
            return Err(ContractError::InvalidArgument(format!(
                "C3: slot {slot}: shape has {n} dimensions but stride has {m}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::DType;

    fn spec<'a>(slot: u32, shape: &'a [u64], stride: &'a [i64]) -> OperandSpec<'a> {
        OperandSpec {
            slot,
            dtype: DType::F32,
            shape,
            stride,
            alignment: 0,
        }
    }

    fn is_err(r: &Result<(), ContractError>, prefix: &str) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(prefix))
    }

    #[test]
    fn declared_check_c1_one_declared_two_slots() {
        let d = [spec(0, &[2], &[1])];
        let r = validate_declared_operands(&d, &BindingLayout::new(vec![0, 1]));
        assert!(is_err(&r, "C1:"));
    }

    #[test]
    fn declared_check_c1_matching_counts_ok() {
        let d = [spec(0, &[2], &[1]), spec(1, &[2, 3], &[3, 1])];
        let r = validate_declared_operands(&d, &BindingLayout::new(vec![0, 1]));
        assert!(r.is_ok());
    }

    #[test]
    fn declared_check_empty_ok() {
        let r = validate_declared_operands(&[], &BindingLayout::new(vec![]));
        assert!(r.is_ok());
    }

    #[test]
    fn declared_check_c3_dimension_mismatch() {
        let d = [spec(0, &[2, 3], &[1])];
        let r = validate_declared_operands(&d, &BindingLayout::new(vec![0]));
        assert!(is_err(&r, "C3:"));
    }

    #[test]
    fn declared_check_c3_equal_dimensions_ok() {
        for (shape, stride) in [
            (&[][..], &[][..]),
            (&[4][..], &[1][..]),
            (&[2, 3][..], &[3, 1][..]),
        ] {
            let d = [spec(0, shape, stride)];
            let r = validate_declared_operands(&d, &BindingLayout::new(vec![0]));
            assert!(r.is_ok());
        }
    }

    #[test]
    fn declared_check_c3_empty_stride_with_shape() {
        let d = [spec(0, &[4], &[])];
        let r = validate_declared_operands(&d, &BindingLayout::new(vec![0]));
        assert!(is_err(&r, "C3:"));
    }

    #[test]
    fn declared_check_c3_message_names_second_slot() {
        let d = [spec(5, &[2], &[1]), spec(7, &[2, 2], &[1])];
        let r = validate_declared_operands(&d, &BindingLayout::new(vec![5, 7]));
        assert!(matches!(r, Err(ContractError::InvalidArgument(m)) if m.contains("slot 7")));
    }

    #[test]
    fn declared_check_c1_comes_before_c3() {
        let d = [spec(0, &[2, 3], &[1])];
        let r = validate_declared_operands(&d, &BindingLayout::new(vec![0, 1]));
        assert!(is_err(&r, "C1:"));
    }
}
