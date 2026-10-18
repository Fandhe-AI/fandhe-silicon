//! 代わりの実行の結果を比べる新しい比べ方の前段と、ビット同一の条件・binary16 の順序の差の条件・f32 の和の相対の条件（spec TASK-928 の 2/8・3/8・4/8）。
//!
//! 代わりの実行の結果を CPU 参照と比べる側（検証用のテスト・測定）から呼ばれる。
//! feature `test-support` の裏にだけ公開される。標準ライブラリだけで書き、確保も panic もしない。
//! 要素ごとの順序（NaN、無限大、零）は `special_values` の判定をそのまま使い、作り直さない。
//! その順序で決まらない要素だけを数値の条件へ回す。ここにあるのは前段、ビット同一の条件、binary16 の順序の差の条件、f32 の和の相対の条件（と余裕の記録）だけ。
//! 置き場・名前・振る舞いの説明は `docs/parity-fallback-compare.md`。
//! spec との対応は TASK-928、`04-behavior/fallback-execution.md` の FB-31・FB-32・FB-35・FB-37 のポインタのみ。

use super::binary16::binary16_bits_to_f32;
use super::special_values::{SpecialVerdict, judge_element_with_special_values};
use super::tolerance::{CompareError, T1};

/// 順序の前段の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderVerdict {
    /// 両方が NaN（payload と符号ビットは比べない）で一致とした。
    BothNan,
    /// 同じ符号の無限大どうしで一致とした。
    SameInfinity,
    /// 有限でない値が絡み、上の二つに当たらない不一致。
    NonFiniteMismatch,
    /// 両方が零（符号は問わない）で一致とした。
    BothZero,
    /// 前段では決まらず、数値の条件へ回す。
    Numeric,
}

impl OrderVerdict {
    /// 前段で結果が決まったときだけ真（`Numeric` 以外）。
    pub const fn is_decided(self) -> bool {
        !matches!(self, Self::Numeric)
    }

    /// 前段で一致と決まった区分のときだけ真。`Numeric` は一致ではない。
    pub const fn is_match(self) -> bool {
        matches!(self, Self::BothNan | Self::SameInfinity | Self::BothZero)
    }
}

/// f32 の 1 要素に順序の前段をかける。
///
/// 有限でない値の 3 区分は `judge_element_with_special_values` に従う（閾値の値によらない）。
/// 両方が零なら符号を問わず `BothZero`。それ以外の有限どうしは `Numeric`。
pub fn judge_order_f32(reference: f32, actual: f32) -> OrderVerdict {
    match judge_element_with_special_values(reference, actual, T1) {
        SpecialVerdict::BothNan => OrderVerdict::BothNan,
        SpecialVerdict::SameInfinity => OrderVerdict::SameInfinity,
        SpecialVerdict::NonFiniteMismatch => OrderVerdict::NonFiniteMismatch,
        SpecialVerdict::Within | SpecialVerdict::Exceeded => {
            if reference == 0.0 && actual == 0.0 {
                OrderVerdict::BothZero
            } else {
                OrderVerdict::Numeric
            }
        }
    }
}

/// binary16 のビット列の 1 要素に順序の前段をかける。
///
/// 正確に f32 へ広げてから [`judge_order_f32`] に通す（NaN・無限大・零の符号は広げても保たれる）。
pub fn judge_order_binary16(reference: u16, actual: u16) -> OrderVerdict {
    judge_order_f32(
        binary16_bits_to_f32(reference),
        binary16_bits_to_f32(actual),
    )
}

/// 代わりの実行の 1 要素の比べの結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackVerdict {
    /// 両方が NaN で一致とした。
    BothNan,
    /// 同じ符号の無限大どうしで一致とした。
    SameInfinity,
    /// 有限でない値が絡む不一致。
    NonFiniteMismatch,
    /// 両方が零（符号は問わない）で一致とした。
    BothZero,
    /// ビット列が同一で一致とした。
    BitIdentical,
    /// ビット列が異なり、不一致とした。
    BitsDiffer,
    /// binary16 の順序の差が閾値以下で一致とした（ビット同一の組も含む）。
    WithinOrderDistance,
    /// binary16 の順序の差が閾値を超え、不一致とした。
    OrderDistanceExceeded,
    /// f32 の和の誤差が、要素の絶対値の和に対する相対の閾値以下で一致とした。
    WithinRelativeSum,
    /// f32 の和の誤差が相対の閾値を超えた（または Σ|x| が使えない値）ため不一致とした。
    RelativeSumExceeded,
}

impl FallbackVerdict {
    /// 一致として数える区分のときだけ真。
    pub const fn is_match(self) -> bool {
        matches!(
            self,
            Self::BothNan
                | Self::SameInfinity
                | Self::BothZero
                | Self::BitIdentical
                | Self::WithinOrderDistance
                | Self::WithinRelativeSum
        )
    }
}

/// u32 の 1 要素をビット同一で比べる。整数なので順序の前段は通さない。
pub fn judge_bits_u32(reference: u32, actual: u32) -> FallbackVerdict {
    if reference == actual {
        FallbackVerdict::BitIdentical
    } else {
        FallbackVerdict::BitsDiffer
    }
}

/// f32 の 1 要素を、順序の前段のあとでビット同一かどうかで比べる。
///
/// 前段で決まれば、その区分に写す。決まらなければ（零は前段で決まるので符号つきの零は残らない）
/// `to_bits` が等しいときだけ一致。
pub fn judge_bits_f32(reference: f32, actual: f32) -> FallbackVerdict {
    match judge_order_f32(reference, actual) {
        OrderVerdict::BothNan => FallbackVerdict::BothNan,
        OrderVerdict::SameInfinity => FallbackVerdict::SameInfinity,
        OrderVerdict::NonFiniteMismatch => FallbackVerdict::NonFiniteMismatch,
        OrderVerdict::BothZero => FallbackVerdict::BothZero,
        OrderVerdict::Numeric => judge_bits_u32(reference.to_bits(), actual.to_bits()),
    }
}

/// 列どうしの比べの集計。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackComparison {
    compared: usize,
    mismatched: usize,
    first_mismatch: Option<usize>,
}

impl FallbackComparison {
    /// 比べた要素数。
    pub fn compared(&self) -> usize {
        self.compared
    }

    /// 不一致だった要素数。
    pub fn mismatched(&self) -> usize {
        self.mismatched
    }

    /// 最初に不一致だった要素の添字。無ければ `None`。
    pub fn first_mismatch(&self) -> Option<usize> {
        self.first_mismatch
    }

    /// 1 要素も不一致でないときだけ真。
    pub fn is_match(&self) -> bool {
        self.mismatched == 0
    }
}

/// 長さの検査（長さ違いが先、次に長さ 0）。添字アクセスは検査のあと。
fn check_lengths(reference: usize, actual: usize) -> Result<(), CompareError> {
    if reference != actual {
        return Err(CompareError::LengthMismatch { reference, actual });
    }
    if reference == 0 {
        return Err(CompareError::EmptyInput);
    }
    Ok(())
}

/// 集計に 1 要素の結果を足す。
fn tally(result: &mut FallbackComparison, index: usize, verdict: FallbackVerdict) {
    if !verdict.is_match() {
        result.mismatched += 1;
        result.first_mismatch.get_or_insert(index);
    }
}

/// 要素ごとの判定関数を列に適用して集計する共通の本体。
///
/// 長さの検査が先で、添字アクセスは検査のあと。確保はしない。
fn compare_with<T: Copy>(
    reference: &[T],
    actual: &[T],
    judge: fn(T, T) -> FallbackVerdict,
) -> Result<FallbackComparison, CompareError> {
    check_lengths(reference.len(), actual.len())?;
    let mut result = FallbackComparison {
        compared: reference.len(),
        mismatched: 0,
        first_mismatch: None,
    };
    for (i, (&r, &a)) in reference.iter().zip(actual).enumerate() {
        tally(&mut result, i, judge(r, a));
    }
    Ok(result)
}

/// u32 の列どうしをビット同一で要素ごとに比べる。
///
/// 長さ違いは [`CompareError::LengthMismatch`]、長さ 0 は [`CompareError::EmptyInput`]
/// （長さ違いの検査が先）。確保はしない。
pub fn compare_bits_u32(
    reference: &[u32],
    actual: &[u32],
) -> Result<FallbackComparison, CompareError> {
    compare_with(reference, actual, judge_bits_u32)
}

/// f32 の列どうしを、順序の前段のあとでビット同一かどうかで要素ごとに比べる。
///
/// 長さ違いは [`CompareError::LengthMismatch`]、長さ 0 は [`CompareError::EmptyInput`]
/// （長さ違いの検査が先）。確保はしない。
pub fn compare_bits_f32(
    reference: &[f32],
    actual: &[f32],
) -> Result<FallbackComparison, CompareError> {
    compare_with(reference, actual, judge_bits_f32)
}

/// binary16 のビット列を整数の順序に写す鍵。
///
/// 符号ビットが立っていれば下位 15 ビットの負数、立っていなければそのまま。
/// +0 と −0 はどちらも 0 になり、非正規化数は 0 のとなりに並ぶ。範囲は ±0x7FFF でオーバーフローしない。
fn binary16_order_key(bits: u16) -> i32 {
    if bits & 0x8000 != 0 {
        -i32::from(bits & 0x7FFF)
    } else {
        i32::from(bits)
    }
}

/// binary16 のビット列の順序の差の最大（この値以下を一致とする）。
pub const BINARY16_MAX_ORDER_DISTANCE: u32 = 1;

/// binary16 の 2 つのビット列の、数直線上の順序での差（隣り合う値で 1）。
///
/// 有限の値どうしで使う前提。有限でない値の組は、[`judge_order_distance_binary16`] が
/// 前段で先に決めるので、この差は使わない。
pub fn binary16_order_distance(reference: u16, actual: u16) -> u32 {
    binary16_order_key(reference).abs_diff(binary16_order_key(actual))
}

/// binary16 の 1 要素を、順序の前段のあとで順序の差の条件で比べる。
///
/// 前段（[`judge_order_binary16`]）で決まる組はその区分に写す。決まらない組だけ、
/// 差が [`BINARY16_MAX_ORDER_DISTANCE`] 以下なら `WithinOrderDistance`、超えれば
/// `OrderDistanceExceeded`。最大の有限値と +Inf のようにビット列が隣り合う組は前段で
/// `NonFiniteMismatch` に決まり、差の条件へは進まない。非正規化数も 0 に落とさずこの条件で比べる。
/// ビット同一の組も `WithinOrderDistance` で、`BitIdentical` とは区別しない。
pub fn judge_order_distance_binary16(reference: u16, actual: u16) -> FallbackVerdict {
    match judge_order_binary16(reference, actual) {
        OrderVerdict::BothNan => FallbackVerdict::BothNan,
        OrderVerdict::SameInfinity => FallbackVerdict::SameInfinity,
        OrderVerdict::NonFiniteMismatch => FallbackVerdict::NonFiniteMismatch,
        OrderVerdict::BothZero => FallbackVerdict::BothZero,
        OrderVerdict::Numeric => {
            if binary16_order_distance(reference, actual) <= BINARY16_MAX_ORDER_DISTANCE {
                FallbackVerdict::WithinOrderDistance
            } else {
                FallbackVerdict::OrderDistanceExceeded
            }
        }
    }
}

/// binary16 のビット列の列どうしを、順序の前段と順序の差の条件で要素ごとに比べる。
///
/// 長さ違いは [`CompareError::LengthMismatch`]、長さ 0 は [`CompareError::EmptyInput`]
/// （長さ違いの検査が先）。確保はしない。
pub fn compare_order_distance_binary16(
    reference: &[u16],
    actual: &[u16],
) -> Result<FallbackComparison, CompareError> {
    compare_with(reference, actual, judge_order_distance_binary16)
}

/// binary16 のビット列が等しい要素の数を返す。記録だけに使い、判定には使わない。
///
/// 長さの検査は [`compare_order_distance_binary16`] と同じ（長さ違いが先、次に長さ 0）。
pub fn count_bit_identical_binary16(
    reference: &[u16],
    actual: &[u16],
) -> Result<usize, CompareError> {
    check_lengths(reference.len(), actual.len())?;
    Ok(reference.iter().zip(actual).filter(|(r, a)| r == a).count())
}

/// f32 の和の相対の閾値（誤差が Σ|x| のこの倍以下なら一致）。緩めない。
pub const SUM_RELATIVE_BOUND: f64 = 1e-3;

/// 前段で決まった区分を `FallbackVerdict` に写す。`Numeric` は `None`。
fn decided_verdict(order: OrderVerdict) -> Option<FallbackVerdict> {
    match order {
        OrderVerdict::BothNan => Some(FallbackVerdict::BothNan),
        OrderVerdict::SameInfinity => Some(FallbackVerdict::SameInfinity),
        OrderVerdict::NonFiniteMismatch => Some(FallbackVerdict::NonFiniteMismatch),
        OrderVerdict::BothZero => Some(FallbackVerdict::BothZero),
        OrderVerdict::Numeric => None,
    }
}

/// `abs_sum` が条件に使える値（有限で 0 以上）かどうか。
fn abs_sum_is_usable(abs_sum: f64) -> bool {
    abs_sum.is_finite() && abs_sum >= 0.0
}

/// f32 の和の 1 要素を、順序の前段のあとで相対の条件で比べる。
///
/// `reference` は CPU 参照の和を f32 に 1 回丸めた値、`abs_sum` はその出力に足した値の絶対値の和（Σ|x|）。
/// 前段（[`judge_order_f32`]）で決まる組はその区分に写し、このとき `abs_sum` は見ない。
/// 決まらない組だけ、誤差 `|actual - reference|`（f64 で計算）が [`SUM_RELATIVE_BOUND`] * `abs_sum`
/// 以下なら `WithinRelativeSum`、超えれば `RelativeSumExceeded`。境目は含む（`<=`。
/// `tolerance::judge_element` の厳密な `<` とは違う）。割り算は使わない。
/// `abs_sum` が有限でない・負・NaN のときは fail-closed で `RelativeSumExceeded`。
/// `abs_sum` が 0 で誤差が正なら不一致。
pub fn judge_relative_sum_f32(reference: f32, actual: f32, abs_sum: f64) -> FallbackVerdict {
    if let Some(v) = decided_verdict(judge_order_f32(reference, actual)) {
        return v;
    }
    if !abs_sum_is_usable(abs_sum) {
        return FallbackVerdict::RelativeSumExceeded;
    }
    let abs_err = (f64::from(actual) - f64::from(reference)).abs();
    if abs_err <= SUM_RELATIVE_BOUND * abs_sum {
        FallbackVerdict::WithinRelativeSum
    } else {
        FallbackVerdict::RelativeSumExceeded
    }
}

/// f32 の和の 1 要素の余裕（誤差 / Σ|x|）。記録だけに使い、判定には使わない。
///
/// 前段で一致と決まる組は `0.0`、前段で不一致と決まる組は `+Inf`。前段で決まらない組は
/// `abs_sum` が有限で正なら `|actual - reference| / abs_sum`。`abs_sum` が 0 なら、誤差 0 で `0.0`、
/// それ以外は `+Inf`。`abs_sum` が有限でない・負・NaN なら `+Inf`。
/// 判定は掛け算の形なので、境目では余裕の値と判定がずれうる。判定は [`judge_relative_sum_f32`] だけが決める。
pub fn relative_sum_margin(reference: f32, actual: f32, abs_sum: f64) -> f64 {
    let order = judge_order_f32(reference, actual);
    if order.is_match() {
        return 0.0;
    }
    if order.is_decided() || !abs_sum_is_usable(abs_sum) {
        return f64::INFINITY;
    }
    let abs_err = (f64::from(actual) - f64::from(reference)).abs();
    if abs_sum > 0.0 {
        abs_err / abs_sum
    } else if abs_err == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

/// f32 の和の列を相対の条件で要素ごとに比べ、余裕を `margins` に全件書く。
///
/// 長さの検査の順: `reference` と `actual` の違いは `LengthMismatch { reference, actual }`、
/// `abs_sums`・`margins` が `reference` と違えば `LengthMismatch { reference: reference.len(), actual: その長さ }`、
/// 最後に長さ 0 の `EmptyInput`。すべて通ってから書き込む（エラーのとき `margins` は変わらない）。
/// 集計の形は他の比べと同じ。余裕の最大などは呼び出し側が `margins` から出す。確保はしない。
pub fn compare_relative_sum_f32(
    reference: &[f32],
    actual: &[f32],
    abs_sums: &[f64],
    margins: &mut [f64],
) -> Result<FallbackComparison, CompareError> {
    if reference.len() != actual.len() {
        return Err(CompareError::LengthMismatch {
            reference: reference.len(),
            actual: actual.len(),
        });
    }
    if abs_sums.len() != reference.len() {
        return Err(CompareError::LengthMismatch {
            reference: reference.len(),
            actual: abs_sums.len(),
        });
    }
    if margins.len() != reference.len() {
        return Err(CompareError::LengthMismatch {
            reference: reference.len(),
            actual: margins.len(),
        });
    }
    check_lengths(reference.len(), actual.len())?;
    let mut result = FallbackComparison {
        compared: reference.len(),
        mismatched: 0,
        first_mismatch: None,
    };
    for (i, ((&r, &a), &s)) in reference.iter().zip(actual).zip(abs_sums).enumerate() {
        tally(&mut result, i, judge_relative_sum_f32(r, a, s));
        margins[i] = relative_sum_margin(r, a, s);
    }
    Ok(result)
}
