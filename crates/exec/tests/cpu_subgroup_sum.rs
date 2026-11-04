//! 下-2 の公開のパスから `subgroup_sum_u32`・`subgroup_sum_f32` を `Route::Cpu`・host の形で呼び、
//! core の入力の組と CPU の参照を比べ、`query_ex`・`memory`・`queue` の数えを確かめる（TASK-931）。
//! 置き場の説明は docs/exec-cpu-route.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::parity::fallback_compare::{
    F32SubnormalHandling, compare_bits_f32, compare_bits_u32, compare_relative_sum_f32,
};
use fandhe_silicon_core::parity::fallback_inputs::{
    FALLBACK_LENGTHS, FallbackInputKind, generate_subgroup_sum_f32_fg,
    generate_subgroup_sum_f32_fs, generate_subgroup_sum_f32_fx, generate_subgroup_sum_u32_u,
};
use fandhe_silicon_core::parity::fallback_reference::{
    reference_subgroup_sum_f32, reference_subgroup_sum_u32, subgroup_sum_output_len,
};
#[cfg(feature = "fault-injection")]
use fandhe_silicon_exec::UnavailableReason;
use fandhe_silicon_exec::{
    Exec, F32DenormalBasis, F32DenormalBehavior, OpId, OptIn, Outcome, Route, RouteReport,
    SubgroupSumF32Operands, SubgroupSumU32Operands,
};

const CPU_ONLY: OptIn = OptIn {
    allow_gpu_emulated: false,
    allow_cpu: true,
};

fn q(level: CapabilityLevel) -> CapabilityQuery {
    CapabilityQuery {
        level,
        unsupported_reason: None,
        evidence: None,
    }
}

/// 経路を `Route::Cpu` に選ばせる組。
#[derive(Clone, Copy)]
enum Mode {
    /// `Exec::new` に `allow_cpu` を立てた印を渡す。上書きは `None`。
    New,
    /// `Exec::deny_by_default` に、呼び出しごとの上書きで `allow_cpu` を立てた印を渡す。
    Override,
    /// `Route::Native` を外す指定を付け、呼び出しごとの上書きで `allow_cpu` を立てた印を渡す。
    #[cfg(feature = "fault-injection")]
    Forced,
}

/// 組に応じた `Exec` と呼び出しごとの上書きを作る。
#[cfg_attr(not(feature = "fault-injection"), allow(unused_variables))]
fn make_exec(dev: &Scripted, mode: Mode, op: OpId) -> (Exec<'_>, Option<OptIn>) {
    match mode {
        Mode::New => (Exec::new(dev, CPU_ONLY), None),
        Mode::Override => (Exec::deny_by_default(dev), Some(CPU_ONLY)),
        #[cfg(feature = "fault-injection")]
        Mode::Forced => (
            Exec::deny_by_default(dev).force_unsupported(op),
            Some(CPU_ONLY),
        ),
    }
}

/// 報告・数え・`query_route` との一致を照らす。
fn check_common(
    dev: &Scripted,
    report: &RouteReport,
    level: CapabilityLevel,
    mode: Mode,
    op: OpId,
) {
    assert_eq!(report.route, Some(Route::Cpu));
    assert_eq!(dev.query_ex_calls(), 1);
    assert_eq!(dev.memory_calls(), 0);
    assert_eq!(dev.queue_calls(), 0);
    let dev2 = Scripted::answering(q(level));
    let (exec2, over2) = make_exec(&dev2, mode, op);
    assert_eq!(exec2.query_route(op, over2).ok().as_ref(), Some(report));
}

/// u32 の入口を 1 回呼び、出力と報告を返す。
fn run_u32(x: &[u32], level: CapabilityLevel, mode: Mode) -> (Vec<u32>, RouteReport) {
    let n = u64::try_from(x.len()).expect("len fits u64");
    let mut out = vec![0u32; subgroup_sum_output_len(x.len())];
    let dev = Scripted::answering(q(level));
    let (exec, over) = make_exec(&dev, mode, OpId::SubgroupSumU32);
    let report =
        match exec.subgroup_sum_u32(over, n, SubgroupSumU32Operands::Host { x, out: &mut out }) {
            Ok(Outcome::Ran { report }) => report,
            _ => panic!("expected Ran"),
        };
    check_common(&dev, &report, level, mode, OpId::SubgroupSumU32);
    assert!(report.f32_denormal.is_none());
    (out, report)
}

/// f32 の入口を 1 回呼び、出力と報告を返す。
fn run_f32(x: &[f32], level: CapabilityLevel, mode: Mode) -> (Vec<f32>, RouteReport) {
    let n = u64::try_from(x.len()).expect("len fits u64");
    let mut out = vec![0f32; subgroup_sum_output_len(x.len())];
    let dev = Scripted::answering(q(level));
    let (exec, over) = make_exec(&dev, mode, OpId::SubgroupSumF32);
    let report =
        match exec.subgroup_sum_f32(over, n, SubgroupSumF32Operands::Host { x, out: &mut out }) {
            Ok(Outcome::Ran { report }) => report,
            _ => panic!("expected Ran"),
        };
    check_common(&dev, &report, level, mode, OpId::SubgroupSumF32);
    let d = report.f32_denormal.expect("f32 judgment");
    assert_eq!(d.behavior, F32DenormalBehavior::Preserve);
    assert_eq!(d.basis, F32DenormalBasis::Observed);
    assert_eq!(d.documented, None);
    (out, report)
}

fn check_u32(x: &[u32], level: CapabilityLevel, mode: Mode) -> RouteReport {
    let (out, report) = run_u32(x, level, mode);
    let reference = reference_subgroup_sum_u32(x).expect("reference");
    assert_eq!(out.len(), subgroup_sum_output_len(x.len()));
    let cmp = compare_bits_u32(&reference, &out).expect("compare");
    assert!(
        cmp.is_match(),
        "mismatched={} first={:?}",
        cmp.mismatched(),
        cmp.first_mismatch()
    );
    report
}

/// 相対の条件で比べ、余裕の最大・ブロックの数・報告を返す。
fn check_f32_relative(x: &[f32], level: CapabilityLevel, mode: Mode) -> (f64, usize, RouteReport) {
    let (out, report) = run_f32(x, level, mode);
    let r = reference_subgroup_sum_f32(x, F32SubnormalHandling::Preserve).expect("reference");
    assert_eq!(out.len(), subgroup_sum_output_len(x.len()));
    let mut margins = vec![0.0f64; r.len()];
    let cmp =
        compare_relative_sum_f32(r.values(), &out, r.abs_sums(), &mut margins).expect("compare");
    assert!(
        cmp.is_match(),
        "mismatched={} first={:?}",
        cmp.mismatched(),
        cmp.first_mismatch()
    );
    let max = margins.iter().copied().fold(0.0, f64::max);
    (max, margins.len(), report)
}

#[test]
fn u32_outputs_match_bits_over_lengths() {
    // 長さごとにビット同一で比べる（FB-43）。
    for n in FALLBACK_LENGTHS {
        let x = generate_subgroup_sum_u32_u(n).expect("u");
        check_u32(&x, CapabilityLevel::Unsupported, Mode::New);
        check_u32(&x, CapabilityLevel::Unsupported, Mode::Override);
    }
}

#[test]
fn integer_f32_outputs_match_bits_over_lengths() {
    // 整数値の入力はビット同一で比べる（FB-29）。
    for n in FALLBACK_LENGTHS {
        let x = generate_subgroup_sum_f32_fx(n).expect("fx");
        let r = reference_subgroup_sum_f32(&x, F32SubnormalHandling::Preserve).expect("reference");
        for mode in [Mode::New, Mode::Override] {
            let (out, _) = run_f32(&x, CapabilityLevel::Unsupported, mode);
            let cmp = compare_bits_f32(r.values(), &out).expect("compare");
            assert!(
                cmp.is_match(),
                "mismatched={} first={:?}",
                cmp.mismatched(),
                cmp.first_mismatch()
            );
        }
    }
}

#[test]
fn fg_is_within_relative_bound_over_lengths() {
    // 相対の条件で比べ、余裕の最大を行に出す（FB-31）。
    for n in FALLBACK_LENGTHS {
        let x = generate_subgroup_sum_f32_fg(n).expect("fg");
        let (max, blocks, _) = check_f32_relative(&x, CapabilityLevel::Unsupported, Mode::New);
        assert!(max.is_finite());
        println!(
            "margin {} n={n} blocks={blocks} max={max:e}",
            FallbackInputKind::SubgroupSumF32Fg.name()
        );
    }
}

#[test]
fn fs_is_compared_with_preserving_reference_over_lengths() {
    for n in FALLBACK_LENGTHS {
        let x = generate_subgroup_sum_f32_fs(n).expect("fs");
        check_f32_relative(&x, CapabilityLevel::Unsupported, Mode::New);
    }
}

#[cfg(feature = "fault-injection")]
#[test]
fn forced_native_off_runs_cpu() {
    let xu = generate_subgroup_sum_u32_u(1000).expect("u");
    let report = check_u32(&xu, CapabilityLevel::S2, Mode::Forced);
    assert_eq!(
        report.native_reason,
        Some(UnavailableReason::ForcedUnsupported)
    );
    let xf = generate_subgroup_sum_f32_fs(1000).expect("fs");
    let (_, _, report) = check_f32_relative(&xf, CapabilityLevel::S2, Mode::Forced);
    assert_eq!(
        report.native_reason,
        Some(UnavailableReason::ForcedUnsupported)
    );
}
