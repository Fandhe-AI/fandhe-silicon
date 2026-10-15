//! 性能を測る共通の道具の土台（投入 1 回あたりの標本・run ごとの統計・run 間の集計）。
//!
//! 測定専用の別 workspace `measure/` に置く crate で、製品の crate（`crates/`）からは使われない。
//! 測定の腕（各背後実装を呼ぶコード）から使われ、標本を集める `sample`、
//! run ごとの統計を出す `stats`、run 間の集計を出す `across` を提供する。
//! 記録の封筒と本体を検証つきで作り JSON に書き出す `record`（と部品の `json`）も持つ。
//! 本 crate は device に触れない。GPU 時間は呼び手が測った値を受け取るだけである。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-27（ポインタのみ）。

pub mod across;
pub mod json;
pub mod record;
pub mod sample;
pub mod stats;

use std::fmt;

/// 本 crate の失敗。0 や無限大で埋めず、必ずこの型で返す。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PerfError {
    /// 標本・run の入力が空。
    Empty,
    /// 時間の値が `u64` の ns に収まらない。
    DurationOverflow,
    /// 値が統計を正確に扱える上限（[`stats::MAX_EXACT_NS`]）を超えている。
    ValueTooLarge,
    /// 有限でない値、または負の値。
    InvalidValue,
    /// 標本の数の指定が不正（計測の回数が 0 など）。
    InvalidCount,
}

impl fmt::Display for PerfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::Empty => "empty input",
            Self::DurationOverflow => "duration does not fit in u64 nanoseconds",
            Self::ValueTooLarge => "value exceeds the exactly representable limit",
            Self::InvalidValue => "value is not finite or is negative",
            Self::InvalidCount => "invalid sample count",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PerfError {}
