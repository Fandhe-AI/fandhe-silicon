//! feature `native-blocking-wait` と `ext-interop` が既定でも他の feature の含意でも有効にならないことの確認。
//!
//! contract の `Cargo.toml` の `[features]` を外部 crate なしで読んで確かめる
//! （spec TASK-233 の 1/4、BUILD-11〜13 のポインタ）。コンパイル失敗の側は
//! `src/cfg_gate_doctest.rs` の doctest が受け持つ。想定外の形の行は黙って通さず panic で止める。

const GATED: [&str; 2] = ["native-blocking-wait", "ext-interop"];

/// `[features]` の (名前, 有効化リスト) を返す。複数行の配列など読めない形は panic。
fn features() -> Vec<(String, Vec<String>)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let text = std::fs::read_to_string(path).expect("Cargo.toml must be readable");
    let mut in_features = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_features = line == "[features]";
            continue;
        }
        if !in_features || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, rest) = line
            .split_once('=')
            .unwrap_or_else(|| panic!("unreadable feature line: {line}"));
        let rest = rest.split('#').next().unwrap_or("").trim();
        let inner = rest
            .strip_prefix('[')
            .and_then(|r| r.strip_suffix(']'))
            .unwrap_or_else(|| panic!("feature list must be a one-line array: {line}"));
        let items = inner
            .split(',')
            .map(|i| i.trim().trim_matches('"').to_string())
            .filter(|i| !i.is_empty())
            .collect();
        out.push((name.trim().to_string(), items));
    }
    assert!(!out.is_empty(), "no [features] entries found");
    out
}

#[test]
fn gated_features_are_declared() {
    let f = features();
    for g in GATED {
        assert!(f.iter().any(|(n, _)| n == g), "feature {g} is not declared");
    }
}

#[test]
fn gated_features_are_not_in_default() {
    let f = features();
    let (_, default) = f
        .iter()
        .find(|(n, _)| n == "default")
        .expect("default feature");
    for g in GATED {
        assert!(
            !default.iter().any(|i| i == g),
            "{g} must not be in default: {default:?}"
        );
    }
}

#[test]
fn gated_features_are_not_implied_by_other_features() {
    for (name, items) in features() {
        for g in GATED {
            assert!(
                !items.iter().any(|i| i == g),
                "feature {name} must not enable {g}: {items:?}"
            );
        }
    }
}
