//! `Exec` 越しの f32 の非正規化数の判定の保持の試験と、その試験が使う部品（TASK-925）。
//!
//! 試験用の device・模擬の runner の組み立て・呼ばれた (操作, 経路) の数えは、この module の
//! private な部品として持つ。crate の中の試験だけが `Exec::with_probe` で runner を差す。

use crate::denormal::ProbeInput;
use crate::entry::Exec;
use crate::operands::{AtomicAddOperands, SubgroupSumF32Operands};
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
