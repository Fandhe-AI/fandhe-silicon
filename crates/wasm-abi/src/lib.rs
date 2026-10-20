//! fandhe-silicon の wasm32 向け cdylib の入口（spec TASK-237・BUILD-19・BUILD-20 のポインタ）。
//! scalar 版と simd128 版（`RUSTFLAGS` で命令を足し target-dir を分ける）の 2 通りでビルドする。手順は `docs/wasm32-build.md`。
//!
//! 依存は `fandhe-silicon-contract` だけで、向きは「本 crate → contract」の一方向。
//! feature の組 `contract-core,cpu-isa,cpu-ops` を契約の crate へ転送する。
//! 現状は export を持たない。契約の crate の CPU 向けの feature はまだコードを持たないため、
//! 使うシンボルは無く、リンクに入れるだけである。
#![forbid(unsafe_code)]

use fandhe_silicon_contract as _;
