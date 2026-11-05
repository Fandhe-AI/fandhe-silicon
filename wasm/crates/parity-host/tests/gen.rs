//! `wasm-parity-host` の入力・参照・書き出しの試験（spec TASK-994 のポインタ）。
//! 期待の値は生成器と `DefaultCpuOps` の呼び出しで作り、値を手で写さない。

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, SplitMix64, generate_f32,
};
use wasm_parity_host::{GEMM_CASES, GenError, build_cases, f32_le_bytes, generate, run};

/// 試験ごとの一時ディレクトリ。破棄で消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("wasm-parity-host-{}-{tag}", std::process::id()));
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

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

fn read_f32(path: &Path) -> Vec<f32> {
    let bytes = fs::read(path).unwrap();
    let (chunks, rest) = bytes.as_chunks::<4>();
    assert!(rest.is_empty());
    chunks.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

/// manifest の `"key": value` の行から値（引用符とカンマを除く）を、現れる順に集める。
fn manifest_values(text: &str, key: &str) -> Vec<String> {
    let prefix = format!("\"{key}\": ");
    text.lines()
        .filter_map(|l| l.trim().strip_prefix(prefix.as_str()))
        .map(|v| v.trim_end_matches(',').trim_matches('"').to_string())
        .collect()
}

#[test]
fn square_cases_follow_fresh_generator() {
    let cases = build_cases().unwrap();
    for (case, size) in cases.iter().take(2).zip([64usize, 256]) {
        let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
        let expected: Vec<f32> = (0..2 * size * size)
            .map(|_| rng.next_signed_unit_f32())
            .collect();
        let mut actual = case.a.clone();
        actual.extend_from_slice(&case.b);
        assert_eq!(bits(&actual), bits(&expected));
        assert_eq!((case.m, case.k, case.n), (size, size, size));
    }
}

#[test]
fn odd_case_matches_gate_row() {
    let row = GemmGateRow::Odd37x41x53;
    let inputs = generate_f32(row).unwrap();
    let case = &build_cases().unwrap()[2];
    assert_eq!(bits(&case.a), bits(inputs.a()));
    assert_eq!(bits(&case.b), bits(inputs.b()));
    assert_eq!((case.m, case.k, case.n), (37, 53, 41));
}

#[test]
fn inputs_are_finite() {
    for case in build_cases().unwrap() {
        assert!(case.a.iter().all(|x| x.is_finite()));
        assert!(case.b.iter().all(|x| x.is_finite()));
    }
}

#[test]
fn native_reference_matches_default_cpu_ops() {
    let dir = TempDir::new("native");
    generate(dir.path()).unwrap();
    for case in build_cases().unwrap() {
        let mut expected = vec![0.0f32; case.m * case.n];
        DefaultCpuOps
            .gemm_scalar(&case.a, &case.b, case.m, case.k, case.n, &mut expected)
            .unwrap();
        let actual = read_f32(&dir.path().join(format!("{}.native-scalar.bin", case.name)));
        assert_eq!(bits(&actual), bits(&expected), "{}", case.name);
    }
}

#[test]
fn manifest_hashes_match_files() {
    let dir = TempDir::new("manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let cases = build_cases().unwrap();
    let names = manifest_values(&text, "name");
    assert_eq!(names.len(), GEMM_CASES.len());
    for (i, case) in cases.iter().enumerate() {
        assert_eq!(names[i], case.name);
        assert_eq!(manifest_values(&text, "kind")[i], "gemm");
        assert_eq!(manifest_values(&text, "m")[i], case.m.to_string());
        assert_eq!(manifest_values(&text, "k")[i], case.k.to_string());
        assert_eq!(manifest_values(&text, "n")[i], case.n.to_string());
        let lens = [case.m * case.k, case.k * case.n, case.m * case.n];
        for (j, (file_key, hash_key)) in [
            ("a_file", "a_sha256"),
            ("b_file", "b_sha256"),
            ("native_scalar_file", "native_scalar_sha256"),
        ]
        .into_iter()
        .enumerate()
        {
            let file = &manifest_values(&text, file_key)[i];
            assert!(
                file.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-')
            );
            let bytes = fs::read(dir.path().join(file)).unwrap();
            assert_eq!(bytes.len(), lens[j] * 4);
            assert_eq!(
                manifest_values(&text, hash_key)[i],
                hex(&sha256_bytes(&bytes))
            );
        }
    }
    let a_bytes = f32_le_bytes(&cases[0].a);
    assert_eq!(fs::read(dir.path().join("gemm-64.a.bin")).unwrap(), a_bytes);
}

#[test]
fn gen_refuses_existing_files() {
    let dir = TempDir::new("refuse");
    generate(dir.path()).unwrap();
    let before = fs::read(dir.path().join("gemm-64.a.bin")).unwrap();
    let manifest_before = fs::read(dir.path().join("manifest.json")).unwrap();
    assert_eq!(generate(dir.path()), Err(GenError::FileExists));
    assert_eq!(fs::read(dir.path().join("gemm-64.a.bin")).unwrap(), before);
    assert_eq!(
        fs::read(dir.path().join("manifest.json")).unwrap(),
        manifest_before
    );
}

#[test]
fn gen_rejects_bad_arguments() {
    let dir = TempDir::new("args");
    let d: OsString = dir.path().as_os_str().to_owned();
    let missing: OsString = dir.path().join("absent").into_os_string();
    assert_eq!(run(&[]), Err(GenError::Usage));
    assert_eq!(run(&["gen".into()]), Err(GenError::Usage));
    assert_eq!(
        run(&["gen".into(), d.clone(), "extra".into()]),
        Err(GenError::Usage)
    );
    assert_eq!(run(&["other".into(), d]), Err(GenError::Usage));
    assert_eq!(
        run(&["gen".into(), missing]),
        Err(GenError::MissingDirectory)
    );
}

#[test]
fn binary_reports_exit_codes() {
    let exe = env!("CARGO_BIN_EXE_wasm-parity-host");
    let dir = TempDir::new("bin");
    let ok = Command::new(exe)
        .arg("gen")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(ok.status.code(), Some(0));
    assert!(ok.stdout.is_empty());

    let missing = dir.path().join("absent-dir");
    let bad = Command::new(exe).arg("gen").arg(&missing).output().unwrap();
    assert_eq!(bad.status.code(), Some(2));
    assert!(bad.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(!stderr.is_empty());
    assert!(!stderr.contains("absent-dir"));

    let usage = Command::new(exe).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));

    let again = Command::new(exe)
        .arg("gen")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(again.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&again.stderr).contains(dir.path().to_str().unwrap()));
}
