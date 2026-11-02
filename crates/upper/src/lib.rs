//! fandhe-silicon の上 crate（wgpu 風の層）。
//!
//! 上位ライブラリ（fandhe-ai・vector-db・fandhe-3d）が使う入口。`fandhe-silicon-core` の
//! 契約の型と操作の上に組み立て、チップ固有の型は公開面へ出さない（`fandhe-silicon-exec` への依存は
//! Cargo.toml にあるが、このソースは exec の項目を使っていない）。`unsafe` は使わない（spec BUILD-56）。
//!
//! 公開の型: `UpperDevice`・`Queue`（device と投入の窓口）、`CommandEncoder`・`CommandBuffer`（記録先と投入前の値）、
//! `SubmitId`（投入の印）、`ComputePass`（compute のパス）、`Buffer`（バッファの包み）。
#![forbid(unsafe_code)]

mod buffer;
mod device;
mod encoder;
mod pass;

pub use buffer::Buffer;
pub use device::{Queue, UpperDevice};
pub use encoder::{CommandBuffer, CommandEncoder, SubmitId};
pub use pass::ComputePass;
