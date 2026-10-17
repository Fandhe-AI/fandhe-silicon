//! `hash::sha256_bytes`・`hash::sha256_u64` を固定の既知の値と比べる試験（TASK-471・ART-28 のポインタ）。
//!
//! 表に行を足すだけで値を増やせる形にしている。feature なしで動く。

use fandhe_silicon_core::hash::{sha256_bytes, sha256_u64};

/// (名前, 入力を作る関数, 期待する 64 桁の 16 進)
type Case = (&'static str, fn() -> Vec<u8>, &'static str);

const CASES: &[Case] = &[
    (
        "empty",
        Vec::new,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        "abc",
        || b"abc".to_vec(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    ),
];

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn known_values_match() {
    for (name, input, expected) in CASES {
        let data = input();
        assert_eq!(to_hex(&sha256_bytes(&data)), *expected, "digest: {name}");
        let head = u64::from_str_radix(&expected[..16], 16).unwrap();
        assert_eq!(sha256_u64(&data), head, "u64: {name}");
    }
}
