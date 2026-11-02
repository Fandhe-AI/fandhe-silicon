//! fandhe-silicon の下-2 crate（共通の操作と代わりの実行）。
//!
//! `fandhe-silicon-core` の共通型だけに依存し、下-1 の背後実装を共通型経由で呼び出す。
//! 能力が無い場合の代わりの実行もここで扱う。
//!
//! 公開の型は opt-in の印 [`OptIn`]、経路 [`Route`]、操作の識別子 [`OpId`]、
//! 使えない理由 [`UnavailableReason`]（`docs/spec/05-tasks.md` の TASK-919）。
#![forbid(unsafe_code)]

mod opt_in;
mod route;

pub use opt_in::OptIn;
pub use route::{OpId, Route, UnavailableReason};
