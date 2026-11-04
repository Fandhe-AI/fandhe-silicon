//! 経路の突き合わせの数え上げ（`EvidenceTally`）と、試験の枠への受け渡しの振る舞いの確認（spec TASK-922）。
//! 公開のパスだけを使い、値はすべてダミーの合成の列。環境は読まず、設定は `RunSetting::from_lookup` へ渡す。
//! 置き場と範囲の説明は docs/route-evidence.md。

use fandhe_silicon_core::declaration;
use fandhe_silicon_core::route_evidence::{
    ArtifactDigests, CallCheck, CallEvidence, EvidenceError, EvidenceTally, MARKER_CLEAR,
    MARKER_GPU_EMULATED, MARKER_NATIVE, ReportedRoute, RunCounts, check_call, fill_sentinel,
};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, IndeterminateReason, Label, MachineCombination, RunResult, RunSetting,
    Subject, Verdict, judge,
};
use std::ffi::OsString;

const WRITTEN: &[u8] = b"\x01\x02\x03\x04";

const ROUTES: [Option<ReportedRoute>; 4] = [
    Some(ReportedRoute::Native),
    Some(ReportedRoute::GpuEmulated),
    Some(ReportedRoute::Cpu),
    None,
];

/// 食い違わせる値の種類。
#[derive(Clone, Copy, Debug, PartialEq)]
enum Field {
    Marker,
    Counts,
    Output,
    Artifact,
}

const FIELDS: [Field; 4] = [Field::Marker, Field::Counts, Field::Output, Field::Artifact];

fn digests() -> ArtifactDigests {
    ArtifactDigests::from_artifacts(b"native-artifact", b"gpu-emulated-artifact").unwrap()
}

fn counts(native: u64, gpu_emulated: u64, cpu: u64) -> RunCounts {
    RunCounts {
        native,
        gpu_emulated,
        cpu,
        probe: 0,
    }
}

/// 経路ごとに一致する値。出力は呼び出しの前に番兵で埋めた列を、書き換わった形で渡す。
fn matching(route: Option<ReportedRoute>, output: &[u8]) -> CallEvidence<'_> {
    let d = digests();
    let (marker, after, artifact) = match route {
        Some(ReportedRoute::Native) => (MARKER_NATIVE, counts(1, 0, 0), Some(d.native())),
        Some(ReportedRoute::GpuEmulated) => {
            (MARKER_GPU_EMULATED, counts(0, 1, 0), Some(d.gpu_emulated()))
        }
        Some(ReportedRoute::Cpu) => (MARKER_CLEAR, counts(0, 0, 1), None),
        None => (MARKER_CLEAR, counts(0, 0, 0), None),
    };
    CallEvidence {
        marker,
        before: RunCounts::default(),
        after,
        output,
        artifact,
    }
}

fn untouched_output() -> Vec<u8> {
    let mut v = vec![0u8; 4];
    fill_sentinel(&mut v);
    v
}

fn output_for(route: Option<ReportedRoute>) -> Vec<u8> {
    match route {
        None => untouched_output(),
        Some(_) => WRITTEN.to_vec(),
    }
}

/// 1 つの値だけを食い違わせる。出力は経路の有無で反対側にする。
fn corrupt(
    route: Option<ReportedRoute>,
    field: Field,
    ev: &mut CallEvidence<'_>,
    out: &mut Vec<u8>,
) {
    match field {
        Field::Marker => ev.marker = 0x7F,
        Field::Counts => ev.after.cpu += 1 + u64::from(route == Some(ReportedRoute::Cpu)),
        Field::Output => {
            *out = match route {
                None => WRITTEN.to_vec(),
                Some(_) => untouched_output(),
            }
        }
        Field::Artifact => ev.artifact = Some([0x11; 32]),
    }
}

fn counted(route: Option<ReportedRoute>, ev: &CallEvidence<'_>, d: &ArtifactDigests) -> CallCheck {
    check_call(route, ev, Some(d)).unwrap()
}

fn mismatch_of(t: &EvidenceTally, f: Field) -> u64 {
    match f {
        Field::Marker => t.marker_mismatches(),
        Field::Counts => t.count_mismatches(),
        Field::Output => t.output_mismatches(),
        Field::Artifact => t.artifact_mismatches(),
    }
}

#[test]
fn all_routes_match() {
    let d = digests();
    let mut t = EvidenceTally::new();
    for route in ROUTES {
        let out = output_for(route);
        let ev = matching(route, &out);
        t.record(&counted(route, &ev, &d)).unwrap();
    }
    assert_eq!((t.calls(), t.matched(), t.mismatched()), (4, 4, 0));
    assert_eq!(t.run_result(), RunResult::Verdict(Verdict::Match));
}

#[test]
fn one_injected_mismatch_is_detected() {
    let d = digests();
    for route in ROUTES {
        for field in FIELDS {
            // Cpu と None の経路は期待する成果物が None なので、Some を渡すと成果物を食い違わせられる。
            let mut t = EvidenceTally::new();
            for good in ROUTES {
                let out = output_for(good);
                let ev = matching(good, &out);
                t.record(&counted(good, &ev, &d)).unwrap();
            }
            let mut out = output_for(route);
            let mut ev = matching(route, &[]);
            corrupt(route, field, &mut ev, &mut out);
            let ev = CallEvidence { output: &out, ..ev };
            t.record(&counted(route, &ev, &d)).unwrap();

            assert_eq!(t.mismatched(), 1, "{route:?} {field:?}");
            assert_eq!(t.matched(), 4, "{route:?} {field:?}");
            for f in FIELDS {
                let want = u64::from(f == field);
                assert_eq!(mismatch_of(&t, f), want, "{route:?} {field:?} {f:?}");
            }
            assert_eq!(
                t.run_result(),
                RunResult::Verdict(Verdict::Mismatch),
                "{route:?} {field:?}"
            );
        }
    }
}

#[test]
fn probe_runs_are_counted_apart() {
    let d = digests();
    let mut t = EvidenceTally::new();
    for (i, route) in ROUTES.into_iter().enumerate() {
        let out = output_for(route);
        let mut ev = matching(route, &out);
        ev.before.probe = 10;
        ev.after.probe = 10 + i as u64;
        t.record(&counted(route, &ev, &d)).unwrap();
    }
    assert_eq!(t.matched(), 4);
    assert_eq!(t.mismatched(), 0);
    assert_eq!(t.probe_runs(), 1 + 2 + 3);
}

#[test]
fn artifact_not_compared_is_counted() {
    let mut t = EvidenceTally::new();
    let out = output_for(Some(ReportedRoute::Native));
    let ev = matching(Some(ReportedRoute::Native), &out);
    t.record_call(Some(ReportedRoute::Native), &ev, None)
        .unwrap();
    assert_eq!(t.matched(), 1);
    assert_eq!(t.artifact_not_compared(), 1);
    assert_eq!(t.artifact_mismatches(), 0);
}

#[test]
fn input_errors_are_not_counted() {
    let d = digests();
    let mut t = EvidenceTally::new();
    let out = output_for(Some(ReportedRoute::Native));
    let ev = matching(Some(ReportedRoute::Native), &out);
    t.record_call(Some(ReportedRoute::Native), &ev, Some(&d))
        .unwrap();
    let before = t.clone();

    let empty = CallEvidence { output: &[], ..ev };
    assert_eq!(
        t.record_call(Some(ReportedRoute::Native), &empty, Some(&d)),
        Err(EvidenceError::EmptyOutput)
    );
    assert_eq!(t, before);

    let mut decreased = ev;
    decreased.before.probe = 5;
    assert_eq!(
        t.record_call(Some(ReportedRoute::Native), &decreased, Some(&d)),
        Err(EvidenceError::CountsDecreased)
    );
    assert_eq!(t, before);

    assert_eq!(
        ArtifactDigests::from_artifacts(b"same", b"same"),
        Err(EvidenceError::SameArtifactDigest)
    );
    assert_eq!(t, before);
}

#[test]
fn empty_tally_is_indeterminate() {
    assert_eq!(
        EvidenceTally::new().run_result(),
        RunResult::Indeterminate(IndeterminateReason::EvidenceMissing)
    );
}

#[test]
fn tally_overflow_keeps_counts() {
    let d = digests();
    let out = output_for(Some(ReportedRoute::Native));
    let mut ev = matching(Some(ReportedRoute::Native), &out);
    ev.after.probe = u64::MAX;
    let big = counted(Some(ReportedRoute::Native), &ev, &d);
    let mut t = EvidenceTally::new();
    t.record(&big).unwrap();
    let after_first = t.clone();
    assert_eq!(t.record(&big), Err(EvidenceError::TallyOverflow));
    assert_eq!(t, after_first);
}

fn label_of(tally: &EvidenceTally) -> Label {
    let subject = Subject::from_parts("route-evidence-frame", &[], &[]).unwrap();
    let declaration = declaration!([MachineCombination::MetalMacMini]).unwrap();
    let setting = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    judge(&subject, &declaration, &setting, |_| tally.run_result())
        .label()
        .clone()
}

#[test]
fn labels_through_the_frame() {
    let d = digests();

    let mut t = EvidenceTally::new();
    assert_eq!(
        label_of(&t),
        Label::Indeterminate(IndeterminateReason::EvidenceMissing)
    );

    for route in ROUTES {
        let out = output_for(route);
        let ev = matching(route, &out);
        t.record(&counted(route, &ev, &d)).unwrap();
    }
    assert_eq!(label_of(&t), Label::Ok);

    let out = output_for(Some(ReportedRoute::Cpu));
    let mut ev = matching(Some(ReportedRoute::Cpu), &out);
    ev.marker = MARKER_NATIVE;
    t.record(&counted(Some(ReportedRoute::Cpu), &ev, &d))
        .unwrap();
    let label = label_of(&t);
    assert_eq!(label, Label::Ng);
    assert!(label.fails_test());
}
