//! 変異体の検出の結果を数える純粋関数の置き場（変異体ごとの分類と、検出率の下限・上限・セルのラベル）。
//!
//! 測定専用の別 workspace `measure/` に置く crate で、製品の crate（`crates/`）からは使われない。
//! 変異体 1 件の入力から分類を出す `classify` を持つ。
//! 件数から検出率の下限・上限を出す `rate::bounds`・ラベルを出す `rate::label`・セルのラベルを出す `rate::judge_cell` も持つ。
//! 件数を引数で受けて下限・上限とラベルを 1 行の JSON で出すコマンド `detect-bounds`（`src/bin/`）もある。
//! 本 crate は device に触れず、ファイルを読まず、時計を使わない。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-1082（ポインタのみ）。

pub mod classify;
pub mod rate;

use std::fmt;

/// 本 crate の失敗。不整合な入力を分類に倒さず、必ずこの型で返す。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DetectError {
    /// 数える組が空。
    EmptyCounted,
    /// 数える組に重複がある。
    DuplicateCounted,
    /// 数える組のうち結果が無い組がある。
    MissingResult,
    /// 数える組に無い組の結果がある。
    UnexpectedResult,
    /// 同じ組の結果が重複している。
    DuplicateResult,
    /// 分母が 0。
    ZeroDenominator,
    /// 件数または積の計算があふれた。
    Overflow,
}

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::EmptyCounted => "counted pairs are empty",
            Self::DuplicateCounted => "counted pairs contain a duplicate",
            Self::MissingResult => "a counted pair has no result",
            Self::UnexpectedResult => "a result is not in the counted pairs",
            Self::DuplicateResult => "a pair has more than one result",
            Self::ZeroDenominator => "denominator is zero",
            Self::Overflow => "arithmetic overflow",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for DetectError {}
