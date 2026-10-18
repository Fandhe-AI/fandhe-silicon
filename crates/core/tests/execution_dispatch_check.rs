//! `execution` の C8・C4・C5・C6 の検査を外の crate から公開のパスで呼ぶ統合テスト（TASK-421・TASK-423 のポインタ。BIND-35〜BIND-37・BIND-42〜BIND-44）。
//!
//! 長さ 0 の次元・`u64` のあふれ・上限超過（C8）、バッファの残容量不足（C4）、offset の alignment 違反（C5）、
//! 定数の長さが宣言の必要バイト数と違う場合（C6）が `InvalidArgument`（文言の先頭が各検査名）で拒否され、
//! 境界ちょうど・同じ長さは通ることを確かめる。feature なしで動く。
//! alignment 0 の宣言は C5 を判定せず、C4・C6 は判定することも確かめる（TASK-425・BIND-59 のポインタ）。

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

/// variant と文言の先頭が検査名 `name` であることだけを見る（`OperandCheckError` は `non_exhaustive`）。
fn is_rejected_by(r: &Result<(), OperandCheckError>, name: &str) -> bool {
    matches!(r, Err(OperandCheckError::InvalidArgument(m)) if m.starts_with(name))
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

/// variant と文言の先頭 `C8` だけを見る。
fn is_c8(r: &Result<(), OperandCheckError>) -> bool {
    is_rejected_by(r, "C8")
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

/// 容量 16 のバッファに必要 400 バイトの宣言を束縛すると C4 で拒否される（ほかの検査には当たらない入力）。
#[test]
fn c4_buffer_capacity_exceeded_is_rejected() {
    let shape = [100u64];
    let s = spec(0, DType::F32, &shape);
    assert_eq!(required_bytes(&s), Some(400));
    let r = validate_dispatch_operands(
        &[s],
        &[OperandBinding { slot: 0, offset: 0 }],
        &[],
        |_| Some(16),
        u64::MAX,
    );
    assert!(is_rejected_by(&r, "C4"), "{r:?}");
}

/// 必要 16 バイトちょうどの容量なら C4 を通る。
#[test]
fn c4_buffer_capacity_sufficient_is_accepted() {
    let shape = [4u64];
    let s = spec(0, DType::F32, &shape);
    assert_eq!(required_bytes(&s), Some(16));
    let r = validate_dispatch_operands(
        &[s],
        &[OperandBinding { slot: 0, offset: 0 }],
        &[],
        |_| Some(16),
        u64::MAX,
    );
    assert_eq!(r, Ok(()));
}

/// alignment 16 の宣言に offset 4 を束縛すると C5 で拒否される（容量は不明にして C4 を避ける）。
#[test]
fn c5_alignment_violation_is_rejected() {
    let shape = [1u64];
    let s = OperandSpec {
        alignment: 16,
        ..spec(0, DType::F32, &shape)
    };
    let r = validate_dispatch_operands(
        &[s],
        &[OperandBinding { slot: 0, offset: 4 }],
        &[],
        |_| None,
        u64::MAX,
    );
    assert!(is_rejected_by(&r, "C5"), "{r:?}");
}

/// 同じ宣言で offset 16 なら alignment を満たし、C5 を通る。
#[test]
fn c5_alignment_satisfied_is_accepted() {
    let shape = [1u64];
    let s = OperandSpec {
        alignment: 16,
        ..spec(0, DType::F32, &shape)
    };
    let r = validate_dispatch_operands(
        &[s],
        &[OperandBinding {
            slot: 0,
            offset: 16,
        }],
        &[],
        |_| None,
        u64::MAX,
    );
    assert_eq!(r, Ok(()));
}

/// alignment 0 の宣言は offset が 0 でなくても C5 を判定せず通る。alignment 16 なら同じ offset で拒否される（TASK-425・BIND-59 のポインタ）。
#[test]
fn c5_zero_alignment_with_nonzero_offset_is_accepted() {
    let shape = [1u64];
    let zero = OperandSpec {
        alignment: 0,
        ..spec(0, DType::F32, &shape)
    };
    for offset in [4u64, 16, 1_000, 2_000] {
        let r = validate_dispatch_operands(
            &[zero],
            &[OperandBinding { slot: 0, offset }],
            &[],
            |_| None,
            u64::MAX,
        );
        assert_eq!(r, Ok(()), "offset {offset} with alignment 0");
    }
    let sixteen = OperandSpec {
        alignment: 16,
        ..zero
    };
    for offset in [4u64, 1_000] {
        let r = validate_dispatch_operands(
            &[sixteen],
            &[OperandBinding { slot: 0, offset }],
            &[],
            |_| None,
            u64::MAX,
        );
        assert!(
            is_rejected_by(&r, "C5"),
            "offset {offset} with alignment 16"
        );
    }
}

/// alignment 0 でも C4・C6 は判定される（C5 だけを飛ばす。TASK-425・BIND-59 のポインタ）。
#[test]
fn c5_zero_alignment_still_applies_other_checks() {
    let shape = [4u64];
    let zero = OperandSpec {
        alignment: 0,
        ..spec(0, DType::F32, &shape)
    };
    let bindings = [OperandBinding { slot: 0, offset: 4 }];
    // C4: 16 バイトに対し offset 4 から残り 12 なら拒否、容量 20 ならちょうど通る。
    assert!(is_rejected_by(
        &run(&[zero], &bindings, &[], Some(16)),
        "C4"
    ));
    assert_eq!(run(&[zero], &bindings, &[], Some(20)), Ok(()));
    // C6: 定数が宣言の必要バイト数と違えば拒否、同じなら通る。
    let c_shape = [3u64];
    let declared = [zero, spec(1, DType::U32, &c_shape)];
    let short = [OperandConstant {
        slot: 1,
        bytes: &[0u8; 8],
    }];
    assert!(is_rejected_by(
        &run(&declared, &bindings, &short, None),
        "C6"
    ));
    let exact = [OperandConstant {
        slot: 1,
        bytes: &[0u8; 12],
    }];
    assert_eq!(run(&declared, &bindings, &exact, None), Ok(()));
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
    assert!(is_rejected_by(&run(&declared, &[], &consts, None), "C6"));
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
