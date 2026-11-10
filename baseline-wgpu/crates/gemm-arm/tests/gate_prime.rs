//! 門の 1 セルを `run_g3prime` で流す device の試験（TASK-1010 のポインタ）。
//!
//! device の要る専用の回でだけ `--ignored` で流す。既定の `cargo test` では流れない。
//! 枠の入口はテストのバイナリにつき 1 回なので、試験は 1 本だけにする。
//! 標準エラーには何も書かず、標準出力には結果の行と sha256 の行だけを出す。

// `run_g3prime` は `unsafe fn`。
#![allow(unsafe_code)]

use baseline_wgpu_gemm_arm::device::ArmError;
use baseline_wgpu_gemm_arm::run_g3prime;
use fandhe_silicon_core::parity::gemm_gate::run_selected_from_env;
use fandhe_silicon_core::parity::gemm_population::GemmGateArm;

/// 32 バイトを 64 桁の小文字の 16 進にする。
fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn prime_gate_cell() {
    let mut digest = None;
    // 枠は PARITY-36 のポインタ。環境変数で選んだ 1 セルを、腕が合うときだけ直接の経路で流す。
    let outcome = run_selected_from_env(|cell, inputs| {
        if cell.arm() != GemmGateArm::G3Prime {
            return Err(ArmError::WrongArm);
        }
        // SAFETY: この試験は `#[ignore]` 付きで、専用の device の回の手順でだけ流す。
        // その手順で流すソース（この crate の WGSL）は、`run_g3prime` の `# Safety` の条件を満たすと確かめたものだけ。
        // 捕捉したテキストは、そのソースから同じプロセスで、この workspace の `Cargo.lock` で固定した版の wgpu が作ったもので、
        // 回の手順がその sha256 を回の記録の値と照らす。この試験のバイナリは試験が 1 本で、呼び出しの間に
        // 同じプロセスのほかのコードが同じ形の Debug の記録を出さない。
        // 入口は device を作る前に検証レイヤーの有効を確かめ、Metal でコンパイルする前に、受け取る記録が
        // ちょうど 1 つであることと、言語版・束縛の番号・大きさの表の形を確かめ、通らなければ `Err` を返す。
        let (out, sha) = unsafe { run_g3prime(cell, inputs) }?;
        digest = Some(sha);
        Ok(out)
    });
    let result = match outcome {
        Ok(result) => result,
        Err(e) => panic!("gate cell failed: {e}"),
    };
    let digest = digest.expect("arm did not report captured digest");
    assert!(result.comparison().is_match(), "{result}");
    assert_eq!(result.bit_mismatches(), 0, "{result}");
    assert_eq!(result.max_abs_diff(), 0.0, "{result}");
    assert!(result.validation_layer(), "{result}");
    println!("{result}");
    println!("captured_sha256={}", hex(&digest));
}
