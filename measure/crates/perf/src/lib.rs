//! 性能を測る共通の道具の土台（投入 1 回あたりの標本・run ごとの統計・run 間の集計）。
//!
//! 測定専用の別 workspace `measure/` に置く crate で、製品の crate（`crates/`）からは使われない。
//! 測定の腕（各背後実装を呼ぶコード）から使われ、標本を集める `sample`、
//! run ごとの統計を出す `stats`、run 間の集計を出す `across` を提供する。
//! 記録の封筒と本体を検証つきで作り JSON に書き出す `record`（と部品の `json`）も持つ。
//! 封筒の経路の証拠・環境・比較基準の版の欄は `evidence` が持つ。
//! 2 つの値の比から「同等」「差がある」の区分を出す `ratio` も持つ（適用範囲の一覧に載る試験だけ）。
//! run ごとの記録を厳格に読む `jsonread` と、読んだ記録から run 間の集計を出す `runs` を持ち、
//! コマンド `across-runs`（標準出力に 1 行の JSON を出す。ファイルは作らない）がそれらを呼ぶ。
//! 照合の結果（一致・不一致）から、性能値を採る run だけを選ぶ門の規則を `gate` が持つ。
//! PoC ごとの run 間の変動の幅の規則（閉じた設定の定数表と、幅の規則に当たったかを出す純粋な関数）を `variation` が持つ。
//! 本 crate は device に触れない。GPU 時間は呼び手が測った値を受け取るだけである。
//!
//! spec との対応: `docs/spec/05-tasks.md` の TASK-27（ポインタのみ）。

pub mod across;
pub mod evidence;
pub mod gate;
pub mod json;
pub mod jsonread;
pub mod ratio;
pub mod record;
pub mod runs;
pub mod sample;
pub mod stats;
pub mod variation;

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
    /// 比の分母が 0。
    ZeroDenominator,
}

impl fmt::Display for PerfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::Empty => "empty input",
            Self::DurationOverflow => "duration does not fit in u64 nanoseconds",
            Self::ValueTooLarge => "value exceeds the exactly representable limit",
            Self::InvalidValue => "value is not finite or is negative",
            Self::InvalidCount => "invalid sample count",
            Self::ZeroDenominator => "ratio denominator is zero",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PerfError {}
