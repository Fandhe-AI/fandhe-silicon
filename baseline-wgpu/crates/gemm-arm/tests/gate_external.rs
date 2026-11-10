//! 門の 1 セルを `run_g1`・`run_g2` で流す device の試験（TASK-1010 のポインタ）。
//!
//! device の要る専用の回でだけ `--ignored` で流す。既定の `cargo test` では流れない。
//! 枠の入口はテストのバイナリにつき 1 回なので、試験は 1 本だけにする。
//! 標準エラーには何も書かず、標準出力には結果の行と sha256 の行だけを出す。

// `run_g2` は `unsafe fn`。
#![allow(unsafe_code)]

use baseline_wgpu_gemm_arm::device::{ArmError, run_g1};
use baseline_wgpu_gemm_arm::run_g2;
use fandhe_silicon_core::parity::gemm_gate::run_selected_from_env;
use fandhe_silicon_core::parity::gemm_population::GemmGateArm;

/// 32 バイトを 64 桁の小文字の 16 進にする。
fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn external_gate_cell() {
    let mut digest = None;
    // 枠は PARITY-36 のポインタ。環境変数で選んだ 1 セルを腕に応じた経路で流す。
    let outcome = run_selected_from_env(|cell, inputs| {
        let (out, sha) = match cell.arm() {
            GemmGateArm::G1 => run_g1(cell, inputs)?,
            GemmGateArm::G2 => {
                // SAFETY: この試験は `#[ignore]` 付きで、専用の device の回の手順でだけ流す。
                // その手順が環境変数で渡すソースは `run_g2` の `# Safety` の条件を満たすと確かめたものだけ。
                // 入口は device を作る前に検証レイヤーの有効と sha256 の照合を確かめ、通らなければ `Err` を返す。
                unsafe { run_g2(cell, inputs) }?
            }
            _ => return Err(ArmError::WrongArm),
        };
        digest = Some(sha);
        Ok(out)
    });
    let result = match outcome {
        Ok(result) => result,
        Err(e) => panic!("gate cell failed: {e}"),
    };
    let digest = digest.expect("arm did not report source digest");
    assert!(result.comparison().is_match(), "{result}");
    assert_eq!(result.bit_mismatches(), 0, "{result}");
    assert_eq!(result.max_abs_diff(), 0.0, "{result}");
    assert!(result.validation_layer(), "{result}");
    println!("{result}");
    println!("source_sha256={}", hex(&digest));
}
