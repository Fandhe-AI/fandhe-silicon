//! 2 本の WGSL を `wgpu::naga` の再公開で解析・検証する試験（device は使わない）。
//!
//! 確かめる事項: 要る能力があれば通る、能力が無ければ拒否される、入口が 1 つずつで名前と
//! workgroup の大きさが定数と一致する。項目のポインタ: PARITY-36。

use baseline_wgpu_gemm_arm::{COOPMAT_F16ACC_WGSL, COOPMAT_F32_WGSL, ENTRY_POINT, WORKGROUP_SIZE};
use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};

fn parse_and_validate(source: &str, caps: Capabilities) -> Result<wgpu::naga::Module, String> {
    let module = wgpu::naga::front::wgsl::parse_str(source).map_err(|e| format!("parse: {e:?}"))?;
    Validator::new(ValidationFlags::all(), caps)
        .validate(&module)
        .map_err(|e| format!("validate: {e:?}"))?;
    Ok(module)
}

fn f16acc_caps() -> Capabilities {
    Capabilities::COOPERATIVE_MATRIX | Capabilities::SHADER_FLOAT16
}

#[test]
fn f32_passes_with_cooperative_matrix() {
    parse_and_validate(COOPMAT_F32_WGSL, Capabilities::COOPERATIVE_MATRIX).expect("f32 shader");
}

#[test]
fn f16acc_passes_with_cooperative_matrix_and_float16() {
    parse_and_validate(COOPMAT_F16ACC_WGSL, f16acc_caps()).expect("f16acc shader");
}

#[test]
fn both_are_rejected_without_capabilities() {
    assert!(parse_and_validate(COOPMAT_F32_WGSL, Capabilities::empty()).is_err());
    assert!(parse_and_validate(COOPMAT_F16ACC_WGSL, Capabilities::empty()).is_err());
}

#[test]
fn f16acc_is_rejected_without_float16() {
    assert!(parse_and_validate(COOPMAT_F16ACC_WGSL, Capabilities::COOPERATIVE_MATRIX).is_err());
}

#[test]
fn entry_points_match_constants() {
    let cases = [
        (COOPMAT_F32_WGSL, Capabilities::COOPERATIVE_MATRIX),
        (COOPMAT_F16ACC_WGSL, f16acc_caps()),
    ];
    for (source, caps) in cases {
        let module = parse_and_validate(source, caps).expect("shader");
        assert_eq!(module.entry_points.len(), 1);
        let entry = &module.entry_points[0];
        assert_eq!(entry.name, ENTRY_POINT);
        assert_eq!(entry.workgroup_size, WORKGROUP_SIZE);
    }
}
