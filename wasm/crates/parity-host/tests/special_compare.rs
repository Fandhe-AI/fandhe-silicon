//! 照合スクリプトの `--recount` の `recount:`・`recount-cpu:` の件数を、共通の道具の
//! `T1` と `compare_f32_with_special_values` の答えと突き合わせる試験（spec TASK-999 のポインタ。
//! 閾値の比べの所在は TASK-995、語の規則の所在は TASK-25）。特殊値の 2 種類の保存の形と、
//! 既存の 2 種類（ビット同一の比べのまま）の両方を流す。期待は共通の道具の呼び出しと式で作る。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::T1;
use wasm_parity_host::{ELEMENTWISE_SPECIAL_CASES, GEMM_SPECIAL_CASES, f32_le_bytes, generate};

/// 試験ごとの一時ディレクトリ。破棄で消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("wasm-parity-special-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        TempDir(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

/// 保存の形の種類。
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    GemmSpecial,
    ElementwiseSpecial,
    Gemm,
    Elementwise,
}

type Combo = (&'static str, &'static str);

const GEMM_SCALAR: [Combo; 2] = [("gemm_scalar", "scalar"), ("gemm_scalar", "simd128")];
const GEMM_ALL: [Combo; 4] = [
    ("gemm_scalar", "scalar"),
    ("gemm_scalar", "simd128"),
    ("gemm_simd", "scalar"),
    ("gemm_simd", "simd128"),
];
const ELEMENTWISE_ALL: [Combo; 4] = [
    ("elementwise_scalar", "scalar"),
    ("elementwise_scalar", "simd128"),
    ("elementwise_simd", "scalar"),
    ("elementwise_simd", "simd128"),
];

/// `outputs.json` の 1 行分の指定。`bytes` は行の出力のバイト列。
struct Row {
    id: String,
    export: &'static str,
    build: &'static str,
    case: String,
    bytes: Vec<u8>,
}

/// `dir/name` にバイト列を書き、sha256 の 16 進を返す。
fn file(dir: &Path, name: &str, bytes: &[u8]) -> String {
    fs::write(dir.join(name), bytes).unwrap();
    hex(&sha256_bytes(bytes))
}

/// 参照ファイルの鍵の断片（特殊値の種類だけ。ほかは空）。
fn cpu_part(dir: &Path, shape: Shape, cpu: &[f32]) -> String {
    if matches!(shape, Shape::GemmSpecial | Shape::ElementwiseSpecial) {
        let sha = file(dir, "c.cpu-reference.bin", &f32_le_bytes(cpu));
        format!(
            ", \"cpu_reference_file\": \"c.cpu-reference.bin\", \"cpu_reference_sha256\": \"{sha}\""
        )
    } else {
        String::new()
    }
}

/// ケース `c`（語数はネイティブの語数）の保存の形を `dir` に書く。`cpu` は特殊値の種類だけが使う。
fn write_form(dir: &Path, shape: Shape, native: &[f32], cpu: &[f32], rows: &[Row]) {
    let len = native.len();
    let nat_sha = file(dir, "c.native-scalar.bin", &f32_le_bytes(native));
    let cpu_part = cpu_part(dir, shape, cpu);
    let manifest = match shape {
        Shape::Gemm | Shape::GemmSpecial => {
            let a_sha = file(dir, "c.a.bin", &f32_le_bytes(&[1.0]));
            let b_sha = file(dir, "c.b.bin", &f32_le_bytes(&vec![1.0; len]));
            let kind = if shape == Shape::Gemm {
                "gemm"
            } else {
                "gemm-special"
            };
            format!(
                concat!(
                    "{{\"cases\": [{{\"name\": \"c\", \"kind\": \"{kind}\", \"m\": 1, \"k\": 1, \"n\": {len}, ",
                    "\"a_file\": \"c.a.bin\", \"b_file\": \"c.b.bin\", \"native_scalar_file\": \"c.native-scalar.bin\", ",
                    "\"a_sha256\": \"{a_sha}\", \"b_sha256\": \"{b_sha}\", \"native_scalar_sha256\": \"{nat_sha}\"{cpu_part}}}]}}",
                ),
                kind = kind,
                len = len,
                a_sha = a_sha,
                b_sha = b_sha,
                nat_sha = nat_sha,
                cpu_part = cpu_part
            )
        }
        Shape::Elementwise | Shape::ElementwiseSpecial => {
            let in_sha = file(dir, "c.input.bin", &f32_le_bytes(&vec![1.0; len]));
            let kind = if shape == Shape::Elementwise {
                "elementwise"
            } else {
                "elementwise-special"
            };
            format!(
                concat!(
                    "{{\"cases\": [{{\"name\": \"c\", \"kind\": \"{kind}\", \"n\": {len}, \"op_code\": 0, ",
                    "\"c_bits\": \"00000000\", \"input_file\": \"c.input.bin\", \"native_scalar_file\": \"c.native-scalar.bin\", ",
                    "\"input_sha256\": \"{in_sha}\", \"native_scalar_sha256\": \"{nat_sha}\"{cpu_part}}}]}}",
                ),
                kind = kind,
                len = len,
                in_sha = in_sha,
                nat_sha = nat_sha,
                cpu_part = cpu_part
            )
        }
    };
    fs::write(dir.join("manifest.json"), manifest).unwrap();
    write_outputs(dir, rows);
}

/// `outputs.json` と各行の `<id>.bin` を書く。
fn write_outputs(dir: &Path, rows: &[Row]) {
    let mut entries = Vec::new();
    for r in rows {
        let sha = file(dir, &format!("{}.bin", r.id), &r.bytes);
        entries.push(format!(
            concat!(
                "{{\"id\": \"{}\", \"export\": \"{}\", \"build\": \"{}\", \"case\": \"{}\", ",
                "\"output_file\": \"{}.bin\", \"output_sha256\": \"{}\"}}",
            ),
            r.id, r.export, r.build, r.case, r.id, sha
        ));
    }
    fs::write(
        dir.join("outputs.json"),
        format!("{{\"rows\": [{}]}}", entries.join(", ")),
    )
    .unwrap();
}

/// `--recount` の終了コードと標準出力。
fn run_recount(dir: &Path) -> (i32, String) {
    let out = Command::new("bash")
        .arg(root().join("scripts/wasm-node-check.sh"))
        .arg("--recount")
        .arg(dir)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// 行頭が `<kind>: <id> ` の行の `key=` の値。
fn field(text: &str, kind: &str, id: &str, key: &str) -> String {
    let prefix = format!("{kind}: {id} ");
    let line = text
        .lines()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no line {prefix}: {text}"));
    let key = format!("{key}=");
    line.split(' ')
        .find_map(|t| t.strip_prefix(key.as_str()))
        .unwrap_or_else(|| panic!("no field {key}: {line}"))
        .to_string()
}

fn has_line(text: &str, kind: &str, id: &str) -> bool {
    let prefix = format!("{kind}: {id} ");
    text.lines().any(|l| l.starts_with(&prefix))
}

/// 共通の道具の不一致の件数。
fn core_count(reference: &[f32], actual: &[f32]) -> usize {
    let cmp = compare_f32_with_special_values(reference, actual, T1).unwrap();
    cmp.exceeded() + cmp.non_finite_mismatch()
}

/// 組ごとに同じ出力を持つ行を作る。
fn rows_for(combos: &[Combo], actual: &[f32]) -> Vec<Row> {
    combos
        .iter()
        .enumerate()
        .map(|(i, &(export, build))| Row {
            id: format!("r{i}"),
            export,
            build,
            case: "c".to_string(),
            bytes: f32_le_bytes(actual),
        })
        .collect()
}

/// 特殊値の種類の保存の形を 4 組の行で流し、行ごとの件数と全体の出力を共通の道具の件数と突き合わせる。
/// 戻りは (ネイティブとの件数, CPU 参照との件数)。
fn check_special(
    tag: &str,
    shape: Shape,
    native: &[f32],
    cpu: &[f32],
    actual: &[f32],
) -> (usize, usize) {
    let combos = if shape == Shape::GemmSpecial {
        GEMM_ALL
    } else {
        ELEMENTWISE_ALL
    };
    let want_n = core_count(native, actual);
    let want_c = core_count(cpu, actual);
    let dir = TempDir::new(tag);
    let rows = rows_for(&combos, actual);
    write_form(dir.path(), shape, native, cpu, &rows);
    let (code, out) = run_recount(dir.path());
    assert_eq!(code, i32::from(want_n + want_c != 0), "{tag}: {out}");
    for r in &rows {
        let n = actual.len().to_string();
        assert_eq!(
            field(&out, "recount", &r.id, "mismatches"),
            want_n.to_string(),
            "{tag}: {out}"
        );
        assert_eq!(
            field(&out, "recount-cpu", &r.id, "mismatches"),
            want_c.to_string(),
            "{tag}: {out}"
        );
        assert_eq!(field(&out, "recount", &r.id, "n"), n, "{tag}: {out}");
        assert!(has_line(&out, "t1", &r.id), "{tag}: {out}");
    }
    let zero = if want_n == 0 && want_c == 0 {
        rows.len()
    } else {
        0
    };
    let summary = format!(
        "recount-summary: rows={} zero={zero} nonzero={} uncompared=0",
        rows.len(),
        rows.len() - zero
    );
    assert!(out.lines().any(|l| l == summary), "{tag}: {out}");
    (want_n, want_c)
}

fn w(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// 規則では等しくビットが違う語の組（参照, 実測）。
fn bit_different_equal_pairs() -> Vec<(f32, f32)> {
    vec![
        (w(0x7fc0_0000), w(0x7fc0_0001)),
        (w(0x7fc0_0001), w(0x7fc0_0000)),
        (w(0x7fc0_0000), w(0xffc0_0000)),
        (w(0xffc0_0000), w(0x7fc0_0000)),
        (w(0x7f80_0001), w(0x7fc0_0000)),
        (w(0x7fc0_0000), w(0x7f80_0001)),
        (0.0, -0.0),
        (-0.0, 0.0),
    ]
}

/// ほかの語の組（参照, 実測）。
fn other_pairs() -> Vec<(f32, f32)> {
    let next = |x: f32| f32::from_bits(x.to_bits() + 1);
    vec![
        (f32::NAN, 1.0),
        (1.0, f32::NAN),
        (f32::NAN, f32::INFINITY),
        (f32::INFINITY, f32::NAN),
        (f32::INFINITY, f32::NEG_INFINITY),
        (f32::NEG_INFINITY, f32::INFINITY),
        (f32::INFINITY, f32::INFINITY),
        (1.0, f32::INFINITY),
        (f32::INFINITY, 1.0),
        (1000.0, 1000.5),
        (1000.0, 1001.0),
        (1000.0, 1002.0),
        (-1000.0, -1001.0),
        (0.0, 1e-5_f32),
        (0.0, next(1e-5_f32)),
    ]
}

#[test]
fn recount_agrees_with_core_on_special_words() {
    let mut pairs = bit_different_equal_pairs();
    pairs.extend(other_pairs());
    let reference: Vec<f32> = pairs.iter().map(|p| p.0).collect();
    let actual: Vec<f32> = pairs.iter().map(|p| p.1).collect();
    for shape in [Shape::GemmSpecial, Shape::ElementwiseSpecial] {
        let name = if shape == Shape::GemmSpecial {
            "g"
        } else {
            "e"
        };
        for (i, &(r, a)) in pairs.iter().enumerate() {
            // (a) ネイティブ側に語を置く、(b) CPU 参照側に語を置く。
            check_special(&format!("{name}-a{i}"), shape, &[r], &[a], &[a]);
            check_special(&format!("{name}-b{i}"), shape, &[a], &[r], &[a]);
        }
        // (c) 全部の語を並べた列。
        let (n1, c1) = check_special(&format!("{name}-c1"), shape, &reference, &actual, &actual);
        assert!(0 < n1 && n1 < pairs.len(), "{n1}");
        assert_eq!(c1, 0);
        let (n2, c2) = check_special(&format!("{name}-c2"), shape, &actual, &reference, &actual);
        assert_eq!(n2, 0);
        assert!(0 < c2 && c2 < pairs.len(), "{c2}");
        // (d) 両方が 0 でなく件数が違う列。
        let mut native = reference.clone();
        native.rotate_left(2);
        let mut cpu = reference.clone();
        cpu.rotate_left(1);
        let (n, c) = check_special(&format!("{name}-d"), shape, &native, &cpu, &reference);
        assert!(n > 0 && c > 0 && n != c, "{n} {c}");
    }
}

#[test]
fn recount_agrees_with_core_on_generated_special_files() {
    let dir = TempDir::new("generated");
    generate(dir.path()).unwrap();

    let mut names: Vec<&str> = GEMM_SPECIAL_CASES.iter().map(|c| c.name).collect();
    names.extend(ELEMENTWISE_SPECIAL_CASES.iter().map(|c| c.name));
    let mut on_disk: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with("special-"))
        .filter_map(|n| n.strip_suffix(".native-scalar.bin").map(str::to_string))
        .collect();
    on_disk.sort();
    let mut want_names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    want_names.sort();
    assert_eq!(on_disk, want_names);

    let read_f32 = |name: &str| -> Vec<f32> {
        let bytes = fs::read(dir.path().join(name)).unwrap();
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect()
    };
    let mut rows = Vec::new();
    let mut expect = Vec::new();
    let mut native_special = false;
    let mut input_special_cases = 0usize;
    for case in &names {
        let native = read_f32(&format!("{case}.native-scalar.bin"));
        let cpu = read_f32(&format!("{case}.cpu-reference.bin"));
        // 空振りの防止: 非有限の語を持つケースがあり、ネイティブの出力にも非有限の語がある。
        let any_special = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.starts_with(&format!("{case}.")) && n.ends_with(".bin"))
            .any(|n| read_f32(&n).iter().any(|v| !v.is_finite()));
        input_special_cases += usize::from(any_special);
        native_special |= native.iter().any(|v| !v.is_finite());
        let want = core_count(&cpu, &native);
        assert_eq!(want, 0, "{case}");
        let bytes = fs::read(dir.path().join(format!("{case}.native-scalar.bin"))).unwrap();
        let combos = if case.starts_with("special-gemm-") {
            GEMM_ALL
        } else {
            ELEMENTWISE_ALL
        };
        for (export, build) in combos {
            let id = format!("{case}-{build}-{}", export.replace('_', "-"));
            expect.push((id.clone(), native.len(), want));
            rows.push(Row {
                id,
                export,
                build,
                case: case.to_string(),
                bytes: bytes.clone(),
            });
        }
    }
    assert!(native_special && input_special_cases > 0);
    write_outputs(dir.path(), &rows);
    let (code, out) = run_recount(dir.path());
    assert_eq!(code, 0, "{out}");
    for (id, n, want) in &expect {
        assert_eq!(field(&out, "recount", id, "mismatches"), "0", "{out}");
        assert_eq!(field(&out, "recount", id, "n"), n.to_string(), "{out}");
        assert_eq!(
            field(&out, "recount-cpu", id, "mismatches"),
            want.to_string(),
            "{out}"
        );
        assert_eq!(field(&out, "recount-cpu", id, "n"), n.to_string(), "{out}");
    }
    let r = rows.len();
    assert_eq!(r, names.len() * 4);
    let summary = format!("recount-summary: rows={r} zero={r} nonzero=0 uncompared=0");
    assert!(out.lines().any(|l| l == summary), "{out}");
}

#[test]
fn existing_kinds_stay_bit_exact() {
    let pairs = bit_different_equal_pairs();
    let reference: Vec<f32> = pairs.iter().map(|p| p.0).collect();
    let actual: Vec<f32> = pairs.iter().map(|p| p.1).collect();
    // 規則では等しいが、ビット列は全語で違う。
    assert_eq!(core_count(&reference, &actual), 0);
    assert!(
        reference
            .iter()
            .zip(&actual)
            .all(|(r, a)| r.to_bits() != a.to_bits())
    );
    let bit_diff = pairs.len();

    let cases: [(Shape, &[Combo], &str); 2] = [
        (Shape::Gemm, &GEMM_SCALAR, "gemm"),
        (Shape::Elementwise, &ELEMENTWISE_ALL, "elementwise"),
    ];
    for (shape, combos, tag) in cases {
        let dir = TempDir::new(&format!("exact-{tag}"));
        let rows = rows_for(combos, &actual);
        write_form(dir.path(), shape, &reference, &[], &rows);
        let (code, out) = run_recount(dir.path());
        assert_eq!(code, 1, "{tag}: {out}");
        for r in &rows {
            assert_eq!(
                field(&out, "recount", &r.id, "mismatches"),
                bit_diff.to_string(),
                "{tag}: {out}"
            );
            assert!(!has_line(&out, "recount-cpu", &r.id), "{tag}: {out}");
            assert!(!has_line(&out, "t1", &r.id), "{tag}: {out}");
        }
    }

    // 陽性対照: 同じ語を特殊値の種類に当てると不一致が 0 件になる。
    for (shape, combos, tag) in [
        (Shape::GemmSpecial, GEMM_ALL, "control-g"),
        (Shape::ElementwiseSpecial, ELEMENTWISE_ALL, "control-e"),
    ] {
        let dir = TempDir::new(tag);
        let rows = rows_for(&combos, &actual);
        write_form(dir.path(), shape, &reference, &reference, &rows);
        let (code, out) = run_recount(dir.path());
        assert_eq!(code, 0, "{out}");
        for r in &rows {
            assert_eq!(field(&out, "recount", &r.id, "mismatches"), "0", "{out}");
        }
    }
}
