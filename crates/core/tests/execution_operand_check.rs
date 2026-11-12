//! `execution` の C1・C3 と C4〜C8 を続けて判定する `validate_operands` を、外の crate から公開のパスで直接呼ぶ統合テスト（TASK-426 のポインタ）。
//!
//! C1・C3 に当たる入力がその拒否で返り、C4〜C8 を判定せず容量の閉包も呼ばれないこと、
//! C1・C3 を通る入力では `validate_dispatch_operands` を単独で呼んだ結果と同じ値が返ること、
//! すべての検査を通る入力が `Ok(())` になること、検査が通る 6 通りの入力で 3 つの関数がどれも `Ok(())` を返すこと、C4〜C8 の違反が複数ある入力で C8 → C7 → C4 → C5 → C6 の順に最初の 1 件が返ること、同じ slot の宣言が複数あるときの C8・C7・C4・C5・C6 の扱い、署名が固定であることを確かめる。feature なしで動く。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    BindingLayout, DType, OperandBinding, OperandConstant, OperandSpec, validate_declared_operands,
    validate_dispatch_operands, validate_operands,
};
use std::cell::Cell;

/// 容量を返す閉包の型。
type Cap = fn(&OperandBinding) -> Option<u64>;

/// 試験 1 件分の入力（名前・宣言・束縛・定数・容量の閉包）。
type Case<'a> = (
    &'a str,
    &'a [OperandSpec<'a>],
    &'a [OperandBinding],
    &'a [OperandConstant<'a>],
    Cap,
);

/// 宣言を 1 件作る。
fn spec<'a>(
    slot: u32,
    dtype: DType,
    shape: &'a [u64],
    stride: &'a [i64],
    alignment: u32,
) -> OperandSpec<'a> {
    OperandSpec {
        slot,
        dtype,
        shape,
        stride,
        alignment,
    }
}

/// 結果が `InvalidArgument` で、文言の先頭が `name` かを返す（`ContractError` は `#[non_exhaustive]` なので `matches!` で見る）。
fn is_rejected_by(r: &Result<(), ContractError>, name: &str) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(name))
}

/// 宣言 1 件に `slots` が 2 件だと C1 で拒否され、C8 にも当たる入力でも容量の閉包は呼ばれない（BIND-55）。
#[test]
fn validate_operands_returns_c1_without_dispatch_checks() {
    let d = [spec(0, DType::F32, &[0], &[1], 0)];
    let layout = BindingLayout::new(vec![0, 1]);
    let bindings = [OperandBinding { slot: 0, offset: 0 }];
    let calls = Cell::new(0u32);
    let r = validate_operands(
        &d,
        &layout,
        &bindings,
        &[],
        |_| {
            calls.set(calls.get() + 1);
            Some(0)
        },
        0,
    );
    assert!(is_rejected_by(&r, "C1"), "{r:?}");
    assert_eq!(calls.get(), 0);
}

/// `shape` 2 次元・`stride` 1 次元の宣言は C3 で拒否され、容量の閉包は呼ばれない（BIND-55）。
#[test]
fn validate_operands_returns_c3_without_dispatch_checks() {
    let d = [spec(0, DType::F32, &[0, 4], &[1], 0)];
    let layout = BindingLayout::new(vec![0]);
    let bindings = [OperandBinding { slot: 0, offset: 0 }];
    let calls = Cell::new(0u32);
    let r = validate_operands(
        &d,
        &layout,
        &bindings,
        &[],
        |_| {
            calls.set(calls.get() + 1);
            Some(0)
        },
        0,
    );
    assert!(is_rejected_by(&r, "C3"), "{r:?}");
    assert_eq!(calls.get(), 0);
}

/// C1・C3 を通る入力では C4〜C8 の拒否と `Ok` が `validate_dispatch_operands` を単独で呼んだ結果と一致する（BIND-55）。
#[test]
fn validate_operands_runs_dispatch_checks_after_declared_checks() {
    let b0 = OperandBinding { slot: 0, offset: 0 };
    let b0_off4 = OperandBinding { slot: 0, offset: 4 };
    let b9 = OperandBinding { slot: 9, offset: 0 };
    let layout = BindingLayout::new(vec![0]);
    let bytes8 = [0u8; 8];
    let c6 = [OperandConstant {
        slot: 0,
        bytes: &bytes8,
    }];

    let c8_d = [spec(0, DType::F32, &[0], &[1], 0)];
    let c7_d = [spec(0, DType::F32, &[4], &[1], 0)];
    let c4_d = [spec(0, DType::F32, &[4], &[1], 4)];
    let c5_d = [spec(0, DType::F32, &[4], &[1], 16)];
    let c6_d = [spec(0, DType::U32, &[3], &[1], 0)];
    let ok_d = [spec(0, DType::F32, &[4], &[1], 0)];

    let cap_zero: Cap = |_| Some(0);
    let cap_none: Cap = |_| None;
    let cap_8: Cap = |_| Some(8);
    let cap_16: Cap = |_| Some(16);

    let cases: [Case<'_>; 6] = [
        ("C8", &c8_d, &[b0], &[], cap_zero),
        ("C7", &c7_d, &[b9], &[], cap_none),
        ("C4", &c4_d, &[b0], &[], cap_8),
        ("C5", &c5_d, &[b0_off4], &[], cap_none),
        ("C6", &c6_d, &[], &c6, cap_none),
        ("Ok", &ok_d, &[b0], &[], cap_16),
    ];
    for (name, d, bindings, consts, cap) in cases {
        let alone = validate_dispatch_operands(d, bindings, consts, cap, u64::MAX);
        if name == "Ok" {
            assert_eq!(alone, Ok(()));
        } else {
            assert!(is_rejected_by(&alone, name), "{name}: {alone:?}");
        }
        let composed = validate_operands(d, &layout, bindings, consts, cap, u64::MAX);
        assert_eq!(composed, alone, "{name}");
    }
}

/// すべての検査を通る入力は `Ok(())` で、容量の閉包は 1 回以上呼ばれる（BIND-55）。
#[test]
fn validate_operands_accepts_input_that_passes_every_check() {
    let d = [spec(0, DType::F32, &[4], &[1], 16)];
    let layout = BindingLayout::new(vec![0]);
    let bindings = [OperandBinding {
        slot: 0,
        offset: 16,
    }];
    let calls = Cell::new(0u32);
    let r = validate_operands(
        &d,
        &layout,
        &bindings,
        &[],
        |_| {
            calls.set(calls.get() + 1);
            Some(32)
        },
        4_000,
    );
    assert_eq!(r, Ok(()));
    assert!(calls.get() >= 1);
}

/// 引数と戻り値の型を署名ごとコンパイル時に固定する。
#[test]
fn validate_operands_return_type_is_pinned() {
    type Signature = fn(
        &[OperandSpec<'_>],
        &BindingLayout,
        &[OperandBinding],
        &[OperandConstant<'_>],
        Cap,
        u64,
    ) -> Result<(), ContractError>;
    let f: Signature = validate_operands::<Cap>;
    let r = f(
        &[],
        &BindingLayout::new(vec![]),
        &[],
        &[],
        |_| None,
        u64::MAX,
    );
    assert_eq!(r, Ok(()));
}

/// 3 つの関数（C1・C3、C4〜C8、続けて呼ぶ関数）が同じ入力でどれも `Ok(())` を返すことを確かめる。
fn assert_all_accept(
    declared: &[OperandSpec<'_>],
    layout: &BindingLayout,
    bindings: &[OperandBinding],
    consts: &[OperandConstant<'_>],
    cap: Cap,
    max: u64,
) {
    assert_eq!(validate_declared_operands(declared, layout), Ok(()));
    assert_eq!(
        validate_dispatch_operands(declared, bindings, consts, cap, max),
        Ok(())
    );
    assert_eq!(
        validate_operands(declared, layout, bindings, consts, cap, max),
        Ok(())
    );
}

/// 宣言 2 件と束縛 2 件の件数が一致する入力は 3 つの関数ともに通る（BIND-23）。
#[test]
fn sequential_checks_accept_matching_counts() {
    let d = [
        spec(0, DType::F32, &[4], &[1], 4),
        spec(1, DType::F32, &[4], &[1], 4),
    ];
    let layout = BindingLayout::new(vec![0, 1]);
    let bindings = [
        OperandBinding { slot: 0, offset: 0 },
        OperandBinding { slot: 1, offset: 0 },
    ];
    assert_all_accept(&d, &layout, &bindings, &[], |_| None, u64::MAX);
}

/// 容量が必要バイト数とちょうど等しい入力は 3 つの関数ともに通る（BIND-23）。
#[test]
fn sequential_checks_accept_exact_capacity() {
    let d = [spec(0, DType::F32, &[4], &[1], 4)];
    let layout = BindingLayout::new(vec![0]);
    let bindings = [OperandBinding { slot: 0, offset: 0 }];
    assert_all_accept(&d, &layout, &bindings, &[], |_| Some(16), u64::MAX);
}

/// alignment の倍数の offset が容量に収まる入力は 3 つの関数ともに通る（BIND-23）。
#[test]
fn sequential_checks_accept_aligned_offset() {
    let d = [spec(0, DType::F32, &[4], &[1], 16)];
    let layout = BindingLayout::new(vec![0]);
    let bindings = [OperandBinding {
        slot: 0,
        offset: 16,
    }];
    assert_all_accept(&d, &layout, &bindings, &[], |_| Some(1024), u64::MAX);
}

/// 定数の長さが宣言のバイト数と一致する入力は 3 つの関数ともに通る（BIND-23）。
#[test]
fn sequential_checks_accept_constant_length() {
    let d = [spec(0, DType::U32, &[3], &[1], 4)];
    let layout = BindingLayout::new(vec![0]);
    let bytes = [0u8; 12];
    let consts = [OperandConstant {
        slot: 0,
        bytes: &bytes,
    }];
    assert_all_accept(&d, &layout, &[], &consts, |_| None, u64::MAX);
}

/// 必要バイト数が上限とちょうど等しい入力は 3 つの関数ともに通る（BIND-23）。
#[test]
fn sequential_checks_accept_exact_limit() {
    let d = [spec(0, DType::F32, &[1000], &[1], 4)];
    let layout = BindingLayout::new(vec![0]);
    assert_all_accept(&d, &layout, &[], &[], |_| None, 4_000);
}

/// 行列積の A・B・C と次元の定数を宣言した入力は 3 つの関数ともに通る（BIND-23）。
#[test]
fn sequential_checks_accept_gemm() {
    let d = [
        spec(0, DType::F32, &[37, 53], &[53, 1], 4),
        spec(1, DType::F32, &[53, 41], &[41, 1], 4),
        spec(2, DType::F32, &[37, 41], &[41, 1], 4),
        spec(3, DType::U32, &[3], &[1], 4),
    ];
    let layout = BindingLayout::new(vec![0, 1, 2, 3]);
    let bindings = [
        OperandBinding { slot: 0, offset: 0 },
        OperandBinding { slot: 1, offset: 0 },
        OperandBinding { slot: 2, offset: 0 },
    ];
    let bytes = [0u8; 12];
    let consts = [OperandConstant {
        slot: 3,
        bytes: &bytes,
    }];
    assert_all_accept(&d, &layout, &bindings, &consts, |_| Some(1 << 20), u64::MAX);
}

/// 宣言・束縛・定数・容量を渡して `validate_dispatch_operands` を呼ぶ（上限は `u64::MAX`）。
fn dispatch(
    declared: &[OperandSpec<'_>],
    bindings: &[OperandBinding],
    consts: &[OperandConstant<'_>],
    cap: Cap,
) -> Result<(), ContractError> {
    validate_dispatch_operands(declared, bindings, consts, cap, u64::MAX)
}

/// 違反を 1 つずつ外していくと、返る拒否が C8 → C7 → C4 → C5 → C6 の順に進み、最後に `Ok(())` になる（BIND-45）。
#[test]
fn dispatch_checks_return_first_violation_in_order() {
    let cap16: Cap = |_| Some(16);
    let cap_none: Cap = |_| None;
    let bytes8 = [0u8; 8];
    let bytes12 = [0u8; 12];
    let c8_d = [
        spec(2, DType::F32, &[0], &[], 0),
        spec(0, DType::F32, &[100], &[], 16),
        spec(1, DType::U32, &[3], &[], 0),
    ];
    let fixed_d = [
        spec(2, DType::F32, &[1], &[], 0),
        spec(0, DType::F32, &[100], &[], 16),
        spec(1, DType::U32, &[3], &[], 0),
    ];
    let b9 = OperandBinding { slot: 9, offset: 0 };
    let b0_off4 = OperandBinding { slot: 0, offset: 4 };
    let b0_off16 = OperandBinding {
        slot: 0,
        offset: 16,
    };
    let c8_consts = [OperandConstant {
        slot: 1,
        bytes: &bytes8,
    }];
    let ok_consts = [OperandConstant {
        slot: 1,
        bytes: &bytes12,
    }];

    let r = dispatch(&c8_d, &[b9, b0_off4], &c8_consts, cap16);
    assert!(is_rejected_by(&r, "C8"), "{r:?}");
    let r = dispatch(&fixed_d, &[b9, b0_off4], &c8_consts, cap16);
    assert!(is_rejected_by(&r, "C7"), "{r:?}");
    let r = dispatch(&fixed_d, &[b0_off4], &c8_consts, cap16);
    assert!(is_rejected_by(&r, "C4"), "{r:?}");
    let r = dispatch(&fixed_d, &[b0_off4], &c8_consts, cap_none);
    assert!(is_rejected_by(&r, "C5"), "{r:?}");
    let r = dispatch(&fixed_d, &[b0_off16], &c8_consts, cap_none);
    assert!(is_rejected_by(&r, "C6"), "{r:?}");
    let r = dispatch(&fixed_d, &[b0_off16], &ok_consts, cap_none);
    assert_eq!(r, Ok(()));
}

/// 隣り合う 2 つの検査に同時に当たる入力では、前の検査の拒否だけが返る（BIND-45）。
#[test]
fn dispatch_checks_return_earlier_of_two_violations() {
    let bytes8 = [0u8; 8];

    // C8 と C7
    let d = [spec(0, DType::F32, &[0], &[], 0)];
    let r = dispatch(&d, &[OperandBinding { slot: 9, offset: 0 }], &[], |_| None);
    assert!(is_rejected_by(&r, "C8"), "{r:?}");

    // C7 と C4
    let d = [spec(0, DType::F32, &[100], &[], 0)];
    let bindings = [
        OperandBinding { slot: 9, offset: 0 },
        OperandBinding { slot: 0, offset: 0 },
    ];
    let r = dispatch(&d, &bindings, &[], |_| Some(4));
    assert!(is_rejected_by(&r, "C7"), "{r:?}");

    // C4 と C5
    let d = [spec(0, DType::F32, &[100], &[], 16)];
    let r = dispatch(&d, &[OperandBinding { slot: 0, offset: 4 }], &[], |_| {
        Some(16)
    });
    assert!(is_rejected_by(&r, "C4"), "{r:?}");

    // C5 と C6
    let d = [
        spec(0, DType::F32, &[4], &[], 16),
        spec(1, DType::U32, &[3], &[], 0),
    ];
    let consts = [OperandConstant {
        slot: 1,
        bytes: &bytes8,
    }];
    let r = dispatch(
        &d,
        &[OperandBinding { slot: 0, offset: 4 }],
        &consts,
        |_| None,
    );
    assert!(is_rejected_by(&r, "C5"), "{r:?}");
}

/// 同じ slot の宣言が複数あるとき、C8 は前後どちらの宣言の違反も拒否する（BIND-45）。
#[test]
fn duplicate_declarations_c8_checks_every_declaration() {
    let zero_last = [
        spec(0, DType::F32, &[1], &[], 0),
        spec(0, DType::F32, &[0], &[], 0),
    ];
    let zero_first = [
        spec(0, DType::F32, &[0], &[], 0),
        spec(0, DType::F32, &[1], &[], 0),
    ];
    for d in [&zero_last, &zero_first] {
        let r = dispatch(d, &[], &[], |_| None);
        assert!(is_rejected_by(&r, "C8"), "{r:?}");
    }
}

/// 同じ slot の宣言が複数あっても、C7 は宣言の重複を拒否せず、束縛の側の重複と未宣言だけを拒否する（BIND-45）。
#[test]
fn duplicate_declarations_c7_checks_every_declaration() {
    let d = [
        spec(0, DType::F32, &[1], &[], 0),
        spec(0, DType::F32, &[1], &[], 0),
        spec(1, DType::F32, &[1], &[], 0),
    ];
    let b = |slot| OperandBinding { slot, offset: 0 };
    let r = dispatch(&d, &[b(1)], &[], |_| None);
    assert_eq!(r, Ok(()));
    let r = dispatch(&d, &[b(5)], &[], |_| None);
    assert!(is_rejected_by(&r, "C7"), "{r:?}");
    let r = dispatch(&d, &[b(0), b(0)], &[], |_| None);
    assert!(is_rejected_by(&r, "C7"), "{r:?}");
}

/// 同じ slot の宣言が複数あるとき、C4 は最後の宣言で判定し、前後を入れ替えると結果も入れ替わる（BIND-45）。
#[test]
fn duplicate_declarations_c4_uses_last_declaration() {
    let small_then_large = [
        spec(0, DType::F32, &[1], &[], 0),
        spec(0, DType::F32, &[16], &[], 0),
    ];
    let large_then_small = [
        spec(0, DType::F32, &[16], &[], 0),
        spec(0, DType::F32, &[1], &[], 0),
    ];
    let bindings = [OperandBinding { slot: 0, offset: 0 }];
    let r = dispatch(&small_then_large, &bindings, &[], |_| Some(4));
    assert!(is_rejected_by(&r, "C4"), "{r:?}");
    let r = dispatch(&small_then_large, &bindings, &[], |_| Some(64));
    assert_eq!(r, Ok(()));
    let r = dispatch(&large_then_small, &bindings, &[], |_| Some(4));
    assert_eq!(r, Ok(()));
}

/// 同じ slot の宣言が複数あるとき、C5 は先頭の宣言で判定し、前後を入れ替えると結果も入れ替わる（BIND-45）。
#[test]
fn duplicate_declarations_c5_uses_first_declaration() {
    let aligned_first = [
        spec(0, DType::F32, &[1], &[], 16),
        spec(0, DType::F32, &[1], &[], 0),
    ];
    let aligned_last = [
        spec(0, DType::F32, &[1], &[], 0),
        spec(0, DType::F32, &[1], &[], 16),
    ];
    let bindings = [OperandBinding { slot: 0, offset: 4 }];
    let r = dispatch(&aligned_first, &bindings, &[], |_| None);
    assert!(is_rejected_by(&r, "C5"), "{r:?}");
    let r = dispatch(&aligned_last, &bindings, &[], |_| None);
    assert_eq!(r, Ok(()));
}

/// 同じ slot の宣言が複数あるとき、C6 は先頭の宣言の大きさで判定し、前後を入れ替えると結果も入れ替わる（BIND-45）。
#[test]
fn duplicate_declarations_c6_uses_first_declaration() {
    let small_then_large = [
        spec(0, DType::F32, &[1], &[], 0),
        spec(0, DType::F32, &[16], &[], 0),
    ];
    let large_then_small = [
        spec(0, DType::F32, &[16], &[], 0),
        spec(0, DType::F32, &[1], &[], 0),
    ];
    let bytes4 = [0u8; 4];
    let bytes64 = [0u8; 64];
    let c4 = [OperandConstant {
        slot: 0,
        bytes: &bytes4,
    }];
    let c64 = [OperandConstant {
        slot: 0,
        bytes: &bytes64,
    }];
    assert_eq!(dispatch(&small_then_large, &[], &c4, |_| None), Ok(()));
    let r = dispatch(&small_then_large, &[], &c64, |_| None);
    assert!(is_rejected_by(&r, "C6"), "{r:?}");
    assert_eq!(dispatch(&large_then_small, &[], &c64, |_| None), Ok(()));
    let r = dispatch(&large_then_small, &[], &c4, |_| None);
    assert!(is_rejected_by(&r, "C6"), "{r:?}");
}
