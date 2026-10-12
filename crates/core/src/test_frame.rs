//! 契約の試験の共通の枠の入口（spec TASK-23。範囲は「試験ごとの宣言と、機材・許可が無いときに走らせないこと」「結果のラベルの決定」「組み合わせごとの集計」）。
//!
//! feature `test-support` の裏にだけ公開される（置き場と範囲の説明は `docs/test-frame.md` の「置き場と有効になる条件」）。
//! 製品の経路（`contract`・`exec`・`upper` の非テストのコード）からは参照しない。
//! 環境変数から読んだ値はこのモジュールの外へ渡さない。実機の device を取って宣言と照合する経路と、
//! 複数の試験をまたぐ集計の経路は持たない。
//!
//! 試験の側の使い方: 試験の先頭で [`Declaration`] を作り、[`RunSetting::from_env`] で得た設定とともに
//! [`decide`] へ渡す。[`Decision::NotRun`] なら、その理由を明示して試験を終える（暗黙に成功にしない）。
//! 結果は [`Subject`] とともに [`judge`] へ渡して [`Outcome`] にする（ラベルは [`Label`]）。`Outcome` は `Termination` を実装しないので試験の戻り値にはできず、呼び出し側がラベルを明示的に扱う。
//! 集めた `Outcome` は [`Summary`] で組み合わせごとのラベルの件数にでき、`Display` で表にできる。
//! 試験ごとの記録の部品は `Outcome::record_parts`（[`RecordPart`]）で得る。[`judge_and_export`] は `judge` の後に、
//! 環境変数 [`RECORD_DIR_ENV`] のディレクトリへ部品を書く（未設定なら何も書かない）。

pub mod combination;
pub mod decision;
pub mod declaration;
pub mod export;
pub mod identifier;
pub mod label;
pub mod outcome;
pub mod record;
pub mod setting;
pub mod summary;
pub mod threshold;

pub use combination::{Backend, Machine, MachineCombination};
pub use decision::{Decision, NotRunReason, decide};
pub use declaration::{Declaration, DeclarationError, Permission, PermissionError};
pub use export::{
    ExportError, RECORD_DIR_ENV, export_from_env, export_from_lookup, export_parts,
    judge_and_export, judge_and_export_with, record_file_name,
};
pub use identifier::{BehaviorId, IdentifierError, MAX_BEHAVIOR_ID_LEN, MAX_TEST_ID_LEN, TestId};
pub use label::{IndeterminateReason, Label, LabelKind, PENDING_REASON};
pub use outcome::{
    MAX_BEHAVIORS, MAX_THRESHOLDS, Outcome, RunResult, Subject, SubjectError, Verdict, judge,
};
pub use record::{BODY_KIND, RecordPart};
pub use setting::{COMBINATION_ENV, PERMISSIONS_ENV, RunSetting};
pub use summary::{LabelCounts, Summary, SummaryError};
pub use threshold::{APPROVED_THRESHOLDS, ThresholdId, ThresholdIdError};

/// 試験の宣言を、関数 API の呼び出しとして書くための薄いマクロ。
///
/// 第 1 引数は走らせる組み合わせの配列、省略できる第 2 引数は要る許可の名前の配列。
/// 戻り値は [`Declaration::from_parts`] と同じ `Result` で、検査の失敗も関数と同じ形で返る。
///
/// ```
/// use fandhe_silicon_core::declaration;
/// use fandhe_silicon_core::test_frame::MachineCombination;
///
/// let d = declaration!([MachineCombination::MetalMacMini], ["fault-injection"]);
/// assert!(d.is_ok());
/// let none = declaration!([]);
/// assert!(none.is_err());
/// ```
#[macro_export]
macro_rules! declaration {
    ([$($combination:expr),* $(,)?]) => {
        $crate::test_frame::Declaration::from_parts(&[$($combination),*], &[])
    };
    ([$($combination:expr),* $(,)?], [$($permission:expr),* $(,)?]) => {
        $crate::test_frame::Declaration::from_parts(&[$($combination),*], &[$($permission),*])
    };
}
