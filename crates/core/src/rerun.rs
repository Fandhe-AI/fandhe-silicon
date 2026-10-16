//! 契約の版を変えた後の流し直しで使う、照合 1 件の型の入口（spec TASK-29。範囲は「照合 1 件の表し方」と「照合の集まり」だけ）。
//!
//! feature `test-support` の裏にだけ公開される（置き場の説明は `docs/rerun-frame.md`）。
//! 製品の経路（`contract`・`exec`・`upper` の非テストのコード）からは参照しない。
//! 結果は `test_frame` の [`Outcome`](crate::test_frame::Outcome)・[`Label`](crate::test_frame::Label) をそのまま使い、
//! 同じ役割の型を新しく作らない。ファイル・環境変数・標準出力には触れず、照合を走らせる口と比べる道具は持たない。
//!
//! 使い方: 照合を [`Check`] として実装し、[`CheckSet::new`] で集める。1 件ずつ
//! `judge(c.subject(), c.declaration(), setting, |m| c.run(m))` に渡すと `Outcome` になる。

pub mod check;
pub mod set;

pub use check::Check;
pub use set::{CheckSet, CheckSetError, MAX_CHECKS};
