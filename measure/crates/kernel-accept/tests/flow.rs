//! `flow` の純粋な部品の試験（TASK-485 のポインタ）。device には触れない。値はすべて試験の中で計算した合成のもの。

use std::ffi::OsString;
use std::path::PathBuf;

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::gemm_inputs::{GemmGateRow, GemmInputDtype};
use fandhe_silicon_core::parity::gemm_population::{GemmGateArm, gate_cells};
use measure_kernel_accept::flow::{
    ADD_ENV, ADDEND_SEED, DispatchShape, FLOW_ROWS, FlowError, FlowKernels, PLAIN_GEMM_ENV,
    RELU_ENV, Role, SPECIALIZED_GEMM_ENV, add_shape, addend, f32_from_ne_bytes, f32_ne_bytes,
    gemm_role, gemm_shape, load_kernel, plain_gemm_shape, reference_add, reference_relu,
    relu_shape, resolve_entry, specialized_gemm_shape, u32_ne_bytes,
};
use measure_kernel_accept::manifest::{Manifest, parse};
use measure_metal_gemm_arm::launch::launch_for;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn as_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks(4)
        .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn const_at(shape: &DispatchShape, slot: u32) -> Vec<u32> {
    let (_, b) = shape
        .constants
        .iter()
        .find(|(s, _)| *s == slot)
        .expect("slot");
    as_u32s(b)
}

struct Fixture {
    dir: PathBuf,
    manifest: Manifest,
    sources: [String; 2],
}

impl Fixture {
    /// 合成の 2 ファイル（特殊化する行を含むものと含まないもの）と、その一覧。
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("kernel-accept-flow-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let sources = [
            String::from("// synthetic one\nkernel void k_spec() {}\nkernel void k_plain() {}\n"),
            String::from("// synthetic two\nkernel void k_add() {}\nkernel void k_relu() {}\n"),
        ];
        std::fs::write(dir.join("one.src"), &sources[0]).expect("write");
        std::fs::write(dir.join("two.src"), &sources[1]).expect("write");
        let rev = hex(&sha256_bytes(b"rev"))[..40].to_owned();
        let h1 = hex(&sha256_bytes(sources[0].as_bytes()));
        let h2 = hex(&sha256_bytes(sources[1].as_bytes()));
        let text = [
            format!("{rev} a/one.src {h1} specialized k_spec"),
            format!("{rev} a/one.src {h1} msl k_plain"),
            format!("{rev} b/two.src {h2} msl k_add"),
            format!("{rev} b/two.src {h2} msl k_relu"),
        ]
        .join("\n");
        Self {
            dir,
            manifest: parse(&text).expect("manifest"),
            sources,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn lookup_with<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| OsString::from(v))
    }
}

const GOOD: [(&str, &str); 4] = [
    (SPECIALIZED_GEMM_ENV, "k_spec"),
    (PLAIN_GEMM_ENV, "k_plain"),
    (ADD_ENV, "k_add"),
    (RELU_ENV, "k_relu"),
];

#[test]
fn roles_map_rows_and_modes() {
    assert_eq!(gemm_role(GemmGateRow::Sq512), Some(Role::SpecializedGemm));
    assert_eq!(gemm_role(GemmGateRow::Sq2048), Some(Role::SpecializedGemm));
    assert_eq!(gemm_role(GemmGateRow::Odd37x41x53), Some(Role::PlainGemm));
    assert_eq!(FLOW_ROWS.len(), 3);
    assert_eq!(
        Role::SpecializedGemm.mode(),
        measure_kernel_accept::manifest::Mode::Specialized
    );
    for r in [Role::PlainGemm, Role::Add, Role::Relu] {
        assert_eq!(r.mode(), measure_kernel_accept::manifest::Mode::Msl);
    }
}

#[test]
fn specialized_shapes_follow_the_launch_values() {
    for row in [GemmGateRow::Sq512, GemmGateRow::Sq2048] {
        let cell = gate_cells()
            .into_iter()
            .find(|c| {
                c.arm() == GemmGateArm::G6 && c.row() == row && c.dtype() == GemmInputDtype::F32
            })
            .expect("cell");
        let launch = launch_for(cell).expect("launch");
        assert_eq!(launch.eff_m, row.m());
        assert_eq!(launch.eff_n, row.n());
        assert_eq!(launch.eff_k, row.k());
        assert_eq!(launch.threadgroup_memory % 16, 0);

        let s = specialized_gemm_shape(row).expect("shape");
        let (m, n, k) = (row.m() as u32, row.n() as u32, row.k() as u32);
        let (gh, gw) = (launch.groups_height as u32, launch.groups_width as u32);
        assert_eq!(s.threads, [launch.threads as u32, 1, 1]);
        assert_eq!(s.groups, [gw, gh, 1]);
        assert_eq!(s.layout, vec![0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            s.threadgroup_memory,
            Some((0, launch.threadgroup_memory as u32))
        );
        assert_eq!(s.function_constants.len(), 18);
        assert_eq!(s.constants.len(), 4);
        for (_, b) in &s.constants {
            assert!(b.len() == 12 || b.len() == 16);
        }
        assert_eq!(s.constants[0].1.len(), 12);
        assert_eq!(const_at(&s, 3), vec![m, n, k]);
        assert_eq!(const_at(&s, 4), vec![k, n, 0, 0]);
        assert_eq!(const_at(&s, 5), vec![0, 0, gh, gw]);
        assert_eq!(const_at(&s, 6), vec![1, k, 0, 0]);
        assert_eq!(gemm_shape(row).expect("same").groups, s.groups);
    }
    assert!(gate_cells().into_iter().any(|c| c.arm() == GemmGateArm::G6
        && c.row() == GemmGateRow::Sq2048
        && c.dtype() == GemmInputDtype::F32));
}

#[test]
fn plain_shape_uses_sixteen_square_groups() {
    let row = GemmGateRow::Odd37x41x53;
    let s = plain_gemm_shape(row).expect("shape");
    let (m, n, k) = (row.m() as u32, row.n() as u32, row.k() as u32);
    assert_eq!(s.threads, [16, 16, 1]);
    assert_eq!(s.groups, [n.div_ceil(16), m.div_ceil(16), 1]);
    assert_eq!(s.layout, vec![0, 1, 2, 3]);
    assert_eq!(s.constants.len(), 1);
    assert_eq!(s.constants[0].0, 3);
    assert_eq!(s.constants[0].1.len(), 12);
    assert_eq!(const_at(&s, 3), vec![m, n, k]);
    assert_eq!(s.threadgroup_memory, None);
    assert!(s.function_constants.is_empty());
}

#[test]
fn elementwise_shapes_follow_the_length() {
    for (len, groups) in [(1usize, 1u32), (256, 1), (257, 2), (512, 2), (513, 3)] {
        let a = add_shape(len).expect("add");
        assert_eq!(a.threads, [256, 1, 1]);
        assert_eq!(a.groups, [groups, 1, 1]);
        assert_eq!(a.layout, vec![0, 1, 2, 3]);
        assert_eq!(const_at(&a, 3), vec![len as u32]);
        let r = relu_shape(len).expect("relu");
        assert_eq!(r.threads, [256, 1, 1]);
        assert_eq!(r.groups, [groups, 1, 1]);
        assert_eq!(r.layout, vec![0, 1, 2]);
        assert_eq!(const_at(&r, 2), vec![len as u32]);
        assert!(a.threadgroup_memory.is_none() && r.threadgroup_memory.is_none());
    }
    assert!(add_shape(0).is_err());
    assert!(relu_shape(0).is_err());
    if usize::BITS > 32 {
        assert_eq!(add_shape(usize::MAX).unwrap_err(), FlowError::SizeOverflow);
    }
}

#[test]
fn addend_is_deterministic_with_the_gemm_output_length() {
    for row in FLOW_ROWS {
        let a = addend(row).expect("addend");
        assert_eq!(a.len(), row.m() * row.n());
        if row == GemmGateRow::Odd37x41x53 {
            assert_eq!(a, addend(row).expect("again"));
        }
    }
    let mut rng = fandhe_silicon_core::parity::gemm_inputs::SplitMix64::new(ADDEND_SEED);
    let a = addend(GemmGateRow::Odd37x41x53).expect("addend");
    assert_eq!(a[0].to_bits(), rng.next_signed_unit_f32().to_bits());
    assert_eq!(a[1].to_bits(), rng.next_signed_unit_f32().to_bits());
}

#[test]
fn reference_add_and_relu_follow_the_formulas() {
    let g = [1.0f32, -2.0, 0.5, 0.0];
    let b = [0.5f32, 1.0, -0.5, 0.0];
    let s = reference_add(&g, &b).expect("add");
    assert_eq!(s, vec![1.5, -1.0, 0.0, 0.0]);
    assert_eq!(
        reference_add(&g, &b[..3]).unwrap_err(),
        FlowError::LengthMismatch
    );
    let r = reference_relu(&[-1.0, 0.0, -0.0, 2.5, f32::NAN]);
    assert_eq!(r[0].to_bits(), 0.0f32.to_bits());
    assert_eq!(r[1].to_bits(), 0.0f32.to_bits());
    assert_eq!(r[2].to_bits(), 0.0f32.to_bits());
    assert_eq!(r[3], 2.5);
    assert_eq!(r[4].to_bits(), 0.0f32.to_bits());
}

#[test]
fn byte_conversions_round_trip_and_reject_odd_lengths() {
    let v = [1.0f32, -0.0, 3.5];
    let b = f32_ne_bytes(&v);
    assert_eq!(b.len(), 12);
    let back = f32_from_ne_bytes(&b).expect("back");
    assert_eq!(
        back.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        v.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(
        f32_from_ne_bytes(&b[..5]).unwrap_err(),
        FlowError::LengthMismatch
    );
    assert_eq!(u32_ne_bytes(&[1, 2]).len(), 8);
}

#[test]
fn entries_resolve_with_the_expected_rows() {
    let f = Fixture::new("ok");
    let lookup = lookup_with(&GOOD);
    for role in [
        Role::SpecializedGemm,
        Role::PlainGemm,
        Role::Add,
        Role::Relu,
    ] {
        resolve_entry(&f.manifest, role, &lookup).expect("row");
    }
    let k = load_kernel(&f.manifest, &f.dir, Role::SpecializedGemm, &lookup).expect("kernel");
    assert_eq!(k.entry(), "k_spec");
    assert_eq!(k.source(), f.sources[0]);
    for row in FLOW_ROWS {
        let ks = FlowKernels::load(&f.manifest, &f.dir, row, &lookup).expect("kernels");
        assert_eq!(ks.add.entry(), "k_add");
        assert_eq!(ks.relu.entry(), "k_relu");
        assert_eq!(ks.gemm.role(), gemm_role(row).unwrap());
    }
}

#[test]
fn entry_checks_fail_closed_without_leaking_values() {
    let f = Fixture::new("err");
    let secret_name = "zz_unlisted_name";
    let long = "a".repeat(65);
    let cases: Vec<(Vec<(&str, &str)>, FlowError)> = vec![
        (vec![], FlowError::EntryMissing(Role::Add)),
        (vec![(ADD_ENV, "")], FlowError::EntryMissing(Role::Add)),
        (
            vec![(ADD_ENV, "1bad")],
            FlowError::EntryMalformed(Role::Add),
        ),
        (
            vec![(ADD_ENV, "bad-name")],
            FlowError::EntryMalformed(Role::Add),
        ),
        (
            vec![(ADD_ENV, long.as_str())],
            FlowError::EntryMalformed(Role::Add),
        ),
        (
            vec![(ADD_ENV, secret_name)],
            FlowError::EntryNotListed(Role::Add),
        ),
        (
            vec![(ADD_ENV, "k_spec")],
            FlowError::ModeMismatch(Role::Add),
        ),
    ];
    for (pairs, expected) in cases {
        let lookup = lookup_with(&pairs);
        let err = resolve_entry(&f.manifest, Role::Add, &lookup).unwrap_err();
        assert_eq!(err, expected);
        let text = err.to_string();
        assert!(!text.contains(secret_name) && !text.contains("k_spec") && !text.contains("bad"));
    }
    let lookup = lookup_with(&[(SPECIALIZED_GEMM_ENV, "k_plain")]);
    assert_eq!(
        resolve_entry(&f.manifest, Role::SpecializedGemm, &lookup).unwrap_err(),
        FlowError::ModeMismatch(Role::SpecializedGemm)
    );
}

#[test]
fn a_manifest_listing_an_entry_twice_does_not_parse() {
    let rev = hex(&sha256_bytes(b"rev"))[..40].to_owned();
    let h = hex(&sha256_bytes(b"x"));
    let text = [
        format!("{rev} a/x.src {h} msl dup_name"),
        format!("{rev} a/x.src {h} msl dup_name"),
    ]
    .join("\n");
    assert!(parse(&text).is_err());
    assert!(
        !FlowError::EntryListedTwice(Role::Relu)
            .to_string()
            .contains("dup_name")
    );
}

#[test]
fn unreadable_or_mismatched_sources_are_rejected_before_any_device() {
    let f = Fixture::new("src");
    let lookup = lookup_with(&GOOD);
    std::fs::write(f.dir.join("two.src"), "// tampered\n").expect("write");
    let err = FlowKernels::load(&f.manifest, &f.dir, GemmGateRow::Sq512, &lookup).unwrap_err();
    assert_eq!(err, FlowError::SourceUnreadable(Role::Add));
    assert!(!err.to_string().contains("k_add"));
    std::fs::remove_file(f.dir.join("one.src")).expect("remove");
    let err = load_kernel(&f.manifest, &f.dir, Role::PlainGemm, &lookup).unwrap_err();
    assert_eq!(err, FlowError::SourceUnreadable(Role::PlainGemm));
}

#[test]
fn gemm_input_lengths_are_checked_against_the_row() {
    use fandhe_silicon_core::parity::gemm_inputs::generate_f32;
    use measure_kernel_accept::flow::check_gemm_input_lengths;

    let odd = generate_f32(GemmGateRow::Odd37x41x53).expect("inputs");
    assert_eq!(
        check_gemm_input_lengths(GemmGateRow::Odd37x41x53, &odd),
        Ok(())
    );
    assert_eq!(
        check_gemm_input_lengths(GemmGateRow::Sq512, &odd),
        Err(FlowError::LengthMismatch)
    );
}

#[test]
fn kernel_roles_are_checked_against_the_row_and_stage() {
    use measure_kernel_accept::flow::{check_gemm_role, check_role};

    let f = Fixture::new("role");
    let lookup = lookup_with(&GOOD);
    let spec = load_kernel(&f.manifest, &f.dir, Role::SpecializedGemm, &lookup).expect("spec");
    let plain = load_kernel(&f.manifest, &f.dir, Role::PlainGemm, &lookup).expect("plain");
    let add = load_kernel(&f.manifest, &f.dir, Role::Add, &lookup).expect("add");
    assert_eq!(check_gemm_role(GemmGateRow::Sq512, &spec), Ok(()));
    assert_eq!(check_gemm_role(GemmGateRow::Odd37x41x53, &plain), Ok(()));
    assert_eq!(
        check_gemm_role(GemmGateRow::Sq512, &plain),
        Err(FlowError::RoleMismatch(Role::SpecializedGemm))
    );
    assert_eq!(
        check_gemm_role(GemmGateRow::Odd37x41x53, &spec),
        Err(FlowError::RoleMismatch(Role::PlainGemm))
    );
    assert_eq!(check_role(&add, Role::Add), Ok(()));
    assert_eq!(
        check_role(&add, Role::Relu),
        Err(FlowError::RoleMismatch(Role::Relu))
    );
    assert!(
        !FlowError::RoleMismatch(Role::Add)
            .to_string()
            .contains("k_add")
    );
}
