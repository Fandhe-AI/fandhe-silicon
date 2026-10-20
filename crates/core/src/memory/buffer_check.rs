//! バッファの引数検査の純粋関数（TASK-309 の 1/4。MEM-70・MEM-74〜MEM-76 のポインタ）。
//!
//! `memory` モジュールの一部。背後実装の `upload`（書き込み先の offset と長さ）・`download`
//! （読み出し元の offset と長さ）・確保が、背後実装の API を呼ぶ前にこの検査を通す。
//! 背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。

use crate::error::ContractError;

/// バッファの部分範囲 `[offset, offset + len)` が確保長に収まるかを判定する。
///
/// `upload` と `download` のどちらにも使える。`usize` は受けないので、`bytes.len()` の `u64` への
/// 変換は呼び出し側（背後実装）が行う。判定の順は、終端 `offset + len` が `u64` であふれる、
/// 終端が `allocation_len` を超える。終端が `allocation_len` ちょうどなら通る。
/// `len` が 0 でも、`offset` が `allocation_len` を超えていれば拒否する。
///
/// # Errors
///
/// 終端が `u64` であふれるか、`allocation_len` を超えるとき `ContractError::InvalidArgument`。
pub fn validate_buffer_range(
    offset: u64,
    len: u64,
    allocation_len: u64,
) -> Result<(), ContractError> {
    let Some(end) = offset.checked_add(len) else {
        return Err(ContractError::InvalidArgument(format!(
            "buffer range end overflows u64 (offset {offset}, len {len})"
        )));
    };
    if end > allocation_len {
        return Err(ContractError::InvalidArgument(format!(
            "buffer range end ({end}) exceeds allocation length ({allocation_len})"
        )));
    }
    Ok(())
}

/// 確保のバイト数が上限 `max_buffer_bytes` を超えないかを判定する。
///
/// 上限ちょうどは通す。この関数は上限だけを見る（0 バイトなどの他の条件は見ない）。
///
/// # Errors
///
/// `byte_len` が `max_buffer_bytes` より大きいとき `ContractError::InvalidArgument`。
pub fn validate_allocation_size(byte_len: u64, max_buffer_bytes: u64) -> Result<(), ContractError> {
    if byte_len > max_buffer_bytes {
        return Err(ContractError::InvalidArgument(format!(
            "allocation size ({byte_len}) exceeds max_buffer_bytes ({max_buffer_bytes})"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_invalid(r: Result<(), ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn buffer_check_range_within_allocation_passes() {
        assert!(validate_buffer_range(4, 8, 16).is_ok());
        assert!(validate_buffer_range(8, 8, 16).is_ok());
        assert!(validate_buffer_range(16, 0, 16).is_ok());
    }

    #[test]
    fn buffer_check_range_one_byte_over_is_rejected() {
        assert!(is_invalid(validate_buffer_range(8, 9, 16)));
        assert!(is_invalid(validate_buffer_range(17, 0, 16)));
    }

    #[test]
    fn buffer_check_range_end_overflow_is_rejected() {
        assert!(is_invalid(validate_buffer_range(u64::MAX, 1, u64::MAX)));
    }

    #[test]
    fn buffer_check_allocation_size_boundary() {
        assert!(validate_allocation_size(64, 64).is_ok());
        assert!(is_invalid(validate_allocation_size(65, 64)));
    }
}
