//! `record` / `json` の公開テスト。run ごとの記録を書く側の振る舞い（TASK-27 のポインタ）を確かめる。
//! 書き込みは一時ディレクトリだけで、作業ツリーと環境変数は変えない。

use measure_perf::PerfError;
use measure_perf::json;
use measure_perf::record::*;
use measure_perf::sample::{PhaseSamples, SampleCounts, SubmissionSample};
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
    assert!(!j.contains("null"));
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
        "\"body\":{\"sample_counts\":{\"cold\":1,\"warmup\":0,\"measured\":2},",
        "\"submission_count\":3,\"dispatch_count\":4,",
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
        assert_eq!(j, swap);
    }
}
