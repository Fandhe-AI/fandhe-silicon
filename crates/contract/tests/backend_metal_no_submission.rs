//! Metal の背後実装（`src/backend_metal.rs`）がコマンドバッファを積まない（投入しない）ことを、
//! ソースの文字列で機械的に守る。
//!
//! `contents()` 越しの `copy_nonoverlapping` による読み書きの 2 つの `// SAFETY:` は、
//! GPU がそのバッファを同時に使っていないことの根拠を「この背後実装は投入しない」に置いている
//! （モジュール冒頭のコメントも同じ前提）。その前提がコメントだけで残らないよう、投入に当たる
//! 呼び出しがコード本体に現れたら落とす（spec TASK-158 のポインタ）。
//! 対象はソースの文字列なので feature・target ではゲートせず、Metal の型にも device にも触らない
//! （どの OS でも流れる）。`unsafe` の個数の試験（`backend_metal_unsafe_count.rs`）と同じ読み方をする。

const SOURCE: &str = include_str!("../src/backend_metal.rs");

/// 投入（コマンドバッファの生成・確定・完了待ち・エンコーダの生成）に当たる語。部分一致で見る。
/// 追加するときは、同期の根拠を書き直す変更と同じ PR の中で表も直す。
const FORBIDDEN_WORDS: &[&str] = &[
    "commandBuffer",
    "MTLCommandBuffer",
    "commit",
    "CommandEncoder",
    "addCompletedHandler",
    "waitUntilCompleted",
];

/// コマンドキューを保持するだけで使わないフィールド。構造体の宣言と初期化以外で読み出さない。
const UNUSED_QUEUE_FIELDS: &[&str] = &["_queue"];

/// 行頭（空白を除く）が `//`・`///`・`//!` の行を外した本体の各行を返す。
fn code_lines() -> impl Iterator<Item = (usize, &'static str)> {
    SOURCE
        .lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line))
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
}

const FIX_HINT: &str = "If you are adding command buffer submission to the Metal backend, \
rewrite the synchronization rationale of the two `// SAFETY:` comments around `contents()` in \
src/backend_metal.rs (and the module-level comment) first, then update the tables in this test.";

#[test]
fn backend_metal_code_has_no_command_submission_words() {
    let mut hits = Vec::new();
    for (no, line) in code_lines() {
        for word in FORBIDDEN_WORDS {
            if line.contains(word) {
                hits.push(format!(
                    "src/backend_metal.rs:{no}: `{word}` in `{}`",
                    line.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "backend_metal.rs must not submit command buffers:\n{}\n{FIX_HINT}",
        hits.join("\n")
    );
}

#[test]
fn backend_metal_command_queue_field_is_never_read() {
    let mut hits = Vec::new();
    for (no, line) in code_lines() {
        for field in UNUSED_QUEUE_FIELDS {
            let access = format!(".{field}");
            if line.contains(&access) {
                hits.push(format!(
                    "src/backend_metal.rs:{no}: `{access}` in `{}`",
                    line.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the command queue field must only be initialized, never read:\n{}\n{FIX_HINT}",
        hits.join("\n")
    );
}
