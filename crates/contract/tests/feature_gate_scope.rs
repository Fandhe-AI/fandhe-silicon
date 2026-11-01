//! feature `native-blocking-wait` と `ext-interop` が既定でも他の feature の含意でも有効にならないことの確認。
//!
//! contract の `Cargo.toml` の `[features]` を外部 crate なしで読んで確かめる
//! （spec TASK-233 の 1/4、BUILD-11〜13 のポインタ）。コンパイル失敗の側は
//! `src/cfg_gate_doctest.rs` の doctest が受け持つ。
//! あわせて `scripts/build-profiles.sh` の本番の 4 組（`# spec BUILD-1`〜`BUILD-4` の注記の行）のうち、
//! feature の含意をたどって `ext-interop` に届くのは BUILD-4 の組だけであることを確かめる
//! （TASK-233 の 6/7、BUILD-13 のポインタ）。想定外の形の行は黙って通さず panic で止める。

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

/// `scripts/build-profiles.sh` の `# spec BUILD-{n}` の注記の行から、本番の 4 組を BUILD-1〜4 の順で返す。
fn production_sets() -> Vec<Vec<String>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/build-profiles.sh"
    );
    let text = std::fs::read_to_string(path).expect("build-profiles.sh must be readable");
    (1..=4)
        .map(|n| {
            let tag = format!("# spec BUILD-{n}");
            let lines: Vec<&str> = text
                .lines()
                .filter(|l| l.trim_end().ends_with(&tag))
                .collect();
            assert_eq!(
                lines.len(),
                1,
                "expected exactly one line for {tag}: {lines:?}"
            );
            let line = lines[0];
            let inner = line
                .split('"')
                .nth(1)
                .unwrap_or_else(|| panic!("no quoted set: {line}"));
            let set: Vec<String> = inner
                .split(',')
                .map(|i| i.trim().to_string())
                .filter(|i| !i.is_empty())
                .collect();
            assert!(!set.is_empty(), "empty set: {line}");
            set
        })
        .collect()
}

/// `roots` から feature の含意をたどって届く feature の集合（`dep:` と `/` を含む項目は辿らない）。
fn closure(
    features: &[(String, Vec<String>)],
    roots: &[String],
) -> std::collections::BTreeSet<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack: Vec<String> = roots.to_vec();
    while let Some(name) = stack.pop() {
        if name.contains('/') || name.starts_with("dep:") || !seen.insert(name.clone()) {
            continue;
        }
        let (_, items) = features
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("feature {name} is not declared"));
        stack.extend(items.iter().cloned());
    }
    seen
}

/// 4 組のうち `ext-interop` に届くのが BUILD-4 の組だけでないときの違反を返す。
fn ext_interop_violations(features: &[(String, Vec<String>)], sets: &[Vec<String>]) -> Vec<String> {
    if sets.len() != 4 {
        return vec![format!("expected 4 production sets, got {}", sets.len())];
    }
    let mut out = Vec::new();
    for (i, set) in sets.iter().enumerate() {
        let reached = closure(features, set).contains("ext-interop");
        if reached != (i == 3) {
            out.push(format!("BUILD-{}: ext-interop reached = {reached}", i + 1));
        }
    }
    out
}

#[test]
fn ext_interop_is_reached_only_from_the_interop_production_set() {
    let v = ext_interop_violations(&features(), &production_sets());
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn ext_interop_check_catches_a_wrong_production_set() {
    let f = features();
    let base = production_sets();
    for i in 0..4 {
        let mut sets = base.clone();
        if i == 3 {
            sets[i].retain(|x| x != "ext-interop");
        } else {
            sets[i].push("ext-interop".to_string());
        }
        assert!(
            !ext_interop_violations(&f, &sets).is_empty(),
            "set {} change not caught",
            i + 1
        );
    }
    let mut sets = base.clone();
    sets.pop();
    assert!(!ext_interop_violations(&f, &sets).is_empty());
}

#[test]
fn ext_interop_check_follows_implications() {
    let mut f = features();
    let (_, items) = f
        .iter_mut()
        .find(|(n, _)| n == "backend-vulkan")
        .expect("backend-vulkan feature");
    items.push("ext-interop".to_string());
    assert!(!ext_interop_violations(&f, &production_sets()).is_empty());
}
