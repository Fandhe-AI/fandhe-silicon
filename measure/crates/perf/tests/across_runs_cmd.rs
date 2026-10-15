//! run 間の集計の出力（`runs`・`jsonread` とコマンド `across-runs`）の公開テスト（TASK-27 のポインタ）。
//!
//! 記入例の記録（`docs/run-record.example.json`）の文字列を部分的に書き換えて run を合成し、
//! 集計・混在の拒否・1 件ごとの整合・出力の形・コマンドの終了コードを確かめる。
//! 書き込みは一時ディレクトリだけで、終わりに消す。作業ツリーと環境変数は変えない。

use measure_perf::across::across_runs;
use measure_perf::jsonread::{self, MAX_BYTES, ReadError};
use measure_perf::runs::{AcrossError, aggregate, render, run_input};
use measure_perf::stats::run_stats;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn example() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/run-record.example.json");
    std::fs::read_to_string(p).unwrap()
}

/// 記入例の CPU の標本（3 個）と ID だけ差し替えた記録。中央値は標本の真ん中。
fn rec(id: &str, cpu: [u64; 3]) -> String {
    let mut s = [cpu[0], cpu[1], cpu[2]];
    s.sort_unstable();
    example()
        .replace(
            "\"run_id\":\"example-run-a\"",
            &format!("\"run_id\":\"{id}\""),
        )
        .replace(
            "\"measured\":[10,20,30]",
            &format!("\"measured\":[{},{},{}]", cpu[0], cpu[1], cpu[2]),
        )
        .replace(
            "\"cpu_ns\":{\"median\":20,\"q1\":20,\"q3\":30,\"p95\":30,\"min\":10,\"max\":30}",
            &format!(
                "\"cpu_ns\":{{\"median\":{},\"q1\":{},\"q3\":{},\"p95\":{},\"min\":{},\"max\":{}}}",
                s[1], s[1], s[2], s[2], s[0], s[2]
            ),
        )
}

fn input(text: &str) -> Result<measure_perf::runs::RunInput, AcrossError> {
    run_input(&jsonread::parse(text.as_bytes()).unwrap())
}

fn agg(texts: &[String]) -> Result<String, AcrossError> {
    let inputs: Vec<_> = texts.iter().map(|t| input(t)).collect::<Result<_, _>>()?;
    aggregate(&inputs).map(|a| render(&a))
}

#[test]
fn a1_odd_count() {
    let out = agg(&[
        rec("run-a", [10, 20, 30]),
        rec("run-b", [100, 200, 300]),
        rec("run-c", [40, 50, 60]),
    ])
    .unwrap();
    assert!(out.contains("\"run_count\":3,\"run_ids\":[\"run-a\",\"run-b\",\"run-c\"]"));
    assert!(out.contains(
        "\"cpu_ns\":{\"median\":50,\"min\":20,\"max\":200,\"spread_abs\":180,\"spread_ratio\":10.000}"
    ));
}

#[test]
fn a2_even_count_with_quarter_steps() {
    // 中央値が 20 と 21（run 内は奇数個なので整数）→ 20.5。.5 の run 間の平均は .25 / .75 になる。
    let out = agg(&[rec("run-a", [10, 20, 30]), rec("run-b", [10, 21, 30])]).unwrap();
    assert!(out.contains("\"median\":20.5,\"min\":20,\"max\":21,\"spread_abs\":1,"));
    assert!(out.contains("\"spread_ratio\":1.050"));
    let mut even = rec("run-a", [10, 20, 30]);
    even = even
        .replace("\"measured\":[10,20,30]", "\"measured\":[10,20,21,30]")
        .replace("\"measured\":3}", "\"measured\":4}")
        .replace("\"measured\":[5,6,7]", "\"measured\":[5,6,7,8]")
        .replace("\"median\":20,", "\"median\":20.5,")
        .replace("\"median\":6,", "\"median\":6.5,");
    let other = even.replace("run-a", "run-b");
    assert!(agg(&[even.clone(), other]).is_ok());
    let third = even
        .replace("run-a", "run-c")
        .replace("\"measured\":[10,20,21,30]", "\"measured\":[10,20,22,30]")
        .replace("\"median\":20.5,", "\"median\":21,");
    let out = agg(&[even, third]).unwrap();
    assert!(out.contains("\"median\":20.75,"), "{out}");
}

#[test]
fn a3_matches_float_across_runs() {
    let a = across_runs(&[20.0, 200.0, 50.0]).unwrap();
    let out = agg(&[
        rec("run-a", [10, 20, 30]),
        rec("run-b", [100, 200, 300]),
        rec("run-c", [40, 50, 60]),
    ])
    .unwrap();
    assert_eq!(a.median, 50.0);
    assert_eq!(a.spread_abs, 180.0);
    assert!(out.contains("\"median\":50,"));
    let s = run_stats(&[10, 20, 30]).unwrap();
    assert_eq!(s.median, 20.0);
}

#[test]
fn a4_zero_min_ratio_is_null_and_ratio_truncates() {
    let out = agg(&[rec("run-a", [0, 0, 5]), rec("run-b", [1, 3, 5])]).unwrap();
    assert!(
        out.contains("\"spread_abs\":3,\"spread_ratio\":null}"),
        "{out}"
    );
    let out = agg(&[rec("run-a", [1, 3, 5]), rec("run-b", [1, 4, 5])]).unwrap();
    assert!(out.contains("\"spread_ratio\":1.333}"), "{out}");
}

#[test]
fn a5_gpu_band_and_null() {
    let out = agg(&[rec("run-a", [10, 20, 30])]).unwrap();
    assert!(out.contains(
        "\"gpu_ns\":{\"median\":6,\"min\":6,\"max\":6,\"spread_abs\":0,\"spread_ratio\":1.000}"
    ));
    let null_gpu = |id: &str| {
        rec(id, [10, 20, 30])
            .replace("\"measured\":[5,6,7]", "\"measured\":[5,null,7]")
            .replace(
                "\"gpu_ns\":{\"median\":6,\"q1\":6,\"q3\":7,\"p95\":7,\"min\":5,\"max\":7}",
                "\"gpu_ns\":null",
            )
    };
    let out = agg(&[rec("run-a", [10, 20, 30]), null_gpu("run-b")]).unwrap();
    assert!(out.ends_with("\"gpu_ns\":null}"), "{out}");
    // 全件で取得不能でも CPU の集計は出る。
    assert!(agg(&[null_gpu("run-a"), null_gpu("run-b")]).is_ok());
}

#[test]
fn a6_rejects_mixed_and_invalid_sets() {
    assert_eq!(agg(&[]), Err(AcrossError::Empty));
    assert_eq!(
        agg(&[rec("run-a", [1, 2, 3]), rec("run-a", [1, 2, 3])]),
        Err(AcrossError::DuplicateRunId)
    );
    let base = rec("run-a", [1, 2, 3]);
    let other = rec("run-b", [1, 2, 3]);
    let cases = [
        (
            "machine_combination",
            "\"machine_combination\":\"metal-mac-mini\"",
            "\"machine_combination\":\"vulkan-linux-nvidia\"",
        ),
        (
            "session_ref",
            "\"session_ref\":\"example-session\"",
            "\"session_ref\":\"other-session\"",
        ),
        (
            "task_ids",
            "\"task_ids\":[\"TASK-27\"]",
            "\"task_ids\":[\"TASK-28\"]",
        ),
        (
            "behavior_ids",
            "\"behavior_ids\":[]",
            "\"behavior_ids\":[\"BHV-1\"]",
        ),
        (
            "sample_counts",
            "\"warmup\":1,\"measured\":3",
            "\"warmup\":2,\"measured\":3",
        ),
        (
            "dispatch_count",
            "\"dispatch_count\":6",
            "\"dispatch_count\":7",
        ),
        (
            "input_sha256",
            "\"input_sha256\":{\"example-item\":\"4444",
            "\"input_sha256\":{\"example-item\":\"4445",
        ),
        (
            "kernel_sha256",
            "\"kernel_sha256\":{\"example-item\":\"1111",
            "\"kernel_sha256\":{\"example-item\":\"1112",
        ),
        (
            "build_profile",
            "\"build_profile\":\"release\"",
            "\"build_profile\":\"dev\"",
        ),
        (
            "feature_set",
            "\"feature_set\":[\"example-feature\"]",
            "\"feature_set\":[\"other-feature\"]",
        ),
        (
            "target_features",
            "\"target_features\":[]",
            "\"target_features\":[\"avx2\"]",
        ),
        (
            "shader_compile_options",
            "\"shader_compile_options\":[\"-example\"]",
            "\"shader_compile_options\":[\"-other\"]",
        ),
        (
            "repo_commit",
            "\"repo_commit\":\"0000000000",
            "\"repo_commit\":\"1111111111",
        ),
        (
            "submission_count",
            "\"submission_count\":5",
            "\"submission_count\":6",
        ),
    ];
    for (name, from, to) in cases {
        assert!(other.contains(from), "{name}");
        let r = agg(&[base.clone(), other.replace(from, to)]);
        assert!(
            matches!(
                r,
                Err(AcrossError::Heterogeneous(_)) | Err(AcrossError::Field(_))
            ),
            "{name}: {r:?}"
        );
    }
    let with_baseline = other.replacen(
        "\"body\":",
        "\"wgpu_baseline\":{\"basis\":\"x\"},\"body\":",
        1,
    );
    assert!(agg(&[base.clone(), with_baseline]).is_err());
    for label in ["not-run", "indeterminate"] {
        let r = agg(&[
            base.clone(),
            other.replace("\"label\":\"ok\"", &format!("\"label\":\"{label}\"")),
        ]);
        assert_eq!(r, Err(AcrossError::NotMeasured));
    }
}

#[test]
fn a6b_key_order_does_not_make_runs_heterogeneous() {
    let base = rec("run-a", [1, 2, 3]);
    let from = "\"cold\":1,\"warmup\":1,\"measured\":3";
    assert!(base.contains(from));
    let other = rec("run-b", [1, 2, 3]).replace(from, "\"measured\":3,\"warmup\":1,\"cold\":1");
    assert!(agg(&[base.clone(), other]).is_ok());
    let a = base.replacen(
        "\"body\":",
        "\"wgpu_baseline\":{\"x\":1,\"y\":2},\"body\":",
        1,
    );
    let b = rec("run-b", [1, 2, 3]).replacen(
        "\"body\":",
        "\"wgpu_baseline\":{\"y\":2,\"x\":1},\"body\":",
        1,
    );
    assert!(agg(&[a, b]).is_ok());
}

#[test]
fn a7_per_run_consistency() {
    let bad = rec("run-a", [10, 20, 30]).replace("\"median\":20,", "\"median\":21,");
    assert_eq!(input(&bad), Err(AcrossError::Inconsistent("cpu_ns")));
    let bad = rec("run-a", [10, 20, 30]).replace("\"median\":6,", "\"median\":7,");
    assert_eq!(input(&bad), Err(AcrossError::Inconsistent("gpu_ns")));
    let bad = rec("run-a", [10, 20, 30]).replace("\"measured\":[5,6,7]", "\"measured\":[5,null,7]");
    assert_eq!(input(&bad), Err(AcrossError::Inconsistent("gpu_ns")));
}

#[test]
fn a8_rejects_unknown_tokens() {
    for (from, to) in [
        ("\"schema_version\":\"1\"", "\"schema_version\":\"2\""),
        ("\"body_kind\":\"perf-samples\"", "\"body_kind\":\"other\""),
        ("\"machine\":\"mac-mini\"", "\"machine\":\"unknown-box\""),
        ("\"label\":\"ok\"", "\"label\":\"weird\""),
        ("\"run_id\":\"example-run-a\"", "\"run_id\":\"Bad_Id\""),
    ] {
        let t = example().replace(from, to);
        assert!(matches!(input(&t), Err(AcrossError::Field(_))), "{to}");
    }
}

#[test]
fn a9_output_is_deterministic_ascii_single_line() {
    let a = [rec("run-b", [10, 20, 30]), rec("run-a", [10, 20, 30])];
    let out1 = agg(&a).unwrap();
    let out2 = agg(&[a[1].clone(), a[0].clone()]).unwrap();
    assert_eq!(out1, out2);
    assert!(out1.is_ascii() && !out1.contains('\n'));
    assert!(out1.starts_with("{\"output_kind\":\"across-runs\",\"run_count\":2,"));
    assert!(!out1.contains("example-feature") && !out1.contains("Example"));
}

#[test]
fn a10_jsonread_is_strict() {
    let bad: [&[u8]; 12] = [
        b"{\"a\":1,\"a\":2}",
        b"{\"a\":NaN}",
        b"[Infinity]",
        b"{} x",
        b"",
        b"[1,]",
        b"\"\x01\"",
        b"\"\\q\"",
        b"\"\\ud800\"",
        b"[01]",
        b"[1.]",
        "\"é\"".as_bytes(),
    ];
    for b in bad {
        assert!(jsonread::parse(b).is_err(), "{b:?}");
    }
    let deep = "[".repeat(200);
    assert_eq!(jsonread::parse(deep.as_bytes()), Err(ReadError::TooDeep));
    let big = vec![b' '; MAX_BYTES + 1];
    assert_eq!(jsonread::parse(&big), Err(ReadError::TooLarge));
    assert_eq!(
        jsonread::parse(b"{\"a\":1,\"a\":2}"),
        Err(ReadError::DuplicateKey)
    );
    assert!(jsonread::parse(b" {\"a\":[1,2.5,\"\\ud83d\\ude00\",null,true]}\n").is_ok());
    // キーが非常に多い object でも重複検出が線形時間で終わる（O(n^2) の退行を防ぐ）
    let mut many = String::from("{");
    for i in 0..100_000 {
        if i > 0 {
            many.push(',');
        }
        many.push_str(&format!("\"k{i}\":0"));
    }
    many.push('}');
    assert!(jsonread::parse(many.as_bytes()).is_ok());
    let dup = many.replacen("\"k99999\"", "\"k0\"", 1);
    assert_eq!(
        jsonread::parse(dup.as_bytes()),
        Err(ReadError::DuplicateKey)
    );
    let n = jsonread::parse(b"18446744073709551616").unwrap();
    assert_eq!(n.as_u64(), None);
}

// --- コマンド ---

struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("across-runs-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn write(&self, name: &str, body: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cmd(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_across-runs"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn p1_command_matches_render_and_p2_example_reads() {
    let t = Tmp::new("p1");
    let texts = [
        rec("run-a", [10, 20, 30]),
        rec("run-b", [100, 200, 300]),
        rec("run-c", [40, 50, 60]),
    ];
    let paths: Vec<PathBuf> = texts
        .iter()
        .enumerate()
        .map(|(i, s)| t.write(&format!("{i}.json"), s))
        .collect();
    let refs: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    let o = cmd(&refs);
    assert_eq!(o.status.code(), Some(0));
    let expect = format!("{}\n", agg(&texts).unwrap());
    assert_eq!(String::from_utf8(o.stdout).unwrap(), expect);
    assert!(o.stderr.is_empty());

    let ex = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/run-record.example.json");
    let o = cmd(&[ex.as_path()]);
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn p3_exit_codes() {
    let t = Tmp::new("p3");
    assert_eq!(cmd(&[]).status.code(), Some(2));
    assert_eq!(cmd(&[Path::new("--bogus")]).status.code(), Some(2));
    assert_eq!(
        cmd(&[t.0.join("missing.json").as_path()]).status.code(),
        Some(2)
    );
    assert_eq!(cmd(&[t.0.as_path()]).status.code(), Some(1));
    let bad = t.write("bad.json", "{not json");
    assert_eq!(cmd(&[bad.as_path()]).status.code(), Some(1));
    let a = t.write("a.json", &rec("run-a", [1, 2, 3]));
    let b = t.write(
        "b.json",
        &rec("run-b", [1, 2, 3]).replace("mac-mini", "other"),
    );
    assert_eq!(cmd(&[a.as_path(), b.as_path()]).status.code(), Some(1));
    let big = t.write("big.json", &" ".repeat(MAX_BYTES + 1));
    assert_eq!(cmd(&[big.as_path()]).status.code(), Some(1));
}

#[test]
fn p4_errors_do_not_echo_input() {
    let t = Tmp::new("p4");
    let marker = "zz-marker-value";
    let bad = t.write(
        "marker-file.json",
        &rec("run-a", [1, 2, 3]).replace("\"label\":\"ok\"", &format!("\"label\":\"{marker}\"")),
    );
    let o = cmd(&[bad.as_path()]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(
        !err.contains(marker) && !err.contains("marker-file"),
        "{err}"
    );
    assert!(err.starts_with("error: arg 1:"));
}
