//! fandhe-silicon の下-2 crate（共通の操作と代わりの実行）。
//!
//! `fandhe-silicon-core` の共通型だけに依存し、下-1 の背後実装を共通型経由で呼び出す。
//! 能力が無い場合の代わりの実行もここで扱う。
//!
//! 公開の型は opt-in の印 [`OptIn`]、経路 [`Route`]、操作の識別子 [`OpId`]、
//! 使えない理由 [`UnavailableReason`]、
//! 経路の報告 [`RouteReport`]（f32 の非正規化数の項 [`F32DenormalReport`]・
//! [`F32DenormalBehavior`]・[`F32DenormalBasis`]）、結果の値 [`Outcome`]、
//! 列の渡し方 [`F16FmaOperands`]・[`SubgroupSumU32Operands`]・[`SubgroupSumF32Operands`]・
//! [`AtomicAddOperands`]、
//! device を借りて既定の印を持つ下-2 の値 [`Exec`] とその 7 つの入口（`docs/spec/05-tasks.md` の TASK-919）、
//! 入口を呼ぶ前に経路の報告を返す `Exec::query_route`（同 TASK-920）。
//! feature `fault-injection` の下では、指定した操作で `Native` を候補に入れない `Exec::force_unsupported` と
//! 理由の値 `UnavailableReason::ForcedUnsupported` を持つ（同 TASK-921）。
//! 同じ feature の下で、`Exec::inject_denormal` と判定の値 `DenormalInjection`、
//! 報告の根拠の値 `F32DenormalBasis::Injected` も持つ（同 TASK-921）。
#![forbid(unsafe_code)]

mod entry;
#[cfg(feature = "fault-injection")]
mod hooks;
mod operands;
mod opt_in;
mod report;
mod route;
mod select;

pub use entry::Exec;
pub use operands::{
    AtomicAddOperands, F16FmaOperands, SubgroupSumF32Operands, SubgroupSumU32Operands,
};
pub use opt_in::OptIn;
#[cfg(feature = "fault-injection")]
pub use report::DenormalInjection;
pub use report::{F32DenormalBasis, F32DenormalBehavior, F32DenormalReport, Outcome, RouteReport};
pub use route::{OpId, Route, UnavailableReason};
