//! 引数の宣言の C1・C3 と C4〜C8 を続けて判定する純粋関数の置き場（TASK-426 のポインタ）。
//!
//! `declared_check` の `validate_declared_operands`（C1・C3）の後に、`dispatch_check` の
//! `validate_dispatch_operands`（C4〜C8）を呼ぶ。`execution` から再公開される。
//! 違反は契約のエラー型 `ContractError` の `InvalidArgument` で返す。文言は下の 2 関数のものをそのまま返す。
//! 背後実装・ファイル・時刻には触れず、確保が起きるのはエラーの文字列だけ。

use super::{
    BindingLayout, OperandBinding, OperandConstant, OperandSpec, validate_declared_operands,
    validate_dispatch_operands,
};
use crate::error::ContractError;

/// 引数の宣言を C1・C3 の後に C4〜C8 の順で照らす（BIND-55 のポインタ）。
///
/// 先に `validate_declared_operands(declared, layout)` を呼び、拒否されたらその `Err` をそのまま返す。
/// このとき `validate_dispatch_operands` は呼ばれず、`binding_capacity` も呼ばれない。
/// 通れば `validate_dispatch_operands(declared, bindings, consts, binding_capacity, max_buffer_bytes)` の結果をそのまま返す。
///
/// # Errors
///
/// 違反は `ContractError::InvalidArgument`。文言の先頭は `C1:`・`C3:`・`C4:`〜`C8:` のいずれか。
pub fn validate_operands<F>(
    declared: &[OperandSpec<'_>],
    layout: &BindingLayout,
    bindings: &[OperandBinding],
    consts: &[OperandConstant<'_>],
    binding_capacity: F,
    max_buffer_bytes: u64,
) -> Result<(), ContractError>
where
    F: Fn(&OperandBinding) -> Option<u64>,
{
    validate_declared_operands(declared, layout)?;
    validate_dispatch_operands(
        declared,
        bindings,
        consts,
        binding_capacity,
        max_buffer_bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::DType;
    use std::cell::Cell;

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
    fn operand_check_rejects_c1_count_mismatch() {
        let declared = [spec(0, &[4], &[1])];
        let layout = BindingLayout::new(vec![0, 1]);
        let r = validate_operands(&declared, &layout, &[], &[], |_| None, u64::MAX);
        assert!(is_err(&r, "C1:"));
    }

    #[test]
    fn operand_check_rejects_c3_dimension_mismatch() {
        let declared = [spec(0, &[2, 2], &[1])];
        let layout = BindingLayout::new(vec![0]);
        let r = validate_operands(&declared, &layout, &[], &[], |_| None, u64::MAX);
        assert!(is_err(&r, "C3:"));
    }

    #[test]
    fn operand_check_rejects_c8_after_c1_c3_pass() {
        let declared = [spec(0, &[0], &[1])];
        let layout = BindingLayout::new(vec![0]);
        let r = validate_operands(&declared, &layout, &[], &[], |_| None, u64::MAX);
        assert!(is_err(&r, "C8:"));
    }

    #[test]
    fn operand_check_accepts_valid_input() {
        let declared = [spec(0, &[4], &[1])];
        let layout = BindingLayout::new(vec![0]);
        let bindings = [OperandBinding { slot: 0, offset: 0 }];
        let r = validate_operands(&declared, &layout, &bindings, &[], |_| Some(16), u64::MAX);
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn operand_check_c1_wins_over_c8_and_skips_capacity() {
        let declared = [spec(0, &[0], &[1])];
        let layout = BindingLayout::new(vec![0, 1]);
        let bindings = [OperandBinding { slot: 0, offset: 0 }];
        let calls = Cell::new(0u32);
        let r = validate_operands(
            &declared,
            &layout,
            &bindings,
            &[],
            |_| {
                calls.set(calls.get() + 1);
                Some(0)
            },
            u64::MAX,
        );
        assert!(is_err(&r, "C1:"));
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn operand_check_matches_dispatch_check_when_c1_c3_pass() {
        let declared = [spec(0, &[4], &[1])];
        let layout = BindingLayout::new(vec![0]);
        let bindings = [OperandBinding { slot: 0, offset: 8 }];
        let composed = validate_operands(&declared, &layout, &bindings, &[], |_| Some(16), 1024);
        let single = validate_dispatch_operands(&declared, &bindings, &[], |_| Some(16), 1024);
        assert!(is_err(&single, "C4:"));
        assert_eq!(composed, single);
    }
}
