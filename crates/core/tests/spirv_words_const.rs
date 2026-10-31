//! `spirv_words_const!` と展開用の口を外の crate から確かめる統合テスト（TASK-1682・ART-140）。
//!
//! `const`・`static` の初期化子での利用と、展開用の口を実行時に呼んだときの句が実行時の変換の
//! `reason` と一致することを見る。コンパイルが落ちる入力は doctest 側が持つ。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::spirv::{__spirv_words_const, SpirvModule, spirv_words_from_bytes};

const MAGIC: u32 = 0x0723_0203;

const fn bytes_of(words: [u32; 4], swapped: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    let mut i = 0;
    while i < 4 {
        let w = if swapped {
            words[i].swap_bytes()
        } else {
            words[i]
        };
        let b = w.to_ne_bytes();
        let mut j = 0;
        while j < 4 {
            out[i * 4 + j] = b[j];
            j += 1;
        }
        i += 1;
    }
    out
}

const EXPECTED: [u32; 4] = [MAGIC, 0x0001_0000, 0x0102_0304, 0xAABB_CCDD];
const HOST: [u8; 16] = bytes_of(EXPECTED, false);
const SWAPPED: [u8; 16] = bytes_of(EXPECTED, true);

const WORDS: [u32; 4] = fandhe_silicon_core::spirv_words_const!(&HOST);
static SWAPPED_WORDS: [u32; 4] = fandhe_silicon_core::spirv_words_const!(&SWAPPED);

#[test]
fn macro_in_const_and_static_matches_runtime() {
    assert_eq!(WORDS, EXPECTED);
    assert_eq!(SWAPPED_WORDS, EXPECTED);
    assert_eq!(spirv_words_from_bytes(&HOST).unwrap(), WORDS);
    assert_eq!(spirv_words_from_bytes(&SWAPPED).unwrap(), SWAPPED_WORDS);
    assert_eq!(SpirvModule::from_bytes(&HOST).unwrap().into_words(), WORDS);
}

#[test]
fn macro_does_not_clash_with_user_names() {
    const BYTES: [u8; 8] = {
        let a = MAGIC.to_ne_bytes();
        let b = 7u32.to_ne_bytes();
        [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
    };
    const WORDS: [u32; 2] = fandhe_silicon_core::spirv_words_const!(&BYTES);
    assert_eq!(WORDS, [MAGIC, 7]);
}

#[test]
fn same_input_twice_gives_same_value() {
    const A: [u32; 4] = fandhe_silicon_core::spirv_words_const!(&HOST);
    const B: [u32; 4] = fandhe_silicon_core::spirv_words_const!(&HOST);
    assert_eq!(A, B);
}

fn via_const<const N: usize>(bytes: &[u8]) -> Result<Vec<u32>, &'static str> {
    __spirv_words_const::<N>(bytes).map(|a| a.to_vec())
}

fn assert_same_phrase(bytes: &[u8], via: Result<Vec<u32>, &'static str>) {
    let phrase = via.expect_err("expected const-entry rejection");
    let Err(ContractError::ArtifactRejected { reason, .. }) = spirv_words_from_bytes(bytes) else {
        panic!("expected runtime rejection");
    };
    assert_eq!(reason, format!("{phrase} ({} bytes)", bytes.len()));
}

#[test]
fn const_entry_accepts_like_runtime() {
    assert_eq!(
        via_const::<4>(&HOST).unwrap(),
        spirv_words_from_bytes(&HOST).unwrap()
    );
    assert_eq!(
        via_const::<4>(&SWAPPED).unwrap(),
        spirv_words_from_bytes(&SWAPPED).unwrap()
    );
}

#[test]
fn const_entry_phrases_match_runtime_reason() {
    assert_same_phrase(&[], via_const::<0>(&[]));
    for n in 1..=3 {
        assert_same_phrase(&HOST[..n], via_const::<0>(&HOST[..n]));
        assert_same_phrase(&SWAPPED[..n], via_const::<0>(&SWAPPED[..n]));
    }
    for swapped in [false, true] {
        let bad = 0x1234_5678u32;
        let w = if swapped { bad.swap_bytes() } else { bad };
        let b4 = w.to_ne_bytes();
        assert_same_phrase(&b4, via_const::<1>(&b4));
        let mut b8 = b4.to_vec();
        b8.extend(MAGIC.to_ne_bytes());
        assert_same_phrase(&b8, via_const::<2>(&b8));
        let mut b5 = b4.to_vec();
        b5.push(0);
        assert_same_phrase(&b5, via_const::<1>(&b5));
        let src: &[u8] = if swapped { &SWAPPED } else { &HOST };
        for extra in 1..=3 {
            let mut m = src[..4].to_vec();
            m.extend(vec![0u8; extra]);
            assert_same_phrase(&m, via_const::<1>(&m));
        }
    }
}
