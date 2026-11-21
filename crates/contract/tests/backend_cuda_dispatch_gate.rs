//! CUDA の背後実装の、利用者のカーネルを走らせる門を守る（spec TASK-805 のポインタ）。
//!
//! 対象はソースの文字列なので feature・target ではゲートしない（どの組でも同じ結果になる）。

/// 照らす CUDA の背後実装のソース。
const SOURCE: &str = include_str!("../src/backend_cuda.rs");

/// `line` と前後の空白を除いて同じ行の数。コメントの行と、ほかの字句に埋まった行は数えない。
fn exact_lines(line: &str) -> usize {
    SOURCE.lines().filter(|l| l.trim() == line).count()
}

/// 製品のビルドで dispatch を含む投入を拒む門が残っていることを、ソースの行の完全一致で守る。
#[test]
fn backend_cuda_dispatch_execution_gate_stays_closed_outside_unit_tests() {
    assert_eq!(
        exact_lines("const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);"),
        1,
        "dispatch execution must stay disabled in non-test builds"
    );
    assert_eq!(
        exact_lines("check_dispatch_gate(&jobs, DISPATCH_EXECUTION_ENABLED)?;"),
        1,
        "plan must reject recorded dispatches while the gate is closed"
    );
}
