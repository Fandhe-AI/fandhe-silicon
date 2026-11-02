//! fandhe-silicon の上 crate（wgpu 風の層）。
//!
//! 上位ライブラリ（fandhe-ai・vector-db・fandhe-3d）が使う入口。`fandhe-silicon-exec` の
//! 共通操作の上に組み立て、チップ固有の型は公開面へ出さない。`unsafe` は使わない（spec BUILD-56）。
//!
//! 公開の型: `UpperDevice`・`Queue`（device と投入の窓口）、`CommandEncoder`・`CommandBuffer`（記録先と投入前の値）、
//! `SubmitId`（投入の印）。
#![forbid(unsafe_code)]

mod device;
mod encoder;

pub use device::{Queue, UpperDevice};
pub use encoder::{CommandBuffer, CommandEncoder, SubmitId};
