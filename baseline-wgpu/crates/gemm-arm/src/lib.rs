//! 比較基準の別 workspace の、cooperative matrix の WGSL で行列積を流す腕の置き場（spec TASK-1010 のポインタ）。
//!
//! 製品の crate からは使われない。門の run の枠は `fandhe_silicon_core::parity::gemm_gate` を使う。
//! この crate が持つのは WGSL の 2 本（f32 の入力と binary16 の入力）と、詰め物・切り出し・
//! 起動の引数を作る純粋関数（[`layout`]）。
//! device の経路（[`device`]）は wgpu で g3 のセルを 1 つ流し、bin `baseline-wgpu-gemm-gate` が
//! 門の run の枠から 1 プロセスにつき 1 セルを呼ぶ。
//! 実行時に環境変数で受けて照らす WGSL（[`external`]）と、その起動の形（[`layout`] の `tiled_*`）も持つ（TASK-1010 のポインタ）。
//! その WGSL を流す device の経路は 2 つで、`run_g1`（[`device`]。safe）と `run_g2`（橋の module の `unsafe fn`）。

/// 下位の API の unsafe な口を呼ぶ橋。この crate で `unsafe` を持つのはこの module だけ。
#[allow(unsafe_code)]
mod bridge;
pub mod device;
pub mod external;
pub mod layout;

pub use bridge::run_g2;

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
