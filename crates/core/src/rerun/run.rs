//! 照合の集まりを流して結果と記録を出す口（spec TASK-29）。
//!
//! 流し直しの試験の側（4/8 以降）から呼ばれる。1 件ずつ `test_frame::judge` に渡して
//! [`Outcome`] を宣言の順に集め、記録は全部の部品を 1 回の `test_frame::export_parts` にまとめて書く。
//! まとめるのは、途中で失敗したときに、この呼び出しで作ったファイルを全部消せるようにするため。
//! 環境変数・標準出力には触れない。設定（[`RunSetting`]）と書き出し先は呼び出し側が渡す。
//! 前の記録と比べる道具は持たない。

use super::set::CheckSet;
use crate::test_frame::{ExportError, Outcome, RunSetting, export_parts, judge};
use std::path::Path;

/// 集まりの照合を宣言の順に流し、結果を同じ順で返す。
///
/// 未実施の照合は本体を呼ばない（`judge` に従う）。本体が panic したらそのまま伝わる。
pub fn run_checks(set: &CheckSet<'_>, setting: &RunSetting) -> Vec<Outcome> {
    set.checks()
        .iter()
        .map(|c| judge(c.subject(), c.declaration(), setting, |m| c.run(m)))
        .collect()
}

/// 全部の結果の記録の部品を 1 回の呼び出しで `dir` へ書き、書いたファイル名を部品の順に返す。
///
/// 失敗したときは、この呼び出しで作ったファイルが消える。ディレクトリの検査は `export_parts` に従う。
pub fn export_outcomes(dir: &Path, outcomes: &[Outcome]) -> Result<Vec<String>, ExportError> {
    let parts: Vec<_> = outcomes.iter().flat_map(Outcome::record_parts).collect();
    export_parts(dir, &parts)
}

/// 流してから、`dir` があれば記録を書く。`dir` が `None` なら何も書かない。
///
/// 全部の照合を決めたあとに書くので、本体が panic したら 1 件も書かない。
/// 書き出しの失敗は `Err` で返す（結果のラベルは書き出しの成否で変わらない）。
pub fn run_and_export(
    dir: Option<&Path>,
    set: &CheckSet<'_>,
    setting: &RunSetting,
) -> Result<Vec<Outcome>, ExportError> {
    let outcomes = run_checks(set, setting);
    if let Some(dir) = dir {
        export_outcomes(dir, &outcomes)?;
    }
    Ok(outcomes)
}
