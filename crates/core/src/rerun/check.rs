//! 照合 1 件を表す trait。
//!
//! 流し直しの側（集まりの [`CheckSet`](super::CheckSet) を回す呼び出し元）から使われ、
//! 照合の中身（何を比べるか）は実装側が持つ。結果の決め方は `test_frame::judge` に任せる。

use crate::test_frame::{Declaration, MachineCombination, RunResult, Subject};

/// 照合 1 件。ID・対象・宣言と、組み合わせを受けて比べる本体を持つ。
///
/// 本体 [`Check::run`] は `judge` の閉包と同じ型なので、
/// `judge(c.subject(), c.declaration(), setting, |m| c.run(m))` とそのまま組める。
/// 実行の可否（未設定・宣言外の組み合わせ）は `judge` が決め、そのとき `run` は呼ばれない。
pub trait Check {
    /// 照合が自分について宣言する識別子（照合の ID を含む）。
    fn subject(&self) -> &Subject;

    /// 走らせる組み合わせと要る許可の宣言。
    fn declaration(&self) -> &Declaration;

    /// 指定の組み合わせで比べ、結果を返す。
    fn run(&self, on: MachineCombination) -> RunResult;
}
