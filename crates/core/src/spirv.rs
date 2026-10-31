//! SPIR-V のバイト列から 32 ビット語の列を作る操作（feature の条件なし）。
//!
//! device を受け取らない純粋な関数と型で、得た語の列を `PipelineInput::SpirV` に渡して使う。
//! 先頭の語が magic number でない入力・空の入力・長さが 4 の倍数でない入力は
//! `ContractError::ArtifactRejected`（`kind` は `"spir-v"`）で拒む。
//! 先頭の語が逆の語順の magic number のときは、全語のバイトを入れ替えて読む。
//!
//! TASK-1682・ART-138・ART-139 のポインタ。

use crate::error::ContractError;
use crate::execution::PipelineInput;

/// ホストの語順で読んだときの SPIR-V の magic number。
const MAGIC: u32 = 0x0723_0203;
/// 逆の語順で読んだときの magic number。
const MAGIC_SWAPPED: u32 = 0x0302_2307;

/// 空の入力を拒む句。
const REASON_EMPTY: &str = "input is empty";
/// 先頭の語が magic number でない入力を拒む句。
const REASON_MAGIC: &str = "first word is not the SPIR-V magic number";
/// 長さが 4 の倍数でない入力を拒む句。
const REASON_LENGTH: &str = "byte length is not a multiple of 4";

/// 入力を照らし、通れば語ごとにバイトを入れ替える要否を返す。
///
/// 照らす順は 空、先頭の語、長さ。最初に当たった 1 つの句を返す。
const fn check(bytes: &[u8]) -> Result<bool, &'static str> {
    if bytes.is_empty() {
        return Err(REASON_EMPTY);
    }
    let swap = match bytes.first_chunk::<4>() {
        Some(head) => {
            let first = u32::from_ne_bytes(*head);
            if first == MAGIC {
                false
            } else if first == MAGIC_SWAPPED {
                true
            } else {
                return Err(REASON_MAGIC);
            }
        }
        None => return Err(REASON_MAGIC),
    };
    if !bytes.len().is_multiple_of(4) {
        return Err(REASON_LENGTH);
    }
    Ok(swap)
}

/// 4 バイトを 1 語として読む。`swap` が真のときはバイトの順を入れ替える。
const fn word_at(chunk: [u8; 4], swap: bool) -> u32 {
    let word = u32::from_ne_bytes(chunk);
    if swap { word.swap_bytes() } else { word }
}

/// SPIR-V のバイト列から語の列を作る。
///
/// 空・先頭の語が magic number でない・長さが 4 の倍数でない入力は
/// `ContractError::ArtifactRejected`（`kind` は `"spir-v"`）で拒む。
/// `reason` は決まった句と入力のバイト数だけで、入力の中身は載せない。
pub fn spirv_words_from_bytes(bytes: &[u8]) -> Result<Vec<u32>, ContractError> {
    let swap = check(bytes).map_err(|phrase| ContractError::ArtifactRejected {
        kind: "spir-v",
        reason: format!("{phrase} ({} bytes)", bytes.len()),
    })?;
    let (chunks, _) = bytes.as_chunks::<4>();
    let mut words = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        words.push(word_at(*chunk, swap));
    }
    Ok(words)
}

/// 検査を通った SPIR-V の語の列を持つ型。
///
/// 持つ語の列は `from_bytes` が検査した結果で、先頭は magic number。
#[derive(Clone, PartialEq, Eq)]
pub struct SpirvModule {
    words: Vec<u32>,
}

impl SpirvModule {
    /// バイト列から作る。拒む入力は `spirv_words_from_bytes` と同じ。
    pub fn from_bytes(bytes: &[u8]) -> Result<SpirvModule, ContractError> {
        Ok(SpirvModule {
            words: spirv_words_from_bytes(bytes)?,
        })
    }

    /// 語の列を借りて返す。
    pub fn words(&self) -> &[u32] {
        &self.words
    }

    /// 語の列を取り出す。
    pub fn into_words(self) -> Vec<u32> {
        self.words
    }

    /// 語の列と入口の関数名から `PipelineInput::SpirV` を作る。
    pub fn pipeline_input<'a>(&'a self, entry_point: &'a str) -> PipelineInput<'a> {
        PipelineInput::SpirV {
            words: self.words(),
            entry_point,
        }
    }
}

impl std::fmt::Debug for SpirvModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpirvModule")
            .field("words", &self.words.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_order_magic_is_read_as_is() {
        let bytes = MAGIC.to_ne_bytes();
        assert_eq!(spirv_words_from_bytes(&bytes), Ok(vec![MAGIC]));
        let module = SpirvModule::from_bytes(&bytes).unwrap();
        assert_eq!(module.words(), &[MAGIC]);
    }

    #[test]
    fn swapped_magic_is_swapped_back() {
        let bytes = MAGIC.swap_bytes().to_ne_bytes();
        assert_eq!(spirv_words_from_bytes(&bytes), Ok(vec![MAGIC]));
    }

    #[test]
    fn empty_input_is_rejected_with_length() {
        assert_eq!(
            spirv_words_from_bytes(&[]),
            Err(ContractError::ArtifactRejected {
                kind: "spir-v",
                reason: "input is empty (0 bytes)".to_string(),
            })
        );
    }

    #[test]
    fn pipeline_input_borrows_words_and_entry_point() {
        let module = SpirvModule::from_bytes(&MAGIC.to_ne_bytes()).unwrap();
        match module.pipeline_input("main") {
            PipelineInput::SpirV { words, entry_point } => {
                assert_eq!(words, module.words());
                assert_eq!(entry_point, "main");
            }
            _ => panic!("expected SpirV"),
        }
    }
}
