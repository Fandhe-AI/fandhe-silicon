//! `execution` の C8 の検査を外の crate から公開のパスで呼ぶ統合テスト（TASK-421 のポインタ。BIND-35・BIND-36）。
//!
//! 長さ 0 の次元と `u64` のあふれが `InvalidArgument`（文言の先頭 `C8`）で拒否されることを確かめる。feature なしで動く。

use fandhe_silicon_core::execution::{
    DType, OperandCheckError, OperandSpec, required_bytes, validate_dispatch_operands,
};

fn spec(slot: u32, dtype: DType, shape: &[u64]) -> OperandSpec<'_> {
    OperandSpec {
        slot,
        dtype,
        shape,
        stride: &[],
        alignment: 4,
    }
}

fn check(declared: &[OperandSpec<'_>], max: u64) -> Result<(), OperandCheckError> {
    validate_dispatch_operands(declared, &[], &[], |_| None, max)
}

/// variant と文言の先頭 `C8` だけを見る（`OperandCheckError` は `non_exhaustive`）。
fn is_c8(r: &Result<(), OperandCheckError>) -> bool {
    matches!(r, Err(OperandCheckError::InvalidArgument(m)) if m.starts_with("C8"))
}

/// shape に 0 の次元があれば、上限が最大でも C8 で拒否される。
#[test]
fn c8_zero_length_shape_is_rejected() {
    let shape = [0u64];
    let r = check(&[spec(0, DType::F32, &shape)], u64::MAX);
    assert!(is_c8(&r), "{r:?}");
}

/// GEMM の宣言（A・B・C の 3 件）で M=0 なら拒否され、M=1 なら通る（拒否の原因が M=0 だけ）。
#[test]
fn c8_gemm_zero_dimension_is_rejected() {
    let build = |m: u64| {
        let a = [m, 4];
        let b = [4u64, 3];
        let c = [m, 3];
        check(
            &[
                spec(0, DType::F32, &a),
                spec(1, DType::F32, &b),
                spec(2, DType::F32, &c),
            ],
            u64::MAX,
        )
    };
    let rejected = build(0);
    assert!(is_c8(&rejected), "{rejected:?}");
    assert_eq!(build(1), Ok(()));
}

/// 必要バイト数が `u64` をあふれる shape は、panic せず C8 で拒否される（`checked_mul` が `None` を返す）。
#[test]
fn c8_u64_overflow_in_byte_calculation_is_rejected() {
    let shape = [u64::MAX / 2, 4];
    let s = spec(0, DType::F32, &shape);
    assert_eq!(required_bytes(&s), None);
    let r = check(&[s], u64::MAX);
    assert!(is_c8(&r), "{r:?}");
}
