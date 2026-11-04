//! 下-2 の公開のパスから `subgroup_sum_u32`・`subgroup_sum_f32` を `Route::Cpu`・host の形で呼び、
//! core の入力の組と CPU の参照を比べ、`query_ex`・`memory`・`queue` の数えを確かめる（TASK-931）。
//! 置き場の説明は docs/exec-cpu-route.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::cpu_reference::ReferenceError;
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

/// 32 バイトの digest を小文字の 16 進 64 文字にする。
fn hex(d: &[u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// u32 の出力を添字の順にリトルエンディアンで並べた sha256 を返す。
fn u32_digest(out: &[u32]) -> String {
    let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_le_bytes()).collect();
    hex(&sha256_bytes(&bytes))
}

/// f32 の出力のビット列（`0.0` に等しい要素は `0u32`）を添字の順にリトルエンディアンで並べた sha256 を返す。
fn f32_digest(out: &[f32]) -> String {
    let bytes: Vec<u8> = out
        .iter()
        .flat_map(|&v| {
            let bits = if v == 0.0 { 0u32 } else { v.to_bits() };
            bits.to_le_bytes()
        })
        .collect();
    hex(&sha256_bytes(&bytes))
}

#[test]
fn outputs_print_sha256_per_case() {
    // 同じ入力を 2 回呼んだ出力の sha256 を比べ、行に出す（FB-33）。
    for n in FALLBACK_LENGTHS {
        let x = generate_subgroup_sum_u32_u(n).expect("u");
        let a = u32_digest(&run_u32(&x, CapabilityLevel::Unsupported, Mode::New).0);
        let b = u32_digest(&run_u32(&x, CapabilityLevel::Unsupported, Mode::New).0);
        assert_eq!(a, b, "u32 n={n}");
        println!(
            "sha256 {} n={n} {a}",
            FallbackInputKind::SubgroupSumU32U.name()
        );
    }
    type Gen = fn(usize) -> Result<Vec<f32>, ReferenceError>;
    let kinds: [(FallbackInputKind, Gen); 3] = [
        (
            FallbackInputKind::SubgroupSumF32Fx,
            generate_subgroup_sum_f32_fx,
        ),
        (
            FallbackInputKind::SubgroupSumF32Fg,
            generate_subgroup_sum_f32_fg,
        ),
        (
            FallbackInputKind::SubgroupSumF32Fs,
            generate_subgroup_sum_f32_fs,
        ),
    ];
    for (kind, generate) in kinds {
        for n in FALLBACK_LENGTHS {
            let x = generate(n).expect("input");
            let a = f32_digest(&run_f32(&x, CapabilityLevel::Unsupported, Mode::New).0);
            let b = f32_digest(&run_f32(&x, CapabilityLevel::Unsupported, Mode::New).0);
            assert_eq!(a, b, "f32 n={n}");
            println!("sha256 {} n={n} {a}", kind.name());
        }
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
