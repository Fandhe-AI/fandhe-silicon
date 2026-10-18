//! `execution` の C8・C4・C5・C6 の検査を外の crate から公開のパスで呼ぶ統合テスト（TASK-421・TASK-423・TASK-425 のポインタ。BIND-35〜BIND-37・BIND-42〜BIND-44・BIND-58）。
//!
//! 長さ 0 の次元・`u64` のあふれ・上限超過（C8）、バッファの残容量不足（C4）、offset の alignment 違反（C5）、
//! 定数の長さが宣言の必要バイト数と違う場合（C6）が `InvalidArgument`（文言の先頭が各検査名）で拒否され、
//! 境界ちょうど・同じ長さは通ることを確かめる。feature なしで動く。

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

/// slot 0 に F32 `[n]` を宣言して `offset` で束縛し、容量 `cap` で C4 を流す（alignment 4 で offset は 4 の倍数なので C5 には当たらない）。
fn run_c4(n: u64, offset: u64, cap: Option<u64>) -> Result<(), OperandCheckError> {
    let shape = [n];
    let s = spec(0, DType::F32, &shape);
    assert_eq!(required_bytes(&s), Some(n * 4));
    validate_dispatch_operands(
        &[s],
        &[OperandBinding { slot: 0, offset }],
        &[],
        |_| cap,
        u64::MAX,
    )
}

/// 容量が不明（`None`）の束縛は、必要 400 バイトでも offset 0・4・16・1,000・2,000 のどれでも C4 で拒否されない。
/// 対照として同じ入力を容量 16 で流すと C4 で拒否される。
#[test]
fn c4_unknown_capacity_is_not_checked() {
    for offset in [0u64, 4, 16, 1_000, 2_000] {
        assert_eq!(run_c4(100, offset, None), Ok(()), "offset {offset}");
        let r = run_c4(100, offset, Some(16));
        assert!(is_rejected_by(&r, "C4"), "offset {offset}: {r:?}");
    }
}

/// 容量の不明な束縛だけが C4 を飛ばされ、容量の分かる別の束縛は同じ呼び出しの中で判定される。
#[test]
fn c4_unknown_capacity_binding_is_skipped_per_binding() {
    let big = [100u64];
    let fits = [4u64];
    let over = [5u64];
    let cap = |b: &OperandBinding| if b.slot == 0 { None } else { Some(16) };
    let bindings = [
        OperandBinding { slot: 0, offset: 0 },
        OperandBinding { slot: 1, offset: 0 },
    ];
    let ok = validate_dispatch_operands(
        &[spec(0, DType::F32, &big), spec(1, DType::F32, &fits)],
        &bindings,
        &[],
        cap,
        u64::MAX,
    );
    assert_eq!(ok, Ok(()));
    let ng = validate_dispatch_operands(
        &[spec(0, DType::F32, &big), spec(1, DType::F32, &over)],
        &bindings,
        &[],
        cap,
        u64::MAX,
    );
    assert!(is_rejected_by(&ng, "C4"), "{ng:?}");
}

/// 容量 16 で、残り（容量から offset を引いた値。容量以上の offset では 0）ちょうどは通り、1 要素多いと C4 で拒否される。
/// offset が容量以上でも panic せず、折り返さずに 0 として拒否される。
#[test]
fn c4_offset_reduces_remaining_capacity() {
    // (offset, 通る要素数。None は通る F32 が無い, 拒否される要素数)
    let cases: [(u64, Option<u64>, u64); 5] = [
        (0, Some(4), 5),
        (4, Some(3), 4),
        (16, None, 1),
        (1_000, None, 1),
        (2_000, None, 1),
    ];
    for (offset, pass, reject) in cases {
        if let Some(n) = pass {
            assert_eq!(run_c4(n, offset, Some(16)), Ok(()), "offset {offset}");
        }
        let r = run_c4(reject, offset, Some(16));
        assert!(is_rejected_by(&r, "C4"), "offset {offset}: {r:?}");
    }
    // 残り 0 では 1 バイトの Bool 宣言も拒否される。
    let s = spec(0, DType::Bool, &[]);
    assert_eq!(required_bytes(&s), Some(1));
    let r = validate_dispatch_operands(
        &[s],
        &[OperandBinding {
            slot: 0,
            offset: 16,
        }],
        &[],
        |_| Some(16),
        u64::MAX,
    );
    assert!(is_rejected_by(&r, "C4"), "{r:?}");
}

/// 容量 4,096 に大きな offset を束縛しても、残りちょうどは通り、1 要素多いと C4 で拒否される。
#[test]
fn c4_large_offset_subtracts_from_known_capacity() {
    // (offset, 残りちょうどの要素数)
    for (offset, fit) in [(1_000u64, 774u64), (2_000, 524)] {
        assert_eq!(run_c4(fit, offset, Some(4_096)), Ok(()), "offset {offset}");
        let r = run_c4(fit + 1, offset, Some(4_096));
        assert!(is_rejected_by(&r, "C4"), "offset {offset}: {r:?}");
    }
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
