//! `SpirvModule` を外の crate から確かめる統合テスト（TASK-1682・ART-138）。
//!
//! 受け付け・拒否が `spirv_words_from_bytes` と同じになること、`pipeline_input` が語と入口名を
//! 借用で返すことを見る。

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::PipelineInput;
use fandhe_silicon_core::spirv::{SpirvModule, spirv_words_from_bytes};

const MAGIC: u32 = 0x0723_0203;
const WORDS: [u32; 4] = [MAGIC, 0x0001_0000, 0x0102_0304, 0xAABB_CCDD];

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

#[test]
fn from_bytes_accepts_and_exposes_words() {
    let m = SpirvModule::from_bytes(&bytes_of(&WORDS, false)).unwrap();
    assert_eq!(m.words(), WORDS);
    assert_eq!(m.into_words(), WORDS.to_vec());
}

#[test]
fn swapped_input_equals_host_order_module() {
    let host = SpirvModule::from_bytes(&bytes_of(&WORDS, false)).unwrap();
    let swapped = SpirvModule::from_bytes(&bytes_of(&WORDS, true)).unwrap();
    assert_eq!(host, swapped);
    assert_eq!(swapped.words()[0], MAGIC);
}

#[test]
fn from_bytes_rejection_matches_raw_function() {
    let mut inputs: Vec<Vec<u8>> = vec![vec![], vec![1], vec![1, 2], vec![1, 2, 3]];
    for swapped in [false, true] {
        inputs.push(bytes_of(&[0x1234_5678], swapped));
        inputs.push(bytes_of(&[0, MAGIC], swapped));
        let mut odd = bytes_of(&[0x1234_5678], swapped);
        odd.extend([0u8; 3]);
        inputs.push(odd);
        for extra in 1..=3 {
            let mut b = bytes_of(&WORDS, swapped);
            b.extend(vec![0u8; extra]);
            inputs.push(b);
        }
        inputs.push(bytes_of(&WORDS, swapped));
    }
    for x in &inputs {
        assert_eq!(
            SpirvModule::from_bytes(x).map(SpirvModule::into_words),
            spirv_words_from_bytes(x),
            "input len {}",
            x.len()
        );
    }
    assert!(matches!(
        SpirvModule::from_bytes(&[]),
        Err(ContractError::ArtifactRejected { kind: "spir-v", .. })
    ));
}

#[test]
fn pipeline_input_borrows_words_and_entry_point() {
    let m = SpirvModule::from_bytes(&bytes_of(&WORDS, false)).unwrap();
    let entry = "main";
    match m.pipeline_input(entry) {
        PipelineInput::SpirV { words, entry_point } => {
            assert!(std::ptr::eq(words, m.words()));
            assert_eq!(words.len(), m.words().len());
            assert!(std::ptr::eq(entry_point, entry));
        }
        _ => panic!("expected PipelineInput::SpirV"),
    }
}

#[test]
fn same_input_builds_equal_modules() {
    let b = bytes_of(&WORDS, true);
    assert_eq!(
        SpirvModule::from_bytes(&b).unwrap(),
        SpirvModule::from_bytes(&b).unwrap()
    );
}
