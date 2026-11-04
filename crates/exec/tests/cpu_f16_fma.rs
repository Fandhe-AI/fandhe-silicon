//! 下-2 の公開のパスから `f16_fma` を CPU の経路・host の形で呼び、core の入力の組と CPU の参照を
//! 順序の差で比べ、`query_ex`・`memory`・`queue` の数えを確かめる（TASK-930）。
//! 置き場の説明は docs/exec-cpu-route.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::{CapabilityLevel, CapabilityQuery};
use fandhe_silicon_core::parity::fallback_compare::{
    compare_order_distance_binary16, count_bit_identical_binary16,
};
use fandhe_silicon_core::parity::fallback_inputs::{
    F16FmaInputs, FALLBACK_LENGTHS, generate_f16_fma_e1, generate_f16_fma_e2, generate_f16_fma_r,
    generate_f16_fma_s,
};
use fandhe_silicon_core::parity::fallback_reference::reference_f16_fma_slice;
#[cfg(feature = "fault-injection")]
use fandhe_silicon_exec::UnavailableReason;
use fandhe_silicon_exec::{Exec, F16FmaOperands, OpId, OptIn, Outcome, Route};

const C_ONLY: OptIn = OptIn {
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

/// C の経路を選ばせる組。
#[derive(Clone, Copy)]
enum Mode {
    /// `Exec::new` に C の印を渡す。上書きは `None`。
    New,
    /// `Exec::deny_by_default` に、呼び出しごとの上書きで C の印を渡す。
    Override,
    /// Native を外す指定を付け、呼び出しごとの上書きで C の印を渡す。
    #[cfg(feature = "fault-injection")]
    Forced,
}

/// 組に応じた `Exec` と呼び出しごとの上書きを作る。
fn make_exec(dev: &Scripted, mode: Mode) -> (Exec<'_>, Option<OptIn>) {
    match mode {
        Mode::New => (Exec::new(dev, C_ONLY), None),
        Mode::Override => (Exec::deny_by_default(dev), Some(C_ONLY)),
        #[cfg(feature = "fault-injection")]
        Mode::Forced => (
            Exec::deny_by_default(dev).force_unsupported(OpId::F16Fma),
            Some(C_ONLY),
        ),
    }
}

/// 1 回呼び、報告・数え・参照との比べを照らす。参照と出力を返す。
fn check(inputs: &F16FmaInputs, level: CapabilityLevel, mode: Mode) -> (Vec<u16>, Vec<u16>) {
    let n = u64::try_from(inputs.len()).expect("len fits u64");
    let mut out = vec![0u16; inputs.len()];
    let dev = Scripted::answering(q(level));
    let (exec, over) = make_exec(&dev, mode);
    let report = match exec.f16_fma(
        over,
        n,
        F16FmaOperands::Host {
            a: &inputs.a,
            b: &inputs.b,
            c: &inputs.c,
            out: &mut out,
        },
    ) {
        Ok(Outcome::Ran { report }) => report,
        _ => panic!("expected Ran"),
    };
    assert_eq!(report.route, Some(Route::Cpu));
    assert!(report.f32_denormal.is_none());
    assert_eq!(dev.query_ex_calls(), 1);
    assert_eq!(dev.memory_calls(), 0);
    assert_eq!(dev.queue_calls(), 0);

    let dev2 = Scripted::answering(q(level));
    let (exec2, over2) = make_exec(&dev2, mode);
    assert_eq!(exec2.query_route(OpId::F16Fma, over2).ok(), Some(report));

    let reference = reference_f16_fma_slice(inputs).expect("reference");
    let cmp = compare_order_distance_binary16(&reference, &out).expect("compare");
    assert!(
        cmp.is_match(),
        "mismatched={} first={:?}",
        cmp.mismatched(),
        cmp.first_mismatch()
    );
    (reference, out)
}

#[test]
fn e2_and_s_are_within_order_distance() {
    // 非正規化数の要素も 0 に落とさず比べる（FB-30）。
    for inputs in [
        generate_f16_fma_e2().expect("e2"),
        generate_f16_fma_s().expect("s"),
    ] {
        check(&inputs, CapabilityLevel::Unsupported, Mode::New);
        check(&inputs, CapabilityLevel::Unsupported, Mode::Override);
    }
}

#[test]
fn r_is_within_order_distance_over_lengths() {
    for n in FALLBACK_LENGTHS {
        let inputs = generate_f16_fma_r(n).expect("r");
        check(&inputs, CapabilityLevel::Unsupported, Mode::New);
    }
}

#[test]
fn e1_is_judged_by_order_distance_and_counts_bit_identical() {
    // 判定は順序の差。ビット同一の件数は出すだけ（FB-37）。
    for n in FALLBACK_LENGTHS {
        let inputs = generate_f16_fma_e1(n).expect("e1");
        let (reference, out) = check(&inputs, CapabilityLevel::Unsupported, Mode::New);
        let count = count_bit_identical_binary16(&reference, &out).expect("count");
        println!("e1 n={n} bit_identical={count}");
        assert!(count <= n);
    }
}

#[cfg(feature = "fault-injection")]
#[test]
fn forced_native_off_runs_cpu() {
    let inputs = generate_f16_fma_s().expect("s");
    check(&inputs, CapabilityLevel::S2, Mode::Forced);
    let n = u64::try_from(inputs.len()).expect("len fits u64");
    let mut out = vec![0u16; inputs.len()];
    let dev = Scripted::answering(q(CapabilityLevel::S2));
    let (exec, over) = make_exec(&dev, Mode::Forced);
    let Ok(Outcome::Ran { report }) = exec.f16_fma(
        over,
        n,
        F16FmaOperands::Host {
            a: &inputs.a,
            b: &inputs.b,
            c: &inputs.c,
            out: &mut out,
        },
    ) else {
        panic!("expected Ran");
    };
    assert_eq!(
        report.native_reason,
        Some(UnavailableReason::ForcedUnsupported)
    );
}
