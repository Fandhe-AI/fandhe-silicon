//! 下-2 の値 [`Exec`] と、その 7 つの入口。
//!
//! 上の層や利用側が device を借りて [`Exec`] を作り、入口を呼ぶ。4 つの入口は
//! 能力を問い合わせて経路の届き方を決め（`select` の関数）、経路なしなら報告つきの
//! `Ok(Outcome::Unavailable { .. })` を、経路が決まれば `Err(ContractError::Unsupported { .. })` を返す。
//! 3 つの入口は能力を問い合わせず `Err(ContractError::Unsupported { .. })` を返す。
//! 4 つの入口は最初に要素数と出力の数を検査し（`length` の関数。`docs/spec/05-tasks.md` の TASK-924）、
//! 不正なら能力を問い合わせず `Err(ContractError::InvalidArgument(..))` を返す。
//! `Exec::query_route` は 4 つの入口と同じ選び方で、入口を呼ぶ前に経路の報告を返す。
//! どの入口も出力の列・buffer に書かず、下-1 の投入・確保・読み出しを呼ばない。

use crate::length::check_lengths;
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

    /// (`op`, `route`) の f32 の非正規化数の判定を `judgment` に差し込んだ `Exec` を返す。
    ///
    /// 差し込みのある組では、経路の選び方が求める判定がその値になり、その値で経路が決まった報告の
    /// `f32_denormal` の `basis` が `F32DenormalBasis::Injected` になる。差し込みの無い組は
    /// 今までどおり入口の判定を使う。同じ組で呼び直すと後の値が効く。
    /// 判定を求めない操作（`F16Fma`・`SubgroupSumU32`）では報告が変わらない。
    /// `force_unsupported` で指定した操作では `Native` に差し込んでも `Native` は選ばれない。
    /// 3 つの入口・既定の印・`query_ex` を呼ぶ回数（1 回）は変わらない。
    #[cfg(feature = "fault-injection")]
    #[must_use]
    pub fn inject_denormal(
        self,
        op: OpId,
        route: Route,
        judgment: crate::report::DenormalInjection,
    ) -> Self {
        Self {
            ports: self.ports.with_injected(op, route, judgment),
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
        // 差し込みのある組ではその値を返し、無い組は呼び手の判定の関数へ渡す。
        #[cfg(feature = "fault-injection")]
        let judge = {
            let ports = self.ports;
            let mut judge = judge;
            move |route: Route| match ports.injected(op, route) {
                Some(j) => Ok(crate::hooks::judgment_of(j)),
                None => judge(route),
            }
        };
        let output = select_route(&input, judge)?;
        let report = build_report(&input, &output);
        #[cfg(feature = "fault-injection")]
        let report = self.ports.mark_injected_basis(report);
        Ok(report)
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
    /// `OpId::SubgroupSumF32`・`OpId::AtomicAdd` で、差し込みの無い経路に届く呼び出しは
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

    /// f16 の融合積和。最初に `n` を検査し、`n` が 0、`n` × 2 が `u64` であふれる、
    /// device の `max_buffer_bytes` を超える、のどれかなら `Err(ContractError::InvalidArgument(..))` を返し
    /// 能力を問い合わせない。`operands` は読まず、出力に書かない。
    pub fn f16_fma(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        operands: F16FmaOperands<'_>,
    ) -> Result<Outcome, ContractError> {
        check_lengths(OpId::F16Fma, n, None, self.device.limits().max_buffer_bytes)?;
        let _ = operands;
        self.run(OpId::F16Fma, opt_in, "f16_fma")
    }

    /// u32 の subgroup 総和。最初に `n` を検査し、`n` が 0、`n` × 4 が `u64` であふれる、
    /// device の `max_buffer_bytes` を超える、のどれかなら `Err(ContractError::InvalidArgument(..))` を返し
    /// 能力を問い合わせない。`operands` は読まず、出力に書かない。
    pub fn subgroup_sum_u32(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        operands: SubgroupSumU32Operands<'_>,
    ) -> Result<Outcome, ContractError> {
        check_lengths(
            OpId::SubgroupSumU32,
            n,
            None,
            self.device.limits().max_buffer_bytes,
        )?;
        let _ = operands;
        self.run(OpId::SubgroupSumU32, opt_in, "subgroup_sum_u32")
    }

    /// f32 の subgroup 総和。最初に `n` を検査し、`n` が 0、`n` × 4 が `u64` であふれる、
    /// device の `max_buffer_bytes` を超える、のどれかなら `Err(ContractError::InvalidArgument(..))` を返し
    /// 能力を問い合わせない。`operands` は読まず、出力に書かない。
    pub fn subgroup_sum_f32(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        operands: SubgroupSumF32Operands<'_>,
    ) -> Result<Outcome, ContractError> {
        check_lengths(
            OpId::SubgroupSumF32,
            n,
            None,
            self.device.limits().max_buffer_bytes,
        )?;
        let _ = operands;
        self.run(OpId::SubgroupSumF32, opt_in, "subgroup_sum_f32")
    }

    /// atomic の f32 の加算。最初に `n`・`m` を検査し、`n` または `m` が 0、`n` × 4 または `m` × 4 が
    /// `u64` であふれる、device の `max_buffer_bytes` を超える、のどれかなら
    /// `Err(ContractError::InvalidArgument(..))` を返し能力を問い合わせない。`operands` は読まず、出力に書かない。
    pub fn atomic_add(
        &self,
        opt_in: Option<OptIn>,
        n: u64,
        m: u64,
        operands: AtomicAddOperands<'_>,
    ) -> Result<Outcome, ContractError> {
        check_lengths(
            OpId::AtomicAdd,
            n,
            Some(m),
            self.device.limits().max_buffer_bytes,
        )?;
        let _ = operands;
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

    #[test]
    fn entries_reject_lengths() {
        let dev = stub();
        let exec = Exec::deny_by_default(&dev);
        let (mut o16, mut ou, mut of, mut oa) = ([7u16; 1], [7u32; 1], [7f32; 1], [7f32; 1]);
        let (a, x, y) = ([1u16; 1], [1u32; 1], [1f32; 1]);
        let (mut e16, mut eu, mut ef, mut ea) = ([0u16; 0], [0u32; 0], [0f32; 0], [0f32; 0]);
        let bad =
            |r: Result<Outcome, ContractError>| matches!(r, Err(ContractError::InvalidArgument(_)));
        let h16 = (1u64 << 27) + 1;
        let hw = (1u64 << 26) + 1;
        // n = 0
        assert!(bad(exec.f16_fma(
            None,
            0,
            F16FmaOperands::Host {
                a: &[],
                b: &[],
                c: &[],
                out: &mut e16
            }
        )));
        assert!(bad(exec.subgroup_sum_u32(
            None,
            0,
            SubgroupSumU32Operands::Host {
                x: &[],
                out: &mut eu
            }
        )));
        assert!(bad(exec.subgroup_sum_f32(
            None,
            0,
            SubgroupSumF32Operands::Host {
                x: &[],
                out: &mut ef
            }
        )));
        assert!(bad(exec.atomic_add(
            None,
            0,
            1,
            AtomicAddOperands::Host {
                x: &[],
                out: &mut ea
            }
        )));
        // m = 0
        let (x8, mut o8) = ([1f32; 8], [7f32; 8]);
        assert!(bad(exec.atomic_add(
            None,
            8,
            0,
            AtomicAddOperands::Host {
                x: &x8,
                out: &mut o8
            }
        )));
        // n = u64::MAX、上限超え
        for (n16, nw) in [(u64::MAX, u64::MAX), (h16, hw)] {
            assert!(bad(exec.f16_fma(
                None,
                n16,
                F16FmaOperands::Host {
                    a: &a,
                    b: &a,
                    c: &a,
                    out: &mut o16
                }
            )));
            assert!(bad(exec.subgroup_sum_u32(
                None,
                nw,
                SubgroupSumU32Operands::Host {
                    x: &x,
                    out: &mut ou
                }
            )));
            assert!(bad(exec.subgroup_sum_f32(
                None,
                nw,
                SubgroupSumF32Operands::Host {
                    x: &y,
                    out: &mut of
                }
            )));
            assert!(bad(exec.atomic_add(
                None,
                nw,
                1,
                AtomicAddOperands::Host {
                    x: &y,
                    out: &mut oa
                }
            )));
        }
        assert!(bad(exec.atomic_add(
            None,
            1,
            hw,
            AtomicAddOperands::Host {
                x: &y,
                out: &mut oa
            }
        )));
    }
}

/// `Exec::force_unsupported` を `Exec` 越しに確かめる試験。feature `fault-injection` の下だけ。
#[cfg(all(test, feature = "fault-injection"))]
mod forced_tests {
    use super::*;
    use crate::route::UnavailableReason;
    use fandhe_silicon_core::capability::{BackendKind, CapabilityLevel};
    use fandhe_silicon_core::device::{DeviceOwnership, DeviceRequest, Profile};
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

    const OPS: [OpId; 4] = [
        OpId::F16Fma,
        OpId::SubgroupSumU32,
        OpId::SubgroupSumF32,
        OpId::AtomicAdd,
    ];

    #[test]
    fn forced_entry_reports_actual_level() {
        let dev = stub();
        let plain = Exec::deny_by_default(&dev);
        let exec = Exec::deny_by_default(&dev).force_unsupported(OpId::F16Fma);
        let (mut o16, a) = ([7u16; 2], [1u16; 2]);
        let r = exec.f16_fma(
            None,
            2,
            F16FmaOperands::Host {
                a: &a,
                b: &a,
                c: &a,
                out: &mut o16,
            },
        );
        let Ok(Outcome::Unavailable { report }) = r else {
            panic!("expected Unavailable");
        };
        assert_eq!(
            report.native_reason,
            Some(UnavailableReason::ForcedUnsupported)
        );
        assert_eq!(report.observed_level, Some(CapabilityLevel::Unsupported));
        assert_eq!(o16, [7; 2]);
        assert_eq!(exec.query_route(OpId::F16Fma, None).ok(), Some(report));
        for op in &OPS[1..] {
            assert_eq!(
                exec.query_route(*op, None).ok(),
                plain.query_route(*op, None).ok()
            );
        }
    }

    #[test]
    fn force_does_not_touch_three_entries() {
        let dev = stub();
        for o in ALL_OPT.into_iter().map(Some).chain([None]) {
            let exec = OPS
                .into_iter()
                .fold(Exec::deny_by_default(&dev), |e, op| e.force_unsupported(op));
            for (r, name) in [
                (
                    exec.external_interop_shared_memory(o),
                    "external_interop_shared_memory",
                ),
                (exec.hardware_ray_tracing(o), "hardware_ray_tracing"),
                (exec.unified_memory_sharing(o), "unified_memory_sharing"),
            ] {
                let Err(ContractError::Unsupported { op, .. }) = r else {
                    panic!("expected Unsupported");
                };
                assert_eq!(op, name);
            }
        }
    }

    #[test]
    fn force_keeps_default_opt_in() {
        let dev = stub();
        for o in ALL_OPT {
            let exec = OPS
                .into_iter()
                .fold(Exec::new(&dev, o), |e, op| e.force_unsupported(op));
            assert_eq!(exec.default_opt_in(), o);
        }
    }
}
/// `Exec::inject_denormal` を `Exec` 越しに確かめる試験。feature `fault-injection` の下だけ。
#[cfg(all(test, feature = "fault-injection"))]
mod injected_tests {
    use super::*;
    use crate::report::{
        DenormalInjection as Inj, F32DenormalBasis, F32DenormalBehavior, F32DenormalReport,
    };
    use crate::route::UnavailableReason;
    use fandhe_silicon_core::capability::BackendKind;
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

    const ROUTES: [Route; 3] = [Route::Native, Route::GpuEmulated, Route::Cpu];

    fn injected(behavior: F32DenormalBehavior) -> Option<F32DenormalReport> {
        Some(F32DenormalReport {
            behavior,
            basis: F32DenormalBasis::Injected,
            documented: None,
        })
    }

    #[test]
    fn injected_fallback_route_is_reported() {
        let dev = stub();
        let exec = Exec::new(&dev, opt(true, false)).inject_denormal(
            OpId::SubgroupSumF32,
            Route::GpuEmulated,
            Inj::FlushToZero,
        );
        let report = exec.query_route(OpId::SubgroupSumF32, None).unwrap();
        assert_eq!(report.route, Some(Route::GpuEmulated));
        assert_eq!(
            report.f32_denormal,
            injected(F32DenormalBehavior::FlushToZero)
        );
        assert_eq!(
            report.native_reason,
            Some(UnavailableReason::CapabilityUnsupported(Some(
                UnsupportedReason::UnknownFeature
            )))
        );
        let mut out = [7.0f32; 2];
        let x = [1.0f32; 2];
        let r = exec.subgroup_sum_f32(
            None,
            2,
            SubgroupSumF32Operands::Host {
                x: &x,
                out: &mut out,
            },
        );
        assert!(matches!(
            r,
            Err(ContractError::Unsupported {
                op: "subgroup_sum_f32",
                ..
            })
        ));
        assert_eq!(out, [7.0; 2]);
    }

    #[test]
    fn injected_unknown_moves_to_next_route() {
        let dev = stub();
        let exec = Exec::new(&dev, opt(true, true))
            .inject_denormal(OpId::AtomicAdd, Route::GpuEmulated, Inj::Unknown)
            .inject_denormal(OpId::AtomicAdd, Route::Cpu, Inj::Preserve);
        let report = exec.query_route(OpId::AtomicAdd, None).unwrap();
        assert_eq!(report.route, Some(Route::Cpu));
        assert_eq!(report.f32_denormal, injected(F32DenormalBehavior::Preserve));
    }

    #[test]
    fn all_injected_unknown_is_unavailable() {
        let dev = stub();
        let exec = Exec::new(&dev, opt(true, true))
            .inject_denormal(OpId::AtomicAdd, Route::GpuEmulated, Inj::Unknown)
            .inject_denormal(OpId::AtomicAdd, Route::Cpu, Inj::Unknown);
        let report = exec.query_route(OpId::AtomicAdd, None).unwrap();
        assert_eq!(report.route, None);
        assert_eq!(
            report.fallback_reason,
            Some(UnavailableReason::F32DenormalBehaviorUnknown)
        );
        assert_eq!(report.f32_denormal, None);
        let mut out = [7.0f32; 2];
        let x = [1.0f32; 2];
        let r = exec.atomic_add(
            None,
            2,
            1,
            AtomicAddOperands::Host {
                x: &x,
                out: &mut out,
            },
        );
        assert!(matches!(r, Ok(Outcome::Unavailable { report: got }) if got == report));
        assert_eq!(out, [7.0; 2]);
    }

    #[test]
    fn uninjected_route_keeps_unsupported_error() {
        let dev = stub();
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            let exec = Exec::new(&dev, opt(true, true)).inject_denormal(
                op,
                Route::GpuEmulated,
                Inj::Unknown,
            );
            assert!(matches!(
                exec.query_route(op, None),
                Err(ContractError::Unsupported {
                    op: "query_route",
                    ..
                })
            ));
        }
    }

    #[test]
    fn injection_has_no_effect_without_judgment() {
        let dev = stub();
        for op in [OpId::F16Fma, OpId::SubgroupSumU32] {
            let with = ROUTES
                .into_iter()
                .fold(Exec::deny_by_default(&dev), |e, r| {
                    e.inject_denormal(op, r, Inj::FlushToZero)
                });
            let plain = Exec::deny_by_default(&dev);
            for o in ALL_OPT.into_iter().map(Some).chain([None]) {
                assert_eq!(with.query_route(op, o).ok(), plain.query_route(op, o).ok());
            }
        }
    }

    #[test]
    fn last_injection_wins() {
        let dev = stub();
        for (a, b, expected) in [
            (
                Inj::Preserve,
                Inj::FlushToZero,
                F32DenormalBehavior::FlushToZero,
            ),
            (
                Inj::FlushToZero,
                Inj::Preserve,
                F32DenormalBehavior::Preserve,
            ),
        ] {
            let exec = Exec::new(&dev, opt(true, false))
                .inject_denormal(OpId::AtomicAdd, Route::GpuEmulated, a)
                .inject_denormal(OpId::AtomicAdd, Route::GpuEmulated, b);
            let report = exec.query_route(OpId::AtomicAdd, None).unwrap();
            assert_eq!(report.f32_denormal, injected(expected));
        }
    }

    #[test]
    fn injection_with_force() {
        let dev = stub();
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            let build = |o| {
                Exec::new(&dev, o)
                    .force_unsupported(op)
                    .inject_denormal(op, Route::Native, Inj::Preserve)
                    .inject_denormal(op, Route::GpuEmulated, Inj::FlushToZero)
            };
            let report = build(opt(true, false)).query_route(op, None).unwrap();
            assert_eq!(report.route, Some(Route::GpuEmulated));
            assert_eq!(
                report.native_reason,
                Some(UnavailableReason::ForcedUnsupported)
            );
            assert_eq!(
                report.f32_denormal,
                injected(F32DenormalBehavior::FlushToZero)
            );
            let none = build(opt(false, false)).query_route(op, None).unwrap();
            assert_eq!(none.route, None);
            assert_eq!(none.fallback_reason, Some(UnavailableReason::OptInNotGiven));
            assert_eq!(none.f32_denormal, None);
        }
    }

    #[test]
    fn injection_keeps_default_opt_in() {
        let dev = stub();
        for o in ALL_OPT {
            let exec =
                Exec::new(&dev, o).inject_denormal(OpId::AtomicAdd, Route::Cpu, Inj::Preserve);
            assert_eq!(exec.default_opt_in(), o);
        }
    }
}
