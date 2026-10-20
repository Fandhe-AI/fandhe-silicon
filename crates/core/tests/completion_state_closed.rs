//! `CompletionState` が閉じた 3 値であることを、別の crate から固定する公開テスト（TASK-269 の 3/8）。
//!
//! `CompletionState` は `#[non_exhaustive]` を付けない（ERR-18 の扱い。ポインタ）。
//! `_` の腕なしの `match` が通ることで、値が増える・`#[non_exhaustive]` が付くとコンパイルで落ちる。

use fandhe_silicon_core::error::CompletionState;

fn name(s: CompletionState) -> &'static str {
    match s {
        CompletionState::Pending => "pending",
        CompletionState::Complete => "complete",
        CompletionState::Lost => "lost",
    }
}

#[test]
fn completion_state_is_closed_three_values() {
    let names = [
        name(CompletionState::Pending),
        name(CompletionState::Complete),
        name(CompletionState::Lost),
    ];
    assert_eq!(names, ["pending", "complete", "lost"]);
}
