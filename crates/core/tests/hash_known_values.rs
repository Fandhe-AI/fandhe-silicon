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
        "zero_byte",
        || vec![0x00],
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
    ),
    (
        "a_1",
        || b"a".to_vec(),
        "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
    ),
    (
        "abc",
        || b"abc".to_vec(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    ),
    (
        "nist_448bit",
        || b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".to_vec(),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
    ),
    (
        "nist_896bit",
        || {
            b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu".to_vec()
        },
        "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
    ),
    (
        "a_55",
        || vec![b'a'; 55],
        "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
    ),
    (
        "a_56",
        || vec![b'a'; 56],
        "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
    ),
    (
        "a_63",
        || vec![b'a'; 63],
        "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34",
    ),
    (
        "a_64",
        || vec![b'a'; 64],
        "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
    ),
    (
        "a_65",
        || vec![b'a'; 65],
        "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0",
    ),
    (
        "a_119",
        || vec![b'a'; 119],
        "31eba51c313a5c08226adf18d4a359cfdfd8d2e816b13f4af952f7ea6584dcfb",
    ),
    (
        "a_120",
        || vec![b'a'; 120],
        "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c",
    ),
    (
        "a_1000000",
        || vec![b'a'; 1000000],
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
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
