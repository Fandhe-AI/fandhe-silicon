//! 契約の crate の `default` feature と、テスト・診断専用 feature の関係の確認。
//!
//! `cargo metadata --no-deps` で契約の crate の features を読み（外部 crate なしの小さな JSON 読み取り）、
//! `default` の中身の完全一致・既定で無効な feature の不在・テスト・診断専用の 3 つが
//! `default` と本番の組の含意に入らないことを確かめる（spec TASK-226、BUILD-8・BUILD-65、本番の組は
//! BUILD-1〜BUILD-4 のポインタ）。`feature_gate_scope.rs`（TASK-233）は別の範囲として残す。
//! `cargo metadata` は有効な feature に左右されないので、どの feature の組で流しても結果は同じになる。
//! 判定は純粋関数 `violations` に分け、実物の features を 1 か所だけ変えた入力を渡す陽性対照を常設する。
//! 想定外の形は黙って通さず panic で止める。

use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, Stdio};

type Features = BTreeMap<String, Vec<String>>;

const PACKAGE: &str = "fandhe-silicon-contract";
const DEFAULT_EXPECTED: [&str; 3] = ["contract-core", "cpu-isa", "cpu-ops"];
const OFF_BY_DEFAULT: [&str; 6] = [
    "backend-metal",
    "backend-vulkan",
    "backend-cuda",
    "graphics",
    "ext-interop",
    "native-blocking-wait",
];
const TEST_ONLY: [&str; 3] = ["test-support", "fault-injection", "artifact-inspection"];
/// `scripts/build-profiles.sh` の `NATIVE_SETS` の BUILD-1〜BUILD-4 の注記がある 4 行の写し。
const PRODUCTION_SETS: [&str; 4] = [
    "contract-core,cpu-isa,cpu-ops",
    "contract-core,cpu-isa,cpu-ops,backend-metal",
    "contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics",
    "contract-core,cpu-isa,cpu-ops,backend-vulkan,backend-cuda,graphics,ext-interop",
];

// ---- 起動 ----

/// `cargo metadata --no-deps` の出力を返す。依存の解決もネットワークも要らない。
fn cargo_metadata_json() -> String {
    let out = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .stdin(Stdio::null())
        .output()
        .expect("cargo metadata must start");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("cargo metadata output must be UTF-8")
}

// ---- JSON 読み取り ----

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(String),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at {}", c as char, self.i))
        }
    }

    fn lit(&mut self, s: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            Ok(v)
        } else {
            Err(format!("bad literal at {}", self.i))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.b.get(self.i) {
            None => Err("unexpected end".into()),
            Some(b'n') => self.lit("null", Json::Null),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(format!("bad array at {}", self.i)),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Object(items));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    self.eat(b':')?;
                    items.push((k, self.value()?));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Object(items));
                        }
                        _ => return Err(format!("bad object at {}", self.i)),
                    }
                }
            }
            Some(c) if *c == b'-' || c.is_ascii_digit() => {
                let s = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                Ok(Json::Number(
                    String::from_utf8_lossy(&self.b[s..self.i]).into(),
                ))
            }
            Some(_) => Err(format!("unexpected byte at {}", self.i)),
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self
            .b
            .get(self.i..self.i + 4)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .ok_or_else(|| format!("bad \\u escape at {}", self.i))?;
        self.i += 4;
        Ok(h)
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                self.eat(b'\\')?;
                                self.eat(b'u')?;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err("bad surrogate pair".into());
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            char::from_u32(cp).ok_or("bad code point")?
                        }
                        _ => return Err(format!("bad escape at {}", self.i)),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
    }
}

fn parse_json(s: &str) -> Result<Json, String> {
    let mut p = Parser {
        b: s.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("trailing data at {}", p.i));
    }
    Ok(v)
}

fn get<'a>(v: &'a Json, key: &str) -> Option<&'a Json> {
    match v {
        Json::Object(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

/// `packages[].name` がちょうど 1 件一致する package の `features` を返す。0 件・2 件以上・形の違いは panic。
fn contract_features(json: &str) -> Features {
    let root = parse_json(json).expect("metadata must be valid JSON");
    let Some(Json::Array(pkgs)) = get(&root, "packages") else {
        panic!("packages must be an array");
    };
    let hits: Vec<&Json> = pkgs
        .iter()
        .filter(|p| matches!(get(p, "name"), Some(Json::Str(n)) if n == PACKAGE))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly one package named {PACKAGE} expected"
    );
    let Some(Json::Object(feats)) = get(hits[0], "features") else {
        panic!("features must be an object");
    };
    feats
        .iter()
        .map(|(k, v)| {
            let Json::Array(items) = v else {
                panic!("feature {k} must be an array");
            };
            let list = items
                .iter()
                .map(|i| match i {
                    Json::Str(s) => s.clone(),
                    _ => panic!("feature {k} items must be strings"),
                })
                .collect();
            (k.clone(), list)
        })
        .collect()
}

// ---- 判定 ----

/// 有効化リストを辿った閉包。他の crate への転送（`/` を含む）と `dep:` は辿らない。
/// 辿る名前が宣言に無ければ `problems` に積む。
fn closure(map: &Features, roots: &[String], problems: &mut Vec<String>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<String> = roots.to_vec();
    while let Some(n) = stack.pop() {
        if n.contains('/') || n.starts_with("dep:") || !seen.insert(n.clone()) {
            continue;
        }
        match map.get(&n) {
            Some(next) => stack.extend(next.iter().cloned()),
            None => problems.push(format!("feature {n} is referenced but not declared")),
        }
    }
    seen
}

fn split_set(s: &str) -> Vec<String> {
    s.split(',').map(str::to_string).collect()
}

fn violations(map: &Features) -> Vec<String> {
    let mut v = Vec::new();
    for n in OFF_BY_DEFAULT.iter().chain(TEST_ONLY.iter()) {
        if !map.contains_key(*n) {
            v.push(format!("feature {n} is not declared"));
        }
    }
    let Some(default) = map.get("default") else {
        v.push("default is not declared".into());
        return v;
    };
    let got: BTreeSet<&str> = default.iter().map(String::as_str).collect();
    let want: BTreeSet<&str> = DEFAULT_EXPECTED.into_iter().collect();
    if got != want || default.len() != DEFAULT_EXPECTED.len() {
        v.push(format!("default must be exactly {want:?}, got {default:?}"));
    }
    let mut scopes: Vec<(String, Vec<String>)> = vec![("default".into(), default.clone())];
    for s in PRODUCTION_SETS {
        scopes.push((format!("production set {s}"), split_set(s)));
    }
    for (label, roots) in &scopes {
        let reach = closure(map, roots, &mut v);
        let forbidden: &[&str] = if label == "default" {
            &[&OFF_BY_DEFAULT[..], &TEST_ONLY[..]].concat()
        } else {
            &TEST_ONLY
        };
        for f in forbidden {
            if reach.contains(*f) {
                v.push(format!("{f} is reachable from {label}"));
            }
        }
    }
    match map.get("artifact-inspection") {
        Some(l) if l.iter().any(|i| i == "backend-metal") => {}
        _ => v.push("artifact-inspection must require backend-metal".into()),
    }
    v
}

// ---- 試験 ----

fn real() -> Features {
    contract_features(&cargo_metadata_json())
}

#[test]
fn real_manifest_has_no_violations() {
    let v = violations(&real());
    assert!(v.is_empty(), "violations: {v:#?}");
}

#[test]
fn production_sets_match_build_profiles_script() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/build-profiles.sh"
    );
    let text = std::fs::read_to_string(path).expect("build-profiles.sh must be readable");
    for (n, want) in PRODUCTION_SETS.iter().enumerate() {
        let tag = format!("# spec BUILD-{}", n + 1);
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| l.trim_end().ends_with(&tag))
            .collect();
        assert_eq!(lines.len(), 1, "exactly one line tagged {tag} expected");
        let inner = lines[0]
            .split('"')
            .nth(1)
            .unwrap_or_else(|| panic!("no quoted set on line: {}", lines[0]));
        assert_eq!(inner, *want, "set for {tag} differs");
    }
}

#[test]
fn json_reader_handles_nesting_and_escapes() {
    let j = parse_json(
        r#" {"a":[1,-2.5e3,true,false,null,{"b":"x\"\\\/\b\f\n\r\t\u0041\ud83d\ude00"}],"c":{}} "#,
    )
    .expect("valid");
    let Some(Json::Array(a)) = get(&j, "a") else {
        panic!()
    };
    assert_eq!(a.len(), 6);
    assert_eq!(a[1], Json::Number("-2.5e3".into()));
    assert_eq!(
        get(&a[5], "b"),
        Some(&Json::Str("x\"\\/\u{8}\u{c}\n\r\tA\u{1F600}".into()))
    );
    assert_eq!(get(&j, "c"), Some(&Json::Object(vec![])));
}

#[test]
fn json_reader_rejects_malformed_input() {
    for bad in [
        "{",
        "[1,",
        "{\"a\":1} x",
        "\"abc",
        "{\"a\" 1}",
        "[1 2]",
        "\"\\ud83d\"",
        "tru",
    ] {
        assert!(parse_json(bad).is_err(), "must reject: {bad}");
    }
}

fn pkg(name: &str, feat: &str) -> String {
    format!(r#"{{"name":"{name}","features":{{{feat}}}}}"#)
}

#[test]
fn contract_features_picks_the_named_package() {
    let json = format!(
        r#"{{"packages":[{},{}]}}"#,
        pkg("other", r#""x":["y"]"#),
        pkg(PACKAGE, r#""default":["a"],"a":[]"#)
    );
    let m = contract_features(&json);
    assert_eq!(m.len(), 2);
    assert_eq!(m["default"], vec!["a".to_string()]);
}

#[test]
#[should_panic(expected = "exactly one package")]
fn contract_features_panics_without_the_package() {
    contract_features(&format!(r#"{{"packages":[{}]}}"#, pkg("other", "")));
}

#[test]
#[should_panic(expected = "exactly one package")]
fn contract_features_panics_with_duplicate_packages() {
    let p = pkg(PACKAGE, "");
    contract_features(&format!(r#"{{"packages":[{p},{p}]}}"#));
}

#[test]
#[should_panic(expected = "items must be strings")]
fn contract_features_panics_on_non_string_items() {
    contract_features(&format!(
        r#"{{"packages":[{}]}}"#,
        pkg(PACKAGE, r#""a":[1]"#)
    ));
}

// ---- 陽性対照（実物の features を 1 か所だけ変えると落ちること） ----

fn assert_caught(mutate: impl FnOnce(&mut Features), what: &str) {
    let mut m = real();
    mutate(&mut m);
    assert!(!violations(&m).is_empty(), "not caught: {what}");
}

#[test]
fn control_off_by_default_feature_added_to_default() {
    for f in OFF_BY_DEFAULT {
        assert_caught(|m| m.get_mut("default").unwrap().push(f.into()), f);
    }
}

#[test]
fn control_test_only_feature_added_to_default() {
    for f in TEST_ONLY {
        assert_caught(|m| m.get_mut("default").unwrap().push(f.into()), f);
    }
}

#[test]
fn control_test_only_feature_implied_through_default() {
    assert_caught(
        |m| m.get_mut("cpu-ops").unwrap().push("fault-injection".into()),
        "cpu-ops -> fault-injection",
    );
}

#[test]
fn control_test_only_feature_implied_through_production_sets() {
    assert_caught(
        |m| {
            m.get_mut("backend-vulkan")
                .unwrap()
                .push("test-support".into())
        },
        "backend-vulkan -> test-support",
    );
    assert_caught(
        |m| {
            m.get_mut("backend-metal")
                .unwrap()
                .push("artifact-inspection".into())
        },
        "backend-metal -> artifact-inspection",
    );
}

#[test]
fn control_artifact_inspection_loses_backend_metal() {
    assert_caught(
        |m| m.get_mut("artifact-inspection").unwrap().clear(),
        "artifact-inspection emptied",
    );
}

#[test]
fn control_default_removed_or_shrunk() {
    assert_caught(|m| drop(m.remove("default")), "default removed");
    assert_caught(
        |m| m.get_mut("default").unwrap().retain(|i| i != "cpu-ops"),
        "cpu-ops dropped from default",
    );
}

#[test]
fn control_gated_feature_removed() {
    assert_caught(|m| drop(m.remove("graphics")), "graphics removed");
    assert_caught(|m| drop(m.remove("test-support")), "test-support removed");
}
