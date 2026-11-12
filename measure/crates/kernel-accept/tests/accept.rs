//! `accept` の純粋な部品の試験（TASK-485 のポインタ）。値はすべて試験の中で計算した合成のもの。

use std::path::Path;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{FunctionConstant, FunctionConstantValue, PipelineInput};
use fandhe_silicon_core::hash::sha256_bytes;
use measure_kernel_accept::accept::{
    entries_match, error_kind, pipeline_input, select_row, source_file,
};
use measure_kernel_accept::manifest::{Manifest, parse};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn manifest() -> Manifest {
    let r = &hex(&sha256_bytes(b"rev"))[..40];
    let a = hex(&sha256_bytes(b"a"));
    let b = hex(&sha256_bytes(b"b"));
    let text = [
        format!("{r} src/a.metal {a} msl alpha"),
        format!("{r} src/a.metal {a} specialized beta"),
        format!("{r} src/b.metal {b} msl gamma"),
    ]
    .join("\n");
    parse(&text).expect("manifest")
}

#[test]
fn select_row_needs_both_path_and_entry() {
    let m = manifest();
    assert_eq!(
        select_row(&m, "src/a.metal", "beta").map(|r| r.line()),
        Some(2)
    );
    assert!(select_row(&m, "src/b.metal", "beta").is_none());
    assert!(select_row(&m, "src/a.metal", "nope").is_none());
}

#[test]
fn source_file_uses_only_the_file_name() {
    let m = manifest();
    let row = m.find("gamma").expect("row");
    assert_eq!(
        source_file(Path::new("dir"), row),
        Path::new("dir").join("b.metal")
    );
}

#[test]
fn entries_match_needs_same_names_in_same_order() {
    let m = manifest();
    let good = "kernel void alpha(int x) {}\nkernel void beta(int y) {}";
    assert!(entries_match(&m, "src/a.metal", good));
    let swapped = "kernel void beta(int y) {}\nkernel void alpha(int x) {}";
    assert!(!entries_match(&m, "src/a.metal", swapped));
    assert!(!entries_match(
        &m,
        "src/a.metal",
        "kernel void alpha(int x) {}"
    ));
    let extra = format!("{good}\nkernel void delta(int z) {{}}");
    assert!(!entries_match(&m, "src/a.metal", &extra));
    assert!(!entries_match(&m, "src/a.metal", ""));
    assert!(!entries_match(&m, "src/none.metal", good));
}

#[test]
fn pipeline_input_follows_the_row_mode() {
    let m = manifest();
    let constants = [FunctionConstant {
        index: 0,
        value: FunctionConstantValue::U32(1),
    }];
    let plain = pipeline_input(m.find("alpha").expect("row"), "src", &constants);
    assert!(matches!(
        plain,
        PipelineInput::Msl {
            entry_point: "alpha",
            ..
        }
    ));
    let spec = pipeline_input(m.find("beta").expect("row"), "src", &constants);
    match spec {
        PipelineInput::MslSpecialized {
            entry_point,
            function_constants,
            ..
        } => {
            assert_eq!(entry_point, "beta");
            assert_eq!(function_constants.len(), 1);
        }
        _ => panic!("expected the specialized input"),
    }
}

#[test]
fn error_kind_is_fixed_and_drops_the_reason() {
    let msl = ContractError::ArtifactRejected {
        kind: "msl",
        reason: "reason-marker".to_owned(),
    };
    assert_eq!(error_kind(&msl), "artifact-rejected-msl");
    let pipe = ContractError::ArtifactRejected {
        kind: "compute-pipeline",
        reason: "reason-marker".to_owned(),
    };
    assert_eq!(error_kind(&pipe), "artifact-rejected-pipeline");
    let other = ContractError::ArtifactRejected {
        kind: "other",
        reason: "reason-marker".to_owned(),
    };
    assert_eq!(error_kind(&other), "artifact-rejected");
    assert_eq!(error_kind(&ContractError::Timeout), "timeout");
    assert_eq!(
        error_kind(&ContractError::DeviceUnavailable(
            "reason-marker".to_owned()
        )),
        "device-unavailable"
    );
    for e in [&msl, &pipe, &other] {
        assert!(!error_kind(e).contains("marker"));
    }
}
