//! `Exec` 越しの f32 の非正規化数の判定の保持の試験と、その試験が使う部品（TASK-925）。
//!
//! 試験用の device・模擬の runner の組み立て・呼ばれた (操作, 経路) の数えは、この module の
//! private な部品として持つ。crate の中の試験だけが `Exec::with_probe` で runner を差す。

use crate::denormal::ProbeInput;
use crate::entry::Exec;
use crate::operands::{
    AtomicAddOperands, F16FmaOperands, SubgroupSumF32Operands, SubgroupSumU32Operands,
};
use crate::opt_in::OptIn;
use crate::report::{F32DenormalBasis, F32DenormalBehavior, F32DenormalReport, Outcome};
use crate::route::{OpId, Route, UnavailableReason};
use fandhe_silicon_core::capability::FeatureId;
use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityLevel, CapabilityQuery, LimitsTable,
};
use fandhe_silicon_core::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{ContractError, UnsupportedReason};
use fandhe_silicon_core::memory::MemoryOps;
use fandhe_silicon_core::stub_device::StubDevice;
use std::cell::RefCell;

/// どの鍵の問合せにも同じ `CapabilityQuery` を返し、`backend` を選べる試験用の device。
struct QueryDevice {
    inner: StubDevice,
    query: CapabilityQuery,
}

impl QueryDevice {
    fn new(backend: BackendKind, query: CapabilityQuery) -> Self {
        Self {
            inner: StubDevice::new(DeviceRequest {
                backend,
                profile: Profile::ComputeOnly,
                ownership: DeviceOwnership::Shared,
            }),
            query,
        }
    }
}

impl Capabilities for QueryDevice {
    fn query(&self, _feature: FeatureId) -> CapabilityLevel {
        self.query.level
    }

    fn limits(&self) -> &LimitsTable {
        self.inner.limits()
    }

    fn query_ex(&self, _feature: FeatureId) -> CapabilityQuery {
        self.query
    }
}

impl Device for QueryDevice {
    fn backend(&self) -> BackendKind {
        self.inner.backend()
    }

    fn ownership(&self) -> &DeviceOwnership {
        self.inner.ownership()
    }

    fn generation(&self) -> u64 {
        self.inner.generation()
    }

    fn memory(&self) -> &dyn MemoryOps {
        self.inner.memory()
    }

    fn queue(&self) -> &dyn fandhe_silicon_core::execution::CommandQueue {
        self.inner.queue()
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        self.inner.diagnostics()
    }
}

const S2: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::S2,
    unsupported_reason: None,
    evidence: None,
};

const UNKNOWN_FEATURE: CapabilityQuery = CapabilityQuery {
    level: CapabilityLevel::Unsupported,
    unsupported_reason: Some(UnsupportedReason::UnknownFeature),
    evidence: None,
};

const fn opt(g: bool, c: bool) -> OptIn {
    OptIn {
        allow_gpu_emulated: g,
        allow_cpu: c,
    }
}

const G_ONLY: OptIn = opt(true, false);
const C_ONLY: OptIn = opt(false, true);
const BOTH: OptIn = opt(true, true);

/// 保つときの 3 つの出力。ブロックごと（`AtomicAdd` は `i mod 3` ごと）に f32 で順に足した値。
fn preserving(op: OpId, input: &ProbeInput) -> [f32; 3] {
    let mut out = [0.0f32; 3];
    for (i, v) in input.iter().enumerate() {
        let group = match op {
            OpId::AtomicAdd => i % 3,
            _ => i / 256,
        };
        out[group] += *v;
    }
    out
}

/// 呼ばれた (操作, 経路) を順に記録する。
type CallLog = RefCell<Vec<(OpId, Route)>>;

fn calls_of(log: &CallLog, op: OpId, route: Route) -> usize {
    log.borrow().iter().filter(|c| **c == (op, route)).count()
}

fn preserving_runner(
    log: &CallLog,
) -> impl Fn(OpId, Route, &ProbeInput) -> Result<[f32; 3], ContractError> + '_ {
    move |op, route, input| {
        log.borrow_mut().push((op, route));
        Ok(preserving(op, input))
    }
}

fn zero_runner(
    log: &CallLog,
) -> impl Fn(OpId, Route, &ProbeInput) -> Result<[f32; 3], ContractError> + '_ {
    move |op, route, _| {
        log.borrow_mut().push((op, route));
        Ok([0.0; 3])
    }
}

/// 判定できない値（3 つ目だけ保つときの値）を返す runner。
fn unknown_runner(
    log: &CallLog,
) -> impl Fn(OpId, Route, &ProbeInput) -> Result<[f32; 3], ContractError> + '_ {
    move |op, route, input| {
        log.borrow_mut().push((op, route));
        let keep = preserving(op, input);
        Ok([0.0, 0.0, keep[2]])
    }
}

/// 入口を n = 2（`AtomicAdd` は m = 1）で呼んだ結果。
fn call_entry(exec: &Exec<'_>, op: OpId, over: Option<OptIn>) -> Result<Outcome, ContractError> {
    let (x, mut out) = ([1.0f32; 2], [0.0f32; 2]);
    match op {
        OpId::SubgroupSumF32 => exec.subgroup_sum_f32(
            over,
            2,
            SubgroupSumF32Operands::Host {
                x: &x,
                out: &mut out,
            },
        ),
        _ => exec.atomic_add(
            over,
            2,
            1,
            AtomicAddOperands::Host {
                x: &x,
                out: &mut out[..1],
            },
        ),
    }
}

#[test]
fn query_route_and_entries_share_judgment() {
    for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
        let entry_name = match op {
            OpId::SubgroupSumF32 => "subgroup_sum_f32",
            _ => "atomic_add",
        };
        let expected = |r: &crate::report::RouteReport| {
            assert_eq!(r.route, Some(Route::Native));
            assert_eq!(
                r.f32_denormal,
                Some(F32DenormalReport {
                    behavior: F32DenormalBehavior::Preserve,
                    basis: F32DenormalBasis::Observed,
                    documented: None,
                })
            );
        };
        // query_route を先に呼ぶ順。
        let dev = QueryDevice::new(BackendKind::Metal, S2);
        let log = CallLog::default();
        let runner = preserving_runner(&log);
        let exec = Exec::deny_by_default(&dev).with_probe(&runner);
        let first = exec.query_route(op, None).expect("report");
        expected(&first);
        assert_eq!(log.borrow().len(), 1);
        let entry = call_entry(&exec, op, None);
        assert!(matches!(
            entry,
            Err(ContractError::Unsupported { op: name, .. }) if name == entry_name
        ));
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(exec.query_route(op, None), Ok(first));
        assert_eq!(log.borrow().len(), 1);

        // 入口を先に呼ぶ順。
        let log = CallLog::default();
        let runner = preserving_runner(&log);
        let exec = Exec::deny_by_default(&dev).with_probe(&runner);
        assert!(matches!(
            call_entry(&exec, op, None),
            Err(ContractError::Unsupported { .. })
        ));
        assert_eq!(log.borrow().len(), 1);
        expected(&exec.query_route(op, None).expect("report"));
        assert_eq!(log.borrow().len(), 1);
    }
}

#[test]
fn each_exec_value_keeps_its_own_judgment() {
    let dev = QueryDevice::new(BackendKind::Metal, S2);
    let log = CallLog::default();
    let runner = preserving_runner(&log);
    let a = Exec::deny_by_default(&dev).with_probe(&runner);
    let b = Exec::deny_by_default(&dev).with_probe(&runner);
    assert!(a.query_route(OpId::SubgroupSumF32, None).is_ok());
    assert_eq!(log.borrow().len(), 1);
    assert!(b.query_route(OpId::SubgroupSumF32, None).is_ok());
    assert_eq!(log.borrow().len(), 2);
    assert!(a.query_route(OpId::SubgroupSumF32, None).is_ok());
    assert!(b.query_route(OpId::SubgroupSumF32, None).is_ok());
    assert_eq!(log.borrow().len(), 2);
}

#[test]
fn flush_to_zero_and_fallback_routes_are_reported() {
    let op = OpId::SubgroupSumF32;
    // 3 つとも 0 を返す runner。
    let dev = QueryDevice::new(BackendKind::Metal, S2);
    let log = CallLog::default();
    let runner = zero_runner(&log);
    let exec = Exec::deny_by_default(&dev).with_probe(&runner);
    let report = exec.query_route(op, None).expect("report");
    assert_eq!(report.route, Some(Route::Native));
    assert_eq!(
        report.f32_denormal,
        Some(F32DenormalReport {
            behavior: F32DenormalBehavior::FlushToZero,
            basis: F32DenormalBasis::Observed,
            documented: None,
        })
    );

    // Native が使えない device では、決まった後の経路で試しの実行を流さない。
    let dev = QueryDevice::new(BackendKind::Metal, UNKNOWN_FEATURE);
    for (over, routes) in [
        (G_ONLY, vec![Route::GpuEmulated]),
        (C_ONLY, vec![Route::Cpu]),
        (BOTH, vec![Route::GpuEmulated]),
    ] {
        let log = CallLog::default();
        let runner = preserving_runner(&log);
        let exec = Exec::deny_by_default(&dev).with_probe(&runner);
        let report = exec.query_route(op, Some(over)).expect("report");
        let called: Vec<Route> = log.borrow().iter().map(|c| c.1).collect();
        assert_eq!(called, routes);
        assert_eq!(report.route, Some(routes[0]));
        assert_eq!(
            report.native_reason,
            Some(UnavailableReason::CapabilityUnsupported(Some(
                UnsupportedReason::UnknownFeature
            )))
        );
    }

    // 判定できない値は保持され、経路は選ばれない。
    let log = CallLog::default();
    let runner = unknown_runner(&log);
    let exec = Exec::deny_by_default(&dev).with_probe(&runner);
    let first = exec.query_route(op, Some(BOTH)).expect("report");
    assert_eq!(first.route, None);
    assert_eq!(log.borrow().len(), 2);
    assert_eq!(exec.query_route(op, Some(BOTH)), Ok(first));
    assert_eq!(calls_of(&log, op, Route::GpuEmulated), 1);
    assert_eq!(calls_of(&log, op, Route::Cpu), 1);
}

#[cfg(feature = "fault-injection")]
mod ports {
    use super::*;
    use crate::report::DenormalInjection;

    #[test]
    fn judgment_survives_ports() {
        let dev = QueryDevice::new(BackendKind::Metal, S2);
        let log = CallLog::default();
        let runner = preserving_runner(&log);
        let exec = Exec::deny_by_default(&dev).with_probe(&runner);
        assert!(exec.query_route(OpId::SubgroupSumF32, None).is_ok());
        assert_eq!(log.borrow().len(), 1);
        let exec = exec.force_unsupported(OpId::F16Fma).inject_denormal(
            OpId::AtomicAdd,
            Route::Native,
            DenormalInjection::Preserve,
        );
        assert!(exec.query_route(OpId::SubgroupSumF32, None).is_ok());
        assert_eq!(log.borrow().len(), 1);
    }
}

mod documented {
    use super::*;

    fn report_of(
        backend: BackendKind,
        query: CapabilityQuery,
        op: OpId,
        over: Option<OptIn>,
        zero: bool,
    ) -> crate::report::RouteReport {
        let dev = QueryDevice::new(backend, query);
        let log = CallLog::default();
        let runner = preserving_runner(&log);
        let zeros = zero_runner(&log);
        let exec = if zero {
            Exec::deny_by_default(&dev).with_probe(&zeros)
        } else {
            Exec::deny_by_default(&dev).with_probe(&runner)
        };
        exec.query_route(op, over).expect("report")
    }

    #[test]
    fn documented_is_set_for_one_combination() {
        let op = OpId::AtomicAdd;
        let r = report_of(BackendKind::Cuda, S2, op, None, false);
        assert_eq!(r.route, Some(Route::Native));
        assert_eq!(
            r.f32_denormal,
            Some(F32DenormalReport {
                behavior: F32DenormalBehavior::Preserve,
                basis: F32DenormalBasis::Observed,
                documented: Some(F32DenormalBehavior::FlushToZero),
            })
        );
        let r = report_of(BackendKind::Cuda, S2, op, None, true);
        assert_eq!(
            r.f32_denormal,
            Some(F32DenormalReport {
                behavior: F32DenormalBehavior::FlushToZero,
                basis: F32DenormalBasis::Observed,
                documented: Some(F32DenormalBehavior::FlushToZero),
            })
        );
        // 判定できない値で経路が決まらない報告は `f32_denormal` を持たない。
        let dev = QueryDevice::new(BackendKind::Cuda, S2);
        let log = CallLog::default();
        let runner = unknown_runner(&log);
        let exec = Exec::deny_by_default(&dev).with_probe(&runner);
        let r = exec.query_route(op, None).expect("report");
        assert_eq!(r.route, None);
        assert_eq!(r.f32_denormal, None);
    }

    #[test]
    fn documented_is_none_elsewhere() {
        let docs = |r: crate::report::RouteReport| r.f32_denormal.map(|d| d.documented);
        let r = report_of(BackendKind::Cuda, S2, OpId::SubgroupSumF32, None, false);
        assert_eq!(r.route, Some(Route::Native));
        assert_eq!(docs(r), Some(None));
        for (over, route) in [(G_ONLY, Route::GpuEmulated), (C_ONLY, Route::Cpu)] {
            let r = report_of(
                BackendKind::Cuda,
                UNKNOWN_FEATURE,
                OpId::AtomicAdd,
                Some(over),
                false,
            );
            assert_eq!(r.route, Some(route));
            assert_eq!(docs(r), Some(None));
        }
        for backend in [
            BackendKind::Metal,
            BackendKind::Vulkan,
            BackendKind::Dx12Compute,
        ] {
            let r = report_of(backend, S2, OpId::AtomicAdd, None, false);
            assert_eq!(r.route, Some(Route::Native));
            assert_eq!(docs(r), Some(None));
        }
    }

    #[cfg(feature = "fault-injection")]
    #[test]
    fn documented_with_injection() {
        use crate::report::DenormalInjection;
        let dev = QueryDevice::new(BackendKind::Cuda, S2);
        let log = CallLog::default();
        let runner = preserving_runner(&log);
        let exec = Exec::deny_by_default(&dev)
            .with_probe(&runner)
            .inject_denormal(OpId::AtomicAdd, Route::Native, DenormalInjection::Preserve);
        let r = exec.query_route(OpId::AtomicAdd, None).expect("report");
        assert_eq!(r.route, Some(Route::Native));
        assert_eq!(
            r.f32_denormal,
            Some(F32DenormalReport {
                behavior: F32DenormalBehavior::Preserve,
                basis: F32DenormalBasis::Injected,
                documented: Some(F32DenormalBehavior::FlushToZero),
            })
        );
        assert_eq!(log.borrow().len(), 0);
    }
}

/// 拒否した呼び出し・経路の無い呼び出し・判定を求めない操作で、試しの実行が流れないことの試験。
///
/// 回数は模擬の runner が呼ばれた数の、各呼び出しの前後の差で見る。戻り値の文言は照らさない。
mod rejected {
    use super::*;

    const NONE: OptIn = opt(false, false);
    const OVERRIDES: [Option<OptIn>; 5] =
        [None, Some(NONE), Some(G_ONLY), Some(C_ONLY), Some(BOTH)];
    const DEFAULTS: [OptIn; 4] = [NONE, G_ONLY, C_ONLY, BOTH];
    /// `StubDevice` の `max_buffer_bytes`（1 << 28）を f32 の 4 バイトで超える最小の要素数。
    const OVER_LIMIT: u64 = (1 << 26) + 1;
    const S0: CapabilityQuery = CapabilityQuery {
        level: CapabilityLevel::S0,
        unsupported_reason: None,
        evidence: None,
    };
    const S1: CapabilityQuery = CapabilityQuery {
        level: CapabilityLevel::S1,
        unsupported_reason: None,
        evidence: None,
    };

    fn devices() -> [QueryDevice; 2] {
        [
            QueryDevice::new(BackendKind::Metal, S2),
            QueryDevice::new(BackendKind::Metal, UNKNOWN_FEATURE),
        ]
    }

    /// 長さを指定して f32 の 2 つの入口を呼ぶ。`x_len` は渡す列の長さで、`n` とは独立。
    fn call_with(
        exec: &Exec<'_>,
        op: OpId,
        over: Option<OptIn>,
        n: u64,
        m: u64,
        x_len: usize,
    ) -> Result<Outcome, ContractError> {
        let (x, mut out) = (vec![1.0f32; x_len], [0.0f32; 2]);
        match op {
            OpId::SubgroupSumF32 => exec.subgroup_sum_f32(
                over,
                n,
                SubgroupSumF32Operands::Host {
                    x: &x,
                    out: &mut out,
                },
            ),
            _ => exec.atomic_add(
                over,
                n,
                m,
                AtomicAddOperands::Host {
                    x: &x,
                    out: &mut out[..1],
                },
            ),
        }
    }

    fn is_invalid(r: &Result<Outcome, ContractError>) -> bool {
        matches!(r, Err(ContractError::InvalidArgument(_)))
    }

    #[test]
    fn rejected_calls_do_not_run_judgment() {
        // (n, m, 列の長さ)。FB-60 の拒否の入力。
        let cases: [(u64, u64, usize); 5] = [
            (0, 1, 0),
            (8, 0, 8),
            (u64::MAX, 1, 1),
            (OVER_LIMIT, 1, 1),
            (1, OVER_LIMIT, 1),
        ];
        for dev in devices() {
            for default in DEFAULTS {
                for over in OVERRIDES {
                    let log = CallLog::default();
                    let runner = preserving_runner(&log);
                    let exec = Exec::new(&dev, default).with_probe(&runner);
                    for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
                        for (n, m, len) in cases {
                            // `m` は `atomic_add` だけの引数なので、m だけが不正な組は総和には当てない。
                            if op == OpId::SubgroupSumF32 && m != 1 {
                                continue;
                            }
                            let before = log.borrow().len();
                            let r = call_with(&exec, op, over, n, m, len);
                            assert!(is_invalid(&r), "{op:?} n={n} m={m}: {r:?}");
                            assert_eq!(log.borrow().len(), before);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rejection_does_not_consume_judgment() {
        for dev in devices() {
            for default in DEFAULTS {
                for over in OVERRIDES {
                    for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
                        for query_first in [false, true] {
                            let log = CallLog::default();
                            let runner = preserving_runner(&log);
                            let exec = Exec::new(&dev, default).with_probe(&runner);
                            let r = call_with(&exec, op, over, 0, 0, 0);
                            assert!(is_invalid(&r));
                            assert_eq!(log.borrow().len(), 0);
                            // 効く印と device から、経路が届くかを試験の側で求める。
                            let eff = over.unwrap_or(default);
                            let reachable = dev.query.level == CapabilityLevel::S2
                                || eff.allow_gpu_emulated
                                || eff.allow_cpu;
                            if query_first {
                                let r = exec.query_route(op, over);
                                assert!(!matches!(r, Err(ContractError::InvalidArgument(_))));
                            } else {
                                assert!(!is_invalid(&call_entry(&exec, op, over)));
                            }
                            assert_eq!(log.borrow().len(), usize::from(reachable));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn calls_without_route_do_not_run_judgment() {
        for query in [S0, S1] {
            let dev = QueryDevice::new(BackendKind::Metal, query);
            // 上書きで両方拒否する場合と、上書き無しで既定の印が全拒否の場合。
            for over in [Some(NONE), None] {
                for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
                    let log = CallLog::default();
                    let runner = preserving_runner(&log);
                    let exec = Exec::deny_by_default(&dev).with_probe(&runner);
                    let r = call_entry(&exec, op, over);
                    assert!(matches!(r, Ok(Outcome::Unavailable { .. })), "{r:?}");
                    assert_eq!(log.borrow().len(), 0);
                    let report = exec.query_route(op, over).expect("report");
                    assert_eq!(report.route, None);
                    assert_eq!(log.borrow().len(), 0);
                }
            }
        }
    }

    #[cfg(feature = "fault-injection")]
    #[test]
    fn forced_calls_without_opt_in_do_not_run_judgment() {
        let dev = QueryDevice::new(BackendKind::Metal, S2);
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            let log = CallLog::default();
            let runner = preserving_runner(&log);
            let exec = Exec::deny_by_default(&dev)
                .with_probe(&runner)
                .force_unsupported(op);
            let r = call_entry(&exec, op, Some(NONE));
            assert!(matches!(r, Ok(Outcome::Unavailable { .. })), "{r:?}");
            assert_eq!(log.borrow().len(), 0);
            // 同じ Exec で opt-in を許すと、外した Native の代わりの経路で 1 回だけ流れる。
            let report = exec.query_route(op, Some(G_ONLY)).expect("report");
            assert_eq!(report.route, Some(Route::GpuEmulated));
            assert_eq!(calls_of(&log, op, Route::GpuEmulated), 1);
            assert_eq!(calls_of(&log, op, Route::Native), 0);
        }
    }

    #[test]
    fn f16_and_u32_never_run_judgment() {
        let (a, mut o16) = ([1u16; 2], [0u16; 2]);
        let (xu, mut ou) = ([1u32; 2], [0u32; 1]);
        for dev in devices() {
            for default in DEFAULTS {
                for over in OVERRIDES {
                    let log = CallLog::default();
                    let runner = preserving_runner(&log);
                    let exec = Exec::new(&dev, default).with_probe(&runner);
                    let _ = exec.f16_fma(
                        over,
                        2,
                        F16FmaOperands::Host {
                            a: &a,
                            b: &a,
                            c: &a,
                            out: &mut o16,
                        },
                    );
                    let _ = exec.subgroup_sum_u32(
                        over,
                        2,
                        SubgroupSumU32Operands::Host {
                            x: &xu,
                            out: &mut ou,
                        },
                    );
                    let _ = exec.query_route(OpId::F16Fma, over);
                    let _ = exec.query_route(OpId::SubgroupSumU32, over);
                    assert_eq!(log.borrow().len(), 0);
                }
            }
        }
    }
}
/// 試しの実行が契約のエラーで失敗したとき、`query_route` と入口がそのまま `Err` で返し、
/// 保持せず、後の経路にも進まないことの試験。
///
/// 失敗は core の `FaultInjecting<StubDevice>` を模擬の投入に通して作る（下-1 は呼ばない）。
/// 回数は模擬の runner と `submit_call_count()` の、各呼び出しの前後の差で見る。文言は照らさない。
mod failure {
    use super::*;
    use fandhe_silicon_core::fault::decorator::{FaultInjecting, FaultScript};

    const OPS: [OpId; 2] = [OpId::SubgroupSumF32, OpId::AtomicAdd];
    const OVERRIDES: [Option<OptIn>; 2] = [None, Some(BOTH)];

    fn fault(script: FaultScript) -> FaultInjecting<StubDevice> {
        FaultInjecting::new(
            StubDevice::new(DeviceRequest {
                backend: BackendKind::Metal,
                profile: Profile::ComputeOnly,
                ownership: DeviceOwnership::Shared,
            }),
            script,
        )
    }

    /// 確保・投入・完了待ちを順に呼び、最初の `Err` をそのまま返す。
    fn simulate(f: &FaultInjecting<StubDevice>) -> Result<(), ContractError> {
        f.allocate_with(crate::denormal::INPUT_LEN as u64 * 4, |_, _| Ok(()))?;
        f.submit_with(|_| Ok(()))?;
        f.wait_blocking_with(|_| Ok(()))
    }

    fn expected(script: &FaultScript) -> ContractError {
        simulate(&fault(script.clone())).unwrap_err()
    }

    fn scripts() -> [FaultScript; 3] {
        [
            FaultScript::AllocationFails,
            FaultScript::RuntimeError {
                backend: BackendKind::Metal,
                detail: "injected".into(),
            },
            FaultScript::CompletionUnknown,
        ]
    }

    fn preserved() -> Option<F32DenormalReport> {
        Some(F32DenormalReport {
            behavior: F32DenormalBehavior::Preserve,
            basis: F32DenormalBasis::Observed,
            documented: None,
        })
    }

    /// 呼ばれた (操作, 経路) を記録し、模擬の投入が通れば保つときの出力を返す。
    fn faulting_runner<'a>(
        f: &'a FaultInjecting<StubDevice>,
        log: &'a CallLog,
    ) -> impl Fn(OpId, Route, &ProbeInput) -> Result<[f32; 3], ContractError> + 'a {
        move |op, route, input| {
            log.borrow_mut().push((op, route));
            simulate(f)?;
            Ok(preserving(op, input))
        }
    }

    #[test]
    fn injected_failures_are_returned() {
        // FB-60: 試しの実行の失敗は戻り値の `Err` で返る。
        let dev = QueryDevice::new(BackendKind::Metal, S2);
        for script in scripts() {
            let submits = u64::from(!matches!(script, FaultScript::AllocationFails));
            for op in OPS {
                for over in OVERRIDES {
                    let f = fault(script.clone());
                    let log = CallLog::default();
                    let runner = faulting_runner(&f, &log);
                    let exec = Exec::deny_by_default(&dev).with_probe(&runner);

                    let (n0, s0) = (log.borrow().len(), f.submit_call_count());
                    let r = exec.query_route(op, over);
                    assert_eq!(r, Err(expected(&script)));
                    assert_eq!(log.borrow().len(), n0 + 1);
                    assert_eq!(f.submit_call_count(), s0 + submits);

                    let (n1, s1) = (log.borrow().len(), f.submit_call_count());
                    let r = call_entry(&exec, op, over);
                    assert!(matches!(r, Err(ref e) if *e == expected(&script)), "{r:?}");
                    assert_eq!(log.borrow().len(), n1 + 1);
                    assert_eq!(f.submit_call_count(), s1 + submits);

                    assert_eq!(calls_of(&log, op, Route::Native), 2);
                    assert_eq!(calls_of(&log, op, Route::GpuEmulated), 0);
                    assert_eq!(calls_of(&log, op, Route::Cpu), 0);
                }
            }
        }
    }

    #[test]
    fn failure_is_not_kept() {
        let dev = QueryDevice::new(BackendKind::Metal, S2);
        // (a) いつも失敗する runner は、呼ぶたびに同じ `Err` を返して毎回流れる。
        for script in scripts() {
            let f = fault(script.clone());
            let log = CallLog::default();
            let runner = faulting_runner(&f, &log);
            let exec = Exec::deny_by_default(&dev).with_probe(&runner);
            for _ in 0..2 {
                assert_eq!(
                    exec.query_route(OpId::SubgroupSumF32, None),
                    Err(expected(&script))
                );
            }
            assert_eq!(calls_of(&log, OpId::SubgroupSumF32, Route::Native), 2);
        }
        // (b) 1 回目だけ失敗する runner は、2 回目で保つ扱いが報告され、3 回目は流れない。
        for script in scripts() {
            for op in OPS {
                let f = fault(script.clone());
                let log = CallLog::default();
                let runner = |op: OpId, route: Route, input: &ProbeInput| {
                    log.borrow_mut().push((op, route));
                    if log.borrow().len() == 1 {
                        simulate(&f)?;
                    }
                    Ok(preserving(op, input))
                };
                let exec = Exec::deny_by_default(&dev).with_probe(&runner);
                assert_eq!(exec.query_route(op, None), Err(expected(&script)));
                for _ in 0..2 {
                    let report = exec.query_route(op, None).expect("report");
                    assert_eq!(report.route, Some(Route::Native));
                    assert_eq!(report.f32_denormal, preserved());
                }
                assert_eq!(log.borrow().len(), 2);
            }
        }
    }

    #[test]
    fn failure_after_unknown_route_is_returned() {
        let dev = QueryDevice::new(BackendKind::Metal, S2);
        for script in scripts() {
            for op in OPS {
                let f = fault(script.clone());
                let log = CallLog::default();
                // Native は判定できない値、GpuEmulated は失敗、Cpu は保つ。
                let runner = |op: OpId, route: Route, input: &ProbeInput| {
                    log.borrow_mut().push((op, route));
                    let keep = preserving(op, input);
                    match route {
                        Route::Native => Ok([0.0, 0.0, keep[2]]),
                        Route::GpuEmulated => simulate(&f).map(|()| keep),
                        _ => Ok(keep),
                    }
                };
                let exec = Exec::deny_by_default(&dev).with_probe(&runner);
                for round in 1..=2 {
                    assert_eq!(exec.query_route(op, Some(BOTH)), Err(expected(&script)));
                    assert_eq!(calls_of(&log, op, Route::Native), 1);
                    assert_eq!(calls_of(&log, op, Route::GpuEmulated), round);
                    assert_eq!(calls_of(&log, op, Route::Cpu), 0);
                }
            }
        }
    }

    #[test]
    fn pass_through_reaches_route() {
        let dev = QueryDevice::new(BackendKind::Metal, S2);
        for op in OPS {
            let f = fault(FaultScript::PassThrough);
            let log = CallLog::default();
            let runner = faulting_runner(&f, &log);
            let exec = Exec::deny_by_default(&dev).with_probe(&runner);
            let report = exec.query_route(op, None).expect("report");
            assert_eq!(report.route, Some(Route::Native));
            assert_eq!(report.f32_denormal, preserved());
        }
    }
}
