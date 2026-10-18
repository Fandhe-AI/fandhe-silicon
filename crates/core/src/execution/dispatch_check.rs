//! dispatch に渡す引数の宣言を、背後実装を呼ぶ前に確かめる純粋関数（TASK-421・TASK-423 のポインタ。BIND-35〜BIND-37・BIND-42〜BIND-44）。
//!
//! `execution` から再公開される。上や下-2 が組み立てた宣言を、下-1 の背後実装へ渡す前に
//! 共通の層で拒否するために置く。C4（残容量）・C5（offset の境界）・C6（定数の長さ）・C8（長さ 0・`u64` のあふれ・上限超過）を判定する。
//! 背後実装・ファイル・時刻には触れず、バッファも確保しない（確保が起きるのはエラーの文字列だけ）。

use super::OperandSpec;

/// 宣言の検査が返すエラー。
///
/// 検査の入力の不正を表す暫定の型で、契約のエラー型そのものではない。
/// 文言は英語で、違反は先頭が `C4:`・`C5:`・`C6:`・`C8:` のいずれかになる。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OperandCheckError {
    /// 入力の宣言が不正（文言を持つ）。
    InvalidArgument(String),
}

/// 検査に使う束縛 1 件（slot と offset）。
///
/// 検査の入力のための形で、背後実装へ渡す束縛の型は別のタスクが定める（TASK-308・TASK-419 のポインタ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandBinding {
    /// 束縛の番号。
    pub slot: u32,
    /// バッファ内の offset（バイト）。
    pub offset: u64,
}

/// 検査に使う定数 1 件（slot とバイト列）。
///
/// 検査の入力のための形で、バイト列は借用で持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandConstant<'a> {
    /// 束縛の番号。
    pub slot: u32,
    /// 定数のバイト列。
    pub bytes: &'a [u8],
}

/// 宣言 1 件の必要バイト数を返す。`u64` であふれたら `None`。
///
/// 1 要素のバイト数に各次元の要素数を `checked_mul` で掛ける。
/// `shape` が空のときは次元の積を 1 とし、1 要素分のバイト数になる。
#[must_use]
pub fn required_bytes(spec: &OperandSpec<'_>) -> Option<u64> {
    spec.shape
        .iter()
        .try_fold(spec.dtype.byte_size(), |acc, &d| acc.checked_mul(d))
}

/// dispatch の引数の宣言を検査する。C4・C5・C6・C8 を判定する。
///
/// 段ごとに全件を見て、最初に当たった違反で `InvalidArgument` を返す。段の順は次のとおり。
///
/// 1. C8: `declared` を先頭から順に、(1) `shape` に 0 の次元がある、(2) 必要バイト数が `u64` を
///    あふれる、(3) 必要バイト数が `max_buffer_bytes` より大きい、の順で調べる。上限ちょうどは通す。
/// 2. C4: `bindings` のうち `binding_capacity` が容量を返すものについて、必要バイト数が
///    容量から `offset` を引いた残り（引き切れなければ 0）より大きければ拒否する。容量が `None` の束縛は見ない。
/// 3. C5: `bindings` のうち、宣言の `alignment` が 0 でないものについて、`offset` が
///    `alignment` の倍数でなければ拒否する。
/// 4. C6: `consts` のバイト列の長さが宣言の必要バイト数と異なれば（短くても長くても）拒否する。
///
/// 宣言の無い `slot` の束縛・定数は、ここでは見ない。
///
/// # Errors
///
/// 上記のいずれかに当たると [`OperandCheckError::InvalidArgument`]（先頭 `C8:`・`C4:`・`C5:`・`C6:`）を返す。
pub fn validate_dispatch_operands<F>(
    declared: &[OperandSpec<'_>],
    bindings: &[OperandBinding],
    consts: &[OperandConstant<'_>],
    binding_capacity: F,
    max_buffer_bytes: u64,
) -> Result<(), OperandCheckError>
where
    F: Fn(&OperandBinding) -> Option<u64>,
{
    for spec in declared {
        let slot = spec.slot;
        if spec.shape.contains(&0) {
            return Err(OperandCheckError::InvalidArgument(format!(
                "C8: slot {slot}: shape contains a zero-length dimension"
            )));
        }
        let Some(req) = required_bytes(spec) else {
            return Err(OperandCheckError::InvalidArgument(format!(
                "C8: slot {slot}: required byte size overflows u64"
            )));
        };
        if req > max_buffer_bytes {
            return Err(OperandCheckError::InvalidArgument(format!(
                "C8: slot {slot}: required bytes ({req}) exceed max_buffer_bytes ({max_buffer_bytes})"
            )));
        }
    }
    // C7（TASK-422 のポインタ）の入る位置。
    check_capacity(declared, bindings, &binding_capacity)?;
    check_alignment(declared, bindings)?;
    check_constant_sizes(declared, consts)
}

/// C4: 容量が分かる束縛について、必要バイト数が残り（容量 - offset）に収まるかを調べる。
///
/// `validate_dispatch_operands` から C8 を通った後に呼ばれる。同じ `slot` の宣言が複数あるときは後ろのものを使う。
fn check_capacity<F>(
    declared: &[OperandSpec<'_>],
    bindings: &[OperandBinding],
    binding_capacity: &F,
) -> Result<(), OperandCheckError>
where
    F: Fn(&OperandBinding) -> Option<u64>,
{
    for b in bindings {
        let Some(spec) = declared.iter().rev().find(|s| s.slot == b.slot) else {
            continue;
        };
        let Some(cap) = binding_capacity(b) else {
            continue;
        };
        let Some(req) = required_bytes(spec) else {
            continue;
        };
        let remaining = cap.saturating_sub(b.offset);
        if req > remaining {
            let slot = b.slot;
            return Err(OperandCheckError::InvalidArgument(format!(
                "C4: slot {slot}: required bytes ({req}) exceed remaining buffer capacity ({remaining})"
            )));
        }
    }
    Ok(())
}

/// C5: 束縛の `offset` が宣言の `alignment` の倍数かを調べる。`alignment` が 0 の宣言は見ない。
///
/// 剰余は 0 の境界を除いた後にだけ取るので、0 除算は起きない。
fn check_alignment(
    declared: &[OperandSpec<'_>],
    bindings: &[OperandBinding],
) -> Result<(), OperandCheckError> {
    for b in bindings {
        let Some(spec) = declared.iter().find(|s| s.slot == b.slot) else {
            continue;
        };
        if spec.alignment == 0 {
            continue;
        }
        let alignment = u64::from(spec.alignment);
        if b.offset % alignment != 0 {
            let (slot, offset) = (b.slot, b.offset);
            return Err(OperandCheckError::InvalidArgument(format!(
                "C5: slot {slot}: offset ({offset}) is not a multiple of alignment ({alignment})"
            )));
        }
    }
    Ok(())
}

/// C6: 定数のバイト列の長さが宣言の必要バイト数と一致するかを調べる。
fn check_constant_sizes(
    declared: &[OperandSpec<'_>],
    consts: &[OperandConstant<'_>],
) -> Result<(), OperandCheckError> {
    for c in consts {
        let Some(spec) = declared.iter().find(|s| s.slot == c.slot) else {
            continue;
        };
        let Some(req) = required_bytes(spec) else {
            continue;
        };
        if u64::try_from(c.bytes.len()) != Ok(req) {
            let (slot, len) = (c.slot, c.bytes.len());
            return Err(OperandCheckError::InvalidArgument(format!(
                "C6: slot {slot}: constant byte length ({len}) differs from expected bytes ({req})"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::DType;

    fn spec(dtype: DType, shape: &[u64]) -> OperandSpec<'_> {
        OperandSpec {
            slot: 0,
            dtype,
            shape,
            stride: &[],
            alignment: 4,
        }
    }

    fn check(declared: &[OperandSpec<'_>], max: u64) -> Result<(), OperandCheckError> {
        validate_dispatch_operands(declared, &[], &[], |_| None, max)
    }

    fn is_c8(r: &Result<(), OperandCheckError>) -> bool {
        matches!(r, Err(OperandCheckError::InvalidArgument(m)) if m.starts_with("C8:"))
    }

    #[test]
    fn dispatch_check_required_bytes_values() {
        assert_eq!(required_bytes(&spec(DType::F32, &[3])), Some(12));
        assert_eq!(required_bytes(&spec(DType::F16, &[2, 2])), Some(8));
        assert_eq!(required_bytes(&spec(DType::Bool, &[])), Some(1));
        assert_eq!(required_bytes(&spec(DType::F32, &[u64::MAX / 2, 4])), None);
    }

    #[test]
    fn dispatch_check_rejects_zero_length_dimension() {
        assert!(is_c8(&check(&[spec(DType::F32, &[0])], u64::MAX)));
        assert!(is_c8(&check(&[spec(DType::F32, &[37, 0])], u64::MAX)));
        // 0 の次元があれば、ほかの次元があふれる大きさでも長さ 0 として拒否する。
        assert!(is_c8(&check(&[spec(DType::F32, &[u64::MAX, 0])], u64::MAX)));
    }

    #[test]
    fn dispatch_check_rejects_u64_overflow() {
        assert!(is_c8(&check(
            &[spec(DType::F32, &[u64::MAX / 2, 4])],
            u64::MAX
        )));
    }

    #[test]
    fn dispatch_check_enforces_max_buffer_bytes() {
        let s = [spec(DType::F32, &[1000])];
        assert!(is_c8(&check(&s, 3999)));
        assert_eq!(check(&s, 4000), Ok(()));
    }

    #[test]
    fn dispatch_check_accepts_empty_declaration() {
        assert_eq!(check(&[], 0), Ok(()));
    }

    #[test]
    fn dispatch_check_reports_first_violation() {
        let a = [3u64];
        let b = [0u64];
        let declared = [
            OperandSpec {
                slot: 1,
                ..spec(DType::F32, &a)
            },
            OperandSpec {
                slot: 2,
                ..spec(DType::F32, &b)
            },
        ];
        let r = check(&declared, u64::MAX);
        assert!(matches!(&r, Err(OperandCheckError::InvalidArgument(m)) if m.contains("slot 2")));
    }

    fn code(r: &Result<(), OperandCheckError>) -> String {
        match r {
            Err(OperandCheckError::InvalidArgument(m)) => m.chars().take(2).collect(),
            _ => String::new(),
        }
    }

    fn run(
        declared: &[OperandSpec<'_>],
        bindings: &[OperandBinding],
        consts: &[OperandConstant<'_>],
        cap: Option<u64>,
    ) -> Result<(), OperandCheckError> {
        validate_dispatch_operands(declared, bindings, consts, |_| cap, u64::MAX)
    }

    fn bind(offset: u64) -> [OperandBinding; 1] {
        [OperandBinding { slot: 0, offset }]
    }

    #[test]
    fn dispatch_check_c4_rejects_insufficient_capacity() {
        let big = [spec(DType::F32, &[100])];
        assert_eq!(code(&run(&big, &bind(0), &[], Some(16))), "C4");
        let ok = [spec(DType::F32, &[4])];
        assert_eq!(run(&ok, &bind(0), &[], Some(16)), Ok(()));
    }

    #[test]
    fn dispatch_check_c4_skips_unknown_capacity() {
        let big = [spec(DType::F32, &[100])];
        assert_eq!(run(&big, &bind(0), &[], None), Ok(()));
    }

    #[test]
    fn dispatch_check_c4_subtracts_offset() {
        let s = [spec(DType::F32, &[4])];
        assert_eq!(code(&run(&s, &bind(4), &[], Some(16))), "C4");
        let one = [spec(DType::F32, &[1])];
        assert_eq!(code(&run(&one, &bind(1000), &[], Some(16))), "C4");
    }

    #[test]
    fn dispatch_check_c5_rejects_misaligned_offset() {
        let s = [OperandSpec {
            alignment: 16,
            ..spec(DType::F32, &[1])
        }];
        assert_eq!(code(&run(&s, &bind(4), &[], None)), "C5");
        assert_eq!(run(&s, &bind(16), &[], None), Ok(()));
    }

    #[test]
    fn dispatch_check_c5_skips_zero_alignment() {
        let s = [OperandSpec {
            alignment: 0,
            ..spec(DType::F32, &[1])
        }];
        assert_eq!(run(&s, &bind(4), &[], None), Ok(()));
    }

    #[test]
    fn dispatch_check_c6_rejects_constant_size_mismatch() {
        let s = [spec(DType::U32, &[3])];
        let c = |bytes| [OperandConstant { slot: 0, bytes }];
        assert_eq!(code(&run(&s, &[], &c(&[0u8; 8]), None)), "C6");
        assert_eq!(code(&run(&s, &[], &c(&[0u8; 16]), None)), "C6");
        assert_eq!(run(&s, &[], &c(&[0u8; 12]), None), Ok(()));
    }

    #[test]
    fn dispatch_check_checks_c8_before_c4_c5_c6() {
        // C8 と C4 の両方に当たる入力は C8。
        let zero = [spec(DType::F32, &[0])];
        assert_eq!(code(&run(&zero, &bind(0), &[], Some(0))), "C8");
        // C4 と C5 の両方に当たる入力は C4。
        let s = [OperandSpec {
            alignment: 16,
            ..spec(DType::F32, &[100])
        }];
        assert_eq!(code(&run(&s, &bind(4), &[], Some(16))), "C4");
        // C5 と C6 の両方に当たる入力は C5。
        let s = [OperandSpec {
            alignment: 16,
            ..spec(DType::U32, &[3])
        }];
        let c = [OperandConstant {
            slot: 0,
            bytes: &[0u8; 8],
        }];
        assert_eq!(code(&run(&s, &bind(4), &c, None)), "C5");
    }

    #[test]
    fn dispatch_check_ignores_undeclared_slots() {
        let s = [spec(DType::F32, &[1])];
        let b = [OperandBinding { slot: 9, offset: 3 }];
        let c = [OperandConstant {
            slot: 9,
            bytes: &[0u8; 1],
        }];
        assert_eq!(run(&s, &b, &c, Some(0)), Ok(()));
    }
}
