//! 有限でない値と符号付きの零を扱う要素ごとの比べ方（spec TASK-25 の 2/4）。
//!
//! 背後実装の出力と CPU 参照を比べる側（検証用のテスト・測定）から呼ばれる。
//! feature `test-support` の裏にだけ公開される。標準ライブラリだけで書き、確保はしない。
//! 置き場・名前・形の説明は `docs/parity-non-finite.md` の「コードの現状の形」。
//! 既存の `tolerance` の関数・型は変えず、有限どうしの判定はそちらの `judge_element` へ委ねる。
//! spec との対応は TASK-25、`04-behavior/backend-parity.md` の PARITY-2〜PARITY-5、
//! `docs/spec/03-poc/kernel-artifacts-and-parity/conditions.md` の 14-3 のポインタのみ。

use super::tolerance::{CompareError, ElementVerdict, Tolerance, judge_element};

/// 有限でない値の規則を含めた 1 要素の比べの結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialVerdict {
    /// 両方が NaN（payload と符号ビットは比べない）で一致とした。
    BothNan,
    /// 同じ符号の無限大どうしで一致とした。
    SameInfinity,
    /// 有限でない値が絡み、上の二つに当たらない不一致。
    NonFiniteMismatch,
    /// 両方が有限で、許容誤差の内。
    Within,
    /// 両方が有限で、許容誤差を超えた。
    Exceeded,
}

impl SpecialVerdict {
    /// 一致として数える区分のときだけ真。
    pub const fn is_match(self) -> bool {
        matches!(self, Self::BothNan | Self::SameInfinity | Self::Within)
    }
}

/// 1 要素を、有限でない値の規則つきで比べる。
///
/// 参照が NaN なら実測も NaN のときだけ一致。参照が無限大なら同じ符号の無限大のときだけ一致。
/// 参照が有限で実測が有限でないときは不一致。両方が有限なら `judge_element` へ委ねる。
/// 列挙していない組は必ず不一致になる。
pub fn judge_element_with_special_values(
    reference: f32,
    actual: f32,
    tolerance: Tolerance,
) -> SpecialVerdict {
    if reference.is_nan() {
        return if actual.is_nan() {
            SpecialVerdict::BothNan
        } else {
            SpecialVerdict::NonFiniteMismatch
        };
    }
    if reference.is_infinite() {
        return if actual == reference {
            SpecialVerdict::SameInfinity
        } else {
            SpecialVerdict::NonFiniteMismatch
        };
    }
    if !actual.is_finite() {
        return SpecialVerdict::NonFiniteMismatch;
    }
    match judge_element(reference, actual, tolerance) {
        ElementVerdict::Within => SpecialVerdict::Within,
        ElementVerdict::Exceeded => SpecialVerdict::Exceeded,
        // 両方が有限なので到達しない。万一の場合も一致にはしない。
        ElementVerdict::NonFinite => SpecialVerdict::NonFiniteMismatch,
    }
}

/// 列どうしの比べの集計（有限でない値の規則つき）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialComparison {
    compared: usize,
    exceeded: usize,
    non_finite_mismatch: usize,
    first_mismatch: Option<usize>,
}

impl SpecialComparison {
    /// 比べた要素数。
    pub fn compared(&self) -> usize {
        self.compared
    }

    /// 許容誤差を超えた要素数。
    pub fn exceeded(&self) -> usize {
        self.exceeded
    }

    /// 有限でない値が絡む不一致の要素数。
    pub fn non_finite_mismatch(&self) -> usize {
        self.non_finite_mismatch
    }

    /// 最初に不一致だった要素の添字。無ければ `None`。
    pub fn first_mismatch(&self) -> Option<usize> {
        self.first_mismatch
    }

    /// 1 要素も不一致でないときだけ真。
    pub fn is_match(&self) -> bool {
        self.exceeded == 0 && self.non_finite_mismatch == 0
    }
}

/// 参照と実測の f32 の列を、有限でない値の規則つきで要素ごとに比べる。
///
/// 長さ違いは [`CompareError::LengthMismatch`]、長さ 0 は [`CompareError::EmptyInput`]
/// （長さ違いの検査が先）。検査が済むまで添字アクセスをしない。確保はしない。
pub fn compare_f32_with_special_values(
    reference: &[f32],
    actual: &[f32],
    tolerance: Tolerance,
) -> Result<SpecialComparison, CompareError> {
    if reference.len() != actual.len() {
        return Err(CompareError::LengthMismatch {
            reference: reference.len(),
            actual: actual.len(),
        });
    }
    if reference.is_empty() {
        return Err(CompareError::EmptyInput);
    }
    let mut result = SpecialComparison {
        compared: reference.len(),
        exceeded: 0,
        non_finite_mismatch: 0,
        first_mismatch: None,
    };
    for (i, (&r, &a)) in reference.iter().zip(actual).enumerate() {
        match judge_element_with_special_values(r, a, tolerance) {
            SpecialVerdict::BothNan | SpecialVerdict::SameInfinity | SpecialVerdict::Within => {
                continue;
            }
            SpecialVerdict::Exceeded => result.exceeded += 1,
            SpecialVerdict::NonFiniteMismatch => result.non_finite_mismatch += 1,
        }
        result.first_mismatch.get_or_insert(i);
    }
    Ok(result)
}
