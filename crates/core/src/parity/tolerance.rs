//! f32 の列どうしを要素ごとに許容誤差で比べる道具と、その閾値の設定（spec TASK-24 の 4/8）。
//!
//! 背後実装の出力と CPU 参照の出力を比べる側（検証用のテスト・測定）から呼ばれる。
//! feature `test-support` の裏にだけ公開される。標準ライブラリだけで書き、確保はしない。
//! 閾値の定義との対応は `docs/spec/02-poc-plan.md` の T-1・PARITY-1 のポインタのみ。

use std::fmt;

/// 相対誤差と絶対誤差の上限の組。値は `f64` で持つ（`f32` で持つと境界の判定がずれる）。
///
/// 外からは [`T1`] だけが手に入り、フィールドを直接書いて作ることはできない
/// （次の `compile_fail` の例が確かめる。stable の rustdoc はエラーコードを照合しないので、
/// 通る例で import が正しいことを担保している）。
///
/// ```
/// use fandhe_silicon_core::parity::tolerance::{T1, Tolerance};
/// let t: Tolerance = T1;
/// assert!(t.relative() > 0.0 && t.absolute() > 0.0);
/// ```
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::parity::tolerance::Tolerance;
/// let _ = Tolerance { relative: 1.0, absolute: 1.0 };
/// ```
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::parity::tolerance::{T1, Tolerance};
/// let _ = Tolerance { relative: 1.0, ..T1 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    relative: f64,
    absolute: f64,
}

impl Tolerance {
    /// 相対誤差の上限（参照の絶対値に対する比。この値未満なら内）。
    pub const fn relative(&self) -> f64 {
        self.relative
    }

    /// 絶対誤差の上限（この値未満なら内）。
    pub const fn absolute(&self) -> f64 {
        self.absolute
    }
}

/// T-1 の閾値の設定（`docs/spec/02-poc-plan.md` の T-1、PARITY-1 のポインタ）。
/// 任意の値を作る口は持たない。
pub const T1: Tolerance = Tolerance {
    relative: 1e-3,
    absolute: 1e-5,
};

/// 1 要素の比べの結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementVerdict {
    /// 許容誤差の内。
    Within,
    /// 許容誤差を超えた。
    Exceeded,
    /// 参照か実測のどちらかが有限でなかった（不一致として数える）。
    NonFinite,
}

/// 1 要素を比べる。
///
/// 絶対誤差が `absolute` 未満、または参照の絶対値に対する相対誤差が `relative` 未満なら
/// [`ElementVerdict::Within`]。比較は厳密な `<`。誤差は `f64` で計算する。
pub fn judge_element(reference: f32, actual: f32, tolerance: Tolerance) -> ElementVerdict {
    if !reference.is_finite() || !actual.is_finite() {
        return ElementVerdict::NonFinite;
    }
    let reference = f64::from(reference);
    let abs_err = (f64::from(actual) - reference).abs();
    // 参照が 0 のとき商は inf になり相対の比較は偽になる（絶対誤差だけで判定される）。
    if abs_err < tolerance.absolute || abs_err / reference.abs() < tolerance.relative {
        ElementVerdict::Within
    } else {
        ElementVerdict::Exceeded
    }
}

/// 列どうしの比べの集計。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    compared: usize,
    exceeded: usize,
    non_finite: usize,
    first_mismatch: Option<usize>,
}

impl Comparison {
    /// 比べた要素数。
    pub fn compared(&self) -> usize {
        self.compared
    }

    /// 許容誤差を超えた要素数。
    pub fn exceeded(&self) -> usize {
        self.exceeded
    }

    /// 有限でない値を含んでいた要素数。
    pub fn non_finite(&self) -> usize {
        self.non_finite
    }

    /// 最初に外れた要素の添字。外れが無ければ `None`。
    pub fn first_mismatch(&self) -> Option<usize> {
        self.first_mismatch
    }

    /// 1 要素も外れていないときだけ真。
    pub fn is_match(&self) -> bool {
        self.exceeded == 0 && self.non_finite == 0
    }
}

/// 列の比べができなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareError {
    /// 入力の長さが 0 だった。
    EmptyInput,
    /// 二つの入力の長さが違った。
    LengthMismatch {
        /// 参照の長さ。
        reference: usize,
        /// 実測の長さ。
        actual: usize,
    },
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "input is empty"),
            Self::LengthMismatch { reference, actual } => write!(
                f,
                "reference has length {reference}, actual has length {actual}"
            ),
        }
    }
}

impl std::error::Error for CompareError {}

/// 参照と実測の f32 の列を要素ごとに比べ、集計を返す。
///
/// 長さ 0 は [`CompareError::EmptyInput`]、長さ違いは [`CompareError::LengthMismatch`]。
/// 検査が済むまで添字アクセスをしない。確保はしない。
pub fn compare_f32(
    reference: &[f32],
    actual: &[f32],
    tolerance: Tolerance,
) -> Result<Comparison, CompareError> {
    if reference.len() != actual.len() {
        return Err(CompareError::LengthMismatch {
            reference: reference.len(),
            actual: actual.len(),
        });
    }
    if reference.is_empty() {
        return Err(CompareError::EmptyInput);
    }
    let mut result = Comparison {
        compared: reference.len(),
        exceeded: 0,
        non_finite: 0,
        first_mismatch: None,
    };
    for (i, (&r, &a)) in reference.iter().zip(actual).enumerate() {
        match judge_element(r, a, tolerance) {
            ElementVerdict::Within => continue,
            ElementVerdict::Exceeded => result.exceeded += 1,
            ElementVerdict::NonFinite => result.non_finite += 1,
        }
        result.first_mismatch.get_or_insert(i);
    }
    Ok(result)
}
