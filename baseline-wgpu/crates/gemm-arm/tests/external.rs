//! 実行時に照らす WGSL の受け取りと、その起動の形の試験（device は使わない。spec TASK-1010 のポインタ）。
//!
//! 環境変数は書き換えず、値は `from_lookup` に closure で渡す。sha256 と WGSL はすべて試験の中で作る合成の値。

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use baseline_wgpu_gemm_arm::external::{
    ExternalWgsl, ExternalWgslError, KERNEL_SHA256_ENV, KERNEL_SOURCE_ENV, MAX_SOURCE_BYTES,
};
use baseline_wgpu_gemm_arm::layout::{LayoutError, tiled_constants_bytes, tiled_workgroup_count};
use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_inputs::GemmGateRow;

/// 試験ごとの一時ディレクトリ。Drop で消す。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gemm-arm-external-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("write scratch file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn lookup_of(path: &Path, digest: &str) -> impl Fn(&str) -> Option<OsString> {
    let path = path.as_os_str().to_owned();
    let digest = OsString::from(digest);
    move |name| match name {
        KERNEL_SOURCE_ENV => Some(path.clone()),
        KERNEL_SHA256_ENV => Some(digest.clone()),
        _ => None,
    }
}

/// 本文を一時ファイルに書き、計算した sha256 を添えて受け取りを試みる。
fn load(bytes: &[u8]) -> Result<ExternalWgsl, ExternalWgslError> {
    let scratch = Scratch::new();
    let path = scratch.write("kernel.wgsl", bytes);
    ExternalWgsl::from_lookup(lookup_of(&path, &hex(&sha256_bytes(bytes))))
}

/// 形の検査を通る合成の WGSL。束縛の無い `var<workgroup>` を 1 つ持つ。
const GOOD: &str = "\
struct Dims { m: u32, n: u32, k: u32, pad: u32 }
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> c: array<f32>;
@group(0) @binding(3) var<uniform> dims: Dims;
var<workgroup> tile: array<f32, 4>;
@compute @workgroup_size(16, 16, 1)
fn synthetic_main(@builtin(global_invocation_id) id: vec3<u32>) {
    tile[0] = a[0] + b[0];
    c[id.x] = tile[0] + f32(dims.m);
}
";

fn with(from: &str, to: &str) -> String {
    assert!(GOOD.contains(from), "pattern must exist");
    GOOD.replace(from, to)
}

fn load_text(text: &str) -> Result<ExternalWgsl, ExternalWgslError> {
    load(text.as_bytes())
}

#[test]
fn good_source_is_accepted() {
    let loaded = load_text(GOOD).expect("accepted");
    assert_eq!(loaded.sha256(), sha256_bytes(GOOD.as_bytes()));
    assert_eq!(loaded.text(), GOOD);
}

#[test]
fn entry_point_name_is_not_compared() {
    load_text(&with("synthetic_main", "another_name")).expect("accepted");
}

#[test]
fn missing_values_are_reported() {
    let scratch = Scratch::new();
    let path = scratch.write("kernel.wgsl", GOOD.as_bytes());
    let digest = hex(&sha256_bytes(GOOD.as_bytes()));
    let only_path = |name: &str| (name == KERNEL_SOURCE_ENV).then(|| path.as_os_str().to_owned());
    let only_digest = |name: &str| (name == KERNEL_SHA256_ENV).then(|| OsString::from(&digest));
    let neither = |_: &str| None;
    assert_eq!(
        ExternalWgsl::from_lookup(only_path).unwrap_err(),
        ExternalWgslError::Missing
    );
    assert_eq!(
        ExternalWgsl::from_lookup(only_digest).unwrap_err(),
        ExternalWgslError::Missing
    );
    assert_eq!(
        ExternalWgsl::from_lookup(neither).unwrap_err(),
        ExternalWgslError::Missing
    );
}

#[test]
fn malformed_digests_are_rejected() {
    let scratch = Scratch::new();
    let path = scratch.write("kernel.wgsl", GOOD.as_bytes());
    let good = hex(&sha256_bytes(GOOD.as_bytes()));
    let cases = [
        good.to_uppercase(),
        good[..63].to_owned(),
        format!("{good}0"),
        format!("g{}", &good[1..]),
        String::new(),
    ];
    for digest in cases {
        assert_eq!(
            ExternalWgsl::from_lookup(lookup_of(&path, &digest)).unwrap_err(),
            ExternalWgslError::InvalidDigest
        );
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_digest_is_rejected() {
    use std::os::unix::ffi::OsStringExt;
    let scratch = Scratch::new();
    let path = scratch.write("kernel.wgsl", GOOD.as_bytes());
    let source = path.as_os_str().to_owned();
    let lookup = move |name: &str| match name {
        KERNEL_SOURCE_ENV => Some(source.clone()),
        KERNEL_SHA256_ENV => Some(OsString::from_vec(vec![0xff; 64])),
        _ => None,
    };
    assert_eq!(
        ExternalWgsl::from_lookup(lookup).unwrap_err(),
        ExternalWgslError::InvalidDigest
    );
}

#[test]
fn digest_mismatch_is_rejected() {
    let scratch = Scratch::new();
    let path = scratch.write("kernel.wgsl", GOOD.as_bytes());
    let mut digest = hex(&sha256_bytes(GOOD.as_bytes()));
    let first = if digest.starts_with('0') { "1" } else { "0" };
    digest.replace_range(0..1, first);
    assert_eq!(
        ExternalWgsl::from_lookup(lookup_of(&path, &digest)).unwrap_err(),
        ExternalWgslError::DigestMismatch
    );
}

#[test]
fn missing_path_and_directory_are_unreadable() {
    let scratch = Scratch::new();
    let digest = hex(&sha256_bytes(GOOD.as_bytes()));
    let missing = scratch.0.join("absent.wgsl");
    assert_eq!(
        ExternalWgsl::from_lookup(lookup_of(&missing, &digest)).unwrap_err(),
        ExternalWgslError::Unreadable
    );
    assert_eq!(
        ExternalWgsl::from_lookup(lookup_of(&scratch.0, &digest)).unwrap_err(),
        ExternalWgslError::Unreadable
    );
}

#[test]
fn size_limit_boundary() {
    let over = vec![b' '; MAX_SOURCE_BYTES + 1];
    assert_eq!(load(&over).unwrap_err(), ExternalWgslError::TooLarge);
    // 上限ちょうどは大きさでは拒否されない（空白だけなので形の検査で落ちる）。
    let exact = vec![b' '; MAX_SOURCE_BYTES];
    let err = load(&exact).unwrap_err();
    assert_ne!(err, ExternalWgslError::TooLarge);
}

#[test]
fn non_utf8_source_is_rejected() {
    assert_eq!(
        load(&[0xff, 0xfe, 0xfd]).unwrap_err(),
        ExternalWgslError::NotUtf8
    );
}

#[test]
fn checks_run_in_a_fixed_order() {
    let scratch = Scratch::new();
    let digest = hex(&sha256_bytes(GOOD.as_bytes()));
    // 形の誤りの sha と存在しないパス: sha の形が先。
    let missing = scratch.0.join("absent.wgsl");
    assert_eq!(
        ExternalWgsl::from_lookup(lookup_of(&missing, "zz")).unwrap_err(),
        ExternalWgslError::InvalidDigest
    );
    // 上限超えで sha が合う: 上限が先（形の検査まで進まない）。
    let over = vec![0xffu8; MAX_SOURCE_BYTES + 1];
    assert_eq!(load(&over).unwrap_err(), ExternalWgslError::TooLarge);
    // UTF-8 でなく sha が違う: sha の照合が先。
    let path = scratch.write("bad.bin", &[0xff, 0xfe]);
    assert_eq!(
        ExternalWgsl::from_lookup(lookup_of(&path, &digest)).unwrap_err(),
        ExternalWgslError::DigestMismatch
    );
}

#[test]
fn entry_point_shape_is_checked() {
    let none = "@group(0) @binding(0) var<storage, read> a: array<f32>;\n";
    // 入口が 0 個の WGSL は束縛が揃わなくても入口の段で落ちる。
    assert_eq!(load_text(none).unwrap_err(), ExternalWgslError::EntryPoint);
    let two =
        format!("{GOOD}@compute @workgroup_size(16, 16, 1)\nfn second() {{ tile[0] = 1.0; }}\n");
    assert_eq!(load_text(&two).unwrap_err(), ExternalWgslError::EntryPoint);
    let not_compute = with(
        "@compute @workgroup_size(16, 16, 1)\nfn synthetic_main(@builtin(global_invocation_id) id: vec3<u32>) {\n    tile[0] = a[0] + b[0];\n    c[id.x] = tile[0] + f32(dims.m);\n}",
        "@fragment\nfn synthetic_main() -> @location(0) vec4<f32> {\n    return vec4<f32>(a[0] + b[0] + f32(dims.m) + c[0], 0.0, 0.0, 0.0);\n}",
    );
    assert!(matches!(
        load_text(&not_compute).unwrap_err(),
        ExternalWgslError::EntryPoint | ExternalWgslError::Invalid
    ));
}

#[test]
fn workgroup_size_is_checked() {
    for size in ["8, 8, 1", "16, 16, 2", "256, 1, 1"] {
        let text = with(
            "@workgroup_size(16, 16, 1)",
            &format!("@workgroup_size({size})"),
        );
        assert_eq!(
            load_text(&text).unwrap_err(),
            ExternalWgslError::WorkgroupSize,
            "{size}"
        );
    }
}

#[test]
fn bindings_are_checked() {
    let cases = [
        (
            "read_write on a",
            with(
                "var<storage, read> a: array<f32>",
                "var<storage, read_write> a: array<f32>",
            ),
        ),
        (
            "read on c",
            with(
                "var<storage, read_write> c: array<f32>",
                "var<storage, read> c: array<f32>",
            )
            .replace("c[id.x] = tile[0]", "tile[1] = c[id.x] + tile[0]"),
        ),
        (
            "u32 elements",
            with("b: array<f32>", "b: array<u32>").replace("a[0] + b[0]", "a[0] + f32(b[0])"),
        ),
        (
            "fixed size array",
            with("a: array<f32>", "a: array<f32, 8>"),
        ),
        (
            "32 byte uniform",
            with(
                "struct Dims { m: u32, n: u32, k: u32, pad: u32 }",
                "struct Dims { m: u32, n: u32, k: u32, p0: u32, p1: u32, p2: u32, p3: u32, p4: u32 }",
            ),
        ),
        (
            "extra binding",
            with(
                "var<workgroup> tile",
                "@group(0) @binding(4) var<storage, read> extra: array<f32>;\nvar<workgroup> tile",
            ),
        ),
        (
            "missing binding 3",
            with("@group(0) @binding(3) var<uniform> dims: Dims;\n", "")
                .replace(" + f32(dims.m)", ""),
        ),
        (
            "group 1",
            with("@group(0) @binding(1)", "@group(1) @binding(1)"),
        ),
    ];
    for (label, text) in cases {
        let err = load_text(&text).unwrap_err();
        assert_eq!(err, ExternalWgslError::Bindings, "{label}");
    }
}

#[test]
fn parse_and_validation_failures_are_reported() {
    assert_eq!(
        load_text("this is not wgsl {").unwrap_err(),
        ExternalWgslError::Parse
    );
    let f16 = with(
        "var<workgroup> tile: array<f32, 4>;",
        "var<workgroup> tile: array<f32, 4>;\nvar<private> half_value: f16;",
    );
    let err = load_text(&format!("enable f16;\n{f16}")).unwrap_err();
    assert_eq!(err, ExternalWgslError::Invalid);
}

#[test]
fn errors_do_not_print_values() {
    let scratch = Scratch::new();
    let marker = "secret_marker_value";
    let text = format!("{GOOD}\nfn {marker}( {{ bad");
    let path = scratch.write("kernel.wgsl", text.as_bytes());
    let digest = hex(&sha256_bytes(text.as_bytes()));
    let err = ExternalWgsl::from_lookup(lookup_of(&path, &digest)).unwrap_err();
    for shown in [format!("{err}"), format!("{err:?}")] {
        assert!(!shown.contains(marker));
        assert!(!shown.contains(&digest));
        assert!(!shown.contains(&*path.to_string_lossy()));
    }
    let ok = load_text(GOOD).expect("accepted");
    let shown = format!("{ok:?}");
    assert!(!shown.contains("synthetic_main"));
    assert!(!shown.contains(&hex(&ok.sha256())));
}

#[test]
fn launch_shape_values() {
    let expected: [(&str, [u32; 3]); 5] = [
        ("sq512", [32, 32, 1]),
        ("sq2048", [128, 128, 1]),
        ("sq4096", [256, 256, 1]),
        ("rect4096x1024x4096", [64, 256, 1]),
        ("odd37x41x53", [3, 3, 1]),
    ];
    assert_eq!(GemmGateRow::ALL.len(), expected.len());
    for (row, (name, want)) in GemmGateRow::ALL.into_iter().zip(expected) {
        assert_eq!(row.name(), name);
        assert_eq!(tiled_workgroup_count(row.m(), row.n()), Ok(want), "{name}");
    }
}

#[test]
fn launch_constants_are_m_n_k_unpadded() {
    let expected: [(&str, [u32; 3]); 5] = [
        ("sq512", [512, 512, 512]),
        ("sq2048", [2048, 2048, 2048]),
        ("sq4096", [4096, 4096, 4096]),
        ("rect4096x1024x4096", [4096, 1024, 4096]),
        ("odd37x41x53", [37, 41, 53]),
    ];
    for (row, (name, [m, n, k])) in GemmGateRow::ALL.into_iter().zip(expected) {
        assert_eq!(row.name(), name);
        let mut want = [0u8; 16];
        want[0..4].copy_from_slice(&m.to_le_bytes());
        want[4..8].copy_from_slice(&n.to_le_bytes());
        want[8..12].copy_from_slice(&k.to_le_bytes());
        assert_eq!(
            tiled_constants_bytes(row.m(), row.n(), row.k()),
            Ok(want),
            "{name}"
        );
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn launch_shape_overflow_is_an_error() {
    let big = u32::MAX as usize + 1;
    assert_eq!(
        tiled_workgroup_count(big, 1),
        Err(LayoutError::DimensionOverflow)
    );
    assert_eq!(
        tiled_workgroup_count(1, big),
        Err(LayoutError::DimensionOverflow)
    );
    assert_eq!(
        tiled_constants_bytes(1, 1, big),
        Err(LayoutError::DimensionOverflow)
    );
    assert!(tiled_workgroup_count(u32::MAX as usize, u32::MAX as usize).is_ok());
    assert!(tiled_constants_bytes(u32::MAX as usize, 1, 1).is_ok());
}
