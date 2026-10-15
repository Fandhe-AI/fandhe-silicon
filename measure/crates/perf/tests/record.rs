//! `record` / `json` の公開テスト。run ごとの記録を書く側の振る舞い（TASK-27 のポインタ）を確かめる。
//! 書き込みは一時ディレクトリだけで、作業ツリーと環境変数は変えない。

use measure_perf::PerfError;
use measure_perf::json;
use measure_perf::record::*;
use measure_perf::sample::{PhaseSamples, SampleCounts, SubmissionSample};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn s(cpu: u64, gpu: Option<u64>) -> SubmissionSample {
    SubmissionSample {
        cpu_ns: cpu,
        gpu_ns: gpu,
    }
}

fn body_with(gpu_missing: bool) -> PerfSamplesBody {
    let samples = PhaseSamples {
        cold: vec![s(100, Some(90))],
        warmup: vec![],
        measured: vec![
            s(10, Some(5)),
            s(20, if gpu_missing { None } else { Some(6) }),
        ],
    };
    let counts = SampleCounts {
        cold: 1,
        warmup: 0,
        measured: 2,
    };
    PerfSamplesBody::new(counts, samples, 3, 4).unwrap()
}

fn envelope(label: Label, machine: Machine, combo: MachineCombination) -> Envelope {
    Envelope {
        run_id: Id::parse("run-1").unwrap(),
        task_ids: vec![RefId::parse("TASK-27").unwrap()],
        behavior_ids: vec![],
        machine_combination: combo,
        machine,
        label,
        started_utc: UtcTime::parse("2026-01-02T03:04:05Z").unwrap(),
        ended_utc: UtcTime::parse("2026-01-02T03:04:06Z").unwrap(),
        session_ref: SessionRef::NoSession,
        evidence: evidence(),
        environment: environment(combo.backend()),
        wgpu_baseline: None,
    }
}

fn name(s: &str) -> Name {
    Name::parse(s).unwrap()
}

fn ver(s: &str) -> VersionText {
    VersionText::parse(s).unwrap()
}

fn zeros(n: usize) -> String {
    "0".repeat(n)
}

fn evidence() -> Evidence {
    Evidence {
        adapter_backend: Some(AdapterBackend::Metal),
        adapter_name: Some(DeviceName::parse("Example GPU (0)").unwrap()),
        kernel_sha256: Some(BTreeMap::from([
            (name("k-b"), Sha256::parse(&zeros(64)).unwrap()),
            (name("k-a"), Sha256::parse(&"1".repeat(64)).unwrap()),
        ])),
        artifact_sha256: None,
        input_sha256: Some(BTreeMap::new()),
        output_sha256: OutputHash::NoOutput,
    }
}

fn snapshot(utc: &str) -> EnvSnapshot {
    EnvSnapshot {
        utc: Some(UtcTime::parse(utc).unwrap()),
        power_source: Some(PowerSource::Ac),
        low_power_mode: Some(false),
        thermal_warning: None,
        loadavg: Some(LoadAvg::parse("0.5 1 12.345").unwrap()),
    }
}

fn backend_env(b: AdapterBackend) -> BackendEnv {
    match b {
        AdapterBackend::Metal => BackendEnv::Metal {
            xcode_version: Some(ver("0.0")),
            metal_toolchain: None,
        },
        AdapterBackend::Vulkan => BackendEnv::Vulkan {
            driver_version: Some(ver("0.0.1")),
            sdk_versions: Some(BTreeMap::from([(name("vulkan-sdk"), ver("0.0.2"))])),
        },
        AdapterBackend::Cuda => BackendEnv::Cuda {
            driver_version: None,
            sdk_versions: None,
        },
    }
}

fn environment(b: AdapterBackend) -> Environment {
    Environment {
        gpu: Some(DeviceName::parse("Example GPU").unwrap()),
        cpu: None,
        os_name: Some(OsName::Linux),
        os_version: Some(ver("0.0.0")),
        rustc_version: None,
        repo_commit: Some(CommitSha::parse(&zeros(40)).unwrap()),
        cargo_lock_sha256: None,
        build_profile: Some(BuildProfile::Release),
        feature_set: Some(vec![OptionToken::parse("backend-metal").unwrap()]),
        target_features: None,
        shader_compile_options: Some(vec![]),
        library_versions: None,
        validation_layers: Some(ValidationLayers::Disabled),
        backend_env: backend_env(b),
        before: snapshot("2026-01-02T03:04:05Z"),
        after: EnvSnapshot::default(),
    }
}

fn record(label: Label, gpu_missing: bool) -> RunRecord {
    RunRecord::new(
        envelope(label, Machine::MacMini, MachineCombination::MetalMacMini),
        Body::PerfSamples(body_with(gpu_missing)),
    )
    .unwrap()
}

#[test]
fn t1_time_validation() {
    assert!(UtcTime::parse("2024-02-29T23:59:59Z").is_ok());
    for bad in [
        "2026-01-02T03:04:05",
        "2026-01-02T03:04:05.5Z",
        "2026-01-02 03:04:05Z",
        "2026-02-30T00:00:00Z",
        "2023-02-29T00:00:00Z",
        "2026-13-01T00:00:00Z",
        "2026-01-02T25:00:00Z",
        "2026-01-02T00:60:00Z",
        "2026-01-02T00:00:60Z",
        "",
        "２０２６-01-02T03:04:05Z",
    ] {
        assert_eq!(UtcTime::parse(bad), Err(RecordError::InvalidTime), "{bad}");
    }
    assert_eq!(
        UtcTime::from_unix_secs(0).unwrap().as_str(),
        "1970-01-01T00:00:00Z"
    );
    assert_eq!(
        UtcTime::from_unix_secs(1_700_000_000).unwrap().as_str(),
        "2023-11-14T22:13:20Z"
    );
    assert!(UtcTime::from_unix_secs(u64::MAX).is_err());
}

#[test]
fn t2_id_validation() {
    assert!(Id::parse("a").is_ok());
    assert!(Id::parse("run-1-b2").is_ok());
    assert!(Id::parse(&"a".repeat(64)).is_ok());
    for bad in ["", "A", "-a", "a-", "a--b", "a b", "あ", "a_b"] {
        assert_eq!(Id::parse(bad), Err(RecordError::InvalidId), "{bad}");
    }
    assert!(Id::parse(&"a".repeat(65)).is_err());
    assert!(RefId::parse("TASK-27").is_ok());
    assert!(RefId::parse("BUILD-54").is_ok());
    for bad in [
        "", "TASK", "task-27", "TASK-", "TASK 27", "TASK-2\n", "TASK-あ",
    ] {
        assert!(RefId::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn t3_reserved_session_word_never_collides() {
    assert_eq!(Id::parse(NO_SESSION), Err(RecordError::InvalidId));
    assert_eq!(SessionRef::NoSession.as_str(), NO_SESSION);
    let id = Id::parse("session-1").unwrap();
    assert_eq!(SessionRef::Session(id).as_str(), "session-1");
    assert!(
        record(Label::Ok, false)
            .to_json()
            .unwrap()
            .contains("\"session_ref\":\"_none\"")
    );
}

#[test]
fn t4_time_order() {
    let mut e = envelope(
        Label::Ok,
        Machine::MacMini,
        MachineCombination::MetalMacMini,
    );
    std::mem::swap(&mut e.started_utc, &mut e.ended_utc);
    let body = Body::PerfSamples(body_with(false));
    assert_eq!(
        RunRecord::new(e.clone(), body.clone()).unwrap_err(),
        RecordError::TimeOrder
    );
    e.ended_utc = e.started_utc.clone();
    assert!(RunRecord::new(e, body).is_ok());
}

fn core_src(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/core/src/test_frame")
        .join(name);
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn t5_spellings_match_core() {
    let comb = core_src("combination.rs");
    let label = core_src("label.rs");
    let decision = core_src("decision.rs");
    for m in Machine::ALL {
        assert!(
            comb.contains(&format!("=> \"{}\"", m.name())),
            "{}",
            m.name()
        );
    }
    for c in MachineCombination::ALL {
        assert!(
            comb.contains(&format!("=> \"{}\"", c.name())),
            "{}",
            c.name()
        );
    }
    for r in NotRunReason::ALL {
        assert!(
            decision.contains(&format!("=> \"{}\"", r.name())),
            "{}",
            r.name()
        );
    }
    for r in IndeterminateReason::ALL {
        assert!(
            label.contains(&format!("=> \"{}\"", r.name())),
            "{}",
            r.name()
        );
    }
    for k in ["ok", "ng", "pending", "not-run", "indeterminate"] {
        assert!(label.contains(&format!("=> \"{k}\"")), "{k}");
    }
    assert!(label.contains(&format!("PENDING_REASON: &str = \"{PENDING_REASON}\"")));
}

#[test]
fn t6_label_reason_only_when_present() {
    let ok = record(Label::Ok, false).to_json().unwrap();
    assert!(!ok.contains("label_reason"));
    let ng = record(Label::Ng, false).to_json().unwrap();
    assert!(!ng.contains("label_reason"));
    let p = record(Label::Pending, false).to_json().unwrap();
    assert!(p.contains("\"label\":\"pending\",\"label_reason\":\"threshold-unapproved\""));
    let n = record(Label::NotRun(NotRunReason::PermissionMissing), false)
        .to_json()
        .unwrap();
    assert!(n.contains("\"label\":\"not-run\",\"label_reason\":\"permission-missing\""));
    let i = record(
        Label::Indeterminate(IndeterminateReason::DeviceMismatch),
        false,
    )
    .to_json()
    .unwrap();
    assert!(i.contains("\"label_reason\":\"device-mismatch\""));
}

#[test]
fn t7_gpu_unavailable_is_null() {
    let j = record(Label::Ok, true).to_json().unwrap();
    assert!(j.contains("\"gpu_ns\":{\"cold\":[90],\"warmup\":[],\"measured\":[5,null]}"));
    assert!(j.contains("\"gpu_ns\":null}}"));
    assert!(j.contains("\"cpu_ns\":{\"median\":15,"));
    let j = record(Label::Ok, false).to_json().unwrap();
    let body = &j[j.find("\"body\":").unwrap()..];
    assert!(body.contains("\"processed_count\":null"));
    assert!(
        !body
            .replace("\"processed_count\":null", "")
            .contains("null")
    );
}

#[test]
fn t8_count_mismatch_and_empty() {
    let mk = |c: usize, w: usize, m: usize, n: usize| {
        let samples = PhaseSamples {
            cold: vec![s(1, None); c],
            warmup: vec![s(1, None); w],
            measured: vec![s(1, None); n],
        };
        PerfSamplesBody::new(
            SampleCounts {
                cold: 0,
                warmup: 0,
                measured: m,
            },
            samples,
            0,
            0,
        )
    };
    assert_eq!(mk(0, 0, 2, 3).unwrap_err(), RecordError::CountMismatch);
    assert_eq!(mk(1, 0, 1, 1).unwrap_err(), RecordError::CountMismatch);
    assert_eq!(mk(0, 1, 1, 1).unwrap_err(), RecordError::CountMismatch);
    assert_eq!(mk(0, 0, 0, 0).unwrap_err(), RecordError::Empty);
    assert!(mk(0, 0, 1, 1).is_ok());
}

#[test]
fn t8b_too_large_value_is_error() {
    let samples = PhaseSamples {
        cold: vec![],
        warmup: vec![],
        measured: vec![s(u64::MAX, None)],
    };
    let counts = SampleCounts {
        cold: 0,
        warmup: 0,
        measured: 1,
    };
    assert_eq!(
        PerfSamplesBody::new(counts, samples, 0, 0).unwrap_err(),
        RecordError::Perf(PerfError::ValueTooLarge)
    );
}

#[test]
fn t9_exact_output_is_deterministic() {
    let expected = concat!(
        "{\"schema_version\":\"1\",\"run_id\":\"run-1\",\"task_ids\":[\"TASK-27\"],\"behavior_ids\":[],",
        "\"machine_combination\":\"metal-mac-mini\",\"machine\":\"mac-mini\",\"label\":\"ok\",",
        "\"body_kind\":\"perf-samples\",\"started_utc\":\"2026-01-02T03:04:05Z\",",
        "\"ended_utc\":\"2026-01-02T03:04:06Z\",\"session_ref\":\"_none\",",
        "\"evidence\":{\"adapter_backend\":\"metal\",\"adapter_name\":\"Example GPU (0)\",",
        "\"kernel_sha256\":{\"k-a\":\"1111111111111111111111111111111111111111111111111111111111111111\",",
        "\"k-b\":\"0000000000000000000000000000000000000000000000000000000000000000\"},",
        "\"artifact_sha256\":null,\"input_sha256\":{},\"output_sha256\":\"_no-output\"},",
        "\"environment\":{\"gpu\":\"Example GPU\",\"cpu\":\"_unavailable\",\"os_name\":\"linux\",",
        "\"os_version\":\"0.0.0\",\"rustc_version\":\"_unavailable\",",
        "\"repo_commit\":\"0000000000000000000000000000000000000000\",",
        "\"cargo_lock_sha256\":\"_unavailable\",\"build_profile\":\"release\",",
        "\"feature_set\":[\"backend-metal\"],\"target_features\":null,\"shader_compile_options\":[],",
        "\"library_versions\":null,\"validation_layers\":\"disabled\",",
        "\"xcode_version\":\"0.0\",\"metal_toolchain\":\"_unavailable\",",
        "\"before\":{\"utc\":\"2026-01-02T03:04:05Z\",\"power_source\":\"ac\",\"low_power_mode\":false,",
        "\"thermal_warning\":null,\"loadavg\":[0.500,1.000,12.345]},",
        "\"after\":{\"utc\":\"_unavailable\",\"power_source\":\"_unavailable\",\"low_power_mode\":null,",
        "\"thermal_warning\":null,\"loadavg\":null}},",
        "\"body\":{\"sample_counts\":{\"cold\":1,\"warmup\":0,\"measured\":2},",
        "\"submission_count\":3,\"dispatch_count\":4,\"processed_count\":null,\"instructions_found\":{},",
        "\"cpu_ns\":{\"cold\":[100],\"warmup\":[],\"measured\":[10,20]},",
        "\"gpu_ns\":{\"cold\":[90],\"warmup\":[],\"measured\":[5,6]},",
        "\"stats\":{\"cpu_ns\":{\"median\":15,\"q1\":10,\"q3\":20,\"p95\":20,\"min\":10,\"max\":20},",
        "\"gpu_ns\":{\"median\":5.5,\"q1\":5,\"q3\":6,\"p95\":6,\"min\":5,\"max\":6}}}}"
    );
    let a = record(Label::Ok, false).to_json().unwrap();
    assert_eq!(a, expected);
    assert_eq!(a, record(Label::Ok, false).to_json().unwrap());
}

#[test]
fn t10_escape_is_ascii_only() {
    let mut out = String::new();
    json::push_string(&mut out, "a\"b\\c\n\t\u{1}é😀");
    assert_eq!(out, "\"a\\\"b\\\\c\\n\\t\\u0001\\u00e9\\ud83d\\ude00\"");
    assert!(out.is_ascii());
    assert!(record(Label::Ok, false).to_json().unwrap().is_ascii());
}

#[test]
fn t11_median_number_format() {
    let mut o = String::new();
    assert!(json::push_median(&mut o, 2.0));
    assert!(json::push_median(&mut o, 1.5));
    assert_eq!(o, "21.5");
    for bad in [f64::NAN, f64::INFINITY, -1.0, 0.25, 1e300] {
        let mut o = String::new();
        assert!(!json::push_median(&mut o, bad));
        assert!(o.is_empty());
    }
    let j = record(Label::Ok, false).to_json().unwrap();
    assert!(!j.contains("NaN") && !j.contains("inf"));
}

#[test]
fn t12_body_kind_and_fields() {
    let b = Body::PerfSamples(body_with(false));
    assert_eq!(b.kind(), "perf-samples");
    let j = record(Label::Ok, false).to_json().unwrap();
    for k in [
        "sample_counts",
        "submission_count",
        "dispatch_count",
        "cpu_ns",
        "gpu_ns",
        "stats",
    ] {
        assert!(j.contains(&format!("\"{k}\":")), "{k}");
    }
    assert!(j.contains("\"body_kind\":\"perf-samples\""));
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("measure-perf-record-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn t13_write_new_does_not_overwrite() {
    let dir = temp_dir("t13");
    let path = dir.join("run.json");
    let r = record(Label::Ok, false);
    write_new(&path, &r).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written, format!("{}\n", r.to_json().unwrap()));
    let err = write_new(&path, &record(Label::Ng, false)).unwrap_err();
    assert_eq!(err, RecordError::Io(std::io::ErrorKind::AlreadyExists));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), written);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn t14_node_machines_use_same_shape() {
    for (m, c) in [
        (Machine::NodeA, MachineCombination::VulkanNodeA),
        (Machine::NodeB, MachineCombination::CudaNodeB),
    ] {
        let mac = record(Label::Ok, false).to_json().unwrap();
        let r = RunRecord::new(
            envelope(Label::Ok, m, c),
            Body::PerfSamples(body_with(false)),
        )
        .unwrap();
        let j = r.to_json().unwrap();
        let swap = mac
            .replace("metal-mac-mini", c.name())
            .replace("\"mac-mini\"", &format!("\"{}\"", m.name()));
        // 背後実装ごとの欄（validation_layers の後から before の前まで）は形が違うので除いて比べる。
        let strip = |s: &str| {
            let a = s.find("\"validation_layers\"").unwrap();
            let b = s.find("\"before\"").unwrap();
            format!("{}{}", &s[..a], &s[b..])
        };
        assert_eq!(strip(&j), strip(&swap));
    }
}

fn bad_all(f: impl Fn(&str) -> bool, bad: &[&str]) {
    for b in bad {
        assert!(!f(b), "{b:?}");
    }
}

#[test]
fn t15_value_types() {
    assert!(Sha256::parse(&zeros(64)).is_ok());
    let (z63, z65, upper, g) = (zeros(63), zeros(65), "A".repeat(64), "g".repeat(64));
    bad_all(|s| Sha256::parse(s).is_ok(), &[&z63, &z65, &upper, &g, ""]);
    assert!(CommitSha::parse(&zeros(40)).is_ok());
    let (z64, f40) = (zeros(64), "F".repeat(40));
    bad_all(|s| CommitSha::parse(s).is_ok(), &[&z64, &f40]);
    assert!(SemVer::parse("0.0.0").is_ok());
    assert!(SemVer::parse("10.2.30").is_ok());
    bad_all(
        |s| SemVer::parse(s).is_ok(),
        &[
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.x",
            "",
            "1..3",
            "1.2.3\n",
            "1234567890.0.0",
        ],
    );
    for ok in ["0", "1.2", "1.2.3-rc.1+b5", "14.0.0"] {
        assert!(VersionText::parse(ok).is_ok(), "{ok}");
    }
    let long = "1".repeat(65);
    bad_all(
        |s| VersionText::parse(s).is_ok(),
        &["", "-1", "1 2", "1/2", "1\n", "v\u{3042}", &long],
    );
    for ok in ["a", "k-1_x.y", "A0"] {
        assert!(Name::parse(ok).is_ok(), "{ok}");
    }
    let a65 = "a".repeat(65);
    bad_all(
        |s| Name::parse(s).is_ok(),
        &["", "_a", "a/b", "a b", "a\"", "-a", &a65],
    );
    for ok in ["Apple M0", "GPU (rev 1), x-y+z"] {
        assert!(DeviceName::parse(ok).is_ok(), "{ok}");
    }
    let a129 = "a".repeat(129);
    bad_all(
        |s| DeviceName::parse(s).is_ok(),
        &[
            "", " a", "a ", "a\nb", "a\u{0}", "\u{3042}", "_a", "a/b", "a\"b", "a\\b", &a129,
        ],
    );
    assert!(OptionToken::parse("-O2").is_ok());
    assert!(OptionToken::parse("target-cpu=native").is_ok());
    bad_all(
        |s| OptionToken::parse(s).is_ok(),
        &[
            "", "a b", "/usr/bin", "~x", "a\"", "a\\b", "a'b", "a\nb", "_x", &a129,
        ],
    );
    assert!(GitRepository::parse("https://example.invalid/x.git").is_ok());
    let userinfo = ["https://", "user", "@example.invalid/x.git"].concat();
    bad_all(
        |s| GitRepository::parse(s).is_ok(),
        &[
            "http://example.invalid/x",
            "https://",
            &userinfo,
            "https://example.invalid/x?a=b",
            "https://example.invalid/x#f",
            "https://example.invalid/ x",
            "git@example.invalid:x.git",
        ],
    );
    assert_eq!(
        LoadAvg::parse("0 1.5 02.250").unwrap().as_milli(),
        [0, 1500, 2250]
    );
    bad_all(
        |s| LoadAvg::parse(s).is_ok(),
        &[
            "",
            "1 2",
            "1 2 3 4",
            "1 2 -3",
            "1 2 NaN",
            "1 2 inf",
            "1e3 2 3",
            "1. 2 3",
            "1.2345 2 3",
            "1  2 3",
            "1234567 2 3",
        ],
    );
}

#[test]
fn t16_reserved_words_are_never_valid_values() {
    assert_ne!(UNAVAILABLE, NO_OUTPUT);
    assert_ne!(UNAVAILABLE, NO_SESSION);
    assert_ne!(NO_OUTPUT, NO_SESSION);
    for w in [UNAVAILABLE, NO_OUTPUT] {
        assert!(Sha256::parse(w).is_err());
        assert!(CommitSha::parse(w).is_err());
        assert!(SemVer::parse(w).is_err());
        assert!(VersionText::parse(w).is_err());
        assert!(Name::parse(w).is_err());
        assert!(DeviceName::parse(w).is_err());
        assert!(OptionToken::parse(w).is_err());
        assert!(GitRepository::parse(w).is_err());
        assert!(LoadAvg::parse(w).is_err());
        assert!(Id::parse(w).is_err());
        for e in AdapterBackend::ALL {
            assert_ne!(e.name(), w);
        }
        for e in OsName::ALL {
            assert_ne!(e.name(), w);
        }
        for e in PowerSource::ALL {
            assert_ne!(e.name(), w);
        }
    }
}

fn json_of(e: Envelope) -> String {
    RunRecord::new(e, Body::PerfSamples(body_with(false)))
        .unwrap()
        .to_json()
        .unwrap()
}

fn metal_envelope() -> Envelope {
    envelope(
        Label::Ok,
        Machine::MacMini,
        MachineCombination::MetalMacMini,
    )
}

#[test]
fn t17_unavailable_is_spelled_by_field_type() {
    let mut e = metal_envelope();
    e.evidence = Evidence {
        adapter_backend: None,
        adapter_name: None,
        kernel_sha256: None,
        artifact_sha256: None,
        input_sha256: None,
        output_sha256: OutputHash::Unavailable,
    };
    e.environment.before = EnvSnapshot::default();
    let j = json_of(e);
    assert!(j.contains(concat!(
        "\"evidence\":{\"adapter_backend\":\"_unavailable\",\"adapter_name\":\"_unavailable\",",
        "\"kernel_sha256\":null,\"artifact_sha256\":null,\"input_sha256\":null,",
        "\"output_sha256\":\"_unavailable\"}"
    )));
    assert!(j.contains(concat!(
        "\"before\":{\"utc\":\"_unavailable\",\"power_source\":\"_unavailable\",",
        "\"low_power_mode\":null,\"thermal_warning\":null,\"loadavg\":null}"
    )));
}

#[test]
fn t18_backend_specific_env_fields() {
    let metal = json_of(metal_envelope());
    assert!(metal.contains("\"xcode_version\"") && metal.contains("\"metal_toolchain\""));
    assert!(!metal.contains("driver_version") && !metal.contains("sdk_versions"));
    for c in [
        MachineCombination::VulkanNodeB,
        MachineCombination::CudaNodeA,
    ] {
        let j = json_of(envelope(Label::Ok, Machine::NodeB, c));
        assert!(
            j.contains("\"driver_version\"") && j.contains("\"sdk_versions\""),
            "{}",
            c.name()
        );
        assert!(!j.contains("xcode_version") && !j.contains("metal_toolchain"));
    }
    let mut e = metal_envelope();
    e.environment.backend_env = backend_env(AdapterBackend::Vulkan);
    assert_eq!(
        RunRecord::new(e, Body::PerfSamples(body_with(false))).unwrap_err(),
        RecordError::BackendMismatch
    );
    for c in MachineCombination::ALL {
        let e = envelope(Label::Ok, Machine::MacMini, c);
        assert!(RunRecord::new(e, Body::PerfSamples(body_with(false))).is_ok());
    }
}

#[test]
fn t19_wgpu_baseline_conditional_fields() {
    let base = |source| WgpuBaseline {
        version: Some(SemVer::parse("0.0.0").unwrap()),
        source,
        lockfile_sha256: None,
        basis: Basis::parse("BASIS-0").unwrap(),
    };
    let mk = |w: Option<WgpuBaseline>| {
        let mut e = metal_envelope();
        e.wgpu_baseline = w;
        json_of(e)
    };
    assert!(!mk(None).contains("wgpu_baseline"));
    let c = mk(Some(base(BaselineSourceSpec::CratesIo)));
    assert!(c.contains(concat!(
        "\"wgpu_baseline\":{\"version\":\"0.0.0\",\"source\":\"crates-io\",",
        "\"lockfile_sha256\":\"_unavailable\",\"basis\":\"BASIS-0\"}"
    )));
    assert!(!c.contains("git_"));
    let g = mk(Some(base(BaselineSourceSpec::Git {
        repository: Some(GitRepository::parse("https://example.invalid/w.git").unwrap()),
        tag: Some(Name::parse("example-tag").unwrap()),
        commit: None,
    })));
    assert!(g.contains(concat!(
        "\"source\":\"git\",\"git_repository\":\"https://example.invalid/w.git\",",
        "\"git_tag\":\"example-tag\",\"git_commit\":\"_unavailable\",\"lockfile_sha256\""
    )));
}

#[test]
fn t20_adapter_backend_is_not_aligned_and_spellings_match() {
    let mut e = envelope(Label::Ok, Machine::NodeB, MachineCombination::CudaNodeB);
    e.evidence.adapter_backend = Some(AdapterBackend::Metal);
    let j = json_of(e);
    assert!(j.contains("\"machine_combination\":\"cuda-node-b\""));
    assert!(j.contains("\"adapter_backend\":\"metal\""));
    let comb = core_src("combination.rs");
    for c in MachineCombination::ALL {
        assert!(c.name().starts_with(c.backend().name()), "{}", c.name());
    }
    for b in ["Metal", "Vulkan", "Cuda"] {
        assert!(comb.contains(&format!("Backend::{b}")), "{b}");
    }
}

#[test]
fn t21_maps_are_ordered_deterministically() {
    let mut e1 = metal_envelope();
    e1.environment.library_versions = Some(BTreeMap::from([
        (name("b"), ver("1")),
        (name("a"), ver("2")),
    ]));
    let mut e2 = e1.clone();
    let mut m = BTreeMap::new();
    m.insert(name("a"), ver("2"));
    m.insert(name("b"), ver("1"));
    e2.environment.library_versions = Some(m);
    let (j1, j2) = (json_of(e1), json_of(e2));
    assert_eq!(j1, j2);
    assert!(j1.contains("\"library_versions\":{\"a\":\"2\",\"b\":\"1\"}"));
}

#[test]
fn t22_numbers_and_power_spellings() {
    let mut e = metal_envelope();
    e.environment.after = snapshot("2026-01-02T03:04:06Z");
    e.environment.after.loadavg = Some(LoadAvg::parse("0 0 123456.999").unwrap());
    let j = json_of(e);
    assert!(j.contains("\"loadavg\":[0.000,0.000,123456.999]"));
    assert!(!j.contains("NaN") && !j.contains("e+") && !j.contains("E+"));
    let probe = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/metal-session.sh"),
    )
    .unwrap();
    for p in PowerSource::ALL {
        assert!(probe.contains(p.name()), "{}", p.name());
    }
    assert_eq!(
        PowerSource::ALL.map(PowerSource::name),
        ["ac", "battery", "ups"]
    );
}

#[test]
fn t23_body_processed_count_and_instructions() {
    let body = body_with(false)
        .with_processed_count(Some(7))
        .with_instructions_found(BTreeMap::from([
            (name("fma"), true),
            (name("atomic"), false),
        ]));
    let j = RunRecord::new(metal_envelope(), Body::PerfSamples(body))
        .unwrap()
        .to_json()
        .unwrap();
    assert!(
        j.contains("\"processed_count\":7,\"instructions_found\":{\"atomic\":false,\"fma\":true},")
    );
    assert_eq!(
        body_with(false).with_processed_count(None),
        body_with(false)
    );
}
