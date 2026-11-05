//! 照合スクリプトの閾値の比べ（`--recount` の `t1:` の行と不一致の件数）を、共通の道具の
//! `T1` と `compare_f32_with_special_values` の答えと突き合わせる試験（spec TASK-995 のポインタ・PARITY-21）。
//! 期待は共通の道具の呼び出しと式で作り、閾値は `T1` の getter で引く。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_inputs::{GEMM_INPUT_SEED, SplitMix64};
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::T1;
use wasm_parity_host::f32_le_bytes;

/// 試験ごとの一時ディレクトリ。破棄で消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("wasm-parity-t1-{}-{tag}", std::process::id()));
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

/// 参照と実測を `m=1`・`k=1`・`n=<長さ>` の `gemm` のケースの保存の形で書き、`--recount` の終了コードと標準出力を返す。
fn recount(tag: &str, reference: &[f32], actual: &[f32]) -> (i32, String) {
    let dir = TempDir::new(tag);
    let a = f32_le_bytes(&[1.0]);
    let b = f32_le_bytes(&vec![1.0; reference.len()]);
    let want = f32_le_bytes(reference);
    let row = f32_le_bytes(actual);
    let files: [(&str, &[u8]); 4] = [
        ("c.a.bin", &a),
        ("c.b.bin", &b),
        ("c.native-scalar.bin", &want),
        ("row.bin", &row),
    ];
    for (name, bytes) in files {
        fs::write(dir.path().join(name), bytes).unwrap();
    }
    let manifest = format!(
        concat!(
            "{{\"cases\": [{{\"name\": \"c\", \"kind\": \"gemm\", \"m\": 1, \"k\": 1, \"n\": {}, ",
            "\"a_file\": \"c.a.bin\", \"b_file\": \"c.b.bin\", ",
            "\"native_scalar_file\": \"c.native-scalar.bin\", ",
            "\"a_sha256\": \"{}\", \"b_sha256\": \"{}\", \"native_scalar_sha256\": \"{}\"}}]}}",
        ),
        reference.len(),
        hex(&sha256_bytes(&a)),
        hex(&sha256_bytes(&b)),
        hex(&sha256_bytes(&want)),
    );
    let outputs = format!(
        concat!(
            "{{\"rows\": [{{\"id\": \"row\", \"export\": \"gemm_simd\", \"build\": \"simd128\", ",
            "\"case\": \"c\", \"output_file\": \"row.bin\", \"output_sha256\": \"{}\"}}]}}",
        ),
        hex(&sha256_bytes(&row)),
    );
    fs::write(dir.path().join("manifest.json"), manifest).unwrap();
    fs::write(dir.path().join("outputs.json"), outputs).unwrap();
    let out = Command::new("bash")
        .arg(root().join("scripts/wasm-node-check.sh"))
        .arg("--recount")
        .arg(dir.path())
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// 行頭が `prefix` の行の、空白区切りの `key=` の値。
fn field<'a>(text: &'a str, prefix: &str, key: &str) -> &'a str {
    let line = text
        .lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line {prefix}: {text}"));
    let key = format!("{key}=");
    line.split(' ')
        .find_map(|t| t.strip_prefix(key.as_str()))
        .unwrap_or_else(|| panic!("no field {key}: {line}"))
}

/// 共通の道具の件数と式で数えた値を、スクリプトの出力と突き合わせる。戻りは不一致の件数。
fn check(tag: &str, reference: &[f32], actual: &[f32]) -> usize {
    let cmp = compare_f32_with_special_values(reference, actual, T1).unwrap();
    let expected = cmp.exceeded() + cmp.non_finite_mismatch();
    let (code, out) = recount(tag, reference, actual);
    assert_eq!(code, i32::from(expected != 0), "{tag}: {out}");
    let got: usize = field(&out, "recount: row ", "mismatches").parse().unwrap();
    assert_eq!(got, expected, "{tag}: {out}");

    let (mut max_abs, mut max_rel) = (0.0f64, 0.0f64);
    let (mut abs_over, mut rel_over) = (0usize, 0usize);
    for (&r, &a) in reference.iter().zip(actual) {
        if !r.is_finite() || !a.is_finite() {
            continue;
        }
        let err = (f64::from(a) - f64::from(r)).abs();
        max_abs = max_abs.max(err);
        if err >= T1.absolute() {
            abs_over += 1;
        }
        if r != 0.0 {
            let rel = err / f64::from(r).abs();
            max_rel = max_rel.max(rel);
            if rel >= T1.relative() {
                rel_over += 1;
            }
        }
    }
    let t1_line = "t1: row ";
    let parse = |key: &str| field(&out, t1_line, key).parse::<f64>().unwrap();
    assert_eq!(
        parse("max-abs").to_bits(),
        max_abs.to_bits(),
        "{tag}: {out}"
    );
    assert_eq!(
        parse("max-rel").to_bits(),
        max_rel.to_bits(),
        "{tag}: {out}"
    );
    assert_eq!(
        field(&out, t1_line, "abs-over"),
        abs_over.to_string(),
        "{tag}: {out}"
    );
    assert_eq!(
        field(&out, t1_line, "rel-over"),
        rel_over.to_string(),
        "{tag}: {out}"
    );
    expected
}

#[test]
fn script_thresholds_match_core() {
    let text = fs::read_to_string(root().join("scripts/wasm-node-check.sh")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let begin: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i] == "# BEGIN t1-thresholds")
        .collect();
    let end: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i] == "# END t1-thresholds")
        .collect();
    assert_eq!((begin.len(), end.len()), (1, 1));
    let block = &lines[begin[0] + 1..end[0]];
    let value = |name: &str| -> f64 {
        let prefix = format!("{name} = ");
        let hits: Vec<&str> = block
            .iter()
            .filter_map(|l| l.strip_prefix(prefix.as_str()))
            .collect();
        assert_eq!(hits.len(), 1, "{name}");
        hits[0].trim().parse().unwrap()
    };
    assert_eq!(value("T1_RELATIVE").to_bits(), T1.relative().to_bits());
    assert_eq!(value("T1_ABSOLUTE").to_bits(), T1.absolute().to_bits());
}

#[test]
fn recount_agrees_with_core_on_boundary_words() {
    let next = |x: f32| f32::from_bits(x.to_bits() + 1);
    let w = f32::from_bits;
    let pairs: Vec<(f32, f32)> = vec![
        (1000.0, 1001.0),
        (1000.0, 999.0),
        (-1000.0, -1001.0),
        (1000.0, 1000.5),
        (0.0, 1e-5_f32),
        (0.0, next(1e-5_f32)),
        (-0.0, 0.0),
        (0.0, -0.0),
        (1.0e-3, 1.005e-3),
        (1024.0, 1024.25),
        (w(0x3580_0000), w(0x35c0_0000)),
        (w(0x7fc0_0000), w(0x7f80_0001)),
        (w(0x7f80_0001), w(0xffc0_0001)),
        (f32::NAN, 1.0),
        (1.0, f32::NAN),
        (f32::INFINITY, f32::INFINITY),
        (f32::INFINITY, f32::NEG_INFINITY),
        (f32::NEG_INFINITY, f32::NEG_INFINITY),
        (1.0, f32::INFINITY),
        (f32::MAX, f32::INFINITY),
    ];
    let mut total = 0;
    for (i, &(r, a)) in pairs.iter().enumerate() {
        total += check(&format!("pair{i}"), &[r], &[a]);
    }
    let reference: Vec<f32> = pairs.iter().map(|p| p.0).collect();
    let actual: Vec<f32> = pairs.iter().map(|p| p.1).collect();
    assert_eq!(check("pairs-all", &reference, &actual), total);
}

#[test]
fn recount_agrees_with_core_on_generated_words() {
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let reference: Vec<f32> = (0..4096)
        .map(|_| rng.next_signed_unit_f32() * 1e-2)
        .collect();
    let actual: Vec<f32> = reference
        .iter()
        .map(|&r| r * (1.0 + rng.next_signed_unit_f32() * 2e-3) + rng.next_signed_unit_f32() * 2e-5)
        .collect();
    let mismatches = check("generated", &reference, &actual);
    assert!(0 < mismatches && mismatches < reference.len());
}
