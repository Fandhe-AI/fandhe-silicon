//! GEMM の門の run の入力の生成の確認（spec TASK-1005 の 2/6）。
//! feature `test-support` の裏にだけ存在する検証用の道具を対象にする。
//!
//! sha256 の値は、本リポの製品のコードが出す既知の出力として固定する
//! （`hash_known_values.rs` と同じ流儀）。大きな行は `--ignored` で、release で流す。

use fandhe_silicon_core::hash::sha256_bytes;
use fandhe_silicon_core::parity::binary16::f32_to_binary16_bits;
use fandhe_silicon_core::parity::gemm_inputs::{
    GEMM_INPUT_SEED, GemmGateRow, GemmInputDtype, SplitMix64, generate_f32,
};

fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn f32_digest(v: &[f32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    hex(sha256_bytes(&bytes))
}

fn u16_digest(v: &[u16]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    hex(sha256_bytes(&bytes))
}

/// (行, A の f32, B の f32, A の binary16, B の binary16, A の +0.0 の件数, B の +0.0 の件数)
type Expected = (
    GemmGateRow,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    usize,
    usize,
);

fn check_row(e: &Expected) {
    let (row, a32, b32, a16, b16, a_zeros, b_zeros) = *e;
    let inputs = generate_f32(row).unwrap();
    let name = row.name();
    assert_eq!(inputs.a().len(), row.a_len().unwrap());
    assert_eq!(inputs.b().len(), row.b_len().unwrap());
    for (label, v, zeros) in [("a", inputs.a(), a_zeros), ("b", inputs.b(), b_zeros)] {
        let mut plus_zero = 0;
        for x in v {
            assert!(x.is_finite(), "{name} {label}");
            assert!((-1.0..1.0).contains(x), "{name} {label}");
            assert_ne!(x.to_bits(), (-0.0_f32).to_bits(), "{name} {label}");
            let scaled = x * 8_388_608.0;
            assert_eq!(
                scaled,
                scaled.trunc(),
                "{name} {label}: not a multiple of 2^-23"
            );
            if x.to_bits() == 0 {
                plus_zero += 1;
            }
        }
        assert_eq!(plus_zero, zeros, "{name} {label} +0.0 count");
    }
    assert_eq!(f32_digest(inputs.a()), a32, "{name} a f32");
    assert_eq!(f32_digest(inputs.b()), b32, "{name} b f32");

    let half = inputs.to_binary16().unwrap();
    for (src, dst) in inputs.a().iter().zip(half.a()) {
        assert_eq!(*dst, f32_to_binary16_bits(*src));
    }
    for (src, dst) in inputs.b().iter().zip(half.b()) {
        assert_eq!(*dst, f32_to_binary16_bits(*src));
    }
    assert_eq!(u16_digest(half.a()), a16, "{name} a binary16");
    assert_eq!(u16_digest(half.b()), b16, "{name} b binary16");

    // 2 回作って同じバイト列。
    let again = generate_f32(row).unwrap();
    assert_eq!(again, inputs);
}

#[test]
fn splitmix64_matches_published_reference_values() {
    let mut rng = SplitMix64::new(0);
    assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
}

#[test]
fn skip_equals_repeated_next() {
    for n in [0_u64, 1, 2, 7, 1000, 139_264] {
        let mut a = SplitMix64::new(GEMM_INPUT_SEED);
        let mut b = SplitMix64::new(GEMM_INPUT_SEED);
        a.skip(n);
        for _ in 0..n {
            b.next_u64();
        }
        assert_eq!(a.next_u64(), b.next_u64(), "n = {n}");
    }
    // 大きな n でも、2 回に分けた skip と同じ。
    let mut a = SplitMix64::new(GEMM_INPUT_SEED);
    let mut b = SplitMix64::new(GEMM_INPUT_SEED);
    a.skip(82_852_254);
    b.skip(80_000_000);
    b.skip(2_852_254);
    assert_eq!(a.next_u64(), b.next_u64());
}

#[test]
fn row_table_is_consistent() {
    let expected = [
        ("sq512", 512, 512, 512),
        ("sq2048", 2048, 2048, 2048),
        ("sq4096", 4096, 4096, 4096),
        ("rect4096x1024x4096", 4096, 1024, 4096),
        ("odd37x41x53", 37, 41, 53),
    ];
    let mut names: Vec<_> = GemmGateRow::ALL.iter().map(|r| r.name()).collect();
    names.sort_unstable();
    let mut want: Vec<_> = expected.iter().map(|e| e.0).collect();
    want.sort_unstable();
    assert_eq!(names, want);
    for (name, m, n, k) in expected {
        let row = *GemmGateRow::ALL.iter().find(|r| r.name() == name).unwrap();
        assert_eq!((row.m(), row.n(), row.k()), (m, n, k));
        assert_eq!(row.a_len().unwrap(), m * k);
        assert_eq!(row.b_len().unwrap(), k * n);
    }
    // 各行の A・B の区間は互いに重ならない（行どうしも、同じ行の A と B も）。
    let mut spans: Vec<(u64, u64, String)> = Vec::new();
    for row in GemmGateRow::ALL {
        spans.push((
            row.a_offset(),
            row.a_offset() + row.a_len().unwrap() as u64,
            format!("{} a", row.name()),
        ));
        spans.push((
            row.b_offset(),
            row.b_offset() + row.b_len().unwrap() as u64,
            format!("{} b", row.name()),
        ));
    }
    spans.sort();
    for w in spans.windows(2) {
        assert!(w[0].1 <= w[1].0, "{} overlaps {}", w[0].2, w[1].2);
    }
}

#[test]
fn dtype_names() {
    assert_eq!(GemmInputDtype::F32.name(), "f32");
    assert_eq!(GemmInputDtype::F16Acc.name(), "f16acc");
}

#[test]
fn small_rows_are_fixed() {
    for e in SMALL {
        check_row(e);
    }
}

#[test]
#[ignore = "large inputs; run with --release"]
fn large_rows_are_fixed() {
    for e in LARGE {
        check_row(e);
    }
}

const SMALL: &[Expected] = &[
    (
        GemmGateRow::Odd37x41x53,
        "8b6bdc24887e48cd2bc3598950e2949c599f27b9c14d2a2c11384d2bb9c454d1",
        "60efc5337595142f4ec8d78f08e8e2663f4a67630672e1d21d7cc2f21f90e19e",
        "3d80f537a8f7bf2b2d5bdd5efabe76051d78ac58d431bd742262c74c9f0f15d2",
        "d9add5c69b4070c3a881c487c62c39ecaee3487de2aa7771e1511689617289be",
        0,
        0,
    ),
    (
        GemmGateRow::Sq512,
        "63126cdc548f22561f8866999d1e59b0702a8abfac03d36d2cd3e5d796635066",
        "10bdd8fbd299a530351c4812ecd1f483cfa2c25b72205fc77c3890e80d85fe9a",
        "9ab10aee1723735cd0fc8dff42ff3e7e037ce42aaed9361357ab6563376f4874",
        "fd0ce7eaedf6a4cf1784d385fafcce641c9eb09692416a718bda82b6b8379468",
        0,
        0,
    ),
];

const LARGE: &[Expected] = &[
    (
        GemmGateRow::Sq2048,
        "604fc37305f4a9d91a6b89ab4a767854d0dfe6d4feadd48be091909b3df0bfa6",
        "b2b445d01d8c70aaf4884882714c70483c7b065e2db3f150c1236317e5015cc7",
        "1d22241c39e47c3840b7a21aaf54d735c947b51b14408ca0b4fca425af2738a1",
        "a9239eb7e0d1458d3c10ba67de11be94d40212c2a5e4d89739d437e18ac92d50",
        2,
        1,
    ),
    (
        GemmGateRow::Sq4096,
        "fcb28a0a4037004f6cc54f430b2a72d963ed564eb0f17a9e3ce4e88899c64a39",
        "3ca251cf5de7a6463be68ec4a95e5774e4edc46db2fd9a83b7551b92b6d4cf1e",
        "40fd09f0edab8fcff9071e0ecffed149b40417e3bbf4d97263f199f6d150eb3f",
        "0bbf3ad4929e1284ecc784db88f27723168015a0bad33fc8c2f53cca52a146c8",
        0,
        0,
    ),
    (
        GemmGateRow::Rect4096x1024x4096,
        "25424a8f94d39669dbf14964e3be44caacc00ac1cc87851bb62378662581b1cd",
        "ad422866b75198433e712b7e4acca1f595dbb8c0438ce92f4281abdfa80c2cb4",
        "683ea57f2d41f5c74d9b382e6f36dfa5d78d2f0f89444b11ad03113c6bab6e11",
        "fe7284d0f0979f8dff7be31b8d74e1f697a031f532c682d5e0d835df7de774f5",
        1,
        0,
    ),
];
