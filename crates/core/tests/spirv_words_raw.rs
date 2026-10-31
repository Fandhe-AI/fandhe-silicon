//! `spirv_words_from_bytes` を外の crate から確かめる統合テスト（TASK-1682・ART-139）。
//!
//! 入力はホストの語順に依らない書き方（`to_ne_bytes`・`swap_bytes`）で作り、受け付けの語の列と
//! 拒否の句・`reason` の形を見る。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::spirv::spirv_words_from_bytes;

const MAGIC: u32 = 0x0723_0203;
const EMPTY: &str = "input is empty";
const NOT_MAGIC: &str = "first word is not the SPIR-V magic number";
const BAD_LEN: &str = "byte length is not a multiple of 4";

fn bytes_of(words: &[u32], swapped: bool) -> Vec<u8> {
    words
        .iter()
        .flat_map(|w| {
            if swapped {
                w.swap_bytes().to_ne_bytes()
            } else {
                w.to_ne_bytes()
            }
        })
        .collect()
}

fn rejected(phrase: &str, len: usize) -> Result<Vec<u32>, ContractError> {
    Err(ContractError::ArtifactRejected {
        kind: "spir-v",
        reason: format!("{phrase} ({len} bytes)"),
    })
}

fn long_words() -> Vec<u32> {
    let mut v = vec![MAGIC];
    v.extend((1..64u32).map(|i| i.wrapping_mul(0x0101_0307)));
    v
}

#[test]
fn accepts_host_order_input() {
    let words = [MAGIC, 0x0001_0000, 0x0102_0304, 0xAABB_CCDD];
    let out = spirv_words_from_bytes(&bytes_of(&words, false)).unwrap();
    assert_eq!(out, words);
    assert_eq!(out[0], MAGIC);
}

#[test]
fn accepts_swapped_order_input_and_swaps_every_word() {
    let words = [MAGIC, 0x0001_0000, 0x0102_0304, 0xAABB_CCDD];
    let out = spirv_words_from_bytes(&bytes_of(&words, true)).unwrap();
    assert_eq!(out, words);
    assert_eq!(
        out,
        spirv_words_from_bytes(&bytes_of(&words, false)).unwrap()
    );
}

#[test]
fn word_count_is_byte_len_over_four() {
    for words in [vec![MAGIC], vec![MAGIC, 1, 2, 3], long_words()] {
        for swapped in [false, true] {
            let bytes = bytes_of(&words, swapped);
            let out = spirv_words_from_bytes(&bytes).unwrap();
            assert_eq!(out.len(), bytes.len() / 4);
            assert_eq!(out, words);
        }
    }
}

#[test]
fn rejects_empty_with_empty_phrase() {
    assert_eq!(spirv_words_from_bytes(&[]), rejected(EMPTY, 0));
}

#[test]
fn rejects_one_to_three_bytes_with_magic_phrase() {
    for swapped in [false, true] {
        let full = bytes_of(&[MAGIC], swapped);
        for n in 1..=3 {
            assert_eq!(spirv_words_from_bytes(&full[..n]), rejected(NOT_MAGIC, n));
        }
    }
}

#[test]
fn rejects_non_magic_first_word_regardless_of_length() {
    let flipped = MAGIC ^ 0x0000_0100;
    for first in [0x1234_5678u32, 0, flipped] {
        for swapped in [false, true] {
            let four = bytes_of(&[first], swapped);
            assert_eq!(spirv_words_from_bytes(&four), rejected(NOT_MAGIC, 4));
            let many = bytes_of(&[first, MAGIC, 7], swapped);
            assert_eq!(spirv_words_from_bytes(&many), rejected(NOT_MAGIC, 12));
            for extra in [1usize, 3] {
                let mut odd = four.clone();
                odd.extend(std::iter::repeat_n(0xEE, extra));
                assert_eq!(spirv_words_from_bytes(&odd), rejected(NOT_MAGIC, 4 + extra));
            }
        }
    }
}

#[test]
fn rejects_magic_with_trailing_bytes_with_length_phrase() {
    for swapped in [false, true] {
        for words in [vec![MAGIC], vec![MAGIC, 5, 6]] {
            let base = bytes_of(&words, swapped);
            for extra in 1..=3usize {
                let mut b = base.clone();
                b.extend(std::iter::repeat_n(0, extra));
                assert_eq!(
                    spirv_words_from_bytes(&b),
                    rejected(BAD_LEN, base.len() + extra)
                );
            }
        }
    }
}

#[test]
fn reason_does_not_leak_input_contents() {
    let bytes = bytes_of(&[0x1234_5678], false);
    let Err(ContractError::ArtifactRejected { reason, .. }) = spirv_words_from_bytes(&bytes) else {
        panic!("expected rejection");
    };
    assert!(reason.contains("(4 bytes)"));
    for leaked in ["1234", "5678", "0x", "305419896"] {
        assert!(!reason.contains(leaked), "reason leaks {leaked}: {reason}");
    }
}

#[test]
fn three_phrases_are_distinct() {
    assert_ne!(EMPTY, NOT_MAGIC);
    assert_ne!(EMPTY, BAD_LEN);
    assert_ne!(NOT_MAGIC, BAD_LEN);
}

#[test]
fn same_input_gives_same_result() {
    let ok = bytes_of(&[MAGIC, 9], true);
    assert_eq!(spirv_words_from_bytes(&ok), spirv_words_from_bytes(&ok));
    let ng = [1u8, 2, 3, 4, 5];
    assert_eq!(spirv_words_from_bytes(&ng), spirv_words_from_bytes(&ng));
}
