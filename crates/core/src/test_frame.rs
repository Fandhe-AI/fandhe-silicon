//! 契約の試験の共通の枠の入口（spec TASK-23 の 3/8。範囲は「試験ごとの宣言と、機材・許可が無いときに走らせないこと」）。
//!
//! feature `test-support` の裏にだけ公開される（置き場と範囲の説明は `docs/test-frame.md` の「現状の形」）。
//! 製品の経路（`contract`・`exec`・`upper` の非テストのコード）からは参照しない。
//! 環境変数から読んだ値はこのモジュールの外へ渡さない。実機の device を取って宣言と照合する経路と、
//! 結果のラベル・集計の出力は持たない（後続の段が扱う）。
//!
//! 試験の側の使い方: 試験の先頭で [`Declaration`] を作り、[`RunSetting::from_env`] で得た設定とともに
//! [`decide`] へ渡す。[`Decision::NotRun`] なら、その理由を明示して試験を終える（暗黙に成功にしない）。

pub mod combination;
pub mod decision;
pub mod declaration;
pub mod setting;

pub use combination::{Backend, Machine, MachineCombination};
pub use decision::{Decision, NotRunReason, decide};
pub use declaration::{Declaration, DeclarationError, Permission, PermissionError};
pub use setting::{COMBINATION_ENV, PERMISSIONS_ENV, RunSetting};

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
