//! 下-2 の値 [`Exec`] と、その 7 つの入口。
//!
//! 上の層や利用側が device を借りて [`Exec`] を作り、入口を呼ぶ。4 つの入口は
//! 能力を問い合わせて経路の届き方を決め（`select` の関数）、経路なしなら報告つきの
//! `Ok(Outcome::Unavailable { .. })` を、経路が決まれば `Err(ContractError::Unsupported { .. })` を返す。
//! 3 つの入口は能力を問い合わせず `Err(ContractError::Unsupported { .. })` を返す。
//! `Exec::query_route` は 4 つの入口と同じ選び方で、入口を呼ぶ前に経路の報告を返す。
//! どの入口も出力の列・buffer に書かず、下-1 の投入・確保・読み出しを呼ばない。

use crate::operands::{
    AtomicAddOperands, F16FmaOperands, SubgroupSumF32Operands, SubgroupSumU32Operands,
};
use crate::opt_in::OptIn;
use crate::report::{Outcome, RouteReport};
use crate::route::{OpId, Route};
use crate::select::{
    DenormalJudgment, build_report, effective_opt_in, select_route, selection_input,
};
use fandhe_silicon_core::capability::FeatureId;
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::feature_key::{
    FEATURE_SHADER_F16, FEATURE_SHADER_FLOAT32_ATOMIC, FEATURE_SUBGROUP,
};

/// 操作に対応する機能の問合せの鍵。
fn feature_of(op: OpId) -> FeatureId {
    match op {
        OpId::F16Fma => FEATURE_SHADER_F16,
        OpId::SubgroupSumU32 | OpId::SubgroupSumF32 => FEATURE_SUBGROUP,
        OpId::AtomicAdd => FEATURE_SHADER_FLOAT32_ATOMIC,
    }
}

/// device を借りて既定の opt-in の印を持つ下-2 の値。FB-14。
///
/// 既定の印は作るときに決まり、作った後に変える関数は無い。入口の `opt_in` に
/// `Some` を渡すと、その呼び出しだけ既定を丸ごと置き換える。
pub struct Exec<'d> {
    device: &'d dyn Device,
    default_opt_in: OptIn,
    /// 操作ごとの試験用の印。
    #[cfg(feature = "fault-injection")]
    ports: crate::hooks::Ports,
}

impl<'d> Exec<'d> {
    /// device と既定の印から作る。
    pub fn new(device: &'d dyn Device, default_opt_in: OptIn) -> Self {
        Self {
            device,
            default_opt_in,
            #[cfg(feature = "fault-injection")]
            ports: crate::hooks::Ports::default(),
        }
    }

    /// 既定の印を `OptIn::default()`（どちらも許さない）にして作る。
    pub fn deny_by_default(device: &'d dyn Device) -> Self {
        Self::new(device, OptIn::default())
    }

    /// `op` で `Native` を候補に入れない `Exec` を返す。
    ///
    /// 指定した操作では能力の段階を見ず、報告の `native_reason` が
    /// `UnavailableReason::ForcedUnsupported` になる。`query_ex` は今までどおり 1 回呼び、
    /// 報告の `observed_level` はその値のまま。`GpuEmulated`・`Cpu` は効く印が許すときだけ候補に入る。
    /// 指定していない操作・3 つの入口・既定の印は変わらない。何度呼んでも指定は足し合わさる。
    #[cfg(feature = "fault-injection")]
    #[must_use]
    pub fn force_unsupported(self, op: OpId) -> Self {
        Self {
            ports: self.ports.with_forced(op),
            ..self
        }
    }

    /// 作るときに決めた既定の印を返す。
    pub fn default_opt_in(&self) -> OptIn {
        self.default_opt_in
    }

    /// 効く印・`query_ex` の 1 回・選び方・報告の組み立てを通す、`run` と `query_route` の共通の処理。
    ///
    /// 判定の関数は経路の選び方が判定を求めたときに呼ばれ、その `Err` はそのまま返る。
    fn route_report(
        &self,
        op: OpId,
        opt_in: Option<OptIn>,
        judge: impl FnMut(Route) -> Result<DenormalJudgment, ContractError>,
    ) -> Result<RouteReport, ContractError> {
        let effective = effective_opt_in(self.default_opt_in, opt_in);
        let query = self.device.query_ex(feature_of(op));
        let input = selection_input(op, effective, query);
        #[cfg(feature = "fault-injection")]
        let input = crate::select::SelectionInput {
            forced: self.ports.is_forced(op),
            ..input
        };
        let output = select_route(&input, judge)?;
        Ok(build_report(&input, &output))
    }

    /// 能力を問い合わせて経路を決め、報告か `Unsupported` を返す共通の処理。
    ///
    /// `route_report` を通し、ここでは入口の関数名の `Unsupported` を判定の関数の `Err` に使う。
    fn run(
        &self,
        op: OpId,
        opt_in: Option<OptIn>,
        name: &'static str,
    ) -> Result<Outcome, ContractError> {
        let report = self.route_report(op, opt_in, |_: Route| Err(self.unsupported(name)))?;
        if report.route.is_none() {
            return Ok(Outcome::Unavailable { report });
        }
        Err(self.unsupported(name))
    }

    /// 入口を呼ぶ前に、同じ選び方で経路の報告を返す。FB-24。
    ///
    /// 同じ device の値・同じ操作・同じ `opt_in` で入口を呼んだときと同じ報告を返す
    /// （入口が `Ok(Outcome::Unavailable { report })` を返す呼び出しでは `report` と等しい）。
    /// 経路が `None` の報告も、経路がある報告も `Ok`。
    /// `OpId::SubgroupSumF32`・`OpId::AtomicAdd` で経路が届く呼び出しは
    /// `Err(ContractError::Unsupported { op: "query_route", backend })` を返す
    /// （`backend` は `Device::backend()` の値）。
    /// `query_ex` を 1 回だけ呼び、下-1 の投入・確保・読み出しを呼ばない。
    pub fn query_route(
        &self,
        op: OpId,
        opt_in: Option<OptIn>,
    ) -> Result<RouteReport, ContractError> {
        self.route_report(op, opt_in, |_: Route| Err(self.unsupported("query_route")))
    }

    fn unsupported(&self, op: &'static str) -> ContractError {
        ContractError::Unsupported {
            op,
            backend: self.device.backend(),
        }
    }

    /// f16 の融合積和。`n` と `operands` は読まず、出力に書かない。
    pub fn f16_fma(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        operands: F16FmaOperands<'_>,
    ) -> Result<Outcome, ContractError> {
        let _ = (n, operands);
        self.run(OpId::F16Fma, opt_in, "f16_fma")
    }

    /// u32 の subgroup 総和。`n` と `operands` は読まず、出力に書かない。
    pub fn subgroup_sum_u32(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        operands: SubgroupSumU32Operands<'_>,
    ) -> Result<Outcome, ContractError> {
        let _ = (n, operands);
        self.run(OpId::SubgroupSumU32, opt_in, "subgroup_sum_u32")
    }

    /// f32 の subgroup 総和。`n` と `operands` は読まず、出力に書かない。
    pub fn subgroup_sum_f32(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        operands: SubgroupSumF32Operands<'_>,
    ) -> Result<Outcome, ContractError> {
        let _ = (n, operands);
        self.run(OpId::SubgroupSumF32, opt_in, "subgroup_sum_f32")
    }

    /// atomic の f32 の加算。`n`・`m` と `operands` は読まず、出力に書かない。
    pub fn atomic_add(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        m: u64,
        operands: AtomicAddOperands<'_>,
    ) -> Result<Outcome, ContractError> {
        let _ = (n, m, operands);
        self.run(OpId::AtomicAdd, opt_in, "atomic_add")
    }

    /// 外部との共有メモリ。能力を問い合わせず `Err(ContractError::Unsupported { .. })` を返す。
    pub fn external_interop_shared_memory(
        &self,
        opt_in: Option<OptIn>,
    ) -> Result<Outcome, ContractError> {
        let _ = opt_in;
        Err(self.unsupported("external_interop_shared_memory"))
    }

    /// ハードウェアのレイトレーシング。能力を問い合わせず `Err(ContractError::Unsupported { .. })` を返す。
    pub fn hardware_ray_tracing(&self, opt_in: Option<OptIn>) -> Result<Outcome, ContractError> {
        let _ = opt_in;
        Err(self.unsupported("hardware_ray_tracing"))
    }

    /// 統合メモリの共有。能力を問い合わせず `Err(ContractError::Unsupported { .. })` を返す。
    pub fn unified_memory_sharing(&self, opt_in: Option<OptIn>) -> Result<Outcome, ContractError> {
        let _ = opt_in;
        Err(self.unsupported("unified_memory_sharing"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::UnavailableReason;
    use fandhe_silicon_core::capability::{BackendKind, Capabilities, CapabilityLevel};
    use fandhe_silicon_core::device::{DeviceOwnership, DeviceRequest, Profile};
    use fandhe_silicon_core::error::UnsupportedReason;
    use fandhe_silicon_core::stub_device::StubDevice;

    fn stub() -> StubDevice {
        StubDevice::new(DeviceRequest {
            backend: BackendKind::Metal,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::Shared,
        })
    }

    const fn opt(g: bool, c: bool) -> OptIn {
        OptIn {
            allow_gpu_emulated: g,
            allow_cpu: c,
        }
    }

    const ALL_OPT: [OptIn; 4] = [
        opt(false, false),
        opt(true, false),
        opt(false, true),
        opt(true, true),
    ];

    fn expect_unavailable(r: Result<Outcome, ContractError>, op: OpId) {
        let Ok(Outcome::Unavailable { report }) = r else {
            panic!("expected Unavailable");
        };
        assert_eq!(
            report,
            RouteReport {
                op,
                route: None,
                native_reason: Some(UnavailableReason::CapabilityUnsupported(Some(
                    UnsupportedReason::UnknownFeature
                ))),
                fallback_reason: Some(UnavailableReason::OptInNotGiven),
                observed_level: Some(CapabilityLevel::Unsupported),
                f32_denormal: None,
            }
        );
    }

    fn is_unsupported(r: &Result<Outcome, ContractError>) -> bool {
        matches!(r, Err(ContractError::Unsupported { .. }))
    }

    #[test]
    fn entry_input_carries_effective_opt_in() {
        let default = opt(true, false);
        let over = opt(false, true);
        let q = stub().query_ex(FEATURE_SUBGROUP);
        for call in [None, Some(over)] {
            let input = selection_input(OpId::SubgroupSumU32, effective_opt_in(default, call), q);
            assert_eq!(input.opt_in, call.unwrap_or(default));
        }
    }

    #[test]
    fn four_entries_without_override_are_unavailable() {
        let dev = stub();
        let exec = Exec::deny_by_default(&dev);
        let (mut o16, mut ou, mut of, mut oa) = ([7u16; 2], [7u32; 2], [7f32; 2], [7f32; 2]);
        let (a, x, y) = ([1u16; 2], [1u32; 2], [1f32; 2]);
        expect_unavailable(
            exec.f16_fma(
                None,
                2,
                F16FmaOperands::Host {
                    a: &a,
                    b: &a,
                    c: &a,
                    out: &mut o16,
                },
            ),
            OpId::F16Fma,
        );
        expect_unavailable(
            exec.subgroup_sum_u32(
                None,
                2,
                SubgroupSumU32Operands::Host {
                    x: &x,
                    out: &mut ou,
                },
            ),
            OpId::SubgroupSumU32,
        );
        expect_unavailable(
            exec.subgroup_sum_f32(
                None,
                2,
                SubgroupSumF32Operands::Host {
                    x: &y,
                    out: &mut of,
                },
            ),
            OpId::SubgroupSumF32,
        );
        expect_unavailable(
            exec.atomic_add(
                None,
                2,
                1,
                AtomicAddOperands::Host {
                    x: &y,
                    out: &mut oa,
                },
            ),
            OpId::AtomicAdd,
        );
        assert_eq!((o16, ou, of, oa), ([7; 2], [7; 2], [7.0; 2], [7.0; 2]));
    }

    #[test]
    fn four_entries_with_cpu_override_are_unsupported_err() {
        let dev = stub();
        let exec = Exec::deny_by_default(&dev);
        let over = Some(opt(false, true));
        let (mut o16, mut ou, mut of, mut oa) = ([7u16; 2], [7u32; 2], [7f32; 2], [7f32; 2]);
        let (a, x, y) = ([1u16; 2], [1u32; 2], [1f32; 2]);
        assert!(is_unsupported(&exec.f16_fma(
            over,
            2,
            F16FmaOperands::Host {
                a: &a,
                b: &a,
                c: &a,
                out: &mut o16,
            },
        )));
        assert!(is_unsupported(&exec.subgroup_sum_u32(
            over,
            2,
            SubgroupSumU32Operands::Host {
                x: &x,
                out: &mut ou
            },
        )));
        assert!(is_unsupported(&exec.subgroup_sum_f32(
            over,
            2,
            SubgroupSumF32Operands::Host {
                x: &y,
                out: &mut of
            },
        )));
        assert!(is_unsupported(&exec.atomic_add(
            over,
            2,
            1,
            AtomicAddOperands::Host {
                x: &y,
                out: &mut oa
            },
        )));
        assert_eq!((o16, ou, of, oa), ([7; 2], [7; 2], [7.0; 2], [7.0; 2]));
    }

    #[test]
    fn three_entries_are_unsupported_for_any_opt_in() {
        let dev = stub();
        let exec = Exec::deny_by_default(&dev);
        for o in ALL_OPT.into_iter().map(Some).chain([None]) {
            assert!(is_unsupported(&exec.external_interop_shared_memory(o)));
            assert!(is_unsupported(&exec.hardware_ray_tracing(o)));
            assert!(is_unsupported(&exec.unified_memory_sharing(o)));
        }
    }

    #[test]
    fn query_route_equals_unavailable_report() {
        let dev = stub();
        let exec = Exec::deny_by_default(&dev);
        let (mut o16, mut ou, mut of, mut oa) = ([7u16; 2], [7u32; 2], [7f32; 2], [7f32; 2]);
        let (a, x, y) = ([1u16; 2], [1u32; 2], [1f32; 2]);
        let outs = [
            (
                OpId::F16Fma,
                exec.f16_fma(
                    None,
                    2,
                    F16FmaOperands::Host {
                        a: &a,
                        b: &a,
                        c: &a,
                        out: &mut o16,
                    },
                ),
            ),
            (
                OpId::SubgroupSumU32,
                exec.subgroup_sum_u32(
                    None,
                    2,
                    SubgroupSumU32Operands::Host {
                        x: &x,
                        out: &mut ou,
                    },
                ),
            ),
            (
                OpId::SubgroupSumF32,
                exec.subgroup_sum_f32(
                    None,
                    2,
                    SubgroupSumF32Operands::Host {
                        x: &y,
                        out: &mut of,
                    },
                ),
            ),
            (
                OpId::AtomicAdd,
                exec.atomic_add(
                    None,
                    2,
                    1,
                    AtomicAddOperands::Host {
                        x: &y,
                        out: &mut oa,
                    },
                ),
            ),
        ];
        for (op, entry) in outs {
            let Ok(Outcome::Unavailable { report }) = entry else {
                panic!("expected Unavailable");
            };
            let Ok(r) = exec.query_route(op, None) else {
                panic!("expected Ok");
            };
            assert_eq!(r, report);
        }
    }

    #[test]
    fn query_route_reports_fallback_route() {
        let dev = stub();
        let exec = Exec::deny_by_default(&dev);
        let over = Some(opt(false, true));
        for op in [OpId::F16Fma, OpId::SubgroupSumU32] {
            let Ok(r) = exec.query_route(op, over) else {
                panic!("expected Ok");
            };
            assert_eq!(r.route, Some(Route::Cpu));
        }
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            let Err(ContractError::Unsupported { op: name, .. }) = exec.query_route(op, over)
            else {
                panic!("expected Unsupported");
            };
            assert_eq!(name, "query_route");
        }
    }

    #[test]
    fn query_route_keeps_default_opt_in() {
        let dev = stub();
        let ops = [
            OpId::F16Fma,
            OpId::SubgroupSumU32,
            OpId::SubgroupSumF32,
            OpId::AtomicAdd,
        ];
        for o in ALL_OPT {
            let exec = Exec::new(&dev, o);
            for op in ops {
                for over in ALL_OPT.into_iter().map(Some).chain([None]) {
                    let _ = exec.query_route(op, over);
                    assert_eq!(exec.default_opt_in(), o);
                }
            }
        }
    }

    #[test]
    fn default_opt_in_is_kept() {
        let dev = stub();
        for o in ALL_OPT {
            let exec = Exec::new(&dev, o);
            assert_eq!(exec.default_opt_in(), o);
            let _ = exec.hardware_ray_tracing(Some(opt(true, true)));
            assert_eq!(exec.default_opt_in(), o);
        }
        assert_eq!(
            Exec::deny_by_default(&dev).default_opt_in(),
            OptIn::default()
        );
    }
}
