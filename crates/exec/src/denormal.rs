//! f32 の非正規化数の判定の中身。固定の入力・3 つの出力からの判定・(操作, 経路) ごとの保持。
//!
//! `entry` の `Exec::route_report` が経路の選び方（`select`）へ渡す判定の関数がここを呼ぶ。
//! 入力は定数と計算だけで作り、判定は 3 つの出力のビット列で決め、保持は `Exec` の値ごとに
//! (操作, 経路) の組あたり 1 つ持つ。crate の外には公開しない（`docs/spec/05-tasks.md` の TASK-925）。

use crate::route::{OpId, Route};
use crate::select::DenormalJudgment;
use fandhe_silicon_core::error::ContractError;
use std::cell::Cell;

/// 入力の要素数。
pub(crate) const INPUT_LEN: usize = 768;
/// 1 ブロックの要素数。
const BLOCK_LEN: usize = 256;
/// 試しの実行の出力の数。
const OUTPUT_LEN: usize = 3;
/// 非正規化数を保つときの 3 つの出力のビット列。
const PRESERVED_BITS: [u32; OUTPUT_LEN] = [0x0000_8080, 0x8000_8080, 0x00FF_FF00];
/// 符号ビット。
const SIGN_BIT: u32 = 0x8000_0000;
/// 3 つ目のブロックの要素のビット列。
const TAIL_BITS: u32 = 0x0000_FFFF;

/// 試しの実行の入力。
pub(crate) type ProbeInput = [f32; INPUT_LEN];

/// 試験が差す模擬の runner の型。(操作, 経路, 入力) から 3 つの出力を返す。
#[cfg(test)]
pub(crate) type ProbeRunner<'d> =
    &'d dyn Fn(OpId, Route, &ProbeInput) -> Result<[f32; OUTPUT_LEN], ContractError>;

/// `SubgroupSumF32` の入力。ブロック 0 は要素 t のビット列が `t + 1`、ブロック 1 はその符号を反転した列、
/// ブロック 2 はすべてビット列 `0x0000_FFFF`。
fn subgroup_input() -> ProbeInput {
    let mut x = [0.0f32; INPUT_LEN];
    for (i, v) in x.iter_mut().enumerate() {
        let t = i % BLOCK_LEN;
        let bits = match i / BLOCK_LEN {
            0 => t as u32 + 1,
            1 => (t as u32 + 1) | SIGN_BIT,
            _ => TAIL_BITS,
        };
        *v = f32::from_bits(bits);
    }
    x
}

/// `AtomicAdd` の入力。添字 i の要素は `SubgroupSumF32` の入力の要素 `(i mod 3) × 256 + i ÷ 3`
/// （出力 `i mod 3` に足す値）。
fn atomic_input() -> ProbeInput {
    let base = subgroup_input();
    let mut x = [0.0f32; INPUT_LEN];
    for (i, v) in x.iter_mut().enumerate() {
        *v = base[(i % OUTPUT_LEN) * BLOCK_LEN + i / OUTPUT_LEN];
    }
    x
}

/// 操作の試しの実行の入力。判定を求めない操作は `None`。
pub(crate) fn input_of(op: OpId) -> Option<ProbeInput> {
    match op {
        OpId::SubgroupSumF32 => Some(subgroup_input()),
        OpId::AtomicAdd => Some(atomic_input()),
        OpId::F16Fma | OpId::SubgroupSumU32 => None,
    }
}

/// `-0.0` を `+0.0` にしたビット列。
fn canonical_bits(v: f32) -> u32 {
    if v == 0.0 { 0 } else { v.to_bits() }
}

/// 3 つの出力から判定を決める。保つ値と一致すれば `Preserve`、3 つとも 0 なら `FlushToZero`、ほかは `Unknown`。
pub(crate) fn classify(out: [f32; OUTPUT_LEN]) -> DenormalJudgment {
    let bits = out.map(canonical_bits);
    if bits == PRESERVED_BITS {
        DenormalJudgment::Preserve
    } else if bits == [0; OUTPUT_LEN] {
        DenormalJudgment::FlushToZero
    } else {
        DenormalJudgment::Unknown
    }
}

/// 判定の添字。判定を求めない操作は `None`。
fn op_slot(op: OpId) -> Option<usize> {
    match op {
        OpId::SubgroupSumF32 => Some(0),
        OpId::AtomicAdd => Some(1),
        OpId::F16Fma | OpId::SubgroupSumU32 => None,
    }
}

/// 経路の添字。
fn route_slot(route: Route) -> usize {
    match route {
        Route::Native => 0,
        Route::GpuEmulated => 1,
        Route::Cpu => 2,
    }
}

/// (操作, 経路) ごとの判定の保持。`Exec` の値ごとに 1 つ持つ。
#[derive(Debug, Default)]
pub(crate) struct HeldJudgments {
    cells: [[Cell<Option<DenormalJudgment>>; 3]; 2],
}

impl HeldJudgments {
    /// (`op`, `route`) の判定を返す。
    ///
    /// 保持があればそれを返し `run` を呼ばない。無ければ `run` を入力つきで 1 回呼び、`Ok` なら判定して
    /// 保持して返す（`Unknown` も保持する）。`run` の `Err` は保持せずそのまま返す。
    /// 判定を求めない操作では `run` を呼ばず `Err(ContractError::InvalidArgument(..))` を返す。
    pub(crate) fn judge(
        &self,
        op: OpId,
        route: Route,
        run: impl FnOnce(&ProbeInput) -> Result<[f32; OUTPUT_LEN], ContractError>,
    ) -> Result<DenormalJudgment, ContractError> {
        let (Some(slot), Some(input)) = (op_slot(op), input_of(op)) else {
            return Err(ContractError::InvalidArgument(
                "operation has no f32 denormal judgment".to_string(),
            ));
        };
        let cell = &self.cells[slot][route_slot(route)];
        if let Some(held) = cell.get() {
            return Ok(held);
        }
        let judgment = classify(run(&input)?);
        cell.set(Some(judgment));
        Ok(judgment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ブロックごと（`AtomicAdd` は `i mod 3` ごと）の和を、順・逆順・f64 の丸めで求める。
    fn sums(op: OpId) -> [[u32; OUTPUT_LEN]; 3] {
        let x = input_of(op).expect("input");
        let mut forward = [0.0f32; OUTPUT_LEN];
        let mut backward = [0.0f32; OUTPUT_LEN];
        let mut wide = [0.0f64; OUTPUT_LEN];
        let group = |i: usize| match op {
            OpId::AtomicAdd => i % OUTPUT_LEN,
            _ => i / BLOCK_LEN,
        };
        for (i, v) in x.iter().enumerate() {
            forward[group(i)] += *v;
            wide[group(i)] += f64::from(*v);
        }
        for (i, v) in x.iter().enumerate().rev() {
            backward[group(i)] += *v;
        }
        [
            forward.map(f32::to_bits),
            backward.map(f32::to_bits),
            wide.map(|w| (w as f32).to_bits()),
        ]
    }

    #[test]
    fn preserve_values_do_not_depend_on_order() {
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            for bits in sums(op) {
                assert_eq!(bits, PRESERVED_BITS);
                assert_eq!(
                    classify(bits.map(f32::from_bits)),
                    DenormalJudgment::Preserve
                );
            }
        }
    }

    #[test]
    fn inputs_are_denormal_only() {
        for op in [OpId::SubgroupSumF32, OpId::AtomicAdd] {
            let x = input_of(op).expect("input");
            assert_eq!(x.len(), 768);
            assert!(x.iter().all(|v| v.is_subnormal()));
        }
        assert!(input_of(OpId::F16Fma).is_none());
        assert!(input_of(OpId::SubgroupSumU32).is_none());
    }

    #[test]
    fn classify_maps_outputs() {
        let keep = PRESERVED_BITS.map(f32::from_bits);
        assert_eq!(classify(keep), DenormalJudgment::Preserve);
        assert_eq!(classify([0.0, 0.0, 0.0]), DenormalJudgment::FlushToZero);
        assert_eq!(classify([-0.0, 0.0, -0.0]), DenormalJudgment::FlushToZero);
        for k in 0..3 {
            let mut zeroed = keep;
            zeroed[k] = 0.0;
            assert_eq!(classify(zeroed), DenormalJudgment::Unknown);
            let mut off = PRESERVED_BITS;
            off[k] += 1;
            assert_eq!(classify(off.map(f32::from_bits)), DenormalJudgment::Unknown);
            let mut flipped = PRESERVED_BITS;
            flipped[k] ^= SIGN_BIT;
            assert_eq!(
                classify(flipped.map(f32::from_bits)),
                DenormalJudgment::Unknown
            );
            for odd in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut other = keep;
                other[k] = odd;
                assert_eq!(classify(other), DenormalJudgment::Unknown);
            }
        }
    }

    #[test]
    fn judgment_is_kept_per_op_and_route() {
        let held = HeldJudgments::default();
        let calls = Cell::new(0u32);
        let keep = PRESERVED_BITS.map(f32::from_bits);
        let seen_ok = Cell::new(true);
        let run = |out: [f32; OUTPUT_LEN], op: OpId| {
            let expected = input_of(op).expect("input");
            let calls = &calls;
            let seen_ok = &seen_ok;
            move |input: &ProbeInput| {
                calls.set(calls.get() + 1);
                if input.map(f32::to_bits) != expected.map(f32::to_bits) {
                    seen_ok.set(false);
                }
                Ok(out)
            }
        };
        let sub = OpId::SubgroupSumF32;
        assert_eq!(
            held.judge(sub, Route::Native, run(keep, sub)),
            Ok(DenormalJudgment::Preserve)
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(
            held.judge(sub, Route::Native, run([0.0; 3], sub)),
            Ok(DenormalJudgment::Preserve)
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(
            held.judge(sub, Route::GpuEmulated, run([0.0; 3], sub)),
            Ok(DenormalJudgment::FlushToZero)
        );
        assert_eq!(calls.get(), 2);
        let atom = OpId::AtomicAdd;
        assert_eq!(
            held.judge(atom, Route::Native, run([0.0, 0.0, keep[2]], atom)),
            Ok(DenormalJudgment::Unknown)
        );
        assert_eq!(calls.get(), 3);
        assert_eq!(
            held.judge(atom, Route::Native, run(keep, atom)),
            Ok(DenormalJudgment::Unknown)
        );
        assert_eq!(calls.get(), 3);
        assert!(seen_ok.get());
    }

    #[test]
    fn errors_are_not_kept() {
        let held = HeldJudgments::default();
        let calls = Cell::new(0u32);
        let op = OpId::SubgroupSumF32;
        let failed = held.judge(op, Route::Cpu, |_| {
            calls.set(calls.get() + 1);
            Err(ContractError::Timeout)
        });
        assert_eq!(failed, Err(ContractError::Timeout));
        let keep = PRESERVED_BITS.map(f32::from_bits);
        let ok = held.judge(op, Route::Cpu, |_| {
            calls.set(calls.get() + 1);
            Ok(keep)
        });
        assert_eq!(ok, Ok(DenormalJudgment::Preserve));
        assert_eq!(calls.get(), 2);
        let again = held.judge(op, Route::Cpu, |_| {
            calls.set(calls.get() + 1);
            Err(ContractError::Timeout)
        });
        assert_eq!(again, Ok(DenormalJudgment::Preserve));
        assert_eq!(calls.get(), 2);
    }
}
