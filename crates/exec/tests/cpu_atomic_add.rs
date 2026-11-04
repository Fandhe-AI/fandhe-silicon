//! 下-2 の公開のパスから `atomic_add` を `Route::Cpu`・host の形で呼び、
//! core の入力の組と CPU の参照を比べ、`query_ex`・`memory`・`queue` の数えを確かめ、
//! 出力の sha256 の行を出す（TASK-932）。置き場の説明は docs/exec-cpu-route.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::fallback_compare::{
    F32SubnormalHandling, compare_bits_f32, compare_relative_sum_f32,
};
use fandhe_silicon_core::parity::fallback_inputs::{
    ATOMIC_ADD_OUTPUT_COUNTS, FALLBACK_LENGTHS, FallbackInputKind, ax_pair_allowed,
    generate_atomic_add_ag, generate_atomic_add_as, generate_atomic_add_ax,
};
use fandhe_silicon_core::parity::fallback_reference::reference_atomic_add_f32;
#[cfg(feature = "fault-injection")]
use fandhe_silicon_exec::UnavailableReason;
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F32DenormalBasis, F32DenormalBehavior, OpId, OptIn, Outcome, Route,
    RouteReport,
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
fn make_exec(dev: &Scripted, mode: Mode) -> (Exec<'_>, Option<OptIn>) {
    match mode {
        Mode::New => (Exec::new(dev, CPU_ONLY), None),
        Mode::Override => (Exec::deny_by_default(dev), Some(CPU_ONLY)),
        #[cfg(feature = "fault-injection")]
        Mode::Forced => (
            Exec::deny_by_default(dev).force_unsupported(OpId::AtomicAdd),
            Some(CPU_ONLY),
        ),
    }
}

/// `atomic_add` を host の形で 1 回呼び、出力と報告を返す。数えと報告も照らす。
fn run(x: &[f32], m: usize, level: CapabilityLevel, mode: Mode) -> (Vec<f32>, RouteReport) {
    let n = u64::try_from(x.len()).expect("len fits u64");
    let m64 = u64::try_from(m).expect("m fits u64");
    let mut out = vec![7.0f32; m];
    let dev = Scripted::answering(q(level));
    let (exec, over) = make_exec(&dev, mode);
    let report = match exec.atomic_add(over, n, m64, AtomicAddOperands::Host { x, out: &mut out }) {
        Ok(Outcome::Ran { report }) => report,
        _ => panic!("expected Ran"),
    };
    assert_eq!(report.route, Some(Route::Cpu));
    assert_eq!(dev.query_ex_calls(), 1);
    assert_eq!(dev.memory_calls(), 0);
    assert_eq!(dev.queue_calls(), 0);
    let dev2 = Scripted::answering(q(level));
    let (exec2, over2) = make_exec(&dev2, mode);
    assert_eq!(
        exec2.query_route(OpId::AtomicAdd, over2).ok().as_ref(),
        Some(&report)
    );
    let d = report.f32_denormal.expect("f32 judgment");
    assert_eq!(d.behavior, F32DenormalBehavior::Preserve);
    assert_eq!(d.basis, F32DenormalBasis::Observed);
    assert_eq!(d.documented, None);
    assert_eq!(out.len(), m);
    (out, report)
}

/// 相対の条件で比べ、余裕の最大と報告を返す。
fn check_relative(x: &[f32], m: usize, level: CapabilityLevel, mode: Mode) -> (f64, RouteReport) {
    let (out, report) = run(x, m, level, mode);
    let r = reference_atomic_add_f32(x, m, F32SubnormalHandling::Preserve).expect("reference");
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
    (max, report)
}

#[test]
fn ax_outputs_match_bits_over_lengths_and_counts() {
    // 整数値の入力はビット同一で比べる（FB-29）。
    let mut pairs = 0usize;
    for m in ATOMIC_ADD_OUTPUT_COUNTS {
        for n in FALLBACK_LENGTHS {
            if !ax_pair_allowed(n, m) {
                continue;
            }
            pairs += 1;
            let x = generate_atomic_add_ax(n).expect("ax");
            let r =
                reference_atomic_add_f32(&x, m, F32SubnormalHandling::Preserve).expect("reference");
            for mode in [Mode::New, Mode::Override] {
                let (out, _) = run(&x, m, CapabilityLevel::Unsupported, mode);
                let cmp = compare_bits_f32(r.values(), &out).expect("compare");
                assert!(
                    cmp.is_match(),
                    "n={n} m={m} mismatched={} first={:?}",
                    cmp.mismatched(),
                    cmp.first_mismatch()
                );
            }
        }
    }
    assert_eq!(pairs, 34);
}

#[test]
fn ag_is_within_relative_bound_over_lengths_and_counts() {
    // 相対の条件で比べ、余裕の最大を行に出す（FB-32）。
    for n in FALLBACK_LENGTHS {
        let x = generate_atomic_add_ag(n).expect("ag");
        for m in ATOMIC_ADD_OUTPUT_COUNTS {
            let (max, _) = check_relative(&x, m, CapabilityLevel::Unsupported, Mode::New);
            assert!(max.is_finite());
            println!(
                "margin {} n={n} m={m} max={max:e}",
                FallbackInputKind::AtomicAddAg.name()
            );
        }
    }
}

#[test]
fn as_patterns_are_compared_with_preserving_reference_over_lengths_and_counts() {
    for n in FALLBACK_LENGTHS {
        for m in ATOMIC_ADD_OUTPUT_COUNTS {
            let x = generate_atomic_add_as(n, m).expect("as");
            check_relative(&x, m, CapabilityLevel::Unsupported, Mode::New);
        }
    }
}

/// 32 バイトの digest を小文字の 16 進 64 文字にする。
fn hex(d: &[u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
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

/// 同じ入力を新しい device・`Exec` で 2 回呼び、sha256 が等しいことを照らして行に出す。
fn print_digest(kind: FallbackInputKind, x: &[f32], n: usize, m: usize) {
    let a = f32_digest(&run(x, m, CapabilityLevel::Unsupported, Mode::New).0);
    let b = f32_digest(&run(x, m, CapabilityLevel::Unsupported, Mode::New).0);
    assert_eq!(a, b, "n={n} m={m}");
    println!("sha256 {} n={n} m={m} {a}", kind.name());
}

#[test]
fn outputs_print_sha256_per_case() {
    // 同じ入力を 2 回呼んだ出力の sha256 を比べ、行に出す（FB-33）。
    for m in ATOMIC_ADD_OUTPUT_COUNTS {
        for n in FALLBACK_LENGTHS {
            if ax_pair_allowed(n, m) {
                let x = generate_atomic_add_ax(n).expect("ax");
                print_digest(FallbackInputKind::AtomicAddAx, &x, n, m);
            }
            let x = generate_atomic_add_ag(n).expect("ag");
            print_digest(FallbackInputKind::AtomicAddAg, &x, n, m);
            let x = generate_atomic_add_as(n, m).expect("as");
            print_digest(FallbackInputKind::AtomicAddAs, &x, n, m);
        }
    }
}

#[cfg(feature = "fault-injection")]
#[test]
fn forced_native_off_runs_cpu() {
    let x = generate_atomic_add_as(1000, 64).expect("as");
    let (_, report) = check_relative(&x, 64, CapabilityLevel::S2, Mode::Forced);
    assert_eq!(
        report.native_reason,
        Some(UnavailableReason::ForcedUnsupported)
    );
}
