//! `execution` の引数の宣言の型を外の crate から組み立てて読み出す試験（TASK-470 のポインタ）。
//!
//! feature なしで動く。`#[non_exhaustive]` の型の `match` には `_` の腕が要る。

use fandhe_silicon_core::execution::{DType, OperandSpec};

/// 型ごとの名前を返す（外の crate からの見え方の確認。`_` の腕が要る）。
fn label(dtype: DType) -> &'static str {
    match dtype {
        DType::F32 => "f32",
        DType::F16 => "f16",
        DType::U32 => "u32",
        DType::I32 => "i32",
        DType::Bool => "bool",
        _ => "other",
    }
}

#[test]
fn execution_dtype_byte_sizes() {
    assert_eq!(DType::F32.byte_size(), 4);
    assert_eq!(DType::F16.byte_size(), 2);
    assert_eq!(DType::U32.byte_size(), 4);
    assert_eq!(DType::I32.byte_size(), 4);
    assert_eq!(DType::Bool.byte_size(), 1);
}

#[test]
fn execution_dtype_is_copy_and_eq() {
    let a = DType::F16;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(DType::F32, DType::I32);
    assert_eq!(label(a), "f16");
}

#[test]
fn execution_operand_spec_keeps_fields() {
    let shape = [2_u64, 3];
    let stride = [3_i64, 1];
    let spec = OperandSpec {
        slot: 1,
        dtype: DType::F32,
        shape: &shape,
        stride: &stride,
        alignment: 16,
    };
    let copy = spec;
    for s in [spec, copy] {
        assert_eq!(s.slot, 1);
        assert_eq!(s.dtype, DType::F32);
        assert_eq!(s.shape, &[2, 3]);
        assert_eq!(s.stride, &[3, 1]);
        assert_eq!(s.alignment, 16);
    }
}

#[test]
fn execution_operand_spec_accepts_empty_dims() {
    let spec = OperandSpec {
        slot: 0,
        dtype: DType::Bool,
        shape: &[],
        stride: &[],
        alignment: 1,
    };
    assert!(spec.shape.is_empty());
    assert!(spec.stride.is_empty());
}
