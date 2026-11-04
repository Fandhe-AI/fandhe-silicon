//! 4 つの入口が最初に呼ぶ、要素数と出力の数の長さの検査（`docs/spec/05-tasks.md` の TASK-924）。
//!
//! 要素数 `n` と出力の数 `m` から buffer ごとの必要バイト数を求め、不正なら
//! `Err(ContractError::InvalidArgument(..))` を返す純粋関数を持つ。device・buffer・列には触れない。
//! `Exec` の 4 つの入口（`entry.rs`）が、能力の問合せより前に呼ぶ。
//! host の形の列の長さと要素数の照合の関数も持つ（TASK-930）。
//! subgroup の総和の出力の数の関数も持つ（TASK-931）。
//! `atomic_add` の列の長さの照合にも使う（TASK-932）。

use crate::route::OpId;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::validate_allocation_size;

/// f16 の要素のバイト数。
const F16_BYTES: u64 = 2;
/// u32・f32 の要素のバイト数。
const WORD_BYTES: u64 = 4;
/// subgroup の総和の 1 ブロックの要素数。
const SUM_BLOCK_ELEMS: u64 = 256;

fn invalid(msg: &str) -> ContractError {
    ContractError::InvalidArgument(msg.to_string())
}

fn mul(a: u64, b: u64) -> Result<u64, ContractError> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("required byte length overflows u64"))
}

/// 列の長さ `len` が要素数 `expected` と同じかを調べる。違えば `Err(ContractError::InvalidArgument(..))`。
///
/// 長さの値だけを受け、列には触れない。`f16_fma` の host の形が 4 つの列それぞれに呼ぶ。
/// `subgroup_sum_u32`・`subgroup_sum_f32` の host の形は `x` に要素数、`out` に出力の数（`subgroup_output_len`）で呼ぶ。
/// `atomic_add` の host の形は `x` に要素数、`out` に出力の数 `m` で呼ぶ。
pub(crate) fn check_host_len(len: usize, expected: u64) -> Result<(), ContractError> {
    if u64::try_from(len) == Ok(expected) {
        Ok(())
    } else {
        Err(ContractError::InvalidArgument(format!(
            "host column length {len} does not match element count {expected}"
        )))
    }
}

/// `op` の `n`（と `AtomicAdd` の `m`）が、拒否される長さでないかを調べる。
///
/// `m` は `OpId::AtomicAdd` のときだけ `Some`。それ以外の組は `Err`。
/// 拒否は `n` が 0、`AtomicAdd` の `m` が 0、必要バイト数が `u64` であふれる、
/// 入力・出力のどちらかの buffer のバイト数が `max_buffer_bytes` を超える（ちょうどは通る）の 4 種。
pub(crate) fn check_lengths(
    op: OpId,
    n: u64,
    m: Option<u64>,
    max_buffer_bytes: u64,
) -> Result<(), ContractError> {
    match (op, m) {
        (OpId::AtomicAdd, Some(_)) => {}
        (OpId::AtomicAdd, None) => return Err(invalid("atomic_add requires an output count")),
        (_, Some(_)) => return Err(invalid("output count is only valid for atomic_add")),
        (_, None) => {}
    }
    if n == 0 {
        return Err(invalid("element count must be greater than zero"));
    }
    if m == Some(0) {
        return Err(invalid("output count must be greater than zero"));
    }
    let (input, output) = required_bytes(op, n, m)?;
    validate_allocation_size(input, max_buffer_bytes)?;
    validate_allocation_size(output, max_buffer_bytes)
}

/// subgroup の総和の出力の数（`n` を 256 で割って切り上げた数）を返す。
/// `required_bytes` と、subgroup の 2 つの入口の host の形の `out` の長さの照合が使う。
pub(crate) fn subgroup_output_len(n: u64) -> u64 {
    n.div_ceil(SUM_BLOCK_ELEMS)
}

/// `op` の入力・出力の buffer の必要バイト数を `(入力, 出力)` の順で求める。
/// `check_lengths` が長さの拒否の判定に使い、`u64` であふれるときは `Err` を返す。
fn required_bytes(op: OpId, n: u64, m: Option<u64>) -> Result<(u64, u64), ContractError> {
    Ok(match op {
        OpId::F16Fma => (mul(n, F16_BYTES)?, mul(n, F16_BYTES)?),
        OpId::SubgroupSumU32 | OpId::SubgroupSumF32 => (
            mul(n, WORD_BYTES)?,
            mul(subgroup_output_len(n), WORD_BYTES)?,
        ),
        OpId::AtomicAdd => (mul(n, WORD_BYTES)?, mul(m.unwrap_or(0), WORD_BYTES)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subgroup_output_len_counts_blocks() {
        assert_eq!(subgroup_output_len(1), 1);
        assert_eq!(subgroup_output_len(256), 1);
        assert_eq!(subgroup_output_len(257), 2);
        assert_eq!(subgroup_output_len(1000), 4);
        assert_eq!(subgroup_output_len(65_539), 257);
        assert_eq!(subgroup_output_len(u64::MAX), u64::MAX / 256 + 1);
    }

    #[test]
    fn host_len_matches_only_the_element_count() {
        assert!(check_host_len(0, 0).is_ok());
        assert!(check_host_len(4, 4).is_ok());
        assert!(check_host_len(usize::MAX, usize::MAX as u64).is_ok());
        for (len, n) in [(3, 4), (5, 4), (1, u64::MAX), (0, 1)] {
            assert!(matches!(
                check_host_len(len, n),
                Err(ContractError::InvalidArgument(_))
            ));
        }
    }

    const OPS: [OpId; 4] = [
        OpId::F16Fma,
        OpId::SubgroupSumU32,
        OpId::SubgroupSumF32,
        OpId::AtomicAdd,
    ];

    fn m_of(op: OpId) -> Option<u64> {
        (op == OpId::AtomicAdd).then_some(1)
    }

    fn rejected(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    fn elem(op: OpId) -> u64 {
        if op == OpId::F16Fma { 2 } else { 4 }
    }

    #[test]
    fn u64_max_length_is_rejected() {
        // FB-41
        for op in OPS {
            for limit in [u64::MAX, 1 << 28] {
                assert!(rejected(check_lengths(op, u64::MAX, m_of(op), limit)));
            }
        }
        assert!(rejected(check_lengths(
            OpId::AtomicAdd,
            1,
            Some(u64::MAX),
            u64::MAX
        )));
    }

    #[test]
    fn smallest_overflowing_length_is_rejected() {
        for op in OPS {
            let e = elem(op);
            let over = u64::MAX / e + 1;
            let ok = u64::MAX / e;
            assert!(rejected(check_lengths(op, over, m_of(op), u64::MAX)));
            if op != OpId::AtomicAdd {
                assert!(check_lengths(op, ok, None, u64::MAX).is_ok());
            }
        }
        assert!(rejected(check_lengths(
            OpId::AtomicAdd,
            1,
            Some(u64::MAX / 4 + 1),
            u64::MAX
        )));
        assert!(check_lengths(OpId::AtomicAdd, u64::MAX / 4, Some(1), u64::MAX).is_ok());
        assert!(check_lengths(OpId::AtomicAdd, 1, Some(u64::MAX / 4), u64::MAX).is_ok());
    }

    #[test]
    fn smallest_length_over_limit_is_rejected() {
        // FB-42
        for limit in [64u64, 65, 66, 67] {
            for op in OPS {
                let over = limit / elem(op) + 1;
                assert!(rejected(check_lengths(op, over, m_of(op), limit)));
                assert!(check_lengths(op, over - 1, m_of(op), limit).is_ok());
            }
            let over = limit / 4 + 1;
            assert!(rejected(check_lengths(
                OpId::AtomicAdd,
                1,
                Some(over),
                limit
            )));
            assert!(check_lengths(OpId::AtomicAdd, 1, Some(over - 1), limit).is_ok());
        }
    }

    #[test]
    fn zero_length_and_zero_outputs_are_rejected() {
        for op in OPS {
            assert!(rejected(check_lengths(op, 0, m_of(op), u64::MAX)));
        }
        assert!(rejected(check_lengths(
            OpId::AtomicAdd,
            8,
            Some(0),
            u64::MAX
        )));
        assert!(rejected(check_lengths(OpId::AtomicAdd, 8, None, u64::MAX)));
        for op in [OpId::F16Fma, OpId::SubgroupSumU32, OpId::SubgroupSumF32] {
            assert!(rejected(check_lengths(op, 8, Some(1), u64::MAX)));
        }
    }

    #[test]
    fn output_bytes_follow_each_op() {
        for op in [OpId::SubgroupSumU32, OpId::SubgroupSumF32] {
            for (n, out) in [(1, 4), (256, 4), (257, 8), (512, 8), (513, 12)] {
                assert_eq!(required_bytes(op, n, None).unwrap(), (n * 4, out));
            }
        }
        assert_eq!(required_bytes(OpId::F16Fma, 3, None).unwrap(), (6, 6));
        assert_eq!(
            required_bytes(OpId::AtomicAdd, 7, Some(5)).unwrap(),
            (28, 20)
        );
    }
}
