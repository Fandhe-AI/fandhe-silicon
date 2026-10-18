//! `execution` の C8・C6 の検査を外の crate から公開のパスで呼ぶ統合テスト（TASK-421・TASK-423 のポインタ。BIND-35〜BIND-37・BIND-44）。
//!
//! 長さ 0 の次元・`u64` のあふれ・上限超過が `InvalidArgument`（文言の先頭 `C8`）で拒否され、上限ちょうどは通ることを確かめる。
//! 定数の長さが宣言の必要バイト数と違えば先頭 `C6` で拒否され、同じ長さなら通ることも確かめる。feature なしで動く。

use fandhe_silicon_core::execution::{
    DType, OperandBinding, OperandCheckError, OperandConstant, OperandSpec, required_bytes,
    validate_dispatch_operands,
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

/// 束縛・定数・束縛の容量を渡せる呼び出し（容量はどの束縛にも同じ値を返す）。
fn run(
    declared: &[OperandSpec<'_>],
    bindings: &[OperandBinding],
    consts: &[OperandConstant<'_>],
    cap: Option<u64>,
) -> Result<(), OperandCheckError> {
    validate_dispatch_operands(declared, bindings, consts, |_| cap, u64::MAX)
}

/// variant と文言の先頭 `code`（`C6` など）だけを見る。
fn is_code(r: &Result<(), OperandCheckError>, code: &str) -> bool {
    matches!(r, Err(OperandCheckError::InvalidArgument(m)) if m.starts_with(code))
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

/// 必要 4,000 バイトの宣言は、上限が 3,999 なら C8 で拒否される。
/// 巨大な宣言（必要バイト数が約 2^63）でも結果が返ること自体が、バッファを確保せずに判定している証拠になる。
#[test]
fn c8_exceeds_max_buffer_bytes_is_rejected_without_allocating() {
    let shape = [1000u64];
    let s = spec(0, DType::F32, &shape);
    assert_eq!(required_bytes(&s), Some(4000));
    let r = check(&[s], 3999);
    assert!(is_c8(&r), "{r:?}");

    let huge_shape = [u64::MAX / 8];
    let huge = spec(0, DType::F32, &huge_shape);
    let n = required_bytes(&huge).expect("does not overflow u64");
    assert!(n > 0);
    let r = check(&[huge], n - 1);
    assert!(is_c8(&r), "{r:?}");
}

/// 上限が必要バイト数ちょうどなら通る（巨大な宣言でも確保せずに通る）。
#[test]
fn c8_within_max_buffer_bytes_is_accepted() {
    let shape = [1000u64];
    assert_eq!(check(&[spec(0, DType::F32, &shape)], 4000), Ok(()));

    let huge_shape = [u64::MAX / 8];
    let huge = spec(0, DType::F32, &huge_shape);
    let n = required_bytes(&huge).expect("does not overflow u64");
    assert_eq!(check(&[huge], n), Ok(()));
}

/// 定数が宣言の必要バイト数（u32 × 3 = 12）より短ければ C6 で拒否される（TASK-423・BIND-44 のポインタ）。
#[test]
fn c6_constant_size_mismatch_is_rejected() {
    let declared = [spec(0, DType::U32, &[3])];
    assert_eq!(required_bytes(&declared[0]), Some(12));
    let consts = [OperandConstant {
        slot: 0,
        bytes: &[0u8; 8],
    }];
    assert!(is_code(&run(&declared, &[], &consts, None), "C6"));
}

/// 定数が必要バイト数ちょうどなら通る。拒否の原因が長さだけであることを示す。
#[test]
fn c6_constant_size_match_is_accepted() {
    let declared = [spec(0, DType::U32, &[3])];
    let consts = [OperandConstant {
        slot: 0,
        bytes: &[0u8; 12],
    }];
    assert_eq!(run(&declared, &[], &consts, None), Ok(()));
}
