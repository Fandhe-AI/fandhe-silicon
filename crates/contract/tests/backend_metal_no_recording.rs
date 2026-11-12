//! Metal の背後実装（`src/backend_metal.rs`）が持つ記録の操作の範囲を、ソースの文字列で機械的に守る
//! （TASK-775 のポインタ）。
//!
//! 持つのは compute の dispatch の記録（compute encoder の生成・バッファ・定数・共有メモリの設定・dispatch）だけ。
//! コピー・クリア・描画・イベント・完了ハンドラに当たる呼び出しがコード本体に現れたら落とす。
//! `contents()` 越しの `copy_nonoverlapping` による読み書きの 2 つの `// SAFETY:` は、最後に使った投入が
//! 完了していることの確かめに根拠を置く。compute encoder へ設定する 3 つの `unsafe` ブロックの `// SAFETY:` は、
//! 呼ぶメソッドごとに 1 つずつあり、CPU 側で確かめた前提の語句を保つ。関数の定数の設定の SAFETY
//! （記録に関わらない `setConstantValue_type_atIndex` の呼び出し）は別に 1 つだけ数える。
//! 対象はソースの文字列なので feature・target ではゲートせず、Metal の型にも device にも触らない
//! （どの OS でも流れる）。`unsafe` の個数の試験（`backend_metal_unsafe_count.rs`）と同じ読み方をする。

const SOURCE: &str = include_str!("../src/backend_metal.rs");

/// compute の dispatch の記録を超える操作（描画・コピー・クリア・イベント・完了ハンドラなど）に当たる語。部分一致で見る。
/// 追加するときは、同期の根拠を書き直す変更と同じ PR の中で表も直す。
const FORBIDDEN_WORDS: &[&str] = &[
    "blitCommandEncoder",
    "renderCommandEncoder",
    "dispatchThreads",
    "addCompletedHandler",
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
    "MTLBlitCommandEncoder",
    "MTLRenderCommandEncoder",
    "MTLBlitPass",
    "MTLComputePass",
    "MTLRenderPass",
];

const MANIFEST: &str = include_str!("../Cargo.toml");

/// `contents()` の 2 つの `// SAFETY:` が含んでいなければならない、根拠の語句。
const SAFETY_RATIONALE: &[&str] = &["最後に使った投入", "完了していること", "表の錠"];

/// 行頭（空白を除く）が `//`・`///`・`//!` の行を外した本体の各行を返す。
fn code_lines() -> impl Iterator<Item = (usize, &'static str)> {
    SOURCE
        .lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line))
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
}

const FIX_HINT: &str = "If you are adding copy or clear operations (or other encoders) to the Metal backend, \
write the synchronization rationale of the `// SAFETY:` comments around `contents()` and of the encoder \
setters in src/backend_metal.rs (and the module-level comment) first, then update the tables in this test. \
Also revisit how `free` and `release_finished` hand buffers to the reuse pool (release only when the last \
submission that used the buffer has finished).";

#[test]
fn backend_metal_code_has_no_words_beyond_compute_dispatch() {
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
        "backend_metal.rs must only record compute dispatches:\n{}\n{FIX_HINT}",
        hits.join("\n")
    );
}

#[test]
fn objc2_metal_features_do_not_include_other_encoder_headers() {
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
        "objc2-metal must not enable these encoder features: {hits:?}\n{FIX_HINT}"
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

/// 関数の定数の設定（`setConstantValue_type_atIndex`）に結びつく `// SAFETY:` を見分ける語句。
const CONSTANT_SAFETY_MARKER: &str = "setConstantValue_type_atIndex";

/// 関数の定数の設定の `// SAFETY:` が含んでいなければならない、根拠の語句。
const CONSTANT_SAFETY_RATIONALE: &[&str] = &["局所", "アラインメント", "長さ", "直前で"];

/// compute encoder へ設定する 3 つの呼び出しの印と、それぞれの `// SAFETY:` が含んでいなければならない語句。
const ENCODER_SAFETY: &[(&str, &[&str])] = &[
    ("setBuffer_offset_atIndex", &["Retained", "offset", "index"]),
    ("setBytes_length_atIndex", &["index", "長さ"]),
    ("setThreadgroupMemoryLength_atIndex", &["index", "長さ"]),
];

/// コメントの記号と改行を除いてつなぐ（行末の折り返しをまたぐ語句を拾えるようにする）。
fn flatten(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_start_matches('/').trim())
        .collect()
}

#[test]
fn backend_metal_contents_safety_comments_state_the_completed_submission_rationale() {
    let comments = safety_comments();
    let contents: Vec<_> = comments
        .iter()
        .filter(|(_, text)| {
            let flat = flatten(text);
            !flat.contains(CONSTANT_SAFETY_MARKER)
                && !ENCODER_SAFETY
                    .iter()
                    .any(|(marker, _)| flat.contains(marker))
        })
        .collect();
    assert_eq!(
        contents.len(),
        2,
        "expected exactly the two `// SAFETY:` comments around `contents()`"
    );
    let mut misses = Vec::new();
    for (no, text) in &contents {
        let flat = flatten(text);
        if !flat.contains("contents()") {
            misses.push(format!(
                "src/backend_metal.rs:{no}: a SAFETY comment is neither around `contents()` nor for a known encoder call"
            ));
        }
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
        "the SAFETY comments lost the completed-submission rationale:\n{}\n{FIX_HINT}",
        misses.join("\n")
    );
}

#[test]
fn backend_metal_encoder_safety_comments_state_the_cpu_side_preconditions() {
    let comments = safety_comments();
    let mut misses = Vec::new();
    for (marker, phrases) in ENCODER_SAFETY {
        let matching: Vec<_> = comments
            .iter()
            .filter(|(_, text)| flatten(text).contains(marker))
            .collect();
        if matching.len() != 1 {
            misses.push(format!(
                "expected exactly one `// SAFETY:` comment for `{marker}`, found {}",
                matching.len()
            ));
            continue;
        }
        let (no, text) = matching[0];
        let flat = flatten(text);
        if flat.contains("contents()") {
            misses.push(format!(
                "src/backend_metal.rs:{no}: the `{marker}` SAFETY must not mention `contents()`"
            ));
        }
        for phrase in *phrases {
            if !flat.contains(phrase) {
                misses.push(format!(
                    "src/backend_metal.rs:{no}: missing `{phrase}` in the `{marker}` SAFETY comment"
                ));
            }
        }
    }
    assert!(
        misses.is_empty(),
        "the encoder SAFETY comments lost their preconditions:\n{}\n{FIX_HINT}",
        misses.join("\n")
    );
}

#[test]
fn backend_metal_constant_safety_comment_keeps_its_rationale() {
    let comments = safety_comments();
    let constant: Vec<_> = comments
        .iter()
        .filter(|(_, text)| flatten(text).contains(CONSTANT_SAFETY_MARKER))
        .collect();
    assert_eq!(
        constant.len(),
        1,
        "expected exactly one `// SAFETY:` comment for `{CONSTANT_SAFETY_MARKER}`"
    );
    let mut constant_misses = Vec::new();
    for (no, text) in &constant {
        let flat = flatten(text);
        if flat.contains("contents()") {
            constant_misses.push(format!(
                "src/backend_metal.rs:{no}: the constant SAFETY must not mention `contents()`"
            ));
        }
        for phrase in CONSTANT_SAFETY_RATIONALE {
            if !flat.contains(phrase) {
                constant_misses.push(format!(
                    "src/backend_metal.rs:{no}: missing `{phrase}` in the constant SAFETY comment"
                ));
            }
        }
    }
    assert!(
        constant_misses.is_empty(),
        "the constant SAFETY comment lost its rationale:\n{}",
        constant_misses.join("\n")
    );
}
