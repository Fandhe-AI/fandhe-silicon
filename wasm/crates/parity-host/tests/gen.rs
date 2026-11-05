//! `wasm-parity-host` の入力・参照・書き出しの試験（spec TASK-994 のポインタ）。
//! 部分 Top-k の入力と参照の試験は spec TASK-996 のポインタ。
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
    assert_eq!(names.len(), GEMM_CASES.len() + TOPK_CASES.len());
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
// ---- 部分 Top-k（spec TASK-996 のポインタ） ----

use fandhe_silicon_core::parity::topk::{TopKEntry, compare_topk, reference_topk_f32};
use wasm_parity_host::{TOPK_CASES, TopkCase, build_topk_cases, native_topk, topk_record_bytes};

fn topk_case(name: &str) -> TopkCase {
    build_topk_cases()
        .unwrap()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap()
}

/// 8 バイトの 1 件ごとに（スロット番号, スコアのビット列）へ読み戻す。
fn read_records(path: &Path) -> Vec<(u32, u32)> {
    let bytes = fs::read(path).unwrap();
    let (chunks, rest) = bytes.as_chunks::<8>();
    assert!(rest.is_empty());
    chunks
        .iter()
        .map(|c| {
            (
                u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                u32::from_le_bytes([c[4], c[5], c[6], c[7]]),
            )
        })
        .collect()
}

fn entries(records: &[(u32, u32)]) -> Vec<TopKEntry> {
    records
        .iter()
        .map(|(slot, b)| TopKEntry::new(*slot, f32::from_bits(*b)))
        .collect()
}

fn pairs_to_entries(pairs: &[(u32, f32)]) -> Vec<TopKEntry> {
    pairs.iter().map(|(s, v)| TopKEntry::new(*s, *v)).collect()
}

/// manifest の `{`〜`}` の 1 要素ごとに、鍵の名前の列を集める。
fn manifest_entry_keys(text: &str) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut current: Option<Vec<String>> = None;
    for line in text.lines() {
        let t = line.trim();
        if line == "    {" {
            current = Some(Vec::new());
        } else if line == "    }" || line == "    }," {
            if let Some(keys) = current.take() {
                out.push(keys);
            }
        } else if let Some(keys) = current.as_mut()
            && let Some(rest) = t.strip_prefix('"')
        {
            keys.push(rest.split('"').next().unwrap().to_string());
        }
    }
    out
}

#[test]
fn topk_batch_follows_fresh_generator() {
    let case = topk_case("topk-batch");
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let expected: Vec<f32> = (0..20_000).map(|_| rng.next_signed_unit_f32()).collect();
    assert_eq!(case.scores.len(), 20_000);
    assert_eq!(case.k, 64);
    assert!(case.scores.iter().all(|x| x.is_finite()));
    assert_eq!(bits(&case.scores), bits(&expected));
}

#[test]
fn topk_ties_input_shape() {
    let case = topk_case("topk-ties");
    let v = &case.scores;
    assert_eq!(v.len(), 200);
    assert_eq!(case.k, 10);
    assert!(v.iter().all(|x| x.is_finite()));
    assert_eq!(v.iter().filter(|x| x.to_bits() == 0x0000_0000).count(), 20);
    assert_eq!(v.iter().filter(|x| x.to_bits() == 0x8000_0000).count(), 0);
    let r = reference_topk_f32(v, 11).unwrap();
    assert_eq!(r[9].score_bits(), r[10].score_bits());
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let expected: Vec<f32> = (0..200usize)
        .map(|i| {
            if i % 10 == 3 {
                0.0
            } else {
                ((rng.next_u64() >> 61) as f32) + 1.0
            }
        })
        .collect();
    assert_eq!(bits(v), bits(&expected));
}

#[test]
fn topk_nonfinite_input_shape() {
    let case = topk_case("topk-nonfinite");
    let v = &case.scores;
    assert_eq!(v.len(), 200);
    assert_eq!(case.k, 10);
    let special = [5usize, 17, 30, 77];
    assert_eq!(v[5].to_bits(), f32::NAN.to_bits());
    assert_eq!(v[17].to_bits(), 0xffc0_0000);
    assert_eq!(v[30], f32::INFINITY);
    assert_eq!(v[77], f32::NEG_INFINITY);
    assert_eq!(v.iter().filter(|x| x.is_nan()).count(), 2);
    for (i, x) in v.iter().enumerate() {
        assert_eq!(x.is_finite(), !special.contains(&i), "{i}");
    }
    let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
    let fresh: Vec<f32> = (0..200).map(|_| rng.next_signed_unit_f32()).collect();
    for i in (0..200).filter(|i| !special.contains(i)) {
        assert_eq!(v[i].to_bits(), fresh[i].to_bits(), "{i}");
    }
}

#[test]
fn native_topk_matches_default_cpu_ops() {
    let dir = TempDir::new("topk-native");
    generate(dir.path()).unwrap();
    for case in build_topk_cases().unwrap() {
        let mut out = vec![(0_u32, 0.0_f32); case.k];
        DefaultCpuOps
            .topk_partial_scalar(&case.scores, case.k, &mut out)
            .unwrap();
        let expected: Vec<u8> = out
            .iter()
            .flat_map(|(slot, score)| {
                let mut r = slot.to_le_bytes().to_vec();
                r.extend_from_slice(&score.to_bits().to_le_bytes());
                r
            })
            .collect();
        let actual = fs::read(dir.path().join(format!("{}.native-scalar.bin", case.name))).unwrap();
        assert_eq!(actual, expected, "{}", case.name);
    }
}

#[test]
fn native_topk_matches_cpu_reference() {
    let dir = TempDir::new("topk-ref");
    generate(dir.path()).unwrap();
    for case in build_topk_cases().unwrap() {
        let records = read_records(&dir.path().join(format!("{}.native-scalar.bin", case.name)));
        let reference = reference_topk_f32(&case.scores, case.k).unwrap();
        let cmp = compare_topk(&reference, &entries(&records)).unwrap();
        assert!(cmp.is_match(), "{}", case.name);
        assert_eq!(cmp.compared(), case.k, "{}", case.name);
    }
}

#[test]
fn native_topk_has_no_non_finite_slots() {
    let dir = TempDir::new("topk-finite");
    generate(dir.path()).unwrap();
    for case in build_topk_cases().unwrap() {
        let records = read_records(&dir.path().join(format!("{}.native-scalar.bin", case.name)));
        assert_eq!(records.len(), case.k);
        assert!(
            records.iter().all(|(_, b)| f32::from_bits(*b).is_finite()),
            "{}",
            case.name
        );
        if case.name == "topk-nonfinite" {
            for slot in [5u32, 17, 30, 77] {
                assert!(records.iter().all(|(s, _)| *s != slot), "{slot}");
            }
        }
    }
}

#[test]
fn topk_record_layout() {
    let bytes = topk_record_bytes(&[(0x0102_0304, 1.5), (7, -0.0)]);
    assert_eq!(bytes.len(), 16);
    assert_eq!(&bytes[..4], &[0x04, 0x03, 0x02, 0x01]);
    assert_eq!(&bytes[4..8], &1.5f32.to_bits().to_le_bytes());
    assert_eq!(&bytes[8..12], &[7, 0, 0, 0]);
    assert_eq!(&bytes[12..16], &[0, 0, 0, 0x80]);
}

/// 8 バイトごとに違う件数を数える。
fn differing_records(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<8>()
        .0
        .iter()
        .zip(b.as_chunks::<8>().0.iter())
        .filter(|(x, y)| x != y)
        .count()
}

#[test]
fn record_compare_agrees_with_compare_topk() {
    for case in build_topk_cases().unwrap() {
        let native = native_topk(&case).unwrap();
        let base_bytes = topk_record_bytes(&native);
        let reference = pairs_to_entries(&native);
        for flip_slot in [true, false] {
            let mut changed = native.clone();
            if flip_slot {
                changed[0].0 ^= 1;
            } else {
                changed[0].1 = f32::from_bits(changed[0].1.to_bits() ^ 1);
            }
            let cmp = compare_topk(&reference, &pairs_to_entries(&changed)).unwrap();
            assert_eq!(cmp.mismatched(), 1, "{}", case.name);
            assert_eq!(
                differing_records(&base_bytes, &topk_record_bytes(&changed)),
                1,
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn topk_manifest_entries_match_files() {
    let dir = TempDir::new("topk-manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let keys = manifest_entry_keys(&text);
    assert_eq!(keys.len(), GEMM_CASES.len() + TOPK_CASES.len());
    let mut want = [
        "name",
        "kind",
        "n",
        "k",
        "scores_file",
        "native_scalar_file",
        "scores_sha256",
        "native_scalar_sha256",
    ];
    want.sort_unstable();
    for (j, spec) in TOPK_CASES.iter().enumerate() {
        // GEMM の 3 要素の後に Top-k の 3 要素が並ぶ。
        let i = GEMM_CASES.len() + j;
        let case = topk_case(spec.name);
        let mut got: Vec<&str> = keys[i].iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, want);
        assert_eq!(manifest_values(&text, "name")[i], case.name);
        assert_eq!(manifest_values(&text, "kind")[i], "topk");
        assert_eq!(
            manifest_values(&text, "n")[i],
            case.scores.len().to_string()
        );
        assert_eq!(manifest_values(&text, "k")[i], case.k.to_string());
        let scores_file = &manifest_values(&text, "scores_file")[j];
        let native_file = &manifest_values(&text, "native_scalar_file")[i];
        for f in [scores_file, native_file] {
            assert!(
                f.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-')
            );
        }
        let scores_bytes = fs::read(dir.path().join(scores_file)).unwrap();
        let native_bytes = fs::read(dir.path().join(native_file)).unwrap();
        assert_eq!(scores_bytes.len(), case.scores.len() * 4);
        assert_eq!(native_bytes.len(), case.k * 8);
        assert_eq!(scores_bytes, f32_le_bytes(&case.scores));
        assert_eq!(
            manifest_values(&text, "scores_sha256")[j],
            hex(&sha256_bytes(&scores_bytes))
        );
        assert_eq!(
            manifest_values(&text, "native_scalar_sha256")[i],
            hex(&sha256_bytes(&native_bytes))
        );
    }
}

#[test]
fn gen_refuses_existing_topk_file() {
    let dir = TempDir::new("topk-refuse");
    let path = dir.path().join("topk-ties.scores.bin");
    fs::write(&path, b"keep").unwrap();
    assert_eq!(generate(dir.path()), Err(GenError::FileExists));
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn native_topk_error_text() {
    assert_eq!(
        GenError::NativeTopkFailed.to_string(),
        "error: native topk_partial_scalar failed"
    );
}
