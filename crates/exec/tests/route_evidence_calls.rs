//! 下-2 の公開のパス（入口の `Outcome::Unavailable` の報告と `Exec::query_route` の報告）が返した経路を、
//! 試験の中の模擬の実行関数が作った値と `route_evidence` で突き合わせる試験（spec TASK-922）。
//! 一致の列が一致に数えられ、4 つの値のどれか 1 つの食い違いがその値の欄に 1 件だけ数えられること、
//! 試験の枠のラベルが一致で `Ok`、不一致で `Ng` になることを確かめる。置き場の説明は docs/route-evidence.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::declaration;
use fandhe_silicon_core::route_evidence::{
    ArtifactDigests, CallEvidence, EvidenceTally, MARKER_CLEAR, MARKER_GPU_EMULATED, MARKER_NATIVE,
    ReportedRoute, RunCounts, SENTINEL_BYTE, fill_sentinel,
};
use fandhe_silicon_core::test_frame::{
    COMBINATION_ENV, Label, MachineCombination, RunSetting, Subject, judge,
};
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OpId, OptIn, Outcome, Route, RouteReport,
    SubgroupSumF32Operands, SubgroupSumU32Operands,
};
use std::ffi::OsString;

const NONE: OptIn = OptIn {
    allow_gpu_emulated: false,
    allow_cpu: false,
};
const G_ONLY: OptIn = OptIn {
    allow_gpu_emulated: true,
    allow_cpu: false,
};
const C_ONLY: OptIn = OptIn {
    allow_gpu_emulated: false,
    allow_cpu: true,
};

/// 模擬の実行が使う出力の列の長さ（バイト）。
const OUT_LEN: usize = 8;

fn q(level: CapabilityLevel) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: None,
        evidence: None,
    }
}

/// 下-2 の `Route` を突き合わせの側の型へ写す。
fn reported(route: Option<Route>) -> Option<ReportedRoute> {
    match route {
        Some(Route::Native) => Some(ReportedRoute::Native),
        Some(Route::GpuEmulated) => Some(ReportedRoute::GpuEmulated),
        Some(Route::Cpu) => Some(ReportedRoute::Cpu),
        None => None,
    }
}

/// 操作ごとに別の成果物の列から SHA-256 の組を作る。
fn digests_for(op: OpId) -> ArtifactDigests {
    let name = format!("{op:?}");
    ArtifactDigests::from_artifacts(
        format!("{name}-native-artifact").as_bytes(),
        format!("{name}-gpu-artifact").as_bytes(),
    )
    .unwrap()
}

/// 経路なしの入口を呼び、報告と、呼んだ後の出力の列（番兵で埋めた物）のバイト列を返す。
fn call_unavailable(exec: &Exec<'_>, op: OpId) -> (RouteReport, Vec<u8>) {
    let s16 = u16::from_ne_bytes([SENTINEL_BYTE; 2]);
    let s32 = u32::from_ne_bytes([SENTINEL_BYTE; 4]);
    let sf = f32::from_bits(s32);
    let in16 = [1u16; 2];
    let in_u = [1u32; 2];
    let in_f = [1.0f32; 2];
    let (mut o16, mut ou, mut of, mut oa) = ([s16; 2], [s32; 2], [sf; 2], [sf; 2]);
    let (result, bytes): (_, Vec<u8>) = match op {
        OpId::F16Fma => {
            let r = exec.f16_fma(
                Some(NONE),
                2,
                F16FmaOperands::Host {
                    a: &in16,
                    b: &in16,
                    c: &in16,
                    out: &mut o16,
                },
            );
            (r, o16.iter().flat_map(|v| v.to_ne_bytes()).collect())
        }
        OpId::SubgroupSumU32 => {
            let r = exec.subgroup_sum_u32(
                Some(NONE),
                2,
                SubgroupSumU32Operands::Host {
                    x: &in_u,
                    out: &mut ou,
                },
            );
            (r, ou.iter().flat_map(|v| v.to_ne_bytes()).collect())
        }
        OpId::SubgroupSumF32 => {
            let r = exec.subgroup_sum_f32(
                Some(NONE),
                2,
                SubgroupSumF32Operands::Host {
                    x: &in_f,
                    out: &mut of,
                },
            );
            (
                r,
                of.iter().flat_map(|v| v.to_bits().to_ne_bytes()).collect(),
            )
        }
        OpId::AtomicAdd => {
            let r = exec.atomic_add(
                Some(NONE),
                2,
                1,
                AtomicAddOperands::Host {
                    x: &in_f,
                    out: &mut oa,
                },
            );
            (
                r,
                oa.iter().flat_map(|v| v.to_bits().to_ne_bytes()).collect(),
            )
        }
        _ => unreachable!(),
    };
    match result {
        Ok(Outcome::Unavailable { report }) => (report, bytes),
        other => panic!("expected Unavailable: {op:?} {other:?}"),
    }
}

/// 仕込む食い違い（模擬の実行の値の 1 つを変える）。
#[derive(Clone, Copy, Debug)]
enum Fault {
    Marker,
    Counts,
    Output,
    Artifact,
}

const FAULTS: [Fault; 4] = [Fault::Marker, Fault::Counts, Fault::Output, Fault::Artifact];

/// 模擬の実行。返した経路を実行したとして、印・後の回数・成果物を返し、出力の列を書き換える。
fn simulate(
    route: Option<ReportedRoute>,
    before: RunCounts,
    out: &mut [u8],
    digests: &ArtifactDigests,
    fault: Option<Fault>,
) -> (u32, RunCounts, Option<[u8; 32]>) {
    let mut after = before;
    let (mut marker, mut artifact, mut write) = match route {
        Some(ReportedRoute::Native) => {
            after.native += 1;
            (MARKER_NATIVE, Some(digests.native()), true)
        }
        Some(ReportedRoute::GpuEmulated) => {
            after.gpu_emulated += 1;
            (MARKER_GPU_EMULATED, Some(digests.gpu_emulated()), true)
        }
        Some(ReportedRoute::Cpu) => {
            after.cpu += 1;
            (MARKER_CLEAR, None, true)
        }
        None => (MARKER_CLEAR, None, false),
    };
    match fault {
        Some(Fault::Marker) => {
            marker = if route == Some(ReportedRoute::Native) {
                MARKER_GPU_EMULATED
            } else {
                MARKER_NATIVE
            };
        }
        Some(Fault::Counts) => {
            if route == Some(ReportedRoute::Native) {
                after.gpu_emulated += 1;
            } else {
                after.native += 1;
            }
        }
        Some(Fault::Output) => {
            if route.is_some() {
                write = false;
            } else if let Some(b) = out.first_mut() {
                *b ^= 0xFF;
            }
        }
        Some(Fault::Artifact) => {
            artifact = if route == Some(ReportedRoute::Native) {
                Some(digests.gpu_emulated())
            } else {
                Some(digests.native())
            };
        }
        None => {}
    }
    if write {
        for (i, b) in out.iter_mut().enumerate() {
            *b = (i as u8) ^ 0x5A;
        }
    }
    (marker, after, artifact)
}

/// 模擬の実行を `fault` つきで行い、その呼び出しを数える。
fn record(
    tally: &mut EvidenceTally,
    route: Option<ReportedRoute>,
    digests: &ArtifactDigests,
    fault: Option<Fault>,
) {
    let before = RunCounts::default();
    let mut out = vec![0u8; OUT_LEN];
    fill_sentinel(&mut out);
    let (marker, after, artifact) = simulate(route, before, &mut out, digests, fault);
    let ev = CallEvidence {
        marker,
        before,
        after,
        output: &out,
        artifact,
    };
    tally.record_call(route, &ev, Some(digests)).unwrap();
}

/// 一致の列の数え上げに `fault` の 1 件だけを足すと、その値の欄にだけ 1 件の不一致が出る。
fn assert_single_fault(
    route: Option<ReportedRoute>,
    digests: &ArtifactDigests,
    base: &EvidenceTally,
) {
    for fault in FAULTS {
        let mut t = base.clone();
        let (calls, matched) = (t.calls(), t.matched());
        record(&mut t, route, digests, Some(fault));
        let ctx = format!("{route:?} {fault:?}");
        assert_eq!(t.calls(), calls + 1, "{ctx}");
        assert_eq!(t.matched(), matched, "{ctx}");
        assert_eq!(t.mismatched(), 1, "{ctx}");
        let fields = [
            t.marker_mismatches(),
            t.count_mismatches(),
            t.output_mismatches(),
            t.artifact_mismatches(),
        ];
        let mut expected = [0u64; 4];
        expected[match fault {
            Fault::Marker => 0,
            Fault::Counts => 1,
            Fault::Output => 2,
            Fault::Artifact => 3,
        }] = 1;
        assert_eq!(fields, expected, "{ctx}");
    }
}

const UNAVAILABLE_OPS: [OpId; 4] = [
    OpId::F16Fma,
    OpId::SubgroupSumU32,
    OpId::SubgroupSumF32,
    OpId::AtomicAdd,
];

/// 経路なしの 4 入口の一致だけを数えた数え上げ。
fn unavailable_tally() -> EvidenceTally {
    let dev = Scripted::answering(q(CapabilityLevel::Unsupported));
    let exec = Exec::deny_by_default(&dev);
    let mut tally = EvidenceTally::new();
    for op in UNAVAILABLE_OPS {
        let (report, bytes) = call_unavailable(&exec, op);
        let route = reported(report.route);
        assert_eq!(route, None, "{op:?}");
        let c = RunCounts::default();
        let ev = CallEvidence {
            marker: MARKER_CLEAR,
            before: c,
            after: c,
            output: &bytes,
            artifact: None,
        };
        let check = tally
            .record_call(route, &ev, Some(&digests_for(op)))
            .unwrap();
        assert!(check.is_match(), "{op:?}");
    }
    tally
}

/// 経路を得る 3 つの組（device の答え・opt-in・期待の経路）。
fn route_cases() -> [(CapabilityLevel, OptIn, ReportedRoute); 3] {
    [
        (CapabilityLevel::S2, NONE, ReportedRoute::Native),
        (
            CapabilityLevel::Unsupported,
            G_ONLY,
            ReportedRoute::GpuEmulated,
        ),
        (CapabilityLevel::Unsupported, C_ONLY, ReportedRoute::Cpu),
    ]
}

/// `route` の模擬の一致を 1 件数え、全 `Fault` の 1 件を足したときの数え上げを確かめる。
fn check_route(op: OpId, route: ReportedRoute) {
    let d = digests_for(op);
    let mut t = EvidenceTally::new();
    record(&mut t, Some(route), &d, None);
    assert_eq!(
        (t.calls(), t.matched(), t.mismatched()),
        (1, 1, 0),
        "{op:?}"
    );
    assert_single_fault(Some(route), &d, &t);
}

#[test]
fn unavailable_entries_match_without_writes() {
    let tally = unavailable_tally();
    assert_eq!(tally.calls(), 4);
    assert_eq!(tally.matched(), 4);
    assert_eq!(tally.mismatched(), 0);
    for op in UNAVAILABLE_OPS {
        assert_single_fault(None, &digests_for(op), &tally);
    }
    // 印に MARKER_NATIVE を仕込んだ 1 件は印の欄に数えられる。
    let mut t = tally.clone();
    record(
        &mut t,
        None,
        &digests_for(OpId::F16Fma),
        Some(Fault::Marker),
    );
    assert_eq!(t.marker_mismatches(), 1);
}

#[test]
fn queried_routes_match_simulated_runs() {
    let mut total = EvidenceTally::new();
    for op in [OpId::F16Fma, OpId::SubgroupSumU32] {
        for (level, over, expected) in route_cases() {
            let dev = Scripted::answering(q(level));
            let exec = Exec::deny_by_default(&dev);
            let report = exec.query_route(op, Some(over)).unwrap();
            let route = reported(report.route);
            assert_eq!(route, Some(expected), "{op:?} {level:?}");
            check_route(op, expected);
            record(&mut total, route, &digests_for(op), None);
        }
    }
    assert_eq!(
        (total.calls(), total.matched(), total.mismatched()),
        (6, 6, 0)
    );
}

fn label_of(tally: &EvidenceTally) -> Label {
    let subject = Subject::from_parts("route-evidence-calls", &[], &[]).unwrap();
    let declaration = declaration!([MachineCombination::MetalMacMini]).unwrap();
    let setting = RunSetting::from_lookup(|n| {
        (n == COMBINATION_ENV).then(|| OsString::from("metal-mac-mini"))
    });
    judge(&subject, &declaration, &setting, |_| tally.run_result())
        .label()
        .clone()
}

#[test]
fn tally_labels_through_the_frame() {
    let mut tally = unavailable_tally();
    for op in [OpId::F16Fma, OpId::SubgroupSumU32] {
        for (_, _, expected) in route_cases() {
            record(&mut tally, Some(expected), &digests_for(op), None);
        }
    }
    assert_eq!((tally.calls(), tally.matched()), (10, 10));
    assert_eq!(label_of(&tally), Label::Ok);
    record(
        &mut tally,
        Some(ReportedRoute::Cpu),
        &digests_for(OpId::F16Fma),
        Some(Fault::Marker),
    );
    let label = label_of(&tally);
    assert_eq!(label, Label::Ng);
    assert!(label.fails_test());
}

#[cfg(feature = "fault-injection")]
mod injected {
    use super::*;
    use fandhe_silicon_exec::DenormalInjection;

    #[test]
    fn injected_routes_match_simulated_runs() {
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            for (level, over, expected) in route_cases() {
                let target = match expected {
                    ReportedRoute::Native => Route::Native,
                    ReportedRoute::GpuEmulated => Route::GpuEmulated,
                    ReportedRoute::Cpu => Route::Cpu,
                };
                let dev = Scripted::answering(q(level));
                let exec = Exec::deny_by_default(&dev).inject_denormal(
                    op,
                    target,
                    DenormalInjection::Preserve,
                );
                let report = exec.query_route(op, Some(over)).unwrap();
                let route = reported(report.route);
                assert_eq!(route, Some(expected), "{op:?} {level:?}");
                check_route(op, expected);
            }
        }
    }
}
