//! SPIR-V のバイト列から 32 ビット語の列を作る操作（feature の条件なし）。
//!
//! device を受け取らない純粋な関数と型で、得た語の列を `PipelineInput::SpirV` に渡して使う。
//! 先頭の語が magic number でない入力・空の入力・長さが 4 の倍数でない入力は
//! `ContractError::ArtifactRejected`（`kind` は `"spir-v"`）で拒む。
//! 先頭の語が逆の語順の magic number のときは、全語のバイトを入れ替えて読む。
//!
//! `const` の文脈では crate の根のマクロ `spirv_words_const!` が同じ検査で語の列を作る。
//!
//! TASK-1682・ART-138・ART-139・ART-140 のポインタ。

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

/// `spirv_words_const!` の展開から呼ぶ口。語の列か、拒む句を返す（panic しない）。
///
/// `N` は語の数で、入力のバイト数の 4 分の 1 と合わないときは長さの句を返す。
#[doc(hidden)]
pub const fn __spirv_words_const<const N: usize>(bytes: &[u8]) -> Result<[u32; N], &'static str> {
    let swap = match check(bytes) {
        Ok(swap) => swap,
        Err(phrase) => return Err(phrase),
    };
    match N.checked_mul(4) {
        Some(len) if len == bytes.len() => {}
        _ => return Err(REASON_LENGTH),
    }
    let mut words = [0u32; N];
    let mut i = 0;
    while i < N {
        let b = i * 4;
        words[i] = word_at([bytes[b], bytes[b + 1], bytes[b + 2], bytes[b + 3]], swap);
        i += 1;
    }
    Ok(words)
}

/// `const` の文脈で SPIR-V のバイト列から語の列 `[u32; 入力のバイト数 / 4]` を作るマクロ。
///
/// 引数は `const` 式の `&[u8]`。`const` や `static` の初期化子に置ける。
/// 受け付けた入力は `spirv_words_from_bytes` と同じ語の列になる。
/// 拒む入力ではコンパイルが失敗し、実行時の `reason` と同じ句が出る。
/// `include_bytes!` の結果も渡せる。
///
/// ```
/// use fandhe_silicon_core::spirv::spirv_words_from_bytes;
/// use fandhe_silicon_core::spirv_words_const;
///
/// const BYTES: &[u8] = &{
///     let a = 0x0723_0203u32.to_ne_bytes();
///     let b = 7u32.to_ne_bytes();
///     [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
/// };
/// const WORDS: [u32; 2] = spirv_words_const!(BYTES);
/// assert_eq!(WORDS[0], 0x0723_0203);
/// assert_eq!(WORDS.to_vec(), spirv_words_from_bytes(BYTES).unwrap());
/// ```
///
/// 逆の語順の入力も同じ語の列になる。
///
/// ```
/// use fandhe_silicon_core::spirv::spirv_words_from_bytes;
/// use fandhe_silicon_core::spirv_words_const;
///
/// const SWAPPED: [u8; 8] = {
///     let a = 0x0723_0203u32.swap_bytes().to_ne_bytes();
///     let b = 7u32.swap_bytes().to_ne_bytes();
///     [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
/// };
/// const WORDS: [u32; 2] = spirv_words_const!(&SWAPPED);
/// assert_eq!(WORDS, [0x0723_0203, 7]);
/// assert_eq!(WORDS.to_vec(), spirv_words_from_bytes(&SWAPPED).unwrap());
/// ```
///
/// `static` の初期化子でも使える。
///
/// ```
/// use fandhe_silicon_core::spirv::spirv_words_from_bytes;
/// use fandhe_silicon_core::spirv_words_const;
///
/// const MAGIC_BYTES: [u8; 4] = 0x0723_0203u32.to_ne_bytes();
/// static WORDS: [u32; 1] = spirv_words_const!(&MAGIC_BYTES);
/// assert_eq!(WORDS.to_vec(), spirv_words_from_bytes(&MAGIC_BYTES).unwrap());
/// ```
///
/// 長さが 4 の倍数でない入力（通る例は上の最初の例）。
///
/// ```compile_fail,E0080
/// use fandhe_silicon_core::spirv_words_const;
///
/// const WORDS: [u32; 1] = spirv_words_const!(&{
///     let a = 0x0723_0203u32.to_ne_bytes();
///     [a[0], a[1], a[2], a[3], 0]
/// });
/// ```
///
/// どちらの語順でも magic number でない入力（通る例は上の最初の例）。
///
/// ```compile_fail,E0080
/// use fandhe_silicon_core::spirv_words_const;
///
/// const WORDS: [u32; 1] = spirv_words_const!(&[1, 2, 3, 4]);
/// ```
///
/// 空の入力（通る例は上の最初の例）。
///
/// ```compile_fail,E0080
/// use fandhe_silicon_core::spirv_words_const;
///
/// const WORDS: [u32; 0] = spirv_words_const!(&[]);
/// ```
#[macro_export]
macro_rules! spirv_words_const {
    ($bytes:expr) => {{
        const __FANDHE_SILICON_SPIRV_BYTES: &[u8] = $bytes;
        #[allow(clippy::uninlined_format_args)]
        const __FANDHE_SILICON_SPIRV_WORDS: [u32; __FANDHE_SILICON_SPIRV_BYTES.len() / 4] =
            match $crate::spirv::__spirv_words_const::<{ __FANDHE_SILICON_SPIRV_BYTES.len() / 4 }>(
                __FANDHE_SILICON_SPIRV_BYTES,
            ) {
                ::core::result::Result::Ok(words) => words,
                ::core::result::Result::Err(msg) => ::core::panic!("{}", msg),
            };
        __FANDHE_SILICON_SPIRV_WORDS
    }};
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

    fn rejected(reason: &str) -> Result<Vec<u32>, ContractError> {
        Err(ContractError::ArtifactRejected {
            kind: "spir-v",
            reason: reason.to_string(),
        })
    }

    #[test]
    fn wrong_magic_is_rejected() {
        let bytes = 0x1234_5678u32.to_ne_bytes();
        assert_eq!(
            spirv_words_from_bytes(&bytes),
            rejected(&format!("{REASON_MAGIC} (4 bytes)"))
        );
    }

    #[test]
    fn one_to_three_bytes_are_rejected_as_magic() {
        for len in 1..4usize {
            let bytes = &MAGIC.to_ne_bytes()[..len];
            assert_eq!(
                spirv_words_from_bytes(bytes),
                rejected(&format!("{REASON_MAGIC} ({len} bytes)"))
            );
        }
    }

    #[test]
    fn magic_with_trailing_bytes_is_rejected_by_length() {
        for extra in 1..4usize {
            for magic in [MAGIC, MAGIC.swap_bytes()] {
                let mut bytes = magic.to_ne_bytes().to_vec();
                bytes.extend(std::iter::repeat_n(0u8, extra));
                assert_eq!(
                    spirv_words_from_bytes(&bytes),
                    rejected(&format!("{REASON_LENGTH} ({} bytes)", bytes.len()))
                );
            }
        }
    }

    #[test]
    fn later_words_follow_the_first_word_byte_order() {
        let later = [0x0102_0304u32, 0xAABB_CCDD];
        let mut host = MAGIC.to_ne_bytes().to_vec();
        let mut swapped = MAGIC.swap_bytes().to_ne_bytes().to_vec();
        for w in later {
            host.extend(w.to_ne_bytes());
            swapped.extend(w.swap_bytes().to_ne_bytes());
        }
        let expected = vec![MAGIC, later[0], later[1]];
        assert_eq!(spirv_words_from_bytes(&host), Ok(expected.clone()));
        assert_eq!(spirv_words_from_bytes(&swapped), Ok(expected));
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

    #[test]
    fn const_macro_matches_runtime() {
        const HOST: [u8; 8] = {
            let a = MAGIC.to_ne_bytes();
            let b = 9u32.to_ne_bytes();
            [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
        };
        const SWAPPED: [u8; 8] = {
            let a = MAGIC.swap_bytes().to_ne_bytes();
            let b = 9u32.swap_bytes().to_ne_bytes();
            [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
        };
        const HOST_WORDS: [u32; 2] = crate::spirv_words_const!(&HOST);
        static SWAPPED_WORDS: [u32; 2] = crate::spirv_words_const!(&SWAPPED);
        assert_eq!(HOST_WORDS, [MAGIC, 9]);
        assert_eq!(HOST_WORDS.to_vec(), spirv_words_from_bytes(&HOST).unwrap());
        assert_eq!(
            SWAPPED_WORDS.to_vec(),
            spirv_words_from_bytes(&SWAPPED).unwrap()
        );
    }

    #[test]
    fn const_entry_rejects_mismatched_length_at_runtime() {
        let bytes = MAGIC.to_ne_bytes();
        assert_eq!(__spirv_words_const::<1>(&bytes), Ok([MAGIC]));
        assert_eq!(__spirv_words_const::<2>(&bytes), Err(REASON_LENGTH));
        assert_eq!(__spirv_words_const::<0>(&bytes), Err(REASON_LENGTH));
        assert_eq!(__spirv_words_const::<1>(&[]), Err(REASON_EMPTY));
        assert_eq!(
            __spirv_words_const::<1>(&0x1234_5678u32.to_ne_bytes()),
            Err(REASON_MAGIC)
        );
    }
}
