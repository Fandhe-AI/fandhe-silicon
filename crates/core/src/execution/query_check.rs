//! パイプライン統計のフラグの組、問合せの結果 1 つの大きさ、問合せの組の個数と解決先の検査（TASK-1877・CMD-127〜CMD-130 のポインタ）。
//!
//! `execution` から再公開される。背後実装の API を呼ばず、確保もしない（エラーの文字列を除く）。
//! フラグの組は 5 つのフラグの和集合で、立てた数・結果 1 件分の stride・各フラグの結果の中での位置を読める。
//! 検査の 2 つの関数は引数の値だけを見る純粋関数で、状態を持たない。

use core::ops::{BitOr, BitOrAssign};

use crate::error::ContractError;
use crate::memory::validate_buffer_range;

/// 問合せの結果 1 つのバイト数。
pub const QUERY_SIZE: u64 = 8;

/// 問合せの組に置ける問合せの個数の上限。
pub const QUERY_SET_MAX_QUERIES: u32 = 4096;

/// 解決先の offset が倍数であるべき値（バイト）。
pub const QUERY_RESOLVE_BUFFER_ALIGNMENT: u64 = 256;

/// 問合せの組の個数が 1 以上 [`QUERY_SET_MAX_QUERIES`] 以下かを検査する。
///
/// # Errors
///
/// `count` が 0、または [`QUERY_SET_MAX_QUERIES`] を超えるとき `ContractError::InvalidArgument`。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::execution::validate_query_set_count;
///
/// assert!(matches!(validate_query_set_count(0), Err(ContractError::InvalidArgument(_))));
/// assert!(matches!(validate_query_set_count(4097), Err(ContractError::InvalidArgument(_))));
/// assert_eq!(validate_query_set_count(1), Ok(()));
/// assert_eq!(validate_query_set_count(4096), Ok(()));
/// ```
pub fn validate_query_set_count(count: u32) -> Result<(), ContractError> {
    if count == 0 || count > QUERY_SET_MAX_QUERIES {
        return Err(ContractError::InvalidArgument(format!(
            "query set count ({count}) is not in 1..=4096"
        )));
    }
    Ok(())
}

/// 解決先の offset の整列、問合せの範囲、書き込みの長さを照らし、書くバイト数 `query_count × stride` を返す。
///
/// 範囲は `first_query + query_count` が `set_count` ちょうどでも通り、書き込みの終端が `dst_size` ちょうどでも通る。
/// `query_count` が 0 のときは、ほかの条件を満たせば 0 を返す。
///
/// # Errors
///
/// 次のいずれかのとき `ContractError::InvalidArgument`。
///
/// - `dst_offset` が [`QUERY_RESOLVE_BUFFER_ALIGNMENT`] の倍数でない
/// - `stride` が 0、または [`QUERY_SIZE`] の倍数でない
/// - `first_query + query_count` が `set_count` を超える
/// - `query_count × stride` が `u64` に収まらない
/// - 書き込みの終端が `u64` に収まらない、または `dst_size` を超える
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::execution::validate_query_resolve;
///
/// assert_eq!(validate_query_resolve(4096, 16, 0, 2, 512, 256), Ok(32));
/// assert!(matches!(
///     validate_query_resolve(4096, 16, 0, 2, 512, 255),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// assert!(matches!(
///     validate_query_resolve(4096, 16, 0, 2, 287, 256),
///     Err(ContractError::InvalidArgument(_))
/// ));
/// ```
pub fn validate_query_resolve(
    set_count: u32,
    stride: u64,
    first_query: u32,
    query_count: u32,
    dst_size: u64,
    dst_offset: u64,
) -> Result<u64, ContractError> {
    if !dst_offset.is_multiple_of(QUERY_RESOLVE_BUFFER_ALIGNMENT) {
        return Err(ContractError::InvalidArgument(format!(
            "query resolve offset ({dst_offset}) is not a multiple of 256"
        )));
    }
    if stride == 0 || !stride.is_multiple_of(QUERY_SIZE) {
        return Err(ContractError::InvalidArgument(format!(
            "query resolve stride ({stride}) is not a non-zero multiple of 8"
        )));
    }
    let in_range = u64::from(first_query)
        .checked_add(u64::from(query_count))
        .is_some_and(|end| end <= u64::from(set_count));
    if !in_range {
        return Err(ContractError::InvalidArgument(format!(
            "query range (first {first_query}, count {query_count}) exceeds query set count ({set_count})"
        )));
    }
    let Some(bytes) = u64::from(query_count).checked_mul(stride) else {
        return Err(ContractError::InvalidArgument(format!(
            "query resolve size overflows u64 (count {query_count}, stride {stride})"
        )));
    };
    validate_buffer_range(dst_offset, bytes, dst_size)?;
    Ok(bytes)
}

/// パイプライン統計の 5 つのフラグの組。
///
/// 外から作れるのは各定数・`empty()`・`|`・`|=` で、生のビットから作る口は無く、5 つ以外のビットを持つ値は作れない。
///
/// ```
/// use fandhe_silicon_core::execution::PipelineStatisticsFlags;
///
/// let f = PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT
///     | PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS;
/// assert_eq!(f.count(), 2);
/// assert_eq!(f.stride(), 16);
/// assert_eq!(f.position(PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT), Some(0));
/// assert_eq!(f.position(PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS), Some(1));
/// assert_eq!(f.position(PipelineStatisticsFlags::VERTEX_SHADER_INVOCATIONS), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineStatisticsFlags(u8);

impl PipelineStatisticsFlags {
    /// 頂点シェーダの呼び出し回数。
    pub const VERTEX_SHADER_INVOCATIONS: Self = Self(1 << 0);
    /// クリッパの呼び出し回数。
    pub const CLIPPER_INVOCATIONS: Self = Self(1 << 1);
    /// クリッパが出力したプリミティブの数。
    pub const CLIPPER_PRIMITIVES_OUT: Self = Self(1 << 2);
    /// フラグメントシェーダの呼び出し回数。
    pub const FRAGMENT_SHADER_INVOCATIONS: Self = Self(1 << 3);
    /// コンピュートシェーダの呼び出し回数。
    pub const COMPUTE_SHADER_INVOCATIONS: Self = Self(1 << 4);

    /// どのフラグも含まない値。
    pub const fn empty() -> Self {
        Self(0)
    }

    /// どのフラグも含まないとき `true`。
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// ビット値を返す。
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// `other` のビットをすべて含むとき `true`。
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// 立てたフラグの数（0〜5）を返す。
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// 問合せ 1 件分の結果のバイト数（立てたフラグの数 × `QUERY_SIZE`）を返す。空なら 0。
    #[must_use]
    pub const fn stride(self) -> u64 {
        self.count() as u64 * QUERY_SIZE
    }

    /// `flag` の結果が 1 件分の結果の中で何番目か（0 始まり）を返す。
    ///
    /// 立てたフラグのうち小さいビットから数える。`flag` が 1 つのフラグでないか、含まれないときは `None`。
    pub const fn position(self, flag: Self) -> Option<u32> {
        if flag.0.count_ones() == 1 && self.contains(flag) {
            Some((self.0 & (flag.0 - 1)).count_ones())
        } else {
            None
        }
    }
}

impl BitOr for PipelineStatisticsFlags {
    type Output = Self;

    /// 2 つのフラグの組の和集合を返す。
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for PipelineStatisticsFlags {
    /// フラグを足す。
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [PipelineStatisticsFlags; 5] = [
        PipelineStatisticsFlags::VERTEX_SHADER_INVOCATIONS,
        PipelineStatisticsFlags::CLIPPER_INVOCATIONS,
        PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT,
        PipelineStatisticsFlags::FRAGMENT_SHADER_INVOCATIONS,
        PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS,
    ];

    #[test]
    fn five_constants_are_distinct_single_bits() {
        for (i, a) in ALL.iter().enumerate() {
            assert_eq!(a.bits().count_ones(), 1);
            for b in &ALL[i + 1..] {
                assert_ne!(a.bits(), b.bits());
            }
        }
    }

    #[test]
    fn empty_has_zero_count_and_stride() {
        let e = PipelineStatisticsFlags::empty();
        assert!(e.is_empty());
        assert_eq!(e.count(), 0);
        assert_eq!(e.stride(), 0);
    }

    #[test]
    fn all_flags_have_count_five_and_stride_forty() {
        let mut all = PipelineStatisticsFlags::empty();
        for f in ALL {
            all |= f;
        }
        assert_eq!(all.count(), 5);
        assert_eq!(all.stride(), 40);
        for (i, f) in ALL.iter().enumerate() {
            assert_eq!(all.position(*f), Some(i as u32));
        }
    }

    #[test]
    fn two_flags_report_count_stride_and_positions() {
        let f = PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT
            | PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS;
        assert_eq!(f.count(), 2);
        assert_eq!(f.stride(), 16);
        assert_eq!(
            f.position(PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT),
            Some(0)
        );
        assert_eq!(
            f.position(PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS),
            Some(1)
        );
    }

    #[test]
    fn position_is_none_for_empty_multi_bit_or_absent_flag() {
        let f = PipelineStatisticsFlags::CLIPPER_INVOCATIONS
            | PipelineStatisticsFlags::FRAGMENT_SHADER_INVOCATIONS;
        assert_eq!(f.position(PipelineStatisticsFlags::empty()), None);
        assert_eq!(f.position(f), None);
        assert_eq!(
            f.position(PipelineStatisticsFlags::VERTEX_SHADER_INVOCATIONS),
            None
        );
    }

    fn is_invalid(r: Result<u64, ContractError>, head: &str) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(head))
    }

    #[test]
    fn set_count_accepts_1_and_4096_rejects_0_and_4097() {
        assert_eq!(validate_query_set_count(1), Ok(()));
        assert_eq!(validate_query_set_count(4096), Ok(()));
        for c in [0, 4097, u32::MAX] {
            assert!(matches!(
                validate_query_set_count(c),
                Err(ContractError::InvalidArgument(m)) if m.starts_with("query set count")
            ));
        }
    }

    #[test]
    fn resolve_returns_bytes_for_valid_input() {
        assert_eq!(validate_query_resolve(4096, 16, 0, 2, 512, 256), Ok(32));
        assert_eq!(validate_query_resolve(4096, 16, 0, 2, 288, 256), Ok(32));
        assert_eq!(validate_query_resolve(4, 8, 2, 2, 8192, 0), Ok(16));
        assert_eq!(validate_query_resolve(4, 8, 0, 0, 256, 256), Ok(0));
    }

    #[test]
    fn resolve_rejects_each_single_fault() {
        assert!(is_invalid(
            validate_query_resolve(4096, 16, 0, 2, 512, 255),
            "query resolve offset"
        ));
        assert!(is_invalid(
            validate_query_resolve(4096, 0, 0, 2, 512, 256),
            "query resolve stride"
        ));
        assert!(is_invalid(
            validate_query_resolve(4096, 12, 0, 2, 512, 256),
            "query resolve stride"
        ));
        assert!(is_invalid(
            validate_query_resolve(4, 8, 3, 2, 8192, 0),
            "query range"
        ));
        assert!(is_invalid(
            validate_query_resolve(u32::MAX, u64::MAX - 7, 0, u32::MAX, u64::MAX, 0),
            "query resolve size"
        ));
        assert!(is_invalid(
            validate_query_resolve(64, 8, 0, 40, u64::MAX, u64::MAX - 255),
            "buffer range"
        ));
        assert!(is_invalid(
            validate_query_resolve(4096, 16, 0, 2, 287, 256),
            "buffer range"
        ));
    }
}
