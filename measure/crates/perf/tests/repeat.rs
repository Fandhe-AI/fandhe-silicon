//! 同じ測定を繰り返したときの記録の形と、読む側の検査（`scripts/perf-record-check.sh`）の公開テスト
//! （TASK-27・TASK-2947 のポインタ。回の run_ids との突き合わせを含む）。
//!
//! 合成の標本と CPU だけのダミー測定を繰り返し、出た記録が決めた形で残ることを確かめる。
//! ダミー測定の封筒は既存の試験用の機材と組み合わせの綴りだけを使い、`session_ref` は回に属さない
//! 予約語にする（判定の根拠にならない）。新しい機材・組み合わせの値は足さない。
//! 書き込みは一時ディレクトリだけで、終わりに消す。作業ツリーと環境変数は変えない。
//! `bash` と `python3` が無ければテストを失敗させる（skip しない）。

use measure_perf::across::across_runs;
use measure_perf::record::*;
use measure_perf::sample::{self, PhaseSamples, SampleCounts, SubmissionSample};
use measure_perf::stats::run_stats;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::process::Command;

fn name(s: &str) -> Name {
    Name::parse(s).unwrap()
}

fn ver(s: &str) -> VersionText {
    VersionText::parse(s).unwrap()
}

fn hex(c: char, n: usize) -> String {
    c.to_string().repeat(n)
}

fn snapshot(utc: &str) -> EnvSnapshot {
    EnvSnapshot {
        utc: Some(UtcTime::parse(utc).unwrap()),
        power_source: Some(PowerSource::Ac),
        low_power_mode: Some(false),
        thermal_warning: Some(false),
        loadavg: Some(LoadAvg::parse("0.5 1 12.345").unwrap()),
    }
}

fn backend_env(b: AdapterBackend) -> BackendEnv {
    match b {
        AdapterBackend::Metal => BackendEnv::Metal {
            xcode_version: Some(ver("0.0")),
            metal_toolchain: Some(ver("0.1")),
        },
        AdapterBackend::Vulkan => BackendEnv::Vulkan {
            driver_version: Some(ver("0.0.1")),
            sdk_versions: Some(BTreeMap::from([(name("vulkan-sdk"), ver("0.0.2"))])),
        },
        AdapterBackend::Cuda => BackendEnv::Cuda {
            driver_version: Some(ver("0.0.3")),
            sdk_versions: Some(BTreeMap::from([(name("cuda-sdk"), ver("0.0.4"))])),
        },
    }
}

/// 全ての欄が取れている環境（値はすべてダミー）。
fn full_environment(b: AdapterBackend, start: &str, end: &str) -> Environment {
    Environment {
        gpu: Some(DeviceName::parse("Example GPU").unwrap()),
        cpu: Some(DeviceName::parse("Example CPU").unwrap()),
        os_name: Some(OsName::Linux),
        os_version: Some(ver("0.0.0")),
        rustc_version: Some(ver("0.0.0")),
        repo_commit: Some(CommitSha::parse(&hex('0', 40)).unwrap()),
        cargo_lock_sha256: Some(Sha256::parse(&hex('2', 64)).unwrap()),
        build_profile: Some(BuildProfile::Release),
        feature_set: Some(vec![OptionToken::parse("example-feature").unwrap()]),
        target_features: Some(vec![]),
        shader_compile_options: Some(vec![OptionToken::parse("-example").unwrap()]),
        library_versions: Some(BTreeMap::from([(name("example-lib"), ver("0.0.5"))])),
        validation_layers: Some(ValidationLayers::Disabled),
        backend_env: backend_env(b),
        before: snapshot(start),
        after: snapshot(end),
    }
}

fn full_evidence(b: AdapterBackend) -> Evidence {
    let table = |c| BTreeMap::from([(name("example-item"), Sha256::parse(&hex(c, 64)).unwrap())]);
    Evidence {
        adapter_backend: Some(b),
        adapter_name: Some(DeviceName::parse("Example GPU (0)").unwrap()),
        kernel_sha256: Some(table('1')),
        artifact_sha256: Some(table('3')),
        input_sha256: Some(table('4')),
        output_sha256: OutputHash::Sha256(Sha256::parse(&hex('5', 64)).unwrap()),
    }
}

fn envelope(
    run_id: &str,
    label: Label,
    combo: MachineCombination,
    machine: Machine,
    session: SessionRef,
) -> Envelope {
    let (start, end) = ("2000-01-01T00:02:00Z", "2000-01-01T00:02:30Z");
    Envelope {
        run_id: Id::parse(run_id).unwrap(),
        task_ids: vec![RefId::parse("TASK-27").unwrap()],
        behavior_ids: vec![],
        machine_combination: combo,
        machine,
        label,
        started_utc: UtcTime::parse(start).unwrap(),
        ended_utc: UtcTime::parse(end).unwrap(),
        session_ref: session,
        evidence: full_evidence(combo.backend()),
        environment: full_environment(combo.backend(), start, end),
        wgpu_baseline: None,
    }
}

fn sub(cpu: u64, gpu: Option<u64>) -> SubmissionSample {
    SubmissionSample {
        cpu_ns: cpu,
        gpu_ns: gpu,
    }
}

/// 合成の標本（値は固定）。`gpu_missing` なら計測の区間に取得不能を 1 つ入れる。
fn synthetic_body(gpu_missing: bool) -> PerfSamplesBody {
    let samples = PhaseSamples {
        cold: vec![sub(100, Some(90))],
        warmup: vec![sub(50, Some(40))],
        measured: vec![
            sub(10, Some(5)),
            sub(20, if gpu_missing { None } else { Some(6) }),
            sub(30, Some(7)),
        ],
    };
    let counts = SampleCounts {
        cold: 1,
        warmup: 1,
        measured: 3,
    };
    PerfSamplesBody::new(counts, samples, 5, 6).unwrap()
}

fn example_record() -> RunRecord {
    RunRecord::new(
        envelope(
            "example-run-a",
            Label::Ok,
            MachineCombination::MetalMacMini,
            Machine::MacMini,
            SessionRef::Session(Id::parse("example-session").unwrap()),
        ),
        Body::PerfSamples(synthetic_body(false)),
    )
    .unwrap()
}

/// 試験用（回に属さない）の封筒。
fn test_envelope(run_id: &str, label: Label, combo: MachineCombination, m: Machine) -> Envelope {
    envelope(run_id, label, combo, m, SessionRef::NoSession)
}

fn make_record(e: Envelope, body: PerfSamplesBody) -> RunRecord {
    RunRecord::new(e, Body::PerfSamples(body)).unwrap()
}

/// CPU だけのダミー測定。GPU 時間は取れない（`None`）。
fn dummy_measure(counts: SampleCounts) -> PhaseSamples {
    sample::collect(counts, || {
        let sum: u64 = (0..1_000u64).map(std::hint::black_box).sum();
        std::hint::black_box(sum);
        Ok::<Option<u64>, Infallible>(None)
    })
    .unwrap()
}

fn work_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("measure-perf-repeat-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn repo_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel)
}

/// 検査スクリプトを `bash` 経由で実行して終了コードを返す（シェルを介さず引数配列で渡す）。
fn run_check(args: &[&Path]) -> i32 {
    let script = repo_path("scripts/perf-record-check.sh");
    let out = Command::new("bash")
        .arg(script)
        .args(args)
        .output()
        .expect("bash and python3 are required");
    out.status.code().expect("terminated by signal")
}

fn check_json(dir: &Path, file: &str, json: &str) -> i32 {
    let p = dir.join(file);
    std::fs::write(&p, format!("{json}\n")).unwrap();
    run_check(&[&p])
}

#[test]
fn r1_synthetic_samples_repeat_to_identical_bytes() {
    let a = example_record().to_json().unwrap();
    let b = example_record().to_json().unwrap();
    assert_eq!(a, b);
}

#[test]
fn r2_cpu_only_dummy_measurement_repeats_with_same_shape() {
    let dir = work_dir("r2");
    let counts = SampleCounts {
        cold: 2,
        warmup: 1,
        measured: 5,
    };
    let mut medians = Vec::new();
    let mut texts = Vec::new();
    for run in ["dummy-run-a", "dummy-run-b"] {
        let samples = dummy_measure(counts);
        medians.push(run_stats(&samples.measured_cpu_ns()).unwrap().median);
        let body = PerfSamplesBody::new(counts, samples, 8, 8).unwrap();
        let e = test_envelope(
            run,
            Label::Ok,
            MachineCombination::MetalMacMini,
            Machine::MacMini,
        );
        let path = dir.join(format!("{run}.json"));
        write_new(&path, &make_record(e, body)).unwrap();
        // 再度の書き込みは上書きせず失敗する。
        let again = make_record(
            test_envelope(
                run,
                Label::Ok,
                MachineCombination::MetalMacMini,
                Machine::MacMini,
            ),
            PerfSamplesBody::new(counts, dummy_measure(counts), 8, 8).unwrap(),
        );
        assert_eq!(
            write_new(&path, &again).unwrap_err(),
            RecordError::Io(std::io::ErrorKind::AlreadyExists)
        );
        assert_eq!(run_check(&[&path]), 0, "{run}");
        texts.push(std::fs::read_to_string(&path).unwrap());
    }
    // 条件の欄は run の ID 以外が一致し、値の欄は件数どおりで GPU は取得不能。
    let head = |t: &str, id: &str| {
        let (h, _) = t.split_once(",\"body\":").unwrap();
        h.replace(id, "RUN")
    };
    assert_eq!(
        head(&texts[0], "dummy-run-a"),
        head(&texts[1], "dummy-run-b")
    );
    for t in &texts {
        let (_, body) = t.split_once(",\"body\":").unwrap();
        let (fixed, _) = body.split_once("\"cpu_ns\":").unwrap();
        assert_eq!(
            fixed,
            "{\"sample_counts\":{\"cold\":2,\"warmup\":1,\"measured\":5},\"submission_count\":8,\"dispatch_count\":8,\"processed_count\":null,\"instructions_found\":{},"
        );
        assert!(t.contains("\"gpu_ns\":{\"cold\":[null,null],\"warmup\":[null],\"measured\":[null,null,null,null,null]}"));
        assert!(t.trim_end().ends_with("\"gpu_ns\":null}}}"));
    }
    assert!(across_runs(&medians).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r3_checker_accepts_every_branch_of_real_output() {
    let dir = work_dir("r3");
    let mut cases: Vec<(String, RunRecord)> = Vec::new();
    for (i, (combo, m)) in [
        (MachineCombination::MetalMacMini, Machine::MacMini),
        (MachineCombination::VulkanNodeA, Machine::NodeA),
        (MachineCombination::CudaNodeB, Machine::NodeB),
    ]
    .into_iter()
    .enumerate()
    {
        cases.push((
            format!("backend{i}"),
            make_record(
                test_envelope("branch-run", Label::Ok, combo, m),
                synthetic_body(false),
            ),
        ));
    }
    let labels = [
        Label::Ok,
        Label::Ng,
        Label::Pending,
        Label::NotRun(NotRunReason::PermissionMissing),
        Label::Indeterminate(IndeterminateReason::DeviceMismatch),
    ];
    for (i, l) in labels.into_iter().enumerate() {
        cases.push((
            format!("label{i}"),
            make_record(
                test_envelope(
                    "branch-run",
                    l,
                    MachineCombination::MetalMacMini,
                    Machine::MacMini,
                ),
                synthetic_body(false),
            ),
        ));
    }
    cases.push((
        "gpu-missing".into(),
        make_record(
            test_envelope(
                "branch-run",
                Label::Ok,
                MachineCombination::MetalMacMini,
                Machine::MacMini,
            ),
            synthetic_body(true),
        ),
    ));
    cases.push((
        "body-extras".into(),
        make_record(
            test_envelope(
                "branch-run",
                Label::Ok,
                MachineCombination::MetalMacMini,
                Machine::MacMini,
            ),
            synthetic_body(false)
                .with_processed_count(Some(7))
                .with_instructions_found(BTreeMap::from([(name("example-insn"), true)])),
        ),
    ));
    for (i, src) in [
        BaselineSourceSpec::CratesIo,
        BaselineSourceSpec::Git {
            repository: Some(GitRepository::parse("https://example.invalid/w.git").unwrap()),
            tag: Some(name("example-tag")),
            commit: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = test_envelope(
            "branch-run",
            Label::Ok,
            MachineCombination::MetalMacMini,
            Machine::MacMini,
        );
        e.wgpu_baseline = Some(WgpuBaseline {
            version: Some(SemVer::parse("0.0.0").unwrap()),
            source: src,
            lockfile_sha256: None,
            basis: Basis::parse("BASIS-0").unwrap(),
        });
        cases.push((
            format!("baseline{i}"),
            make_record(e, synthetic_body(false)),
        ));
    }
    // 取得不能だらけの封筒も形としては合格する。
    let mut e = test_envelope(
        "branch-run",
        Label::Ok,
        MachineCombination::VulkanAmd,
        Machine::Amd,
    );
    e.evidence = Evidence {
        adapter_backend: None,
        adapter_name: None,
        kernel_sha256: None,
        artifact_sha256: None,
        input_sha256: None,
        output_sha256: OutputHash::Unavailable,
    };
    e.environment = Environment {
        gpu: None,
        cpu: None,
        os_name: None,
        os_version: None,
        rustc_version: None,
        repo_commit: None,
        cargo_lock_sha256: None,
        build_profile: None,
        feature_set: None,
        target_features: None,
        shader_compile_options: None,
        library_versions: None,
        validation_layers: None,
        backend_env: BackendEnv::Vulkan {
            driver_version: None,
            sdk_versions: None,
        },
        before: EnvSnapshot::default(),
        after: EnvSnapshot::default(),
    };
    cases.push(("unavailable".into(), make_record(e, synthetic_body(false))));

    for (tag, rec) in &cases {
        let json = rec.to_json().unwrap();
        assert_eq!(check_json(&dir, &format!("{tag}.json"), &json), 0, "{tag}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r4_checker_rejects_added_and_removed_fields() {
    let dir = work_dir("r4");
    let json = make_record(
        test_envelope(
            "mutate-run",
            Label::Pending,
            MachineCombination::MetalMacMini,
            Machine::MacMini,
        ),
        synthetic_body(false),
    )
    .to_json()
    .unwrap();
    assert_eq!(check_json(&dir, "base.json", &json), 0);
    let added = json.replacen('{', "{\"extra_field\":1,", 1);
    assert_eq!(check_json(&dir, "added.json", &added), 1);
    for (tag, from, to) in [
        ("no-dispatch", ",\"dispatch_count\":6", ""),
        (
            "no-reason",
            ",\"label_reason\":\"threshold-unapproved\"",
            "",
        ),
        (
            "bad-time-order",
            "\"ended_utc\":\"2000-01-01T00:02:30Z\"",
            "\"ended_utc\":\"2000-01-01T00:01:00Z\"",
        ),
        ("wrong-length", "\"cold\":[100]", "\"cold\":[100,100]"),
        (
            "bool-for-int",
            "\"submission_count\":5",
            "\"submission_count\":true",
        ),
    ] {
        assert!(json.contains(from), "{tag}");
        let m = json.replacen(from, to, 1);
        assert_eq!(check_json(&dir, &format!("{tag}.json"), &m), 1, "{tag}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r5_session_mode_uses_the_documented_examples() {
    let dir = work_dir("r5");
    let docs_session = repo_path("docs/session-record.example.json");
    let example = repo_path("docs/run-record.example.json");
    assert_eq!(run_check(&[&example]), 0);
    // 実在の検査は回の記録と同じディレクトリの runs/<ID>.json を見るので、記入例を並びの形に写す。
    let session = dir.join("session.json");
    std::fs::copy(&docs_session, &session).unwrap();
    std::fs::create_dir_all(dir.join("runs")).unwrap();
    let example_text = std::fs::read_to_string(&example).unwrap();
    std::fs::write(dir.join("runs/example-run-a.json"), &example_text).unwrap();
    std::fs::write(
        dir.join("runs/example-run-b.json"),
        example_text.replacen("example-run-a", "example-run-b", 1),
    )
    .unwrap();
    assert_eq!(run_check(&[Path::new("--session"), &session]), 0);
    assert_eq!(run_check(&[Path::new("--session"), &session, &example]), 0);
    // 並びの無い記入例を直接渡すと、runs/ が無いので判定に使えない。
    assert_eq!(run_check(&[Path::new("--session"), &docs_session]), 1);
    assert_eq!(
        run_check(&[Path::new("--session"), &docs_session, &example]),
        1
    );
    // 回に属さない run は判定に使えない（除外は成功扱いにしない）。
    let orphan = dir.join("orphan.json");
    let rec = make_record(
        test_envelope(
            "orphan-run",
            Label::Ok,
            MachineCombination::MetalMacMini,
            Machine::MacMini,
        ),
        synthetic_body(false),
    );
    write_new(&orphan, &rec).unwrap();
    assert_eq!(run_check(&[&orphan]), 0);
    assert_eq!(run_check(&[Path::new("--session"), &session, &orphan]), 1);
    // 誤用は 2。
    assert_eq!(run_check(&[]), 2);
    assert_eq!(run_check(&[&dir.join("missing.json")]), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r6_documented_example_is_the_exact_output() {
    let expected = format!("{}\n", example_record().to_json().unwrap());
    let actual = std::fs::read_to_string(repo_path("docs/run-record.example.json")).unwrap();
    assert_eq!(actual, expected);
}
