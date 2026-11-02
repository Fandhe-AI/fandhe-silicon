//! パイプライン統計のフラグの組と、問合せの結果 1 つの大きさ（TASK-1877・CMD-127〜CMD-130 のポインタ）。
//!
//! `execution` から再公開される。背後実装の API を呼ばず、確保もしない `const fn` だけを置く。
//! フラグの組は 5 つのフラグの和集合で、立てた数・結果 1 件分の stride・各フラグの結果の中での位置を読める。

use core::ops::{BitOr, BitOrAssign};

/// 問合せの結果 1 つのバイト数。
pub const QUERY_SIZE: u64 = 8;

/// パイプライン統計の 5 つのフラグの組。
///
/// 外から作れるのは各定数と `|` だけで、5 つ以外のビットを持つ値は作れない。
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
}
