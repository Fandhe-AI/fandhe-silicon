//! `cpu_ops` の `elementwise_scalar` に `AddScalar` で NaN・±Inf を入れた出力を、IEEE 754 の足し算の答えとして
//! 字句で置いた期待と突き合わせる外の試験（TASK-2456 のポインタ）。入力の側の非有限（NaN・±Inf）と定数の側の非有限の
//! 両方を、`DefaultCpuOps` と `ReferenceCpuOps` の両方に当てる。期待は実装の式を使わず、答えの種類で書く。

use fandhe_silicon_contract::cpu_ops::{
    CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp,
};

/// 出力の語の期待。NaN は種類だけ（ペイロードは問わない）、ほかはビット列で比べる。
#[derive(Debug, Clone, Copy)]
enum Want {
    Nan,
    Bits(u32),
}

const NAN_IN: f32 = f32::from_bits(0x7fc0_0001);
const INF: f32 = f32::INFINITY;
const NINF: f32 = f32::NEG_INFINITY;

/// 長さ 7（4 で割って 3 余る）の入力。NaN・+Inf・-Inf・-0.0・+0.0・有限の正負を含む。
const INPUT: [f32; 7] = [NAN_IN, INF, NINF, -0.0, 0.0, 1.5, -2.5];

fn bits(x: f32) -> Want {
    Want::Bits(x.to_bits())
}

/// 定数 `c` ごとの期待（入力の並びと同じ）。
fn cases() -> Vec<(f32, [Want; 7])> {
    vec![
        // 有限の定数: 入力の非有限はそのまま通り、NaN は NaN のまま。
        (
            0.5,
            [
                Want::Nan,
                bits(INF),
                bits(NINF),
                bits(0.5),
                bits(0.5),
                bits(2.0),
                bits(-2.0),
            ],
        ),
        // +Inf の定数: 有限と +Inf は +Inf、-Inf との和と NaN は NaN。
        (
            INF,
            [
                Want::Nan,
                bits(INF),
                Want::Nan,
                bits(INF),
                bits(INF),
                bits(INF),
                bits(INF),
            ],
        ),
        // -Inf の定数: 有限と -Inf は -Inf、+Inf との和と NaN は NaN。
        (
            NINF,
            [
                Want::Nan,
                Want::Nan,
                bits(NINF),
                bits(NINF),
                bits(NINF),
                bits(NINF),
                bits(NINF),
            ],
        ),
        // NaN の定数: 全部 NaN。
        (f32::from_bits(0x7fc0_0002), [Want::Nan; 7]),
        // -0.0 の定数: -0.0 + -0.0 は -0.0、+0.0 + -0.0 は +0.0。
        (
            -0.0,
            [
                Want::Nan,
                bits(INF),
                bits(NINF),
                bits(-0.0),
                bits(0.0),
                bits(1.5),
                bits(-2.5),
            ],
        ),
    ]
}

/// 出力を番兵（NaN でない語）で埋めてから `elementwise_scalar` を呼ぶ。
fn run<T: CpuOps>(ops: &T, c: f32) -> [f32; 7] {
    let mut out = [f32::from_bits(0x1234_5678); 7];
    ops.elementwise_scalar(UnaryElementwiseOp::AddScalar(c), &INPUT, &mut out)
        .unwrap();
    out
}

fn check(label: &str, c: f32, out: &[f32; 7], want: &[Want; 7]) {
    for (i, (got, w)) in out.iter().zip(want).enumerate() {
        match w {
            Want::Nan => assert!(
                got.is_nan(),
                "{label} c={c:?} [{i}]: want NaN, got {:#010x}",
                got.to_bits()
            ),
            Want::Bits(b) => assert_eq!(
                got.to_bits(),
                *b,
                "{label} c={c:?} [{i}]: input {:#010x}",
                INPUT[i].to_bits()
            ),
        }
    }
}

#[test]
fn default_scalar_add_scalar_keeps_non_finite_answers() {
    for (c, want) in cases() {
        check("default", c, &run(&DefaultCpuOps, c), &want);
    }
}

#[test]
fn reference_scalar_add_scalar_keeps_non_finite_answers() {
    for (c, want) in cases() {
        check("reference", c, &run(&ReferenceCpuOps, c), &want);
    }
}
