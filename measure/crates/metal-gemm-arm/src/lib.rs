//! Metal 直接の GEMM の腕の crate（TASK-1008 のポインタ）。
//!
//! 測定 → 製品の一方向だけに依存する。ここには、外部のカーネルのソースを固定の rev・パス・sha256 で
//! 照らす [`source`] module を置く。bin `fetch-kernel-source` が取得し、後続の腕が [`source::load_source`] で読む。

pub mod source;
