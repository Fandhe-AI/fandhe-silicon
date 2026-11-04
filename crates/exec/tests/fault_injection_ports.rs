//! 試験用の 3 つの口を、公開のパスだけから使う試験。
//!
//! 3 つの口は、`common::Scripted::overriding`（能力の問合せの値の差し替え）、
//! `Exec::force_unsupported`（指定した操作で `Native` を候補に入れない）、
//! `Exec::inject_denormal`（f32 の非正規化数の判定の差し込み）。
//! 報告の欄、3 つの入口が変わらないこと、`query_ex` が 1 回であることを確かめる。
//! feature `fault-injection` のビルドでだけ流れる。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityEvidence, CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::feature_key::FEATURE_SUBGROUP;
use fandhe_silicon_exec::{
    AtomicAddOperands, DenormalInjection, Exec, F16FmaOperands, F32DenormalBasis,
    F32DenormalBehavior, OpId, OptIn, Outcome, Route, RouteReport, SubgroupSumF32Operands,
    SubgroupSumU32Operands, UnavailableReason,
};

const fn opt(g: bool, c: bool) -> OptIn {
    OptIn {
        allow_gpu_emulated: g,
        allow_cpu: c,
    }
}

const NONE: OptIn = opt(false, false);
const G_ONLY: OptIn = opt(true, false);
const C_ONLY: OptIn = opt(false, true);
const BOTH: OptIn = opt(true, true);

const OPS: [OpId; 4] = [
    OpId::F16Fma,
    OpId::SubgroupSumU32,
    OpId::SubgroupSumF32,
    OpId::AtomicAdd,
];

/// 上書きの 5 通り（`None` は `Exec` の既定の印を使う）。
const OVERS: [Option<OptIn>; 5] = [None, Some(NONE), Some(G_ONLY), Some(C_ONLY), Some(BOTH)];

/// 能力の段階が `S0`〜`S6` の 7 通り。
const LEVELS: [CapabilityLevel; 7] = [
    CapabilityLevel::S0,
    CapabilityLevel::S1,
    CapabilityLevel::S2,
    CapabilityLevel::S3,
    CapabilityLevel::S4,
    CapabilityLevel::S5,
    CapabilityLevel::S6,
];

const ALL_ROUTES: [Route; 3] = [Route::Native, Route::GpuEmulated, Route::Cpu];

fn q(level: CapabilityLevel, reason: Option<UnsupportedReason>) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: reason,
        evidence: None,
    }
}

/// 判定を求める操作か。
fn needs_judgment(op: OpId) -> bool {
    matches!(op, OpId::SubgroupSumF32 | OpId::AtomicAdd)
}

/// 入口の関数名。
fn entry_name(op: OpId) -> &'static str {
    match op {
        OpId::F16Fma => "f16_fma",
        OpId::SubgroupSumU32 => "subgroup_sum_u32",
        OpId::SubgroupSumF32 => "subgroup_sum_f32",
        OpId::AtomicAdd => "atomic_add",
        _ => unreachable!(),
    }
}

/// `op` の指定した経路すべてに `judgment` を差し込む。
fn inject<'d>(
    mut exec: Exec<'d>,
    op: OpId,
    routes: &[Route],
    judgment: DenormalInjection,
) -> Exec<'d> {
    for &route in routes {
        exec = exec.inject_denormal(op, route, judgment);
    }
    exec
}

/// `query_route` が `Ok` で返ることを期待して読む。
fn route_of(exec: &Exec<'_>, op: OpId, over: Option<OptIn>, ctx: &str) -> RouteReport {
    match exec.query_route(op, over) {
        Ok(r) => r,
        Err(e) => panic!("expected Ok, got {e:?}: {ctx}"),
    }
}

/// 入口を 1 回呼ぶ。返しと、出力の列が呼ぶ前のままかを返す。
fn call_entry(
    exec: &Exec<'_>,
    over: Option<OptIn>,
    op: OpId,
) -> (Result<Outcome, ContractError>, bool) {
    let in16 = [1u16; 2];
    let in_u = [1u32; 2];
    let in_f = [1.0f32; 2];
    let (mut o16, mut ou, mut of, mut oa) = ([7u16; 2], [7u32; 2], [7.0f32; 2], [7.0f32; 2]);
    let r = match op {
        OpId::F16Fma => exec.f16_fma(
            over,
            2,
            F16FmaOperands::Host {
                a: &in16,
                b: &in16,
                c: &in16,
                out: &mut o16,
            },
        ),
        OpId::SubgroupSumU32 => exec.subgroup_sum_u32(
            over,
            2,
            SubgroupSumU32Operands::Host {
                x: &in_u,
                out: &mut ou,
            },
        ),
        OpId::SubgroupSumF32 => exec.subgroup_sum_f32(
            over,
            2,
            SubgroupSumF32Operands::Host {
                x: &in_f,
                out: &mut of,
            },
        ),
        OpId::AtomicAdd => exec.atomic_add(
            over,
            2,
            1,
            AtomicAddOperands::Host {
                x: &in_f,
                out: &mut oa,
            },
        ),
        _ => unreachable!(),
    };
    let untouched = o16 == [7; 2] && ou == [7; 2] && of == [7.0; 2] && oa == [7.0; 2];
    (r, untouched)
}

/// 3 つの入口を呼び、それぞれが自分の関数名の `Unsupported` を返したかを返す。
fn call_three_entries(exec: &Exec<'_>, over: Option<OptIn>) -> [(&'static str, bool); 3] {
    fn is_named_unsupported(r: Result<Outcome, ContractError>, name: &'static str) -> bool {
        matches!(r, Err(ContractError::Unsupported { op, .. }) if op == name)
    }
    [
        (
            "external_interop_shared_memory",
            is_named_unsupported(
                exec.external_interop_shared_memory(over),
                "external_interop_shared_memory",
            ),
        ),
        (
            "hardware_ray_tracing",
            is_named_unsupported(exec.hardware_ray_tracing(over), "hardware_ray_tracing"),
        ),
        (
            "unified_memory_sharing",
            is_named_unsupported(exec.unified_memory_sharing(over), "unified_memory_sharing"),
        ),
    ]
}

#[test]
fn force_applies_to_the_target_op_only() {
    let dev = Scripted::answering(q(CapabilityLevel::S2, None));
    for target in OPS {
        // 判定を求める操作は、届く経路すべてに差し込んで `Ok` で読む。
        let mut exec = Exec::deny_by_default(&dev).force_unsupported(target);
        for op in OPS {
            if needs_judgment(op) {
                exec = inject(
                    exec,
                    op,
                    &[Route::Native, Route::GpuEmulated],
                    DenormalInjection::Preserve,
                );
            }
        }
        let r = route_of(&exec, target, Some(G_ONLY), &format!("{target:?}"));
        assert_eq!(r.op, target);
        assert_eq!(r.route, Some(Route::GpuEmulated), "{target:?}");
        assert_eq!(
            r.native_reason,
            Some(UnavailableReason::ForcedUnsupported),
            "{target:?}"
        );
        assert_eq!(r.fallback_reason, None, "{target:?}");
        assert_eq!(r.observed_level, Some(CapabilityLevel::S2), "{target:?}");
        for op in OPS.into_iter().filter(|op| *op != target) {
            for over in OVERS {
                let ctx = format!("target={target:?} {op:?} {over:?}");
                let r = route_of(&exec, op, over, &ctx);
                assert_eq!(r.route, Some(Route::Native), "{ctx}");
                assert_eq!(r.native_reason, None, "{ctx}");
                assert_eq!(r.fallback_reason, None, "{ctx}");
                assert_eq!(r.observed_level, Some(CapabilityLevel::S2), "{ctx}");
            }
        }
    }
}

#[test]
fn force_without_injection_keeps_unsupported_for_f32_ops() {
    let dev = Scripted::answering(q(CapabilityLevel::S2, None));
    let exec = Exec::deny_by_default(&dev).force_unsupported(OpId::F16Fma);
    // 指定していない操作で、差し込みの無い経路に届く呼び出しは `Err`。
    for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
        let r = exec.query_route(op, None);
        assert!(
            matches!(
                r,
                Err(ContractError::Unsupported {
                    op: "query_route",
                    ..
                })
            ),
            "{op:?}: {r:?}"
        );
    }
}

#[test]
fn forced_op_follows_opt_in() {
    let mut answers: Vec<CapabilityQuery> = LEVELS.iter().map(|l| q(*l, None)).collect();
    answers.push(q(CapabilityLevel::Unsupported, None));
    answers.push(q(
        CapabilityLevel::Unsupported,
        Some(UnsupportedReason::NotSupported),
    ));
    let overs = [Some(NONE), Some(G_ONLY), Some(C_ONLY), Some(BOTH)];
    for answer in answers {
        let dev = Scripted::answering(answer);
        for op in OPS {
            let mut exec = Exec::deny_by_default(&dev).force_unsupported(op);
            if needs_judgment(op) {
                exec = inject(
                    exec,
                    op,
                    &[Route::GpuEmulated, Route::Cpu],
                    DenormalInjection::Preserve,
                );
            }
            for over in overs {
                let ctx = format!("{answer:?} {op:?} {over:?}");
                let want = match over {
                    Some(o) if o == G_ONLY || o == BOTH => Some(Route::GpuEmulated),
                    Some(o) if o == C_ONLY => Some(Route::Cpu),
                    _ => None,
                };
                let r = route_of(&exec, op, over, &ctx);
                assert_eq!(r.op, op, "{ctx}");
                assert_eq!(r.route, want, "{ctx}");
                assert_eq!(
                    r.native_reason,
                    Some(UnavailableReason::ForcedUnsupported),
                    "{ctx}"
                );
                assert_eq!(r.observed_level, Some(answer.level), "{ctx}");
                assert_eq!(
                    r.fallback_reason,
                    want.is_none().then_some(UnavailableReason::OptInNotGiven),
                    "{ctx}"
                );
                match (want, needs_judgment(op)) {
                    (Some(_), true) => {
                        let d = r.f32_denormal.unwrap_or_else(|| panic!("f32 item: {ctx}"));
                        assert_eq!(d.behavior, F32DenormalBehavior::Preserve, "{ctx}");
                        assert_eq!(d.basis, F32DenormalBasis::Injected, "{ctx}");
                        assert_eq!(d.documented, None, "{ctx}");
                    }
                    _ => assert!(r.f32_denormal.is_none(), "{ctx}"),
                }
                // 入口。
                let (entry, untouched) = call_entry(&exec, over, op);
                if op == OpId::F16Fma && want == Some(Route::Cpu) {
                    match entry {
                        Ok(Outcome::Ran { report }) => assert_eq!(report, r, "{ctx}"),
                        other => panic!("expected Ran: {other:?}: {ctx}"),
                    }
                    assert!(!untouched, "{ctx}");
                    continue;
                }
                assert!(untouched, "{ctx}");
                if want.is_none() {
                    match entry {
                        Ok(Outcome::Unavailable { report }) => assert_eq!(report, r, "{ctx}"),
                        other => panic!("expected Unavailable: {other:?}: {ctx}"),
                    }
                } else {
                    assert!(
                        matches!(&entry, Err(ContractError::Unsupported { op: n, .. }) if *n == entry_name(op)),
                        "{entry:?}: {ctx}"
                    );
                }
            }
        }
    }
}

#[test]
fn override_reaches_route_choice() {
    let mut values: Vec<CapabilityQuery> = LEVELS.iter().map(|l| q(*l, None)).collect();
    for reason in [
        None,
        Some(UnsupportedReason::NoApi),
        Some(UnsupportedReason::NotSupported),
        Some(UnsupportedReason::UnknownFeature),
    ] {
        values.push(q(CapabilityLevel::Unsupported, reason));
    }
    // 根拠の有無は報告に影響しない。
    values.push(CapabilityQuery {
        level: CapabilityLevel::S3,
        unsupported_reason: None,
        evidence: Some(CapabilityEvidence::RuntimeApi),
    });
    for value in values {
        let ctx = format!("{value:?}");
        let dev = Scripted::overriding(FEATURE_SUBGROUP, value);
        let exec = Exec::deny_by_default(&dev);
        let r = route_of(&exec, OpId::SubgroupSumU32, None, &ctx);
        assert_eq!(r.observed_level, Some(value.level), "{ctx}");
        match value.level {
            CapabilityLevel::S2
            | CapabilityLevel::S3
            | CapabilityLevel::S4
            | CapabilityLevel::S5
            | CapabilityLevel::S6 => {
                assert_eq!(r.route, Some(Route::Native), "{ctx}");
                assert_eq!(r.native_reason, None, "{ctx}");
                assert_eq!(r.fallback_reason, None, "{ctx}");
            }
            CapabilityLevel::Unsupported => {
                assert_eq!(r.route, None, "{ctx}");
                assert_eq!(
                    r.native_reason,
                    Some(UnavailableReason::CapabilityUnsupported(
                        value.unsupported_reason
                    )),
                    "{ctx}"
                );
                assert_eq!(
                    r.fallback_reason,
                    Some(UnavailableReason::OptInNotGiven),
                    "{ctx}"
                );
            }
            _ => {
                assert_eq!(r.route, None, "{ctx}");
                assert_eq!(
                    r.native_reason,
                    Some(UnavailableReason::CapabilityUnknown),
                    "{ctx}"
                );
                assert_eq!(
                    r.fallback_reason,
                    Some(UnavailableReason::OptInNotGiven),
                    "{ctx}"
                );
            }
        }
        // 別の鍵は内側の値のまま。
        let f = route_of(&exec, OpId::F16Fma, None, &ctx);
        assert_eq!(f.route, None, "{ctx}");
        assert_eq!(
            f.observed_level,
            Some(CapabilityLevel::Unsupported),
            "{ctx}"
        );
        assert_eq!(
            f.native_reason,
            Some(UnavailableReason::CapabilityUnsupported(Some(
                UnsupportedReason::UnknownFeature
            ))),
            "{ctx}"
        );
        assert_eq!(dev.query_ex_calls(), 2, "{ctx}");
    }
}

#[test]
fn injection_reaches_reports() {
    for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
        let dev = Scripted::answering(q(CapabilityLevel::S2, None));
        // (a) Native が判定できず、GpuEmulated の判定で決まる。
        let exec = inject(
            inject(
                Exec::deny_by_default(&dev),
                op,
                &[Route::Native],
                DenormalInjection::Unknown,
            ),
            op,
            &[Route::GpuEmulated],
            DenormalInjection::FlushToZero,
        );
        let r = route_of(&exec, op, Some(G_ONLY), &format!("a {op:?}"));
        assert_eq!(r.route, Some(Route::GpuEmulated), "a {op:?}");
        assert_eq!(
            r.native_reason,
            Some(UnavailableReason::F32DenormalBehaviorUnknown),
            "a {op:?}"
        );
        assert_eq!(r.fallback_reason, None, "a {op:?}");
        let d = r.f32_denormal.unwrap_or_else(|| panic!("a {op:?}"));
        assert_eq!(d.behavior, F32DenormalBehavior::FlushToZero, "a {op:?}");
        assert_eq!(d.basis, F32DenormalBasis::Injected, "a {op:?}");
        assert_eq!(d.documented, None, "a {op:?}");

        // (b) Native の判定が Preserve。
        let exec = inject(
            Exec::deny_by_default(&dev),
            op,
            &[Route::Native],
            DenormalInjection::Preserve,
        );
        let r = route_of(&exec, op, None, &format!("b {op:?}"));
        assert_eq!(r.route, Some(Route::Native), "b {op:?}");
        assert_eq!(r.native_reason, None, "b {op:?}");
        let d = r.f32_denormal.unwrap_or_else(|| panic!("b {op:?}"));
        assert_eq!(d.behavior, F32DenormalBehavior::Preserve, "b {op:?}");
        assert_eq!(d.basis, F32DenormalBasis::Injected, "b {op:?}");
        assert_eq!(d.documented, None, "b {op:?}");

        // (c) どの経路も判定できない。
        let exec = inject(
            Exec::deny_by_default(&dev),
            op,
            &ALL_ROUTES,
            DenormalInjection::Unknown,
        );
        let r = route_of(&exec, op, Some(BOTH), &format!("c {op:?}"));
        assert_eq!(r.route, None, "c {op:?}");
        assert_eq!(
            r.fallback_reason,
            Some(UnavailableReason::F32DenormalBehaviorUnknown),
            "c {op:?}"
        );
        assert!(r.f32_denormal.is_none(), "c {op:?}");
        let (entry, untouched) = call_entry(&exec, Some(BOTH), op);
        assert!(untouched, "c {op:?}");
        match entry {
            Ok(Outcome::Unavailable { report }) => assert_eq!(report, r, "c {op:?}"),
            other => panic!("expected Unavailable: {other:?}: c {op:?}"),
        }
    }
}

#[test]
fn ports_combine() {
    let dev = Scripted::overriding(FEATURE_SUBGROUP, q(CapabilityLevel::S4, None));
    let exec = Exec::new(&dev, BOTH)
        .force_unsupported(OpId::SubgroupSumU32)
        .inject_denormal(
            OpId::SubgroupSumF32,
            Route::Native,
            DenormalInjection::FlushToZero,
        )
        .inject_denormal(
            OpId::AtomicAdd,
            Route::GpuEmulated,
            DenormalInjection::Unknown,
        )
        .inject_denormal(OpId::AtomicAdd, Route::Cpu, DenormalInjection::Preserve);

    // 差し替えた鍵と指定が重なる操作。
    let r = route_of(&exec, OpId::SubgroupSumU32, None, "u32");
    assert_eq!(r.observed_level, Some(CapabilityLevel::S4));
    assert_eq!(r.native_reason, Some(UnavailableReason::ForcedUnsupported));
    assert_eq!(r.route, Some(Route::GpuEmulated));
    assert!(r.f32_denormal.is_none());

    // 差し替えた鍵と差し込みが重なる操作。
    let r = route_of(&exec, OpId::SubgroupSumF32, None, "f32");
    assert_eq!(r.observed_level, Some(CapabilityLevel::S4));
    assert_eq!(r.route, Some(Route::Native));
    assert_eq!(r.native_reason, None);
    let d = r.f32_denormal.unwrap_or_else(|| panic!("f32"));
    assert_eq!(d.behavior, F32DenormalBehavior::FlushToZero);
    assert_eq!(d.basis, F32DenormalBasis::Injected);

    // 差し替えの外の鍵は内側の値で、差し込みで経路が決まる。
    let r = route_of(&exec, OpId::AtomicAdd, None, "atomic");
    assert_eq!(r.observed_level, Some(CapabilityLevel::Unsupported));
    assert_eq!(
        r.native_reason,
        Some(UnavailableReason::CapabilityUnsupported(Some(
            UnsupportedReason::UnknownFeature
        )))
    );
    assert_eq!(r.route, Some(Route::Cpu));
    let d = r.f32_denormal.unwrap_or_else(|| panic!("atomic"));
    assert_eq!(d.behavior, F32DenormalBehavior::Preserve);
    assert_eq!(d.basis, F32DenormalBasis::Injected);

    // どの口も触れない操作は内側の値のまま。
    let r = route_of(&exec, OpId::F16Fma, None, "f16");
    assert_eq!(r.observed_level, Some(CapabilityLevel::Unsupported));
    assert_eq!(
        r.native_reason,
        Some(UnavailableReason::CapabilityUnsupported(Some(
            UnsupportedReason::UnknownFeature
        )))
    );
    assert_eq!(r.route, Some(Route::GpuEmulated));
    assert!(r.f32_denormal.is_none());
    assert_eq!(dev.query_ex_calls(), 4);
}

/// 4 操作の指定と 4 x 3 の全組の差し込みを持つ `Exec`。
fn fully_configured(dev: &Scripted) -> Exec<'_> {
    let mut exec = Exec::deny_by_default(dev);
    for op in OPS {
        exec = inject(
            exec.force_unsupported(op),
            op,
            &ALL_ROUTES,
            DenormalInjection::Preserve,
        );
    }
    exec
}

#[test]
fn ports_do_not_touch_three_entries() {
    let dev = Scripted::answering(q(CapabilityLevel::S2, None));
    let exec = fully_configured(&dev);
    for over in OVERS {
        for (name, ok) in call_three_entries(&exec, over) {
            assert!(ok, "{name} {over:?}");
        }
    }
    assert_eq!(dev.query_ex_calls(), 0);
    assert_eq!(dev.memory_calls(), 0);
    assert_eq!(dev.queue_calls(), 0);
}

#[test]
fn ports_keep_query_ex_once() {
    // kind は 0: 指定のみ、1: 差し込みのみ、2: 両方。
    for kind in 0..3 {
        for op in OPS {
            for over in OVERS {
                for via_entry in [false, true] {
                    let dev = Scripted::overriding(FEATURE_SUBGROUP, q(CapabilityLevel::S2, None));
                    let mut exec = Exec::deny_by_default(&dev);
                    if kind != 1 {
                        exec = exec.force_unsupported(op);
                    }
                    if kind != 0 {
                        exec = inject(exec, op, &ALL_ROUTES, DenormalInjection::Preserve);
                    }
                    let ctx = format!("kind={kind} {op:?} {over:?} entry={via_entry}");
                    if via_entry {
                        let _ = call_entry(&exec, over, op);
                    } else {
                        let _ = exec.query_route(op, over);
                    }
                    assert_eq!(dev.query_ex_calls(), 1, "{ctx}");
                    assert_eq!(dev.memory_calls(), 0, "{ctx}");
                    assert_eq!(dev.queue_calls(), 0, "{ctx}");
                }
            }
        }
    }
}
