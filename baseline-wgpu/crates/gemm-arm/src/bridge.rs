//! 下位の API（wgpu の実験的な機能の同意の印・検査を外したシェーダーの作成）の unsafe な口を呼ぶ橋（spec TASK-1010 のポインタ）。
//!
//! この crate で `unsafe` を持つのはこの module だけ。`device` の経路が device の要求に渡す値をここから受け取る。
//! 外へ出すのは `unsafe fn` の [`run_g2`] だけで、実行時に受けるシェーダーの条件は呼び出し側が保証する。

use fandhe_silicon_core::parity::gemm_gate::{GateInputs, GateOutput};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, GemmGateCell};

use crate::device::{ArmError, check_g2_request, run_tiled};

/// 実験的な機能を使う同意の印を作る。
pub(crate) fn experimental_features() -> wgpu::ExperimentalFeatures {
    // SAFETY: この関数が返す値で要求する device（g3 の経路）が実験的な機能として使うのは
    // cooperative matrix の WGSL の作成と dispatch だけで、その device のシェーダーの作成は
    // 検査つきの口だけを使う。`create_unchecked_module` は `run_g2` が別に要求する device
    // （実験的な機能を要求しない）のシェーダーの作成だけが使い、この値を受けない。
    unsafe { wgpu::ExperimentalFeatures::enabled() }
}

/// 検査を外してシェーダーを作る。呼び出し元は `run_g2` が渡す共通の本体だけ。
fn create_unchecked_module(device: &wgpu::Device, wgsl: &str) -> wgpu::ShaderModule {
    // SAFETY: 渡すテキストは、実行時に渡された sha256 と一致し、compute の入口の数・workgroup の大きさ・
    // 束縛の宣言の形の照らしを通ったものだけ（共通の本体が `ExternalWgsl` から取る）。束縛する buffer の長さは
    // M×K・K×N・M×N の f32 と 16 バイトの uniform で、uniform の値はその M・N・K。呼ぶのは検証レイヤーが
    // 有効なことを確かめた後だけ。テキストの入口が束縛を宣言どおりにだけ使うことは、`run_g2` の
    // `# Safety` で呼び出し側が保証する。
    unsafe {
        device.create_shader_module_trusted(
            wgpu::ShaderModuleDescriptor {
                label: Some("gemm-arm-external-unchecked"),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            },
            wgpu::ShaderRuntimeChecks::unchecked(),
        )
    }
}

/// 実行時に環境変数で受けた WGSL を、検査を外したシェーダーの作成で device に流し、出力とソースの sha256 を返す。
///
/// 手順は `check_g2_request`（腕・dtype・入力の長さ・検証レイヤー）、ソースの受け取り、device の要求、
/// シェーダー・パイプラインの作成、1 回の投入、読み戻しの順。成果物（パス・バイト列・digest）は引数で受けない。
/// 検証レイヤーが無効、ソースの設定が未設定・不正、照合が通らないときは device を作る前に `Err` を返す。
///
/// # Safety
///
/// 環境変数が指すソースの sha256 の照合と形の照らしは、内容がそのバイト列であることと、入口の数・
/// workgroup の大きさ・束縛の宣言の形しか保証しない。この関数はそのソースのシェーダーを
/// `wgpu::ShaderRuntimeChecks::unchecked()` で作るので、呼び出し側は、そのソースの入口が、`@group(0)` の
/// binding 0・1 を f32 の行優先の列 A・B（読み取りだけ）、binding 2 を f32 の行優先の列 C、binding 3 を
/// M・N・K の定数として宣言どおりにだけ使い、範囲は M・N・K から決まり、範囲外のアクセスや競合する書き込み、
/// 無限ループを持たない監査済みのカーネルであることを保証しなければならない。
pub unsafe fn run_g2(
    cell: GemmGateCell,
    inputs: &GateInputs,
) -> Result<(GateOutput, [u8; 32]), ArmError> {
    check_g2_request(cell, inputs)?;
    run_tiled(GemmGateArm::G2, cell, inputs, create_unchecked_module)
}
