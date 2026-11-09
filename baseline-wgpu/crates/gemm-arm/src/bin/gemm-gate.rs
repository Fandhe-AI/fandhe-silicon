//! 門の run の 1 セルを流す bin（`baseline-wgpu-gemm-arm` の g3 の経路）。
//!
//! 使い方: 環境変数 `FANDHE_SILICON_GEMM_GATE_CELL` に g3 のセルの識別名を 1 つ入れて引数なしで起動する。
//! 1 プロセスにつき 1 セルで、複数のセルは `scripts/metal-session.sh run --combination <名前> --validation-layer
//! --gate-cell g3-<行>-<dtype> -- <この bin>` をセルごとに呼ぶ。引数・ファイルは受けない。
//!
//! 終了コード: 0 は結果の 1 行を標準出力へ出し、比べが一致した / 1 は結果の 1 行を出したが一致しない /
//! 2 は誤用または腕・枠の失敗（標準出力は空、標準エラーに固定の英語の文言だけ。環境変数の値やエラーの中身は出さない）。

use baseline_wgpu_gemm_arm::device::run_g3;
use fandhe_silicon_core::parity::gemm_gate::{GateError, run_selected_from_env};
use std::process::ExitCode;

/// 失敗の種類ごとの固定の文言。
fn message<E>(e: &GateError<E>) -> &'static str {
    match e {
        GateError::NotSelected => "error: no gate cell is selected",
        GateError::InvalidSelection => "error: the selected gate cell is invalid",
        GateError::AlreadyRanInProcess => "error: a gate cell already ran in this process",
        GateError::Arm(_) => "error: the arm failed",
        GateError::Reference(_) => "error: reference failed",
        GateError::Compare(_) => "error: comparison failed",
        GateError::OutputLength { .. } => "error: output length mismatch",
        GateError::OutputKindMismatch => "error: output kind mismatch",
    }
}

fn main() -> ExitCode {
    if std::env::args_os().count() > 1 {
        eprintln!("usage: baseline-wgpu-gemm-gate (no arguments; select the cell by environment)");
        return ExitCode::from(2);
    }
    match run_selected_from_env(run_g3) {
        Ok(r) => {
            println!("{r}");
            if r.comparison().is_match() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("{}", message(&e));
            ExitCode::from(2)
        }
    }
}
