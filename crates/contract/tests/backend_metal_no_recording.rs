//! Metal の背後実装（`src/backend_metal.rs`）が記録の操作（エンコーダの生成・バッファの束縛・dispatch）を
//! 持たないことを、ソースの文字列で機械的に守る（TASK-592 のポインタ）。
//!
//! `contents()` 越しの `copy_nonoverlapping` による読み書きの 2 つの `// SAFETY:` は、
//! GPU がそのバッファを同時に使っていないことの根拠を「この背後実装が投入するコマンドバッファは
//! エンコーダを開かず、どのバッファも参照しない」に置いている（モジュール冒頭のコメントも同じ前提）。
//! その前提がコメントだけで残らないよう、記録の操作に当たる呼び出しがコード本体に現れたら落とし、
//! 2 つの SAFETY が新しい根拠の語句を保っていることも確かめる。
//! 対象はソースの文字列なので feature・target ではゲートせず、Metal の型にも device にも触らない
//! （どの OS でも流れる）。`unsafe` の個数の試験（`backend_metal_unsafe_count.rs`）と同じ読み方をする。

const SOURCE: &str = include_str!("../src/backend_metal.rs");

/// 記録の操作（エンコーダの生成・バッファの束縛・dispatch・完了ハンドラ）に当たる語。部分一致で見る。
/// 追加するときは、同期の根拠を書き直す変更と同じ PR の中で表も直す。
const FORBIDDEN_WORDS: &[&str] = &[
    "computeCommandEncoder",
    "blitCommandEncoder",
    "renderCommandEncoder",
    "setBuffer",
    "dispatchThreadgroups",
    "dispatchThreads",
    "addCompletedHandler",
    "MTLComputeCommandEncoder",
    "MTLBlitCommandEncoder",
    "RenderCommandEncoder",
    "parallelRenderCommandEncoder",
    "resourceStateCommandEncoder",
    "accelerationStructureCommandEncoder",
    "encodeSignalEvent",
    "encodeWaitForEvent",
    "useResource",
    "addScheduledHandler",
    "presentDrawable",
    "copyFromBuffer",
    "fillBuffer",
];

/// objc2-metal の feature に入っていてはならない、encoder 系の header 名。
const FORBIDDEN_FEATURES: &[&str] = &[
    "MTLCommandEncoder",
    "MTLBlitCommandEncoder",
    "MTLComputeCommandEncoder",
    "MTLRenderCommandEncoder",
    "MTLBlitPass",
    "MTLComputePass",
    "MTLRenderPass",
];

const MANIFEST: &str = include_str!("../Cargo.toml");

/// 2 つの `// SAFETY:` が含んでいなければならない、新しい根拠の語句。
const SAFETY_RATIONALE: &[&str] = &[
    "エンコーダを開かず",
    "どのバッファも参照しない",
    "記録の操作を持たない",
    "記録の操作を足す変更では",
];

/// 行頭（空白を除く）が `//`・`///`・`//!` の行を外した本体の各行を返す。
fn code_lines() -> impl Iterator<Item = (usize, &'static str)> {
    SOURCE
        .lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line))
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
}

const FIX_HINT: &str = "If you are adding recording operations (copy, clear, dispatch) to the Metal backend, \
rewrite the synchronization rationale of the two `// SAFETY:` comments around `contents()` in \
src/backend_metal.rs (and the module-level comment) first, then update the tables in this test.";

#[test]
fn backend_metal_code_has_no_recording_words() {
    let mut hits = Vec::new();
    for (no, line) in code_lines() {
        for word in FORBIDDEN_WORDS {
            if line.to_lowercase().contains(&word.to_lowercase()) {
                hits.push(format!(
                    "src/backend_metal.rs:{no}: `{word}` in `{}`",
                    line.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "backend_metal.rs must not record commands:\n{}\n{FIX_HINT}",
        hits.join("\n")
    );
}

#[test]
fn objc2_metal_features_do_not_include_encoder_headers() {
    let line = MANIFEST
        .lines()
        .find(|l| l.trim_start().starts_with("objc2-metal"))
        .expect("objc2-metal dependency line");
    let mut hits = Vec::new();
    for feature in FORBIDDEN_FEATURES {
        if line.contains(&format!("\"{feature}\"")) {
            hits.push(*feature);
        }
    }
    assert!(
        hits.is_empty(),
        "objc2-metal must not enable encoder features: {hits:?}\n{FIX_HINT}"
    );
}

/// `// SAFETY:` で始まるコメントの塊（続く `//` の行を含む）を、行番号付きで集める。
fn safety_comments() -> Vec<(usize, String)> {
    let lines: Vec<&str> = SOURCE.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("// SAFETY:") {
            let start = i;
            let mut text = String::new();
            while i < lines.len() && lines[i].trim_start().starts_with("//") {
                text.push_str(lines[i].trim_start());
                text.push('\n');
                i += 1;
            }
            out.push((start + 1, text));
        } else {
            i += 1;
        }
    }
    out
}

#[test]
fn backend_metal_safety_comments_state_the_no_recording_rationale() {
    let comments = safety_comments();
    assert_eq!(
        comments.len(),
        2,
        "expected exactly the two `// SAFETY:` comments around `contents()`"
    );
    let mut misses = Vec::new();
    for (no, text) in &comments {
        // 行末の折り返しをまたぐ語句を拾えるよう、コメントの記号と改行を除いてつなぐ。
        let flat: String = text
            .lines()
            .map(|l| l.trim_start_matches('/').trim())
            .collect();
        for phrase in SAFETY_RATIONALE {
            if !flat.contains(phrase) {
                misses.push(format!(
                    "src/backend_metal.rs:{no}: missing `{phrase}` in the SAFETY comment"
                ));
            }
        }
    }
    assert!(
        misses.is_empty(),
        "the SAFETY comments lost the no-recording rationale:\n{}\n{FIX_HINT}",
        misses.join("\n")
    );
}
