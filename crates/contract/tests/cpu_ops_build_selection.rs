//! `cpu_ops` の `*_simd` の版の選び分けが、ビルド時の `cfg` だけで決まることを固定する試験（spec TASK-1351 のポインタ）。
//!
//! 見るのは次の 9 点。
//! - (a) 実行時に CPU の機能を調べる字句（`is_*_feature_detected`・`std_detect`・`std::detect`）がどのファイルにも無い（コメントも数える）。
//! - (b) `cpu_ops.rs` に、`simd128` が有効な形と無効な形の `#[cfg(...)]` の行がそれぞれ 1 件以上ある。
//! - (c) `core::arch::wasm32`・`std::arch::wasm32` の字句は `cpu_ops/simd128.rs` の中だけにある。
//! - (d) `mod simd128;` の行がちょうど 1 行で、直前の属性が `simd128` が有効な形の `#[cfg(...)]` である。
//! - (e) `cpu_ops.rs` に、`neon` が有効な形と無効な形の `#[cfg(...)]` の行がそれぞれ 1 件以上ある。
//! - (f) `core::arch::aarch64`・`std::arch::aarch64` の字句は `cpu_ops/neon.rs` の中だけにある。
//! - (g) `mod neon;` の行がちょうど 1 行で、直前の属性が `neon` が有効な形の `#[cfg(...)]` と `unsafe_code` を許す属性の順で、
//!   その属性は `cpu_ops.rs` に 1 件だけある（ほかのファイルには無い）。
//! - (h) `gemm_simd_impl`・`elementwise_simd_impl` が 3 枝あり、`simd128`・`neon`・scalar の `cfg` がそれぞれの版を呼ぶ。
//! - (i) `cpu_ops/neon.rs` の `elementwise_lanes` の 4 要素の `Relu` の腕が、比較のマスク（`vcgtq_f32` と `vandq_u32`）で書かれ、
//!   `vmaxq_f32`・`vmaxnmq_f32`・`vcgeq_f32` を使わない（コメントの語は数えない）。
//!
//! 外部 crate もコマンドの起動も使わず、ソースの文字列だけを読む。
//! 判定は純粋関数 `problems` に分け、実物を 1 か所だけ変えた入力を渡す陽性対照を常設する。
//! 想定外の形は黙って通さず、問題として積んで落とす。
//! 条件のない試験なので、feature の組や target に依らず同じ結果になる。

/// 実行時の検出を表す字句。
const DETECTION_WORDS: [&str; 6] = [
    "is_x86_feature_detected",
    "is_aarch64_feature_detected",
    "is_arm_feature_detected",
    "is_wasm_feature_detected",
    "std_detect",
    "std::detect",
];
/// `simd128` が有効なときの `cfg` の形。
const SIMD128_CFG: &str = r#"#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]"#;
/// `simd128` が無効なときの `cfg` の形。
const NOT_SIMD128_CFG: &str =
    r#"#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]"#;
/// wasm32 の arch の字句。
const WASM_ARCH_WORDS: [&str; 2] = ["core::arch::wasm32", "std::arch::wasm32"];
/// `neon` が有効なときの `cfg` の形。
const NEON_CFG: &str = r#"#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]"#;
/// `neon` が無効なときの `cfg` の形。
const NOT_NEON_CFG: &str = r#"#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]"#;
/// aarch64 の arch の字句。
const AARCH64_ARCH_WORDS: [&str; 2] = ["core::arch::aarch64", "std::arch::aarch64"];
const ALLOW_UNSAFE: &str = "#[allow(unsafe_code)]";
const ROOT_FILE: &str = "cpu_ops.rs";
const SIMD128_FILE: &str = "cpu_ops/simd128.rs";
const NEON_FILE: &str = "cpu_ops/neon.rs";
/// 版の選び分けの関数と、それぞれの版が呼ぶ関数の名前。
const SELECTORS: [(&str, &str); 2] = [
    ("fn gemm_simd_impl(", "gemm"),
    ("fn elementwise_simd_impl(", "elementwise"),
];

/// 4 要素の `Relu` の腕が使う比較のマスクの組み込み関数。
const RELU_MASK_WORDS: [&str; 2] = ["vcgtq_f32", "vandq_u32"];
/// 4 要素の `Relu` の腕に置かない組み込み関数（NaN や -0.0 の扱いが比較のマスクと変わる）。
const RELU_FORBIDDEN_WORDS: [&str; 3] = ["vmaxq_f32", "vmaxnmq_f32", "vcgeq_f32"];

type Sources = Vec<(String, String)>;

/// `cpu_ops.rs` と `cpu_ops/` 直下の `.rs` を（ファイル名, 中身）で返す。読めない・0 件は失敗。
fn real_sources() -> Sources {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut out: Sources = Vec::new();
    let root = format!("{src}/{ROOT_FILE}");
    let body = std::fs::read_to_string(&root).unwrap_or_else(|e| panic!("cannot read {root}: {e}"));
    out.push((ROOT_FILE.to_string(), body));
    let dir = format!("{src}/cpu_ops");
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot list {dir}: {e}"));
    let mut sub: Sources = Vec::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("cannot list {dir}: {e}"))
            .path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 name");
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        sub.push((format!("cpu_ops/{name}"), body));
    }
    assert!(!sub.is_empty(), "no .rs files under {dir}");
    sub.sort();
    out.extend(sub);
    out
}

fn text_of<'a>(sources: &'a Sources, name: &str) -> Option<&'a str> {
    sources
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| t.as_str())
}

/// `decl` の行の直前の行が属性ならその行（`trim()` 済み）を返す。`decl` の行がちょうど 1 行でなければ `Err`。
fn attr_before(text: &str, decl: &str) -> Result<Option<String>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let hits: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim() == decl)
        .collect();
    if hits.len() != 1 {
        return Err(format!("expected one `{decl}`, found {}", hits.len()));
    }
    let i = hits[0];
    Ok((i > 0 && lines[i - 1].trim().starts_with("#[")).then(|| lines[i - 1].trim().to_string()))
}

/// `head` で始まる行ごとに、直前に続く属性の行（上から順）と本体の最初の `::` を含む行を返す。
fn items<'a>(text: &'a str, head: &str) -> Vec<(Vec<&'a str>, &'a str)> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if !l.starts_with(head) {
            continue;
        }
        let mut attrs = Vec::new();
        let mut j = i;
        while j > 0 && lines[j - 1].starts_with("#[") {
            j -= 1;
            attrs.insert(0, lines[j]);
        }
        let call = lines[i..]
            .iter()
            .find(|x| x.contains("::") && !x.contains("Result<"))
            .copied()
            .unwrap_or("");
        out.push((attrs, call));
    }
    out
}

/// `neon` の版の項目（`(e)`〜`(h)`）の問題を `out` に積む。
fn neon_problems(sources: &Sources, out: &mut Vec<String>) {
    let Some(root) = text_of(sources, ROOT_FILE) else {
        return;
    };
    // (e)
    for form in [NEON_CFG, NOT_NEON_CFG] {
        if !root.lines().any(|l| l.trim() == form) {
            out.push(format!("(e) {ROOT_FILE}: no `{form}` line"));
        }
    }
    // (f)
    if text_of(sources, NEON_FILE).is_none() {
        out.push(format!("(f) {NEON_FILE} is missing"));
    }
    for (name, text) in sources {
        if name == NEON_FILE {
            continue;
        }
        for word in AARCH64_ARCH_WORDS {
            if text.contains(word) {
                out.push(format!("(f) {name}: `{word}` outside {NEON_FILE}"));
            }
        }
    }
    // (g)
    let want = vec![NEON_CFG, ALLOW_UNSAFE];
    let neon_mods = items(root, "mod neon;");
    if neon_mods.len() != 1 || neon_mods[0].0 != want {
        out.push(format!(
            "(g) {ROOT_FILE}: `mod neon;` attributes are {neon_mods:?}"
        ));
    }
    for (name, text) in sources {
        let n = text.lines().filter(|l| l.trim() == ALLOW_UNSAFE).count();
        let limit = usize::from(name == ROOT_FILE);
        if n != limit || text.matches("allow(unsafe_code)").count() != limit {
            out.push(format!("(g) {name}: {n} `{ALLOW_UNSAFE}` line(s)"));
        }
    }
    // (i)
    if let Some(neon) = text_of(sources, NEON_FILE) {
        relu_mask_problems(neon, out);
    }
    // (h)
    for (head, op) in SELECTORS {
        let found = items(root, head);
        let rows: [(Vec<&str>, String); 3] = [
            (vec![SIMD128_CFG], format!("simd128::{op}(")),
            (vec![NEON_CFG], format!("neon::{op}(")),
            (
                vec![NOT_SIMD128_CFG, NOT_NEON_CFG],
                format!("scalar::{op}("),
            ),
        ];
        if found.len() != rows.len() {
            out.push(format!("(h) {ROOT_FILE}: {} `{head}` item(s)", found.len()));
        }
        for (attrs, call) in rows {
            if !found.iter().any(|(a, c)| *a == attrs && c.contains(&call)) {
                out.push(format!(
                    "(h) {ROOT_FILE}: no `{head}` with {attrs:?} calling `{call}`"
                ));
            }
        }
    }
}

/// `elementwise_lanes` の 4 要素の `Relu` の腕（`Relu =>` の行から `AddScalar` の腕の手前まで）を
/// 行末のコメントを除いて返す。`neon.rs` の外の同名の関数や端数の腕は見ない。
fn relu_lane_arm(neon: &str) -> Result<String, String> {
    let body = neon
        .split_once("fn elementwise_lanes(")
        .ok_or("no `fn elementwise_lanes(`")?
        .1;
    let (_, from) = body
        .split_once("UnaryElementwiseOp::Relu =>")
        .ok_or("no `Relu` arm in `elementwise_lanes`")?;
    let (arm, _) = from
        .split_once("UnaryElementwiseOp::AddScalar")
        .ok_or("no `AddScalar` arm after the `Relu` arm")?;
    let code: Vec<&str> = arm
        .lines()
        .map(|l| l.split_once("//").map_or(l, |(code, _)| code))
        .collect();
    Ok(code.join("\n"))
}

/// `(i)`: 4 要素の `Relu` の腕が比較のマスクで書かれていることの問題を積む。
fn relu_mask_problems(neon: &str, out: &mut Vec<String>) {
    let arm = match relu_lane_arm(neon) {
        Ok(a) => a,
        Err(e) => {
            out.push(format!("(i) {NEON_FILE}: {e}"));
            return;
        }
    };
    for word in RELU_MASK_WORDS {
        if !arm.contains(word) {
            out.push(format!("(i) {NEON_FILE}: lane `Relu` arm lacks `{word}`"));
        }
    }
    for word in RELU_FORBIDDEN_WORDS {
        if arm.contains(word) {
            out.push(format!("(i) {NEON_FILE}: lane `Relu` arm uses `{word}`"));
        }
    }
}

/// 問題の列を返す。先頭の `(a)`〜`(i)` が項目を表す。
fn problems(sources: &Sources) -> Vec<String> {
    let mut out = Vec::new();
    // (a)
    for (name, text) in sources {
        for word in DETECTION_WORDS {
            if text.contains(word) {
                out.push(format!("(a) {name}: runtime detection `{word}`"));
            }
        }
    }
    // (b)・(d)
    match text_of(sources, ROOT_FILE) {
        None => out.push(format!("(b) {ROOT_FILE} is missing")),
        Some(root) => {
            let count = |form: &str| root.lines().filter(|l| l.trim() == form).count();
            if count(SIMD128_CFG) == 0 {
                out.push(format!("(b) {ROOT_FILE}: no `{SIMD128_CFG}` line"));
            }
            if count(NOT_SIMD128_CFG) == 0 {
                out.push(format!("(b) {ROOT_FILE}: no `{NOT_SIMD128_CFG}` line"));
            }
            match attr_before(root, "mod simd128;") {
                Err(e) => out.push(format!("(d) {ROOT_FILE}: {e}")),
                Ok(Some(attr)) if attr == SIMD128_CFG => {}
                Ok(other) => out.push(format!(
                    "(d) {ROOT_FILE}: `mod simd128;` is preceded by {other:?}"
                )),
            }
        }
    }
    // (c)
    if text_of(sources, SIMD128_FILE).is_none() {
        out.push(format!("(c) {SIMD128_FILE} is missing"));
    }
    for (name, text) in sources {
        if name == SIMD128_FILE {
            continue;
        }
        for word in WASM_ARCH_WORDS {
            if text.contains(word) {
                out.push(format!("(c) {name}: `{word}` outside {SIMD128_FILE}"));
            }
        }
    }
    neon_problems(sources, &mut out);
    out
}

/// 指定ファイルの中身だけを `f` で置き換えた複製。
fn with_file(sources: &Sources, name: &str, f: impl Fn(&str) -> String) -> Sources {
    let mut found = false;
    let out = sources
        .iter()
        .map(|(n, t)| {
            if n == name {
                found = true;
                (n.clone(), f(t))
            } else {
                (n.clone(), t.clone())
            }
        })
        .collect();
    assert!(found, "{name} not in sources");
    out
}

/// 陽性対照の改変は必ず入力を変える。
fn changed(orig: &str, new: String) -> String {
    assert_ne!(orig, new, "mutation did not change the input");
    new
}

fn flagged(sources: &Sources, tag: &str) -> bool {
    problems(sources).iter().any(|p| p.starts_with(tag))
}

/// 実物のソースが 9 項目をすべて満たす（CPU-26 のポインタ）。
#[test]
fn selection_is_by_build_cfg_only() {
    let p = problems(&real_sources());
    assert!(p.is_empty(), "problems: {p:#?}");
}

#[test]
fn positive_control_runtime_detection() {
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        changed(
            t,
            format!("{t}\nfn probe() {{ if is_x86_feature_detected!(\"avx2\") {{}} }}\n"),
        )
    });
    assert!(flagged(&s, "(a)"));
}

#[test]
fn positive_control_runtime_detection_in_comment() {
    let s = with_file(&real_sources(), "cpu_ops/scalar.rs", |t| {
        changed(t, format!("{t}\n// std::detect を使わない\n"))
    });
    assert!(flagged(&s, "(a)"));
}

#[test]
fn positive_control_missing_not_cfg() {
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        let kept: Vec<&str> = t.lines().filter(|l| l.trim() != NOT_SIMD128_CFG).collect();
        changed(t, kept.join("\n"))
    });
    assert!(flagged(&s, "(b)"));
}

#[test]
fn positive_control_wasm32_arch_outside_simd128() {
    let s = with_file(&real_sources(), "cpu_ops/scalar.rs", |t| {
        changed(t, format!("{t}\nuse core::arch::wasm32::f32x4;\n"))
    });
    assert!(flagged(&s, "(c)"));
}

#[test]
fn positive_control_simd128_file_missing() {
    let s: Sources = real_sources()
        .into_iter()
        .filter(|(n, _)| n != SIMD128_FILE)
        .collect();
    assert!(flagged(&s, "(c)"));
}

#[test]
fn positive_control_simd128_mod_without_cfg() {
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        let lines: Vec<&str> = t.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.trim() == "mod simd128;")
            .expect("mod simd128; exists");
        // 直前の 1 行だけを消す（同じ形の他の行は残す）。
        let kept: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i + 1 != at)
            .map(|(_, l)| *l)
            .collect();
        changed(t, kept.join("\n"))
    });
    assert!(flagged(&s, "(d)"));
}

#[test]
fn positive_control_neon_selector_falls_to_scalar() {
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        changed(
            t,
            t.replace(
                "neon::gemm(a, b, m, k, n, out)",
                "scalar::gemm(a, b, m, k, n, out)",
            ),
        )
    });
    assert!(flagged(&s, "(h)"));
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        changed(
            t,
            t.replace(
                "neon::elementwise(op, a, out)",
                "scalar::elementwise(op, a, out)",
            ),
        )
    });
    assert!(flagged(&s, "(h)"));
}

#[test]
fn positive_control_neon_cfg_swapped_with_simd128() {
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        changed(
            t,
            t.replace(
                "simd128::gemm(a, b, m, k, n, out)",
                "neon::gemm(a, b, m, k, n, out)",
            ),
        )
    });
    assert!(flagged(&s, "(h)"));
}

#[test]
fn positive_control_scalar_selector_loses_not_neon() {
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        let kept: Vec<&str> = t.lines().filter(|l| l.trim() != NOT_NEON_CFG).collect();
        changed(t, kept.join("\n"))
    });
    assert!(flagged(&s, "(e)"));
    assert!(flagged(&s, "(h)"));
}

#[test]
fn positive_control_aarch64_arch_outside_neon() {
    let s = with_file(&real_sources(), "cpu_ops/scalar.rs", |t| {
        changed(t, format!("{t}\nuse core::arch::aarch64::vaddq_f32;\n"))
    });
    assert!(flagged(&s, "(f)"));
}

#[test]
fn positive_control_neon_file_missing() {
    let s: Sources = real_sources()
        .into_iter()
        .filter(|(n, _)| n != NEON_FILE)
        .collect();
    assert!(flagged(&s, "(f)"));
}

#[test]
fn positive_control_allow_outside_neon_mod() {
    let s = with_file(&real_sources(), "cpu_ops/scalar.rs", |t| {
        changed(t, format!("{t}\n#[allow(unsafe_code)]\nfn extra() {{}}\n"))
    });
    assert!(flagged(&s, "(g)"));
    let s = with_file(&real_sources(), ROOT_FILE, |t| {
        changed(
            t,
            t.replacen("#[allow(unsafe_code)]\nmod neon;", "mod neon;", 1),
        )
    });
    assert!(flagged(&s, "(g)"));
}

/// `neon.rs` の 4 要素の `Relu` の腕のマスクの 2 行を `repl` に置き換えた入力を作る。
fn with_relu_arm(repl: &str) -> Sources {
    const MASK: &str = "let mask = vcgtq_f32(v, vdupq_n_f32(0.0));\n                vreinterpretq_f32_u32(vandq_u32(vreinterpretq_u32_f32(v), mask))";
    with_file(&real_sources(), NEON_FILE, |t| {
        assert!(t.contains(MASK), "mask lines not found in {NEON_FILE}");
        changed(t, t.replacen(MASK, repl, 1))
    })
}

#[test]
fn positive_control_neon_relu_lane_by_scalar_relu() {
    let s = with_relu_arm(
        "let mut lanes = [0.0f32; 4];\n                for (l, &e) in lanes.iter_mut().zip(x) {\n                    *l = relu(e);\n                }\n                load(&lanes)",
    );
    let p = problems(&s);
    assert!(p.iter().any(|m| m.contains("lacks `vcgtq_f32`")), "{p:#?}");
    assert!(p.iter().any(|m| m.contains("lacks `vandq_u32`")), "{p:#?}");
}

#[test]
fn positive_control_neon_relu_mask_uses_ge() {
    let s = with_file(&real_sources(), NEON_FILE, |t| {
        changed(
            t,
            t.replacen(
                "let mask = vcgtq_f32(v, vdupq_n_f32(0.0));",
                "let mask = vcgeq_f32(v, vdupq_n_f32(0.0));",
                1,
            ),
        )
    });
    assert!(flagged(&s, "(i)"));
}

#[test]
fn positive_control_neon_relu_by_max() {
    let s = with_relu_arm("vmaxq_f32(v, vdupq_n_f32(0.0))");
    assert!(flagged(&s, "(i)"));
}

#[test]
fn positive_control_neon_relu_word_in_comment_does_not_count() {
    // 腕の中のコメントにだけ必要な語があっても通らない。
    let s = with_relu_arm("// vcgtq_f32 vandq_u32\n                vmaxq_f32(v, vdupq_n_f32(0.0))");
    let p = problems(&s);
    assert!(p.iter().any(|m| m.contains("lacks `vcgtq_f32`")), "{p:#?}");
}
