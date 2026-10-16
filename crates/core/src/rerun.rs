//! 契約の版を変えた後の流し直しで使う、照合 1 件の型の入口（spec TASK-29。範囲は「照合 1 件の表し方」「照合の集まり」「流す口」「Node での照合の行の写し」）。
//!
//! feature `test-support` の裏にだけ公開される（置き場の説明は `docs/rerun-frame.md`）。
//! 製品の経路（`contract`・`exec`・`upper` の非テストのコード）からは参照しない。
//! 結果は `test_frame` の [`Outcome`](crate::test_frame::Outcome)・[`Label`](crate::test_frame::Label) をそのまま使い、
//! 同じ役割の型を新しく作らない。環境変数・標準出力には触れず、書き出しは `test_frame::export_parts` を通す。前の記録と比べる道具は持たない。
//!
//! 使い方: 照合を [`Check`] として実装し、[`CheckSet::new`] で集める。1 件ずつ
//! [`run_checks`] で流すと `Outcome` が宣言の順に返り、[`run_and_export`] なら記録も書く。

pub mod check;
pub mod reference;
pub mod run;
pub mod set;

pub use check::Check;
pub use reference::{ReferenceSource, RowCheck, RowState, reference_gap};
pub use run::{export_outcomes, run_and_export, run_checks};
pub use set::{CheckSet, CheckSetError, MAX_CHECKS};
