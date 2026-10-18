//! dispatch に渡す引数の宣言を、背後実装を呼ぶ前に確かめる純粋関数（TASK-421 のポインタ。BIND-35〜BIND-37）。
//!
//! `execution` から再公開される。上や下-2 が組み立てた宣言を、下-1 の背後実装へ渡す前に
//! 共通の層で拒否するために置く。今は C8（長さ 0・`u64` のあふれ・上限超過）だけを判定する。
//! 背後実装・ファイル・時刻には触れず、バッファも確保しない（確保が起きるのはエラーの文字列だけ）。

use super::OperandSpec;

/// 宣言の検査が返すエラー。
///
/// 検査の入力の不正を表す暫定の型で、契約のエラー型そのものではない。
/// 文言は英語で、C8 の違反は先頭が `C8:` になる。
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

/// dispatch の引数の宣言を検査する。今は C8 だけを判定する。
///
/// `declared` を先頭から順に見て、宣言ごとに (1) `shape` に 0 の次元がある、
/// (2) 必要バイト数が `u64` をあふれる、(3) 必要バイト数が `max_buffer_bytes` より大きい、
/// の順で調べ、最初に当たった違反で `InvalidArgument`（先頭 `C8:`）を返す。上限ちょうどは通す。
/// C7・C4〜C6 は後から同じ関数に足す（C8 が先頭）。
///
/// # Errors
///
/// 上記のいずれかに当たると [`OperandCheckError::InvalidArgument`] を返す。
pub fn validate_dispatch_operands<F>(
    declared: &[OperandSpec<'_>],
    // 以降の 3 引数は C7（TASK-422）・C4〜C6（TASK-423）が読む。C8 では未使用。
    _bindings: &[OperandBinding],
    _consts: &[OperandConstant<'_>],
    _binding_capacity: F,
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
}
