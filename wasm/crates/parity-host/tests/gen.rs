//! `wasm-parity-host` の入力・参照・書き出しの試験（spec TASK-994 のポインタ）。
//! 部分 Top-k の入力と参照の試験は spec TASK-996 のポインタ。
//! 要素ごと演算の入力と参照の試験は spec TASK-997 のポインタ。
//! -0.0 を含む部分 Top-k の入力と参照の試験は spec TASK-2451 のポインタ。
//! 特殊値の GEMM・要素ごと演算の入力と 2 つの参照、`topk-e-3` の入力と参照の試験は spec TASK-999 のポインタ。
//! 期待の値は生成器と `DefaultCpuOps` の呼び出しで作り、値を手で写さない。
//! ただし -0.0 の節の入力の出どころの名前と、IEEE の並びとの記録の差の件数は、独立の照合のため字句で置く。

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
    assert_eq!(
        names.len(),
        GEMM_CASES.len()
            + TOPK_CASES.len()
            + ELEMENTWISE_CASES.len()
            + SIGNED_ZERO_CASES.len()
            + GEMM_SPECIAL_CASES.len()
            + ELEMENTWISE_SPECIAL_CASES.len()
            + TOPK_E3_CASES.len()
    );
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
    assert_eq!(
        keys.len(),
        GEMM_CASES.len()
            + TOPK_CASES.len()
            + ELEMENTWISE_CASES.len()
            + SIGNED_ZERO_CASES.len()
            + GEMM_SPECIAL_CASES.len()
            + ELEMENTWISE_SPECIAL_CASES.len()
            + TOPK_E3_CASES.len()
    );
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
// ---- 要素ごと演算（spec TASK-997 のポインタ） ----

use fandhe_silicon_contract::cpu_ops::ReferenceCpuOps;
use wasm_parity_host::{
    ELEMENTWISE_CASES, ELEMENTWISE_LEN, ElementwiseCase, build_elementwise_cases,
    native_elementwise,
};

fn elementwise_case(name: &str) -> ElementwiseCase {
    build_elementwise_cases()
        .unwrap()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap()
}

#[test]
fn native_elementwise_error_text() {
    assert_eq!(
        GenError::NativeElementwiseFailed.to_string(),
        "error: native elementwise_scalar failed"
    );
}

#[test]
fn elementwise_input_follows_fresh_generator() {
    assert_eq!(ELEMENTWISE_LEN, 1_000_003);
    for spec in &ELEMENTWISE_CASES {
        let case = elementwise_case(spec.name);
        let mut rng = SplitMix64::new(GEMM_INPUT_SEED);
        let want: Vec<f32> = std::iter::repeat_with(|| rng.next_signed_unit_f32())
            .filter(|x| *x != 0.0)
            .take(1_000_003)
            .collect();
        assert!(bits(&case.input) == bits(&want), "{}", spec.name);
    }
}

#[test]
fn elementwise_input_has_no_special_values() {
    for spec in &ELEMENTWISE_CASES {
        let case = elementwise_case(spec.name);
        assert_eq!(case.input.len(), 1_000_003);
        assert_eq!(case.input.iter().filter(|x| x.is_nan()).count(), 0);
        assert_eq!(case.input.iter().filter(|x| x.is_infinite()).count(), 0);
        let zeros = case
            .input
            .iter()
            .filter(|x| x.to_bits() & 0x7fff_ffff == 0)
            .count();
        assert_eq!(zeros, 0);
    }
}

#[test]
fn elementwise_cases_share_input_bytes() {
    let dir = TempDir::new("ew-share");
    generate(dir.path()).unwrap();
    let a = fs::read(dir.path().join("elementwise-relu.input.bin")).unwrap();
    let b = fs::read(dir.path().join("elementwise-add-scalar.input.bin")).unwrap();
    assert!(a == b);
}

#[test]
fn native_elementwise_matches_default_cpu_ops() {
    let dir = TempDir::new("ew-default");
    generate(dir.path()).unwrap();
    for spec in &ELEMENTWISE_CASES {
        let case = elementwise_case(spec.name);
        let mut out = vec![0.0_f32; case.input.len()];
        DefaultCpuOps
            .elementwise_scalar(case.op, &case.input, &mut out)
            .unwrap();
        let written =
            fs::read(dir.path().join(format!("{}.native-scalar.bin", spec.name))).unwrap();
        assert!(written == f32_le_bytes(&out), "{}", spec.name);
        assert!(native_elementwise(&case).unwrap() == out, "{}", spec.name);
    }
}

#[test]
fn native_elementwise_matches_expression() {
    let dir = TempDir::new("ew-expr");
    generate(dir.path()).unwrap();
    for spec in &ELEMENTWISE_CASES {
        let case = elementwise_case(spec.name);
        let got = read_f32(&dir.path().join(format!("{}.native-scalar.bin", spec.name)));
        let want: Vec<f32> = match spec.op_code {
            0 => case.input.iter().map(|x| x.max(0.0)).collect(),
            1 => case.input.iter().map(|x| x + 0.5).collect(),
            _ => panic!("unexpected op_code"),
        };
        assert!(bits(&got) == bits(&want), "{}", spec.name);
    }
}

#[test]
fn native_elementwise_matches_reference_cpu_ops() {
    let dir = TempDir::new("ew-ref");
    generate(dir.path()).unwrap();
    for spec in &ELEMENTWISE_CASES {
        let case = elementwise_case(spec.name);
        let got = read_f32(&dir.path().join(format!("{}.native-scalar.bin", spec.name)));
        let mut want = vec![0.0_f32; case.input.len()];
        ReferenceCpuOps
            .elementwise_scalar(case.op, &case.input, &mut want)
            .unwrap();
        assert!(bits(&got) == bits(&want), "{}", spec.name);
    }
}

#[test]
fn elementwise_manifest_entries_match_files() {
    let dir = TempDir::new("ew-manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let keys = manifest_entry_keys(&text);
    assert_eq!(
        keys.len(),
        GEMM_CASES.len()
            + TOPK_CASES.len()
            + ELEMENTWISE_CASES.len()
            + SIGNED_ZERO_CASES.len()
            + GEMM_SPECIAL_CASES.len()
            + ELEMENTWISE_SPECIAL_CASES.len()
            + TOPK_E3_CASES.len()
    );
    let mut want = [
        "name",
        "kind",
        "n",
        "op_code",
        "c_bits",
        "input_file",
        "native_scalar_file",
        "input_sha256",
        "native_scalar_sha256",
    ];
    want.sort_unstable();
    let wanted_op_codes = ["0", "1"];
    let wanted_c_bits = ["00000000", "3f000000"];
    for (j, spec) in ELEMENTWISE_CASES.iter().enumerate() {
        // GEMM の 3 要素・Top-k の 3 要素の後に要素ごと演算の 2 要素が並ぶ。
        let i = GEMM_CASES.len() + TOPK_CASES.len() + j;
        let case = elementwise_case(spec.name);
        let mut got: Vec<&str> = keys[i].iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, want);
        assert_eq!(manifest_values(&text, "name")[i], case.name);
        assert_eq!(manifest_values(&text, "kind")[i], "elementwise");
        assert_eq!(manifest_values(&text, "n")[i], "1000003");
        assert_eq!(manifest_values(&text, "op_code")[j], wanted_op_codes[j]);
        assert_eq!(manifest_values(&text, "c_bits")[j], wanted_c_bits[j]);
        let input_file = &manifest_values(&text, "input_file")[j];
        let native_file = &manifest_values(&text, "native_scalar_file")[i];
        for f in [input_file, native_file] {
            assert!(
                f.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-')
            );
        }
        let input_bytes = fs::read(dir.path().join(input_file)).unwrap();
        let native_bytes = fs::read(dir.path().join(native_file)).unwrap();
        assert_eq!(input_bytes.len(), case.input.len() * 4);
        assert_eq!(native_bytes.len(), case.input.len() * 4);
        assert!(input_bytes == f32_le_bytes(&case.input));
        assert_eq!(
            manifest_values(&text, "input_sha256")[j],
            hex(&sha256_bytes(&input_bytes))
        );
        assert_eq!(
            manifest_values(&text, "native_scalar_sha256")[i],
            hex(&sha256_bytes(&native_bytes))
        );
    }
}

#[test]
fn gen_refuses_existing_elementwise_file() {
    let dir = TempDir::new("ew-refuse");
    let path = dir.path().join("elementwise-relu.input.bin");
    fs::write(&path, b"keep").unwrap();
    assert_eq!(generate(dir.path()), Err(GenError::FileExists));
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

// ---- -0.0 を含む部分 Top-k（spec TASK-2451 のポインタ） ----

use fandhe_silicon_core::parity::case_inputs::{CaseId, CaseInput, additional_topk_inputs};
use wasm_parity_host::{
    SIGNED_ZERO_CASES, SignedZeroCaseSpec, SignedZeroSource, build_signed_zero_case,
    build_signed_zero_cases,
};

/// 5 ケースの名前と、入力の出どころの名前（`None` は `CaseId::E4`）。表とは別に字句で持つ。
const SIGNED_ZERO_EXPECTED: [(&str, Option<&str>); 5] = [
    ("topk-e-4", None),
    ("topk-signed-zero-adjacent", Some("signed_zero_adjacent")),
    ("topk-signed-zero-separated", Some("signed_zero_separated")),
    ("topk-signed-zero-cut-at-k", Some("signed_zero_cut_at_k")),
    ("topk-signed-zero-repeated", Some("signed_zero_repeated")),
];

/// 出どころの名前から core の入力（スコア, k）を引く。
fn core_signed_zero_input(source: Option<&str>) -> (Vec<f32>, usize) {
    match source {
        None => match CaseId::E4.input().unwrap() {
            CaseInput::TopK { scores, k } => (scores, k),
            _ => panic!("E4 is not a top-k input"),
        },
        Some(name) => {
            let input = additional_topk_inputs()
                .iter()
                .find(|i| i.name() == name)
                .unwrap();
            (input.scores().to_vec(), input.k())
        }
    }
}

#[test]
fn signed_zero_cases_match_core_inputs() {
    let names: Vec<&str> = SIGNED_ZERO_CASES.iter().map(|s| s.name).collect();
    let want: Vec<&str> = SIGNED_ZERO_EXPECTED.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, want);
    let cases = build_signed_zero_cases().unwrap();
    assert_eq!(cases.len(), SIGNED_ZERO_EXPECTED.len());
    for (case, (name, source)) in cases.iter().zip(SIGNED_ZERO_EXPECTED) {
        let (scores, k) = core_signed_zero_input(source);
        assert_eq!(case.name, name);
        assert_eq!(bits(&case.scores), bits(&scores), "{name}");
        assert_eq!(case.k, k, "{name}");
    }
}

#[test]
fn signed_zero_inputs_hold_negative_zero_before_positive_zero() {
    let dir = TempDir::new("sz-words");
    generate(dir.path()).unwrap();
    for (name, _) in SIGNED_ZERO_EXPECTED {
        let words = bits(&read_f32(&dir.path().join(format!("{name}.scores.bin"))));
        let neg = words.iter().position(|w| *w == 0x8000_0000);
        let pos = words.iter().position(|w| *w == 0x0000_0000);
        assert!(neg.is_some() && pos.is_some(), "{name}");
        assert!(neg.unwrap() < pos.unwrap(), "{name}");
    }
}

#[test]
fn signed_zero_native_matches_core() {
    let dir = TempDir::new("sz-native");
    generate(dir.path()).unwrap();
    for (case, (name, source)) in build_signed_zero_cases()
        .unwrap()
        .iter()
        .zip(SIGNED_ZERO_EXPECTED)
    {
        let records = read_records(&dir.path().join(format!("{name}.native-scalar.bin")));
        let expected: Vec<TopKEntry> = match source {
            None => {
                let output = CaseId::E4.reference().unwrap();
                output.as_topk().unwrap().to_vec()
            }
            Some(input_name) => additional_topk_inputs()
                .iter()
                .find(|i| i.name() == input_name)
                .unwrap()
                .expected(),
        };
        let cmp = compare_topk(&expected, &entries(&records)).unwrap();
        assert!(cmp.is_match(), "{name}");
        assert_eq!(cmp.compared(), case.k, "{name}");
    }
}

/// `partial_cmp` の降順（同点はスロット番号の昇順）で先頭 k 件を並べた記録のバイト列。
/// 比べの相手にだけ使う（-0.0 と +0.0 を同点として扱う並び）。
fn ieee_order_record_bytes(scores: &[f32], k: usize) -> Vec<u8> {
    let mut idx: Vec<usize> = (0..scores.len())
        .filter(|i| scores[*i].is_finite())
        .collect();
    idx.sort_by(|a, b| scores[*b].partial_cmp(&scores[*a]).unwrap().then(a.cmp(b)));
    let picked: Vec<(u32, f32)> = idx
        .iter()
        .take(k)
        .map(|i| (*i as u32, scores[*i]))
        .collect();
    topk_record_bytes(&picked)
}

#[test]
fn signed_zero_native_differs_from_ieee_order() {
    let dir = TempDir::new("sz-ieee");
    generate(dir.path()).unwrap();
    let wanted = [2usize, 2, 1, 4];
    for ((name, _), want) in SIGNED_ZERO_EXPECTED.iter().skip(1).zip(wanted) {
        let case = build_signed_zero_cases()
            .unwrap()
            .into_iter()
            .find(|c| c.name == *name)
            .unwrap();
        let native = fs::read(dir.path().join(format!("{name}.native-scalar.bin"))).unwrap();
        let ieee = ieee_order_record_bytes(&case.scores, case.k);
        assert_eq!(native.len(), case.k * 8, "{name}");
        assert_eq!(ieee.len(), native.len(), "{name}");
        assert_eq!(differing_records(&native, &ieee), want, "{name}");
    }
}

#[test]
fn signed_zero_native_has_no_non_finite_slots() {
    let dir = TempDir::new("sz-finite");
    generate(dir.path()).unwrap();
    for case in build_signed_zero_cases().unwrap() {
        let records = read_records(&dir.path().join(format!("{}.native-scalar.bin", case.name)));
        assert_eq!(records.len(), case.k, "{}", case.name);
        assert!(
            records.iter().all(|(_, b)| f32::from_bits(*b).is_finite()),
            "{}",
            case.name
        );
    }
}

#[test]
fn signed_zero_manifest_entries_match_files() {
    let dir = TempDir::new("sz-manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let keys = manifest_entry_keys(&text);
    let before = GEMM_CASES.len() + TOPK_CASES.len() + ELEMENTWISE_CASES.len();
    assert_eq!(
        keys.len(),
        before
            + SIGNED_ZERO_CASES.len()
            + GEMM_SPECIAL_CASES.len()
            + ELEMENTWISE_SPECIAL_CASES.len()
            + TOPK_E3_CASES.len()
    );
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
    let cases = build_signed_zero_cases().unwrap();
    for (j, case) in cases.iter().enumerate() {
        // GEMM・Top-k・要素ごと演算の要素の後に並ぶ。`manifest_values` は鍵を持つ要素だけの列。
        let i = before + j;
        let k_index = GEMM_CASES.len() + TOPK_CASES.len() + j;
        let scores_index = TOPK_CASES.len() + j;
        let mut got: Vec<&str> = keys[i].iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, want);
        assert_eq!(manifest_values(&text, "name")[i], case.name);
        assert_eq!(manifest_values(&text, "kind")[i], "topk");
        assert_eq!(
            manifest_values(&text, "n")[i],
            case.scores.len().to_string()
        );
        assert_eq!(manifest_values(&text, "k")[k_index], case.k.to_string());
        let scores_file = &manifest_values(&text, "scores_file")[scores_index];
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
            manifest_values(&text, "scores_sha256")[scores_index],
            hex(&sha256_bytes(&scores_bytes))
        );
        assert_eq!(
            manifest_values(&text, "native_scalar_sha256")[i],
            hex(&sha256_bytes(&native_bytes))
        );
    }
}

#[test]
fn gen_refuses_existing_signed_zero_file() {
    let dir = TempDir::new("sz-refuse");
    let path = dir.path().join("topk-e-4.scores.bin");
    fs::write(&path, b"keep").unwrap();
    assert_eq!(generate(dir.path()), Err(GenError::FileExists));
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn signed_zero_case_rejects_unknown_input_name() {
    let spec = SignedZeroCaseSpec {
        name: "topk-unknown",
        source: SignedZeroSource::Additional("no-such-input"),
    };
    assert_eq!(
        build_signed_zero_case(&spec).err(),
        Some(GenError::InputFailed)
    );
}

// ---- 特殊値の入力（spec TASK-999 のポインタ） ----

use fandhe_silicon_core::parity::case_inputs::classify;
use fandhe_silicon_core::parity::special_values::compare_f32_with_special_values;
use fandhe_silicon_core::parity::tolerance::T1;
use wasm_parity_host::{
    GEMM_SPECIAL_CASES, GemmSpecialCaseSpec, build_gemm_special_case, build_gemm_special_cases,
    gemm_special_cpu_reference,
};

/// core の入力のうち GEMM のもの（`CaseId::ALL` の順）の（識別子, 入力）。
fn core_gemm_inputs() -> Vec<(CaseId, Vec<f32>, Vec<f32>)> {
    CaseId::ALL
        .iter()
        .filter_map(|id| match id.input().unwrap() {
            CaseInput::Gemm(g) => Some((*id, g.a().to_vec(), g.b().to_vec())),
            _ => None,
        })
        .collect()
}

fn gs_file(name: &str, suffix: &str, dir: &Path) -> Vec<f32> {
    read_f32(&dir.join(format!("{name}.{suffix}.bin")))
}

#[test]
fn gemm_special_cases_match_core_inputs() {
    let core = core_gemm_inputs();
    assert_eq!(GEMM_SPECIAL_CASES.len(), 14);
    let ids: Vec<CaseId> = GEMM_SPECIAL_CASES.iter().map(|s| s.id).collect();
    let want: Vec<CaseId> = core.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(ids, want);
    for spec in &GEMM_SPECIAL_CASES {
        assert_eq!(spec.name, format!("special-gemm-{}", spec.id.name()));
    }
    let cases = build_gemm_special_cases().unwrap();
    assert_eq!(cases.len(), core.len());
    for (case, (id, a, b)) in cases.iter().zip(core) {
        assert_eq!(bits(&case.a), bits(&a), "{}", id.name());
        assert_eq!(bits(&case.b), bits(&b), "{}", id.name());
    }
}

#[test]
fn gemm_special_files_hold_core_inputs() {
    let dir = TempDir::new("gs-inputs");
    generate(dir.path()).unwrap();
    for (spec, (_, a, b)) in GEMM_SPECIAL_CASES.iter().zip(core_gemm_inputs()) {
        assert_eq!(
            bits(&gs_file(spec.name, "a", dir.path())),
            bits(&a),
            "{}",
            spec.name
        );
        assert_eq!(
            bits(&gs_file(spec.name, "b", dir.path())),
            bits(&b),
            "{}",
            spec.name
        );
    }
}

#[test]
fn gemm_special_cpu_reference_matches_core() {
    let dir = TempDir::new("gs-cpuref");
    generate(dir.path()).unwrap();
    for spec in &GEMM_SPECIAL_CASES {
        let output = spec.id.reference().unwrap();
        let expected = output.as_f32().unwrap();
        let actual = gs_file(spec.name, "cpu-reference", dir.path());
        assert_eq!(bits(&actual), bits(expected), "{}", spec.name);
        assert_eq!(
            bits(&gemm_special_cpu_reference(spec).unwrap()),
            bits(expected)
        );
    }
}

#[test]
fn gemm_special_native_matches_default_cpu_ops() {
    let dir = TempDir::new("gs-native");
    generate(dir.path()).unwrap();
    for (spec, case) in GEMM_SPECIAL_CASES
        .iter()
        .zip(build_gemm_special_cases().unwrap())
    {
        let mut expected = vec![0.0f32; case.m * case.n];
        DefaultCpuOps
            .gemm_scalar(&case.a, &case.b, case.m, case.k, case.n, &mut expected)
            .unwrap();
        let actual = gs_file(spec.name, "native-scalar", dir.path());
        assert_eq!(bits(&actual), bits(&expected), "{}", spec.name);
    }
}

#[test]
fn gemm_special_outputs_follow_design() {
    let dir = TempDir::new("gs-design");
    generate(dir.path()).unwrap();
    for spec in &GEMM_SPECIAL_CASES {
        let design = match spec.id.input().unwrap() {
            CaseInput::Gemm(g) => g.design().to_vec(),
            _ => panic!("not a gemm input"),
        };
        for suffix in ["native-scalar", "cpu-reference"] {
            let classes: Vec<_> = gs_file(spec.name, suffix, dir.path())
                .iter()
                .map(|x| classify(*x))
                .collect();
            assert_eq!(classes, design, "{} {suffix}", spec.name);
        }
    }
}

/// 語の列に含まれる（NaN, +Inf, -Inf, +0, -0）の件数。
fn special_counts(words: &[f32]) -> [usize; 5] {
    let mut counts = [0usize; 5];
    for x in words {
        if x.is_nan() {
            counts[0] += 1;
        } else if x.to_bits() == f32::INFINITY.to_bits() {
            counts[1] += 1;
        } else if x.to_bits() == f32::NEG_INFINITY.to_bits() {
            counts[2] += 1;
        } else if x.to_bits() == 0 {
            counts[3] += 1;
        } else if x.to_bits() == 0x8000_0000 {
            counts[4] += 1;
        }
    }
    counts
}

#[test]
fn gemm_special_inputs_hold_special_values() {
    let dir = TempDir::new("gs-special");
    generate(dir.path()).unwrap();
    let mut file_total = [0usize; 5];
    let mut core_total = [0usize; 5];
    for (spec, (_, a, b)) in GEMM_SPECIAL_CASES.iter().zip(core_gemm_inputs()) {
        for (suffix, core) in [("a", &a), ("b", &b)] {
            let from_file = special_counts(&gs_file(spec.name, suffix, dir.path()));
            let from_core = special_counts(core);
            assert_eq!(from_file, from_core, "{} {suffix}", spec.name);
            for i in 0..5 {
                file_total[i] += from_file[i];
                core_total[i] += from_core[i];
            }
        }
    }
    assert_eq!(file_total, core_total);
    assert!(file_total.iter().all(|c| *c > 0), "{file_total:?}");
}

#[test]
fn gemm_special_native_agrees_with_cpu_reference_by_rule() {
    let dir = TempDir::new("gs-rule");
    generate(dir.path()).unwrap();
    for spec in &GEMM_SPECIAL_CASES {
        let native = gs_file(spec.name, "native-scalar", dir.path());
        let reference = gs_file(spec.name, "cpu-reference", dir.path());
        let cmp = compare_f32_with_special_values(&reference, &native, T1).unwrap();
        assert!(cmp.is_match(), "{}", spec.name);
    }
}

#[test]
fn gemm_special_manifest_entries_match_files() {
    let dir = TempDir::new("gs-manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let keys = manifest_entry_keys(&text);
    let before =
        GEMM_CASES.len() + TOPK_CASES.len() + ELEMENTWISE_CASES.len() + SIGNED_ZERO_CASES.len();
    assert_eq!(
        keys.len(),
        before + GEMM_SPECIAL_CASES.len() + ELEMENTWISE_SPECIAL_CASES.len() + TOPK_E3_CASES.len()
    );
    let mut want = [
        "name",
        "kind",
        "m",
        "k",
        "n",
        "a_file",
        "b_file",
        "native_scalar_file",
        "cpu_reference_file",
        "a_sha256",
        "b_sha256",
        "native_scalar_sha256",
        "cpu_reference_sha256",
    ];
    assert_eq!(want.len(), 13);
    want.sort_unstable();
    let cases = build_gemm_special_cases().unwrap();
    for (j, case) in cases.iter().enumerate() {
        // `manifest_values` は鍵を持つ要素だけの列。鍵ごとに特殊値の要素の位置が違う。
        let i = before + j;
        let m_index = GEMM_CASES.len() + j;
        let k_index = GEMM_CASES.len() + TOPK_CASES.len() + SIGNED_ZERO_CASES.len() + j;
        let mut got: Vec<&str> = keys[i].iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, want);
        assert_eq!(manifest_values(&text, "name")[i], case.name);
        assert_eq!(manifest_values(&text, "kind")[i], "gemm-special");
        assert_eq!(manifest_values(&text, "m")[m_index], case.m.to_string());
        assert_eq!(manifest_values(&text, "k")[k_index], case.k.to_string());
        assert_eq!(manifest_values(&text, "n")[i], case.n.to_string());
        let lens = [
            case.m * case.k,
            case.k * case.n,
            case.m * case.n,
            case.m * case.n,
        ];
        for (file_key, hash_key, index, len) in [
            ("a_file", "a_sha256", m_index, lens[0]),
            ("b_file", "b_sha256", m_index, lens[1]),
            ("native_scalar_file", "native_scalar_sha256", i, lens[2]),
            ("cpu_reference_file", "cpu_reference_sha256", j, lens[3]),
        ] {
            let file = &manifest_values(&text, file_key)[index];
            assert!(
                file.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-'),
                "{file}"
            );
            let bytes = fs::read(dir.path().join(file)).unwrap();
            assert_eq!(bytes.len(), len * 4, "{file}");
            assert_eq!(
                manifest_values(&text, hash_key)[index],
                hex(&sha256_bytes(&bytes)),
                "{file}"
            );
        }
    }
}

#[test]
fn gen_refuses_existing_gemm_special_file() {
    let dir = TempDir::new("gs-refuse");
    let path = dir.path().join("special-gemm-g-01.a.bin");
    fs::write(&path, b"keep").unwrap();
    assert_eq!(generate(dir.path()), Err(GenError::FileExists));
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn cpu_reference_error_text() {
    assert_eq!(
        GenError::CpuReferenceFailed.to_string(),
        "error: cpu reference failed"
    );
}

#[test]
fn gemm_special_case_rejects_non_gemm_input() {
    let spec = GemmSpecialCaseSpec {
        name: "special-gemm-e-2",
        id: CaseId::E2,
    };
    assert_eq!(
        build_gemm_special_case(&spec).err(),
        Some(GenError::InputFailed)
    );
}

#[test]
fn gemm_special_cpu_reference_rejects_non_f32_output() {
    let spec = GemmSpecialCaseSpec {
        name: "special-gemm-e-3",
        id: CaseId::E3,
    };
    assert_eq!(
        gemm_special_cpu_reference(&spec).err(),
        Some(GenError::CpuReferenceFailed)
    );
}
// ---- 特殊値の要素ごと演算と `topk-e-3`（spec TASK-999 のポインタ） ----

use fandhe_silicon_contract::cpu_ops::UnaryElementwiseOp;
use wasm_parity_host::{
    ELEMENTWISE_SPECIAL_CASES, ElementwiseSpecialCaseSpec, TOPK_E3_CASES, TopkE3CaseSpec,
    build_elementwise_special_case, build_elementwise_special_cases, build_topk_e3_case,
    build_topk_e3_cases, elementwise_special_cpu_reference,
};

/// core の `CaseId::E2` の入力の列。
fn core_e2_input() -> Vec<f32> {
    match CaseId::E2.input().unwrap() {
        CaseInput::Relu(v) => v,
        _ => panic!("not a relu input"),
    }
}

fn es_file(name: &str, suffix: &str, dir: &Path) -> Vec<f32> {
    read_f32(&dir.join(format!("{name}.{suffix}.bin")))
}

fn es_spec(op_code: u32) -> &'static ElementwiseSpecialCaseSpec {
    ELEMENTWISE_SPECIAL_CASES
        .iter()
        .find(|s| s.op_code == op_code)
        .unwrap()
}

#[test]
fn elementwise_special_cases_hold_core_input() {
    let dir = TempDir::new("es-inputs");
    generate(dir.path()).unwrap();
    let core = core_e2_input();
    assert_eq!(ELEMENTWISE_SPECIAL_CASES.len(), 2);
    let mut raw = Vec::new();
    for spec in &ELEMENTWISE_SPECIAL_CASES {
        assert_eq!(spec.id, CaseId::E2);
        assert_eq!(
            bits(&es_file(spec.name, "input", dir.path())),
            bits(&core),
            "{}",
            spec.name
        );
        raw.push(fs::read(dir.path().join(format!("{}.input.bin", spec.name))).unwrap());
    }
    assert_eq!(raw[0], raw[1]);
    for case in build_elementwise_special_cases().unwrap() {
        assert_eq!(bits(&case.input), bits(&core), "{}", case.name);
    }
}

#[test]
fn elementwise_special_input_holds_special_values() {
    let dir = TempDir::new("es-special");
    generate(dir.path()).unwrap();
    let from_core = special_counts(&core_e2_input());
    assert!(from_core.iter().all(|c| *c > 0), "{from_core:?}");
    for spec in &ELEMENTWISE_SPECIAL_CASES {
        let from_file = special_counts(&es_file(spec.name, "input", dir.path()));
        assert_eq!(from_file, from_core, "{}", spec.name);
    }
}

#[test]
fn elementwise_special_native_matches_default_cpu_ops() {
    let dir = TempDir::new("es-native");
    generate(dir.path()).unwrap();
    for (spec, case) in ELEMENTWISE_SPECIAL_CASES
        .iter()
        .zip(build_elementwise_special_cases().unwrap())
    {
        let mut expected = vec![0.0f32; case.input.len()];
        DefaultCpuOps
            .elementwise_scalar(case.op, &case.input, &mut expected)
            .unwrap();
        let actual = es_file(spec.name, "native-scalar", dir.path());
        assert_eq!(bits(&actual), bits(&expected), "{}", spec.name);
    }
}

#[test]
fn elementwise_special_relu_cpu_reference_matches_core() {
    let dir = TempDir::new("es-relu-ref");
    generate(dir.path()).unwrap();
    let spec = es_spec(0);
    let output = CaseId::E2.reference().unwrap();
    let expected = output.as_f32().unwrap();
    let actual = es_file(spec.name, "cpu-reference", dir.path());
    assert_eq!(bits(&actual), bits(expected));
    assert_eq!(
        bits(&elementwise_special_cpu_reference(spec, &core_e2_input()).unwrap()),
        bits(expected)
    );
}

#[test]
fn elementwise_special_add_scalar_cpu_reference_matches_expression() {
    let dir = TempDir::new("es-add-ref");
    generate(dir.path()).unwrap();
    let spec = es_spec(1);
    let expected: Vec<f32> = core_e2_input().iter().map(|x| x + 0.5).collect();
    let actual = es_file(spec.name, "cpu-reference", dir.path());
    assert_eq!(bits(&actual), bits(&expected));
}

#[test]
fn elementwise_special_relu_maps_nan_to_zero() {
    let dir = TempDir::new("es-relu-nan");
    generate(dir.path()).unwrap();
    let spec = es_spec(0);
    let input = core_e2_input();
    let native = es_file(spec.name, "native-scalar", dir.path());
    let reference = es_file(spec.name, "cpu-reference", dir.path());
    let nan_positions: Vec<usize> = (0..input.len()).filter(|i| input[*i].is_nan()).collect();
    assert!(!nan_positions.is_empty());
    for i in nan_positions {
        assert_eq!(native[i], 0.0, "native {i}");
        assert_eq!(reference[i], 0.0, "reference {i}");
    }
    assert!(native.iter().all(|x| !x.is_nan()));
    assert!(reference.iter().all(|x| !x.is_nan()));
}

#[test]
fn elementwise_special_native_agrees_with_cpu_reference_by_rule() {
    let dir = TempDir::new("es-rule");
    generate(dir.path()).unwrap();
    for spec in &ELEMENTWISE_SPECIAL_CASES {
        let native = es_file(spec.name, "native-scalar", dir.path());
        let reference = es_file(spec.name, "cpu-reference", dir.path());
        let cmp = compare_f32_with_special_values(&reference, &native, T1).unwrap();
        assert!(cmp.is_match(), "{}", spec.name);
    }
}

#[test]
fn elementwise_special_manifest_entries_match_files() {
    let dir = TempDir::new("es-manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let keys = manifest_entry_keys(&text);
    let ew = ELEMENTWISE_CASES.len();
    let gs = GEMM_SPECIAL_CASES.len();
    let all_before = GEMM_CASES.len() + TOPK_CASES.len() + ew + SIGNED_ZERO_CASES.len() + gs;
    assert_eq!(
        keys.len(),
        all_before + ELEMENTWISE_SPECIAL_CASES.len() + TOPK_E3_CASES.len()
    );
    let mut want = [
        "name",
        "kind",
        "n",
        "op_code",
        "c_bits",
        "input_file",
        "native_scalar_file",
        "cpu_reference_file",
        "input_sha256",
        "native_scalar_sha256",
        "cpu_reference_sha256",
    ];
    assert_eq!(want.len(), 11);
    want.sort_unstable();
    for (j, case) in build_elementwise_special_cases()
        .unwrap()
        .iter()
        .enumerate()
    {
        // `manifest_values` は鍵を持つ要素だけの列。鍵ごとに特殊値の要素の位置が違う。
        let i = all_before + j;
        let input_index = ew + j;
        let ref_index = gs + j;
        let mut got: Vec<&str> = keys[i].iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, want);
        assert_eq!(manifest_values(&text, "name")[i], case.name);
        assert_eq!(manifest_values(&text, "kind")[i], "elementwise-special");
        assert_eq!(manifest_values(&text, "n")[i], case.input.len().to_string());
        assert_eq!(
            manifest_values(&text, "op_code")[input_index],
            case.op_code.to_string()
        );
        assert_eq!(
            manifest_values(&text, "c_bits")[input_index],
            format!("{:08x}", case.c_bits)
        );
        for (file_key, hash_key, index) in [
            ("input_file", "input_sha256", input_index),
            ("native_scalar_file", "native_scalar_sha256", i),
            ("cpu_reference_file", "cpu_reference_sha256", ref_index),
        ] {
            let file = &manifest_values(&text, file_key)[index];
            assert!(
                file.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'-'),
                "{file}"
            );
            let bytes = fs::read(dir.path().join(file)).unwrap();
            assert_eq!(bytes.len(), case.input.len() * 4, "{file}");
            assert_eq!(
                manifest_values(&text, hash_key)[index],
                hex(&sha256_bytes(&bytes)),
                "{file}"
            );
        }
    }
}

#[test]
fn gen_refuses_existing_elementwise_special_file() {
    let dir = TempDir::new("es-refuse");
    let path = dir.path().join("special-elementwise-relu-e-2.input.bin");
    fs::write(&path, b"keep").unwrap();
    assert_eq!(generate(dir.path()), Err(GenError::FileExists));
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn elementwise_special_case_rejects_non_relu_input() {
    let spec = ElementwiseSpecialCaseSpec {
        name: "special-elementwise-relu-e-3",
        id: CaseId::E3,
        op: UnaryElementwiseOp::Relu,
        op_code: 0,
        c_bits: 0,
    };
    assert_eq!(
        build_elementwise_special_case(&spec).err(),
        Some(GenError::InputFailed)
    );
}

#[test]
fn elementwise_special_cpu_reference_rejects_non_f32_output() {
    let spec = ElementwiseSpecialCaseSpec {
        name: "special-elementwise-relu-e-3",
        id: CaseId::E3,
        op: UnaryElementwiseOp::Relu,
        op_code: 0,
        c_bits: 0,
    };
    assert_eq!(
        elementwise_special_cpu_reference(&spec, &[]).err(),
        Some(GenError::CpuReferenceFailed)
    );
}

fn core_e3_input() -> (Vec<f32>, usize) {
    match CaseId::E3.input().unwrap() {
        CaseInput::TopK { scores, k } => (scores, k),
        _ => panic!("not a top-k input"),
    }
}

#[test]
fn topk_e3_case_holds_core_input() {
    let dir = TempDir::new("e3-inputs");
    generate(dir.path()).unwrap();
    let (scores, k) = core_e3_input();
    assert_eq!(TOPK_E3_CASES.len(), 1);
    let spec = &TOPK_E3_CASES[0];
    assert_eq!(spec.name, "topk-e-3");
    assert_eq!(spec.id, CaseId::E3);
    let cases = build_topk_e3_cases().unwrap();
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].k, k);
    assert_eq!(bits(&cases[0].scores), bits(&scores));
    let actual = read_f32(&dir.path().join("topk-e-3.scores.bin"));
    assert_eq!(bits(&actual), bits(&scores));
}

#[test]
fn topk_e3_native_matches_core() {
    let dir = TempDir::new("e3-native");
    generate(dir.path()).unwrap();
    let (_, k) = core_e3_input();
    let records = read_records(&dir.path().join("topk-e-3.native-scalar.bin"));
    let output = CaseId::E3.reference().unwrap();
    let expected = output.as_topk().unwrap().to_vec();
    let cmp = compare_topk(&expected, &entries(&records)).unwrap();
    assert!(cmp.is_match());
    assert_eq!(cmp.compared(), k);
}

#[test]
fn topk_e3_manifest_entry_matches_files() {
    let dir = TempDir::new("e3-manifest");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let keys = manifest_entry_keys(&text);
    let (scores, k) = core_e3_input();
    let all_before = GEMM_CASES.len()
        + TOPK_CASES.len()
        + ELEMENTWISE_CASES.len()
        + SIGNED_ZERO_CASES.len()
        + GEMM_SPECIAL_CASES.len()
        + ELEMENTWISE_SPECIAL_CASES.len();
    assert_eq!(keys.len(), all_before + TOPK_E3_CASES.len());
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
    assert_eq!(want.len(), 8);
    want.sort_unstable();
    let mut got: Vec<&str> = keys[all_before].iter().map(String::as_str).collect();
    got.sort_unstable();
    assert_eq!(got, want);
    // `manifest_values` は鍵を持つ要素だけの列。鍵ごとに `topk-e-3` の位置が違う。
    let k_index =
        GEMM_CASES.len() + TOPK_CASES.len() + SIGNED_ZERO_CASES.len() + GEMM_SPECIAL_CASES.len();
    let scores_index = TOPK_CASES.len() + SIGNED_ZERO_CASES.len();
    assert_eq!(manifest_values(&text, "name")[all_before], "topk-e-3");
    assert_eq!(manifest_values(&text, "kind")[all_before], "topk");
    assert_eq!(
        manifest_values(&text, "n")[all_before],
        scores.len().to_string()
    );
    assert_eq!(manifest_values(&text, "k")[k_index], k.to_string());
    for (file_key, hash_key, index, len) in [
        (
            "scores_file",
            "scores_sha256",
            scores_index,
            scores.len() * 4,
        ),
        (
            "native_scalar_file",
            "native_scalar_sha256",
            all_before,
            k * 8,
        ),
    ] {
        let file = &manifest_values(&text, file_key)[index];
        let bytes = fs::read(dir.path().join(file)).unwrap();
        assert_eq!(bytes.len(), len, "{file}");
        assert_eq!(
            manifest_values(&text, hash_key)[index],
            hex(&sha256_bytes(&bytes)),
            "{file}"
        );
    }
}

#[test]
fn topk_e3_case_rejects_non_topk_input() {
    let spec = TopkE3CaseSpec {
        name: "topk-e-2",
        id: CaseId::E2,
    };
    assert_eq!(build_topk_e3_case(&spec).err(), Some(GenError::InputFailed));
}

/// 表の行の `op` から、glue に渡す演算の番号と定数のビット列を導く。表の `op_code`・`c_bits` との照合の期待に使う。
fn expected_op_fields(op: UnaryElementwiseOp) -> (u32, u32) {
    match op {
        UnaryElementwiseOp::Relu => (0, 0),
        UnaryElementwiseOp::AddScalar(c) => (1, c.to_bits()),
        _ => panic!("unexpected op"),
    }
}

/// 通常と特殊値の要素ごと演算の表の全行で、`op` と `op_code`・`c_bits` が合い、`generate` が書く manifest の値も表と同じになる。
#[test]
fn elementwise_tables_op_matches_op_code_and_c_bits() {
    let rows: Vec<(&str, UnaryElementwiseOp, u32, u32)> = ELEMENTWISE_CASES
        .iter()
        .map(|s| (s.name, s.op, s.op_code, s.c_bits))
        .chain(
            ELEMENTWISE_SPECIAL_CASES
                .iter()
                .map(|s| (s.name, s.op, s.op_code, s.c_bits)),
        )
        .collect();
    assert_eq!(rows.len(), 4);
    for (name, op, op_code, c_bits) in &rows {
        assert_eq!(expected_op_fields(*op), (*op_code, *c_bits), "{name}");
    }
    let dir = TempDir::new("ew-op-fields");
    generate(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let names = manifest_values(&text, "name");
    let kinds = manifest_values(&text, "kind");
    assert_eq!(names.len(), kinds.len());
    let ew_names: Vec<&String> = names
        .iter()
        .zip(&kinds)
        .filter(|(_, k)| k.starts_with("elementwise"))
        .map(|(n, _)| n)
        .collect();
    let codes = manifest_values(&text, "op_code");
    let cbits = manifest_values(&text, "c_bits");
    assert_eq!(ew_names.len(), rows.len());
    assert_eq!(codes.len(), rows.len());
    assert_eq!(cbits.len(), rows.len());
    for (name, op, _, _) in &rows {
        let i = ew_names.iter().position(|n| n.as_str() == *name).unwrap();
        let (code, bits) = expected_op_fields(*op);
        assert_eq!(codes[i], code.to_string(), "{name}");
        assert_eq!(cbits[i], format!("{bits:08x}"), "{name}");
    }
}
