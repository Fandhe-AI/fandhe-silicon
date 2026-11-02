//! 経路・操作の識別子・使えない理由の型。
//!
//! 下-2 の入口が「どの経路で実行するか」「どの操作か」「使えないときの理由」を
//! 表すための値で、`opt_in` の印と合わせて経路の選択と報告に使われる。

use fandhe_silicon_core::error::UnsupportedReason;

/// 操作を実行する経路。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// device の機能をそのまま使う経路。
    Native,
    /// GPU 上の代わりの実行の経路。
    GpuEmulated,
    /// CPU での代わりの実行の経路。
    Cpu,
}

/// 経路を選ぶ対象の操作の識別子。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OpId {
    /// f16 の融合積和。
    F16Fma,
    /// u32 の subgroup 総和。
    SubgroupSumU32,
    /// f32 の subgroup 総和。
    SubgroupSumF32,
    /// アトミックな加算。
    AtomicAdd,
}

/// 経路が使えない理由。FB-23。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnavailableReason {
    /// 能力の問合せが非対応を返した。理由があればそのまま持つ。
    CapabilityUnsupported(Option<UnsupportedReason>),
    /// 能力の問合せが結果を返せなかった。
    CapabilityUnknown,
    /// 代わりの実行を許す印が渡されていない。
    OptInNotGiven,
    /// 代わりの実行では意味を保てない。
    SemanticsNotPreservable,
    /// f32 の非正規化数の扱いが分からない。
    F32DenormalBehaviorUnknown,
}

#[cfg(test)]
mod tests {
    use super::{OpId, Route};

    #[test]
    fn routes_are_distinct() {
        let all = [Route::Native, Route::GpuEmulated, Route::Cpu];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                assert_eq!(i == j, a == b);
            }
        }
    }

    #[test]
    fn op_ids_are_distinct() {
        let all = [
            OpId::F16Fma,
            OpId::SubgroupSumU32,
            OpId::SubgroupSumF32,
            OpId::AtomicAdd,
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                assert_eq!(i == j, a == b);
            }
        }
    }
}
