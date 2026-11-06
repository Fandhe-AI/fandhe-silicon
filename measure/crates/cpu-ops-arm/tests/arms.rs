//! `measure-cpu-ops-arm` の公開テスト（TASK-2960 のポインタ）。
//!
//! aarch64 を要求しない。neon の照合は関数に分けてあり、neon でないビルドでも直接試験する。
//! 書き込みはクレートの一時領域（`CARGO_TARGET_TMPDIR`）の下だけで、終わりに消す。

use measure_cpu_ops_arm::*;
use measure_perf::jsonread;
use measure_perf::sample::SampleCounts;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// 試験ごとに別の空ディレクトリを作る。
fn temp_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("cpu-ops-arm-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

const SMALL: [&str; 6] = ["--cold", "0", "--warmup", "0", "--measured", "1"];

#[test]
fn default_counts_are_cold_1_warmup_20_measured_200() {
    assert_eq!(
        DEFAULT_COUNTS,
        SampleCounts {
            cold: 1,
            warmup: 20,
            measured: 200
        }
    );
    let cfg = parse_args(&args(&["--run-id", "r", "--out-dir", "d"])).unwrap();
    assert_eq!(cfg.counts, DEFAULT_COUNTS);
}

#[test]
fn arm_names_are_ten_and_unique() {
    assert_eq!(ARMS.len(), 10);
    let names: BTreeSet<_> = ARMS.iter().map(|a| a.name).collect();
    assert_eq!(names.len(), 10);
    assert_eq!(ARMS.iter().filter(|a| a.simd).count(), 5);
    for arm in &ARMS {
        // 腕の名前を足した run の ID が形に合う。
        arm_run_id("run", arm).unwrap();
        assert!(known_output_sha256(arm.case).is_some());
    }
}

#[test]
fn run_writes_ten_readable_pending_records() {
    let dir = temp_dir("run");
    let mut a = vec![
        "--run-id",
        "arm-test",
        "--session-ref",
        "sess-1",
        "--out-dir",
    ];
    let d = dir.to_str().unwrap().to_owned();
    a.push(&d);
    a.extend(SMALL);
    let cfg = parse_args(&args(&a)).unwrap();
    let mut stdout = Vec::new();
    run(&cfg, &mut stdout).unwrap();
    let text = String::from_utf8(stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some(format!("neon={NEON}").as_str()));
    assert_eq!(lines.count(), 10);

    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert_eq!(files.len(), 10);
    for path in &files {
        let bytes = fs::read(path).unwrap();
        let v = jsonread::parse(&bytes).unwrap();
        assert_eq!(v.get("label").and_then(|x| x.as_str()), Some("pending"));
        assert_eq!(
            v.get("machine_combination").and_then(|x| x.as_str()),
            Some("metal-mac-mini")
        );
        assert_eq!(v.get("machine").and_then(|x| x.as_str()), Some("mac-mini"));
        assert_eq!(
            v.get("session_ref").and_then(|x| x.as_str()),
            Some("sess-1")
        );
        let tasks: Vec<_> = v
            .get("task_ids")
            .and_then(|x| x.as_array())
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        assert_eq!(tasks, ["TASK-2960"]);
        // 実ホスト名・絶対パスを含まない。
        let s = String::from_utf8(bytes).unwrap();
        assert!(!s.contains(dir.to_str().unwrap()));
    }
    let _ = fs::remove_dir_all(&dir);
}

/// `wasm-parity-host gen` の manifest の `a_sha256`・`b_sha256`・`input_sha256` と同じ値。
#[test]
fn input_hashes_match_the_parity_host_inputs() {
    let known: [(&str, &str, &str); 8] = [
        (
            "gemm-64",
            "a",
            "2259215901630df152c4b1e0df09edf21118c82c285b4a9968901fca8ca2ca52",
        ),
        (
            "gemm-64",
            "b",
            "9d19ef5ea664105fe1cfbdc69868867fcecde4652eb739d1912527d4652d3eeb",
        ),
        (
            "gemm-256",
            "a",
            "0d509543d75aff9fa7372ca461e5c91dd8e9d8e41c81fe3b302e6610b1228b89",
        ),
        (
            "gemm-256",
            "b",
            "e5aae545d81ea7cf7b62df1d03c79055a67967826d0b74ea02c0c68063fbd912",
        ),
        (
            "gemm-37x41x53",
            "a",
            "8b6bdc24887e48cd2bc3598950e2949c599f27b9c14d2a2c11384d2bb9c454d1",
        ),
        (
            "gemm-37x41x53",
            "b",
            "60efc5337595142f4ec8d78f08e8e2663f4a67630672e1d21d7cc2f21f90e19e",
        ),
        (
            "elementwise-relu",
            "input",
            "e9b99b8c78de25bd6fcf651b4674abbb8e6525d4480ce60c5ef8f52f0061ec52",
        ),
        (
            "elementwise-add-scalar",
            "input",
            "e9b99b8c78de25bd6fcf651b4674abbb8e6525d4480ce60c5ef8f52f0061ec52",
        ),
    ];
    for (case, key, expected) in known {
        let table = input_sha256_hex(case).unwrap();
        let got = table
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str());
        assert_eq!(got, Some(expected), "{case} {key}");
    }
}

#[test]
fn known_output_check_is_a_gate_only_when_enabled() {
    for (case, expected) in KNOWN_NEON_OUTPUT_SHA256 {
        // 一致する値は通る。
        check_known_output(case, expected, true).unwrap();
        // 値を変えると、有効なときだけ失敗する（陽性対照）。
        let mut wrong = expected.to_owned();
        wrong.replace_range(0..1, if expected.starts_with('0') { "1" } else { "0" });
        assert_eq!(
            check_known_output(case, &wrong, true),
            Err(ArmError::OutputMismatch),
            "{case}"
        );
        assert_eq!(check_known_output(case, &wrong, false), Ok(()), "{case}");
    }
    assert_eq!(
        check_known_output("no-such-case", "00", true),
        Err(ArmError::UnknownCase)
    );
}

#[test]
fn known_output_sha256_values_are_well_formed() {
    for (_, h) in KNOWN_NEON_OUTPUT_SHA256 {
        assert_eq!(h.len(), 64);
        assert!(
            h.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        );
    }
}

#[test]
fn invalid_arguments_are_rejected() {
    let bad: [&[&str]; 9] = [
        &[],
        &["--out-dir", "d"],
        &["--run-id", "r"],
        &["--run-id", "r", "--out-dir", "d", "--unknown", "x"],
        &["--run-id", "r", "--out-dir", "d", "--run-id", "s"],
        &["--run-id", "r", "--out-dir"],
        &["--run-id", "Bad_ID", "--out-dir", "d"],
        &["--run-id", "r", "--out-dir", "d", "--measured", "0"],
        &["--run-id", "r", "--out-dir", "d", "--repo-commit", "abc"],
    ];
    for a in bad {
        assert!(parse_args(&args(a)).is_err(), "{a:?}");
    }
    // 腕の名前を足すと 64 文字を超える ID は拒否する。
    let long = "a".repeat(60);
    assert!(parse_args(&args(&["--run-id", &long, "--out-dir", "d"])).is_err());
}

#[test]
fn missing_directory_and_existing_files_fail_without_overwriting() {
    let dir = temp_dir("exists");
    let d = dir.to_str().unwrap().to_owned();
    let missing = dir.join("nope").to_str().unwrap().to_owned();
    let mut stdout = Vec::new();

    let mut a = vec!["--run-id", "x1", "--out-dir", &missing];
    a.extend(SMALL);
    let cfg = parse_args(&args(&a)).unwrap();
    assert_eq!(run(&cfg, &mut stdout), Err(ArmError::MissingDirectory));

    let mut a = vec!["--run-id", "x2", "--out-dir", &d];
    a.extend(SMALL);
    let cfg = parse_args(&args(&a)).unwrap();
    let existing = dir.join(format!("x2-{}.json", ARMS[9].name));
    fs::write(&existing, b"keep").unwrap();
    assert_eq!(run(&cfg, &mut stdout), Err(ArmError::FileExists));
    assert_eq!(fs::read(&existing).unwrap(), b"keep");
    // 測る前の確認で止まるので、ほかのファイルは作られない。
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn binary_exits_nonzero_on_bad_arguments() {
    let bin = env!("CARGO_BIN_EXE_cpu-ops-arm");
    let status = Command::new(bin).arg("--bogus").status().unwrap();
    assert!(!status.success());
    let status = Command::new(bin).status().unwrap();
    assert!(!status.success());
    let out = Command::new(bin)
        .args([
            "--run-id",
            "r",
            "--out-dir",
            "/no/such/dir/for/arm",
            "--measured",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    // エラー文に入力のパスを出さない。
    assert!(!String::from_utf8_lossy(&out.stderr).contains("/no/such"));
}
