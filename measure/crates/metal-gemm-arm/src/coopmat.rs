//! cooperative matrix の f32 GEMM の MSL と起動の形を持つ純粋な module（TASK-1009 のポインタ）。
//!
//! どの OS でもコンパイルされ、device も外部コマンドも使わない。MSL の文字列と入口名、
//! 門のセルから引く起動の形 [`CoopmatLaunch`]、offline compiler に渡す引数の列 [`compile_commands`] を持つ。

use std::ffi::OsString;
use std::path::Path;

use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

/// MSL のソース。
pub const COOPMAT_MSL: &str = include_str!("shaders/coopmat_gemm_f32.metal");

/// MSL の入口の名前。
pub const COOPMAT_ENTRY: &str = "coopmat_gemm_f32";

/// threadgroup あたりのスレッド数（x。y・z は 1）。
pub const COOPMAT_THREADS: usize = 32;

/// 作業のディレクトリの下に置く MSL のファイル名。
pub const MSL_FILE_NAME: &str = "coopmat_gemm_f32.metal";

/// 作業のディレクトリの下に置く中間物のファイル名。
pub const AIR_FILE_NAME: &str = "coopmat_gemm_f32.air";

/// 作業のディレクトリの下に置く metallib のファイル名。
pub const METALLIB_FILE_NAME: &str = "coopmat_gemm_f32.metallib";

/// 門の 1 セルを 1 回投入するときの起動の形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoopmatLaunch {
    /// threadgroup あたりのスレッド数（x）。
    pub threads: usize,
    /// threadgroup 数の幅（実効 N / 8）。
    pub groups_width: usize,
    /// threadgroup 数の高さ（実効 M / 8）。
    pub groups_height: usize,
    /// 8 の倍数へ切り上げた後の M。
    pub eff_m: usize,
    /// 8 の倍数へ切り上げた後の N。
    pub eff_n: usize,
    /// 8 の倍数へ切り上げた後の K。
    pub eff_k: usize,
}

/// 行ごとの実効の (M, N, K)。
fn effective_dims(row: GemmGateRow) -> (usize, usize, usize) {
    match row {
        GemmGateRow::Sq512 => (512, 512, 512),
        GemmGateRow::Sq2048 => (2048, 2048, 2048),
        GemmGateRow::Sq4096 => (4096, 4096, 4096),
        GemmGateRow::Rect4096x1024x4096 => (4096, 1024, 4096),
        GemmGateRow::Odd37x41x53 => (40, 48, 56),
    }
}

/// セルの起動の形。腕が `GemmGateArm::G7` の f32 のセルだけ `Some`、ほかは `None`。
pub fn coopmat_launch_for(cell: GemmGateCell) -> Option<CoopmatLaunch> {
    if cell.arm() != GemmGateArm::G7 || cell.dtype() != GemmInputDtype::F32 {
        return None;
    }
    let (eff_m, eff_n, eff_k) = effective_dims(cell.row());
    Some(CoopmatLaunch {
        threads: COOPMAT_THREADS,
        groups_width: eff_n / 8,
        groups_height: eff_m / 8,
        eff_m,
        eff_n,
        eff_k,
    })
}

/// `xcrun` に渡す 2 つの引数の列（`xcrun` 自身は含めない）。
///
/// 1 つ目が MSL から `.air` を作り、2 つ目が `.air` から `.metallib` を作る。
/// 入出力は `dir` の下の固定のファイル名。起動はしない。
pub fn compile_commands(dir: &Path) -> [Vec<OsString>; 2] {
    let msl = dir.join(MSL_FILE_NAME);
    let air = dir.join(AIR_FILE_NAME);
    let lib = dir.join(METALLIB_FILE_NAME);
    let first = vec![
        "-sdk".into(),
        "macosx".into(),
        "metal".into(),
        "-c".into(),
        "-fmetal-math-mode=safe".into(),
        "-o".into(),
        air.clone().into_os_string(),
        msl.into_os_string(),
    ];
    let second = vec![
        "-sdk".into(),
        "macosx".into(),
        "metallib".into(),
        "-o".into(),
        lib.into_os_string(),
        air.into_os_string(),
    ];
    [first, second]
}
