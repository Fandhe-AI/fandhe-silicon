//! プロファイルの切り替えが契約の crate の feature の組とモジュールの `cfg` の単位で行われ、
//! プロファイルごとの crate やプロファイルの名前の feature が無いことを固定する試験（spec TASK-227、BUILD-9 のポインタ）。
//!
//! 見るのは次の 4 点で、`default` の中身や既定で無効な feature の扱いは `feature_default_scope.rs` が、
//! 背後実装と描画のモジュールがコンパイルで見えることは `backend_cfg_gate.rs` が確かめるので重ねない。
//! - (a) ルートの `members` が 5 件で、各 member の `[package] name` にプロファイルを表す語が無い。
//! - (b) 5 つの member の `[features]` の名前にプロファイルを表す語が無い。
//! - (c) `scripts/build-profiles.sh` の `# spec BUILD-1`〜`BUILD-3` の注記の行の組が `default` を土台にした形になる。
//! - (d) 背後実装と描画のモジュールの宣言の直前の属性に、対応する feature の `#[cfg(...)]` がある。
//!
//! 外部 crate もコマンドの起動も使わず、manifest・スクリプト・`lib.rs` の文字列だけを読む。
//! 判定は純粋関数 `problems` に分け、実物を 1 か所だけ変えた入力を渡す陽性対照を常設する。
//! 想定外の形は黙って通さず、問題として積んで落とす。
//! 条件のない試験なので、`make verify` の `cargo test` と `build-profiles.sh` のどの組でも結果は同じになる。

use std::collections::{BTreeMap, BTreeSet};

/// workspace の member（ルートからの相対パス）。
const MEMBERS: [&str; 5] = [
    "crates/core",
    "crates/contract",
    "crates/exec",
    "crates/upper",
    "crates/wasm-abi",
];
const CONTRACT: &str = "crates/contract";
/// プロファイルを表す語（小文字・`-` 区切りに正規化した名前への部分一致）。
const PROFILE_WORDS: [&str; 5] = [
    "cpu-only",
    "compute-only",
    "cpuonly",
    "computeonly",
    "profile",
];

struct Inputs {
    root_manifest: String,
    /// `MEMBERS` の各パスと、その `Cargo.toml` の本文。
    manifests: Vec<(String, String)>,
    build_profiles: String,
    contract_lib: String,
    core_lib: String,
}

fn read(rel: &str) -> String {
    let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn real_inputs() -> Inputs {
    Inputs {
        root_manifest: read("Cargo.toml"),
        manifests: MEMBERS
            .iter()
            .map(|m| (m.to_string(), read(&format!("{m}/Cargo.toml"))))
            .collect(),
        build_profiles: read("scripts/build-profiles.sh"),
        contract_lib: read("crates/contract/src/lib.rs"),
        core_lib: read("crates/core/src/lib.rs"),
    }
}

/// 節の見出し（`[workspace]` など）と、その節の中の行（行末のコメントと空白を除いたもの）を順に返す。
fn lines_by_section(text: &str) -> Vec<(String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = line.to_string();
            continue;
        }
        out.push((section.clone(), line.to_string()));
    }
    out
}

/// `key = value` の行を分ける。
fn key_value(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}

/// 1 行の配列 `[a, b]` の要素（引用符を除く）。
fn one_line_array(value: &str) -> Result<Vec<String>, String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| format!("not a one-line array: {value}"))?;
    Ok(inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// ルートの `[workspace]` の `members`（ちょうど 1 行の、glob を含まない配列）。
fn workspace_members(root: &str) -> Result<Vec<String>, String> {
    let hits: Vec<String> = lines_by_section(root)
        .into_iter()
        .filter(|(s, _)| s == "[workspace]")
        .filter_map(|(_, l)| match key_value(&l) {
            Some(("members", v)) => Some(v.to_string()),
            _ => None,
        })
        .collect();
    if hits.len() != 1 {
        return Err(format!("expected one members line, found {}", hits.len()));
    }
    let members = one_line_array(&hits[0])?;
    if members.iter().any(|m| m.contains('*') || m.contains('?')) {
        return Err(format!("members contains a glob: {members:?}"));
    }
    Ok(members)
}

/// `[package]` の節の `name`（`[[test]]` の `name` は読まない）。
fn package_name(manifest: &str) -> Result<String, String> {
    let hits: Vec<String> = lines_by_section(manifest)
        .into_iter()
        .filter(|(s, _)| s == "[package]")
        .filter_map(|(_, l)| match key_value(&l) {
            Some(("name", v)) => Some(v.trim_matches('"').to_string()),
            _ => None,
        })
        .collect();
    match hits.as_slice() {
        [one] => Ok(one.clone()),
        _ => Err(format!("expected one [package] name, found {}", hits.len())),
    }
}

/// `[features]` の「名前 -> 要素」。節が無ければ空。
fn features(manifest: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut out = BTreeMap::new();
    for (s, l) in lines_by_section(manifest) {
        if s != "[features]" {
            continue;
        }
        let (k, v) = key_value(&l).ok_or_else(|| format!("bad features line: {l}"))?;
        out.insert(k.to_string(), one_line_array(v)?);
    }
    Ok(out)
}

fn normalize(name: &str) -> String {
    name.to_lowercase().replace('_', "-")
}

fn profile_word(name: &str) -> Option<&'static str> {
    let n = normalize(name);
    PROFILE_WORDS.iter().copied().find(|w| n.contains(w))
}

/// 行末（末尾の空白を除く）が `# spec BUILD-<n>` で終わる行がちょうど 1 行あるとき、その最初の引用符の中身の集合。
fn tagged_set(script: &str, n: u32) -> Result<BTreeSet<String>, String> {
    let tag = format!("# spec BUILD-{n}");
    let lines: Vec<&str> = script
        .lines()
        .filter(|l| l.trim_end().ends_with(&tag))
        .collect();
    if lines.len() != 1 {
        return Err(format!("expected one `{tag}` line, found {}", lines.len()));
    }
    let line = lines[0];
    let start = line.find('"').ok_or_else(|| format!("no quote: {line}"))?;
    let rest = &line[start + 1..];
    let end = rest
        .find('"')
        .ok_or_else(|| format!("unterminated: {line}"))?;
    Ok(rest[..end].split(',').map(str::to_string).collect())
}

/// `decl` の行がちょうど 1 行あるとき、その直前から上へ連続する `#[...]` の行。
fn attrs_before(lib: &str, decl: &str) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = lib.lines().collect();
    let hits: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim() == decl)
        .collect();
    if hits.len() != 1 {
        return Err(format!("expected one `{decl}`, found {}", hits.len()));
    }
    let mut attrs = Vec::new();
    let mut i = hits[0];
    while i > 0 && lines[i - 1].trim().starts_with("#[") {
        i -= 1;
        attrs.push(lines[i].trim().to_string());
    }
    Ok(attrs)
}

/// (c) 注記の行の組の関係を確かめる。
fn check_sets(inp: &Inputs, f: &BTreeMap<String, Vec<String>>, out: &mut Vec<String>) {
    let default: BTreeSet<String> = f
        .get("default")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();
    if default.is_empty() {
        out.push("contract default is missing".to_string());
    }
    let sets: Vec<Result<BTreeSet<String>, String>> = (1..=3)
        .map(|n| tagged_set(&inp.build_profiles, n))
        .collect();
    for s in &sets {
        if let Err(e) = s {
            out.push(e.clone());
        }
    }
    let [Ok(b1), Ok(b2), Ok(b3)] = sets.as_slice() else {
        return;
    };
    if *b1 != default {
        out.push(format!("BUILD-1 set differs from default: {b1:?}"));
    }
    let extra2: Vec<&String> = b2.difference(b1).collect();
    if !b1.is_subset(b2)
        || extra2.len() != 1
        || !extra2[0].starts_with("backend-")
        || b2.contains("graphics")
    {
        out.push(format!("BUILD-2 set is not base plus one backend: {b2:?}"));
    }
    let extra3: Vec<&String> = b3.difference(b1).collect();
    let backends = extra3.iter().filter(|n| n.starts_with("backend-")).count();
    if !b1.is_subset(b3)
        || !b3.contains("graphics")
        || backends == 0
        || extra3.len() != backends + 1
    {
        out.push(format!(
            "BUILD-3 set is not base plus backends plus graphics: {b3:?}"
        ));
    }
    for n in b1.iter().chain(b2).chain(b3) {
        if !f.contains_key(n) {
            out.push(format!("feature {n} is not declared in the contract crate"));
        }
    }
}

/// 違反を文字列で返す。空なら構成は期待どおり。
fn problems(inp: &Inputs) -> Vec<String> {
    let mut out = Vec::new();

    // (a) member の構成と package 名。
    match workspace_members(&inp.root_manifest) {
        Ok(m) => {
            let got: BTreeSet<&str> = m.iter().map(String::as_str).collect();
            let want: BTreeSet<&str> = MEMBERS.iter().copied().collect();
            if m.len() != MEMBERS.len() || got != want {
                out.push(format!("workspace members differ: {m:?}"));
            }
        }
        Err(e) => out.push(e),
    }
    let mut contract_features = None;
    for (path, text) in &inp.manifests {
        match package_name(text) {
            Ok(n) => {
                if let Some(w) = profile_word(&n) {
                    out.push(format!("package name {n} contains profile word {w}"));
                }
            }
            Err(e) => out.push(format!("{path}: {e}")),
        }
        // (b) feature の名前。
        match features(text) {
            Ok(f) => {
                for name in f.keys() {
                    if let Some(w) = profile_word(name) {
                        out.push(format!("{path}: feature {name} contains profile word {w}"));
                    }
                }
                if path == CONTRACT {
                    contract_features = Some(f);
                }
            }
            Err(e) => out.push(format!("{path}: {e}")),
        }
    }

    // (c) 注記の行の組。
    match contract_features.filter(|f| !f.is_empty()) {
        None => out.push("contract [features] is missing or unreadable".to_string()),
        Some(f) => check_sets(inp, &f, &mut out),
    }

    // (d) モジュールの宣言の直前の属性。
    for chip in ["metal", "vulkan", "cuda"] {
        let feat = format!("feature = \"backend-{chip}\"");
        match attrs_before(&inp.contract_lib, &format!("pub mod backend_{chip};")) {
            Ok(a)
                if a.iter()
                    .any(|l| l.starts_with("#[cfg(") && l.contains(&feat)) => {}
            Ok(a) => out.push(format!("backend_{chip} lacks its cfg: {a:?}")),
            Err(e) => out.push(e),
        }
    }
    match attrs_before(&inp.core_lib, "pub mod graphics;") {
        Ok(a) if a.iter().any(|l| l == "#[cfg(feature = \"graphics\")]") => {}
        Ok(a) => out.push(format!("graphics module lacks its cfg: {a:?}")),
        Err(e) => out.push(e),
    }
    out
}

/// 本文が実際に変わったことを確かめてから返す（対照が空振りしないように）。
fn changed(orig: &str, new: String) -> String {
    assert_ne!(orig, new, "mutation must change the text");
    new
}

/// `trim` が `line` と等しい行を取り除く。
fn drop_line(text: &str, line: &str) -> String {
    let kept: Vec<&str> = text.lines().filter(|l| l.trim() != line).collect();
    changed(text, kept.join("\n") + "\n")
}

fn manifest_mut<'a>(inp: &'a mut Inputs, path: &str) -> &'a mut String {
    &mut inp
        .manifests
        .iter_mut()
        .find(|(p, _)| p == path)
        .expect("member")
        .1
}

fn assert_flagged(inp: &Inputs, what: &str) {
    assert!(!problems(inp).is_empty(), "no problem reported for: {what}");
}

#[test]
fn real_inputs_have_no_problems() {
    let p = problems(&real_inputs());
    assert!(p.is_empty(), "unexpected problems: {p:?}");
}

#[test]
fn profile_words_exclude_graphics_and_match_normalized_names() {
    assert!(!PROFILE_WORDS.iter().any(|w| w.contains("graphics")));
    assert_eq!(profile_word("Cpu_Only"), Some("cpu-only"));
    assert_eq!(profile_word("profile-x"), Some("profile"));
    assert_eq!(profile_word("graphics"), None);
    assert_eq!(profile_word("cpu-ops"), None);
}

#[test]
fn reader_ignores_test_name_and_follows_sections() {
    let m = "[package]\nname = \"a\" # c\n[[test]]\nname = \"stub_device_profiles\"\n[target.'cfg(x)'.dependencies]\nz = \"1\"\n[features]\nf = []\ng = [\"f\"]\n";
    assert_eq!(package_name(m).as_deref(), Ok("a"));
    let f = features(m).expect("features");
    assert_eq!(f.keys().cloned().collect::<Vec<_>>(), vec!["f", "g"]);
    assert!(
        features("[package]\nname = \"a\"\n")
            .expect("none")
            .is_empty()
    );
}

#[test]
fn reader_rejects_unreadable_shapes() {
    assert!(workspace_members("[workspace]\nmembers = [\n \"a\",\n]\n").is_err());
    assert!(workspace_members("[workspace]\nmembers = [\"crates/*\"]\n").is_err());
    assert!(workspace_members("[workspace]\nresolver = \"3\"\n").is_err());
    assert_eq!(
        workspace_members("[workspace]\nmembers = [\"a\", \"b\"]\n"),
        Ok(vec!["a".to_string(), "b".to_string()])
    );
    assert!(features("[features]\nf = [\n \"a\",\n]\n").is_err());
}

#[test]
fn control_members_added_removed_or_glob() {
    let orig = real_inputs();
    let line = "members = [\"crates/core\", \"crates/contract\", \"crates/exec\", \"crates/upper\", \"crates/wasm-abi\"]";
    assert!(
        orig.root_manifest.contains(line),
        "root members line changed shape"
    );
    for new_line in [
        "members = [\"crates/core\", \"crates/contract\", \"crates/exec\", \"crates/upper\", \"crates/wasm-abi\", \"crates/cpu-only\"]",
        "members = [\"crates/core\", \"crates/contract\", \"crates/exec\", \"crates/upper\"]",
        "members = [\"crates/*\"]",
    ] {
        let mut inp = real_inputs();
        inp.root_manifest = changed(
            &orig.root_manifest,
            orig.root_manifest.replace(line, new_line),
        );
        assert_flagged(&inp, new_line);
    }
}

#[test]
fn control_package_name_with_profile_word() {
    let mut inp = real_inputs();
    let t = manifest_mut(&mut inp, "crates/exec");
    let new = changed(
        t,
        t.replacen(
            "name = \"fandhe-silicon-exec\"",
            "name = \"fandhe-silicon-cpu-only\"",
            1,
        ),
    );
    *t = new;
    assert_flagged(&inp, "package name");
}

#[test]
fn control_profile_feature_in_any_member() {
    for path in MEMBERS {
        let mut inp = real_inputs();
        let t = manifest_mut(&mut inp, path);
        let new = if t.contains("\n[features]\n") {
            changed(
                t,
                t.replacen("\n[features]\n", "\n[features]\nprofile-cpu-only = []\n", 1),
            )
        } else {
            changed(t, format!("{t}\n[features]\nprofile-cpu-only = []\n"))
        };
        *t = new;
        assert_flagged(&inp, path);
    }
}

#[test]
fn control_build_set_notes() {
    let orig = real_inputs();
    let bp = &orig.build_profiles;
    let line_of = |n: u32| -> String {
        bp.lines()
            .find(|l| l.trim_end().ends_with(&format!("# spec BUILD-{n}")))
            .expect("note line")
            .to_string()
    };
    let (l1, l2, l3) = (line_of(1), line_of(2), line_of(3));
    let cases: Vec<(String, String)> = vec![
        (l2.clone(), l2.replacen('"', "\"graphics,", 1)),
        (
            l2.clone(),
            l2.replacen("backend-metal", "backend-metal,backend-cuda", 1),
        ),
        (l3.clone(), l3.replace(",graphics", "")),
        (l1.clone(), l1.replacen('"', "\"undeclared-name,", 1)),
        (l1.clone(), format!("{l1}\n{l1}")),
    ];
    for (from, to) in cases {
        let mut inp = real_inputs();
        inp.build_profiles = changed(bp, bp.replacen(&from, &to, 1));
        assert_flagged(&inp, &to);
    }
}

#[test]
fn control_missing_cfg_attributes() {
    let orig = real_inputs();
    for attr in [
        "#[cfg(feature = \"backend-metal\")]",
        "#[cfg(all(feature = \"backend-vulkan\", not(target_arch = \"wasm32\")))]",
        "#[cfg(all(feature = \"backend-cuda\", not(target_arch = \"wasm32\")))]",
    ] {
        let mut inp = real_inputs();
        inp.contract_lib = drop_line(&orig.contract_lib, attr);
        assert_flagged(&inp, attr);
    }
    let mut inp = real_inputs();
    inp.core_lib = drop_line(&orig.core_lib, "#[cfg(feature = \"graphics\")]");
    assert_flagged(&inp, "graphics cfg");
}

#[test]
fn control_missing_contract_features_section() {
    let mut inp = real_inputs();
    let t = manifest_mut(&mut inp, CONTRACT);
    let new = changed(t, t.replacen("[features]", "[featuresx]", 1));
    *t = new;
    assert_flagged(&inp, "contract features");
}
