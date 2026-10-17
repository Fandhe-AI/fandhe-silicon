//! `hash::sha256_bytes`・`hash::sha256_u64` を外部の道具 `shasum -a 256` の出力と照合する試験
//! （TASK-471・ART-28 のポインタ）。
//!
//! `shasum` が PATH に無いときは試験を失敗させる（飛ばさず、別の道具にも切り替えない）。
//! feature なしで動く。失敗のメッセージには入力の名前と長さだけを出し、中身は出さない。

use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;

use fandhe_silicon_core::hash::{sha256_bytes, sha256_u64};

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `shasum -a 256` に stdin でバイト列を渡し、64 桁の小文字 16 進を返す。
/// 起動失敗・異常終了・出力の形が違うときは panic する。
fn shasum_256(data: &[u8]) -> String {
    let mut child = match Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => panic!(
            "shasum -a 256 could not be started: {:?} (shasum is required on PATH)",
            e.kind()
        ),
    };
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let input = data.to_vec();
    // 大きな入力でパイプが詰まらないよう、書き込みは別スレッドで行う。
    let writer = thread::spawn(move || {
        let r = stdin.write_all(&input);
        drop(stdin);
        r
    });
    let out = child.wait_with_output().expect("wait for shasum");
    writer
        .join()
        .expect("writer thread")
        .expect("write to shasum stdin");
    assert!(out.status.success(), "shasum -a 256 exited abnormally");
    let text = String::from_utf8(out.stdout).expect("shasum output is UTF-8");
    let hex = text
        .strip_suffix("  -\n")
        .unwrap_or_else(|| panic!("unexpected shasum output shape ({} bytes)", text.len()));
    assert!(
        hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "shasum digest is not 64 lowercase hex digits"
    );
    hex.to_string()
}

fn check(name: &str, data: &[u8]) {
    let expected = shasum_256(data);
    assert_eq!(
        to_hex(&sha256_bytes(data)),
        expected,
        "digest: {name} (len {})",
        data.len()
    );
    let head = u64::from_str_radix(&expected[..16], 16).expect("hex head");
    assert_eq!(sha256_u64(data), head, "u64: {name} (len {})", data.len());
}

/// 固定の種の線形合同法で作る擬似乱数のバイト列（外部依存なし）。
fn lcg_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed;
    (0..len)
        .map(|_| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (x >> 56) as u8
        })
        .collect()
}

#[test]
fn small_inputs_match_shasum() {
    check("empty", &[]);
    check("zero_byte", &[0x00]);
    check("ff_byte", &[0xff]);
    check("a_byte", b"a");
    let all: Vec<u8> = (0..=255u8).collect();
    check("all_byte_values", &all);
}

#[test]
fn block_boundaries_match_shasum() {
    for len in [55usize, 56, 57, 63, 64, 65, 119, 120, 127, 128, 129] {
        check("a_repeat", &vec![b'a'; len]);
        check("random", &lcg_bytes(len as u64 + 1, len));
    }
}

#[test]
fn arbitrary_byte_sweep_matches_shasum() {
    for len in 0..=200usize {
        check("sweep", &lcg_bytes(0x5eed_0000 + len as u64, len));
    }
}

#[test]
fn long_inputs_match_shasum() {
    check("a_1000000", &vec![b'a'; 1_000_000]);
    check("random_1000003", &lcg_bytes(42, 1_000_003));
}
