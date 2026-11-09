//! 比較基準の別 workspace の、cooperative matrix の WGSL で行列積を流す腕の置き場（spec TASK-1010 のポインタ）。
//!
//! 製品の crate からは使われない。門の run の枠は `fandhe_silicon_core::parity::gemm_gate` を使う。
//! この crate が持つのは WGSL の 2 本（f32 の入力と binary16 の入力）と、詰め物・切り出し・
//! 起動の引数を作る純粋関数（[`layout`]）。

pub mod layout;

/// f32 の A・B を読み、f32 で累積する行列積の WGSL。
pub const COOPMAT_F32_WGSL: &str = include_str!("shaders/coopmat_f32.wgsl");

/// binary16 の A・B を読み、f32 で累積する行列積の WGSL。
pub const COOPMAT_F16ACC_WGSL: &str = include_str!("shaders/coopmat_f16acc.wgsl");

/// 2 本の WGSL に共通の入口の名前。
pub const ENTRY_POINT: &str = "coopmat_gemm";

/// 2 本の WGSL に共通の workgroup の大きさ。
pub const WORKGROUP_SIZE: [u32; 3] = [32, 1, 1];

/// 1 workgroup が持つ C のタイルの一辺（行・列とも）。
pub const TILE: u32 = 8;
