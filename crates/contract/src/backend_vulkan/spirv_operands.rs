//! SPIR-V のモジュールから storage buffer のオペランドの宣言を取り出す検証用の道具（spec TASK-98 の 4/6）。
//!
//! 検証用のテスト・測定が呼ぶ。製品の経路（契約の入口・能力の問合せ）からは呼ばない。
//! feature `test-support` の裏にだけ公開され、標準ライブラリだけで書く（ファイル IO・外部コマンドの起動はしない）。
//! 入力は信頼しない。範囲外を読まず、panic させず、算術は `checked_*` で行い、確保は読んだ命令の数に比例させる。
//! 型の解決は固定の段数で行い、再帰しない。
//! 取り出しの対象外の形は黙って落とさず [`SpirvExtractError::Unsupported`] で返す。
//! push constant は対象に含めない。モジュール全体の変数が対象で、エントリポイントでの絞り込みはしない。
//! 置き場・ゲートの説明は `docs/vulkan-operand-extraction.md` の「現状の形（4/6）」。
//! spec との対応は `docs/spec/05-tasks.md` の TASK-98 のポインタのみ。
//! 命令・装飾・storage class の番号は Khronos の公開の SPIR-V 仕様のものを使う。

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// SPIR-V 仕様のバイナリ形式（ヘッダー 5 語・命令の先頭の語は上位 16 ビットが語数、下位 16 ビットが opcode）。
const MAGIC: u32 = 0x0723_0203;
const MAGIC_SWAPPED: u32 = 0x0302_2307;
const HEADER_WORDS: usize = 5;

const OP_TYPE_BOOL: u16 = 20;
const OP_TYPE_INT: u16 = 21;
const OP_TYPE_FLOAT: u16 = 22;
const OP_TYPE_VECTOR: u16 = 23;
const OP_TYPE_MATRIX: u16 = 24;
const OP_TYPE_ARRAY: u16 = 28;
const OP_TYPE_RUNTIME_ARRAY: u16 = 29;
const OP_TYPE_STRUCT: u16 = 30;
const OP_TYPE_POINTER: u16 = 32;
const OP_CONSTANT: u16 = 43;
const OP_VARIABLE: u16 = 59;
const OP_DECORATE: u16 = 71;
const OP_MEMBER_DECORATE: u16 = 72;

const DECORATION_BLOCK: u32 = 2;
const DECORATION_BUFFER_BLOCK: u32 = 3;
const DECORATION_ARRAY_STRIDE: u32 = 6;
const DECORATION_BINDING: u32 = 33;
const DECORATION_OFFSET: u32 = 35;
const DECORATION_DESCRIPTOR_SET: u32 = 34;

const STORAGE_CLASS_UNIFORM: u32 = 2;
const STORAGE_CLASS_STORAGE_BUFFER: u32 = 12;

/// 要素の型（SPIR-V の宣言どおり）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScalarKind {
    /// 浮動小数点。
    Float {
        /// ビット幅。
        bits: u32,
    },
    /// 整数。
    Int {
        /// ビット幅。
        bits: u32,
        /// 符号付きなら true。
        signed: bool,
    },
}

/// 全体のバイト長。静的に決まらない場合は 0 や推測で埋めず [`ByteLength::Unsized`] で返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ByteLength {
    /// 固定長の配列のバイト数。
    Fixed(u64),
    /// runtime array。実行時にしか決まらない。
    Unsized,
}

/// 取り出した storage buffer の宣言 1 件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExtractedOperand {
    /// descriptor set の番号。
    pub set: u32,
    /// binding の番号。
    pub binding: u32,
    /// 要素の型。
    pub element: ScalarKind,
    /// 要素 1 個分のバイト数（ビット幅から求める）。
    pub element_bytes: u32,
    /// `ArrayStride` の装飾の値（無ければ `None`）。
    pub array_stride: Option<u32>,
    /// 全体のバイト長。
    pub byte_length: ByteLength,
}

/// 取り出しの対象外の形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsupportedShape {
    /// 要素が `OpTypeBool`。
    Bool,
    /// 要素が `OpTypeVector` / `OpTypeMatrix`。
    VectorOrMatrix,
    /// 要素が struct。
    StructElement,
    /// block の配列。
    BlockArray,
    /// 古い形（`Uniform` + `BufferBlock`）。
    LegacyBufferBlock,
    /// block のメンバーの並びが「配列 1 つ」でない、または block の装飾が無い。
    BlockShape,
    /// ビット幅が 0 か 8 の倍数でない。
    BitWidth,
    /// 配列の長さが 1 語の `OpConstant` でない。
    LengthNotConstant,
    /// `ArrayStride` が 0。
    ZeroStride,
    /// 固定長の配列に `ArrayStride` が無い（長さを推測しない）。
    MissingArrayStride,
    /// `ArrayStride` が要素 1 個分のバイト数より小さい（要素が重なる）。
    StrideSmallerThanElement,
    /// block のメンバーの `Offset` が 0 でない（長さに offset を含めない）。
    NonZeroMemberOffset,
    /// 配列の長さが整数型の定数でない、または正でない。
    InvalidLength,
    /// 上のどれでもない想定外の型。
    OtherType,
}

/// 取り出しで返るエラー。位置は語の番号で持ち、入力の中身は表示に出さない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpirvExtractError {
    /// ヘッダー 5 語に満たない。
    TooShort {
        /// 渡された語数。
        words: usize,
    },
    /// 先頭の語がマジックと違う。
    BadMagic {
        /// 渡された先頭の語。
        found: u32,
    },
    /// マジックのバイト順が逆（受け口は語の列なので、読み込み側でそろえる必要がある）。
    ByteOrderReversed,
    /// 命令の語数が 0。
    ZeroWordCount {
        /// 命令の先頭の語番号。
        offset: usize,
    },
    /// 命令の語数が残りを超える。
    TruncatedInstruction {
        /// 命令の先頭の語番号。
        offset: usize,
        /// 命令が宣言した語数。
        word_count: usize,
        /// 残りの語数。
        remaining: usize,
    },
    /// 命令のオペランドが足りない。
    MissingOperands {
        /// 命令の先頭の語番号。
        offset: usize,
        /// 命令の opcode。
        opcode: u16,
    },
    /// 参照先の id が無い。
    UnknownId {
        /// 参照した id。
        id: u32,
    },
    /// 参照先の id の種類が合わない。
    WrongKind {
        /// 参照先の id。
        id: u32,
    },
    /// `Binding` / `DescriptorSet` の装飾が無い。
    MissingDecoration {
        /// 対象の変数の id。
        id: u32,
    },
    /// 同じ id の定義・装飾が重複している。
    DuplicateDefinition {
        /// 重複した id。
        id: u32,
    },
    /// 同じ（set, binding）が重複している。
    DuplicateBinding {
        /// descriptor set の番号。
        set: u32,
        /// binding の番号。
        binding: u32,
    },
    /// 取り出しの対象外の形。
    Unsupported {
        /// 対象の変数の id。
        id: u32,
        /// 対象外の理由。
        shape: UnsupportedShape,
    },
    /// 大きさの計算が溢れた。
    SizeOverflow,
}

impl fmt::Display for SpirvExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { words } => {
                write!(f, "module is shorter than the header (words={words})")
            }
            Self::BadMagic { found } => write!(f, "bad magic number (found={found:#010x})"),
            Self::ByteOrderReversed => write!(f, "magic number has reversed byte order"),
            Self::ZeroWordCount { offset } => {
                write!(f, "instruction word count is zero (offset={offset})")
            }
            Self::TruncatedInstruction {
                offset,
                word_count,
                remaining,
            } => write!(
                f,
                "instruction exceeds the module (offset={offset}, word_count={word_count}, remaining={remaining})"
            ),
            Self::MissingOperands { offset, opcode } => write!(
                f,
                "instruction has too few operands (offset={offset}, opcode={opcode})"
            ),
            Self::UnknownId { id } => write!(f, "reference to an undefined id (id={id})"),
            Self::WrongKind { id } => {
                write!(f, "id refers to an unexpected kind of definition (id={id})")
            }
            Self::MissingDecoration { id } => write!(
                f,
                "binding or descriptor set decoration is missing (id={id})"
            ),
            Self::DuplicateDefinition { id } => {
                write!(f, "duplicate definition or decoration (id={id})")
            }
            Self::DuplicateBinding { set, binding } => {
                write!(f, "duplicate binding (set={set}, binding={binding})")
            }
            Self::Unsupported { id, shape } => {
                write!(f, "unsupported operand shape (id={id}, shape={shape:?})")
            }
            Self::SizeOverflow => write!(f, "size computation overflows"),
        }
    }
}

impl std::error::Error for SpirvExtractError {}

/// 型の定義のうち、解決に要る部分だけを持つ。
enum TypeDef {
    Bool,
    Int { bits: u32, signed: bool },
    Float { bits: u32 },
    VectorOrMatrix,
    Array { elem: u32, len_id: u32 },
    RuntimeArray { elem: u32 },
    Struct { members: Vec<u32> },
    Pointer { pointee: u32 },
}

/// id ごとの装飾。
#[derive(Default)]
struct Deco {
    set: Option<u32>,
    binding: Option<u32>,
    block: bool,
    buffer_block: bool,
    array_stride: Option<u32>,
}

/// `OpConstant` の結果型と値。
struct Constant {
    type_id: u32,
    value: u32,
    value_words: usize,
}

struct Variable {
    id: u32,
    ptr_type: u32,
    storage_class: u32,
}

/// 語番号 `i` のオペランドを取る。足りなければ `MissingOperands`。
fn op(ops: &[u32], i: usize, offset: usize, opcode: u16) -> Result<u32, SpirvExtractError> {
    ops.get(i)
        .copied()
        .ok_or(SpirvExtractError::MissingOperands { offset, opcode })
}

/// 型・定数・変数で共有する定義済み id の集合に登録する。種類をまたぐ重複も拒否する。
fn claim(defined: &mut BTreeSet<u32>, id: u32) -> Result<(), SpirvExtractError> {
    if defined.insert(id) {
        Ok(())
    } else {
        Err(SpirvExtractError::DuplicateDefinition { id })
    }
}

fn define(
    defined: &mut BTreeSet<u32>,
    types: &mut BTreeMap<u32, TypeDef>,
    id: u32,
    def: TypeDef,
) -> Result<(), SpirvExtractError> {
    claim(defined, id)?;
    types.insert(id, def);
    Ok(())
}

fn set_once(slot: &mut Option<u32>, id: u32, value: u32) -> Result<(), SpirvExtractError> {
    if slot.replace(value).is_some() {
        return Err(SpirvExtractError::DuplicateDefinition { id });
    }
    Ok(())
}

/// SPIR-V のモジュール（語の列）から storage buffer の宣言を取り出す。
///
/// 並びは（set, binding）の昇順。対象外の形・不正な入力は [`SpirvExtractError`] で返す。
pub fn extract_storage_buffer_operands(
    words: &[u32],
) -> Result<Vec<ExtractedOperand>, SpirvExtractError> {
    let magic = *words
        .first()
        .ok_or(SpirvExtractError::TooShort { words: 0 })?;
    if words.len() < HEADER_WORDS {
        return Err(SpirvExtractError::TooShort { words: words.len() });
    }
    if magic == MAGIC_SWAPPED {
        return Err(SpirvExtractError::ByteOrderReversed);
    }
    if magic != MAGIC {
        return Err(SpirvExtractError::BadMagic { found: magic });
    }

    let mut types: BTreeMap<u32, TypeDef> = BTreeMap::new();
    let mut decos: BTreeMap<u32, Deco> = BTreeMap::new();
    // id -> 定数の情報（結果型・最初の語の値・値の語数）。
    let mut constants: BTreeMap<u32, Constant> = BTreeMap::new();
    // 型・定数・変数が共有する定義済み id。
    let mut defined: BTreeSet<u32> = BTreeSet::new();
    // (struct の id, メンバー番号) -> Offset。
    let mut member_offsets: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut variables: Vec<Variable> = Vec::new();

    let mut pos = HEADER_WORDS;
    while pos < words.len() {
        let first = words.get(pos).copied().unwrap_or(0);
        let count = (first >> 16) as usize;
        let opcode = (first & 0xFFFF) as u16;
        if count == 0 {
            return Err(SpirvExtractError::ZeroWordCount { offset: pos });
        }
        let remaining = words.len() - pos;
        if count > remaining {
            return Err(SpirvExtractError::TruncatedInstruction {
                offset: pos,
                word_count: count,
                remaining,
            });
        }
        let end = pos + count;
        let ops = words.get(pos + 1..end).unwrap_or(&[]);
        let o = |i: usize| op(ops, i, pos, opcode);

        match opcode {
            OP_TYPE_BOOL => define(&mut defined, &mut types, o(0)?, TypeDef::Bool)?,
            OP_TYPE_INT => define(
                &mut defined,
                &mut types,
                o(0)?,
                TypeDef::Int {
                    bits: o(1)?,
                    signed: o(2)? != 0,
                },
            )?,
            OP_TYPE_FLOAT => define(
                &mut defined,
                &mut types,
                o(0)?,
                TypeDef::Float { bits: o(1)? },
            )?,
            OP_TYPE_VECTOR | OP_TYPE_MATRIX => {
                define(&mut defined, &mut types, o(0)?, TypeDef::VectorOrMatrix)?;
            }
            OP_TYPE_ARRAY => define(
                &mut defined,
                &mut types,
                o(0)?,
                TypeDef::Array {
                    elem: o(1)?,
                    len_id: o(2)?,
                },
            )?,
            OP_TYPE_RUNTIME_ARRAY => {
                define(
                    &mut defined,
                    &mut types,
                    o(0)?,
                    TypeDef::RuntimeArray { elem: o(1)? },
                )?;
            }
            OP_TYPE_STRUCT => define(
                &mut defined,
                &mut types,
                o(0)?,
                TypeDef::Struct {
                    members: ops.get(1..).unwrap_or(&[]).to_vec(),
                },
            )?,
            OP_TYPE_POINTER => define(
                &mut defined,
                &mut types,
                o(0)?,
                TypeDef::Pointer { pointee: o(2)? },
            )?,
            OP_CONSTANT => {
                // 1 語の値だけを配列の長さに使う。2 語以上の値は語数を覚えて後で拒否する。
                let id = o(1)?;
                let value = o(2)?;
                claim(&mut defined, id)?;
                constants.insert(
                    id,
                    Constant {
                        type_id: o(0)?,
                        value,
                        value_words: ops.len().saturating_sub(2),
                    },
                );
            }
            OP_VARIABLE => {
                let id = o(1)?;
                claim(&mut defined, id)?;
                variables.push(Variable {
                    id,
                    ptr_type: o(0)?,
                    storage_class: o(2)?,
                });
            }
            OP_MEMBER_DECORATE => {
                if o(2)? == DECORATION_OFFSET {
                    let key = (o(0)?, o(1)?);
                    if member_offsets.insert(key, o(3)?).is_some() {
                        return Err(SpirvExtractError::DuplicateDefinition { id: key.0 });
                    }
                }
            }
            OP_DECORATE => {
                let target = o(0)?;
                let kind = o(1)?;
                let d = decos.entry(target).or_default();
                match kind {
                    DECORATION_BLOCK => d.block = true,
                    DECORATION_BUFFER_BLOCK => d.buffer_block = true,
                    DECORATION_ARRAY_STRIDE => set_once(&mut d.array_stride, target, o(2)?)?,
                    DECORATION_BINDING => set_once(&mut d.binding, target, o(2)?)?,
                    DECORATION_DESCRIPTOR_SET => set_once(&mut d.set, target, o(2)?)?,
                    _ => {}
                }
            }
            _ => {}
        }
        pos = end;
    }

    let mut out: Vec<ExtractedOperand> = Vec::new();
    for var in &variables {
        if let Some(operand) = resolve_variable(var, &types, &decos, &constants, &member_offsets)? {
            out.push(operand);
        }
    }
    out.sort_by_key(|x| (x.set, x.binding));
    for pair in out.windows(2) {
        if let [a, b] = pair
            && (a.set, a.binding) == (b.set, b.binding)
        {
            return Err(SpirvExtractError::DuplicateBinding {
                set: a.set,
                binding: a.binding,
            });
        }
    }
    Ok(out)
}

fn lookup(types: &BTreeMap<u32, TypeDef>, id: u32) -> Result<&TypeDef, SpirvExtractError> {
    types.get(&id).ok_or(SpirvExtractError::UnknownId { id })
}

/// 変数 1 つを固定の段数（変数 → pointer → block → 配列 → 要素）で解決する。対象外の変数は `None`。
fn resolve_variable(
    var: &Variable,
    types: &BTreeMap<u32, TypeDef>,
    decos: &BTreeMap<u32, Deco>,
    constants: &BTreeMap<u32, Constant>,
    member_offsets: &BTreeMap<(u32, u32), u32>,
) -> Result<Option<ExtractedOperand>, SpirvExtractError> {
    let unsupported = |shape| SpirvExtractError::Unsupported { id: var.id, shape };
    // Uniform / StorageBuffer 以外（UniformConstant の画像・サンプラ等、`TypeDef` に入れない型を
    // 指す変数を含む）は、型を引く前に対象外として返す。
    if var.storage_class != STORAGE_CLASS_UNIFORM
        && var.storage_class != STORAGE_CLASS_STORAGE_BUFFER
    {
        return Ok(None);
    }
    let TypeDef::Pointer { pointee } = lookup(types, var.ptr_type)? else {
        return Err(SpirvExtractError::WrongKind { id: var.ptr_type });
    };
    let pointee_def = lookup(types, *pointee)?;

    if var.storage_class == STORAGE_CLASS_UNIFORM {
        // 旧形式の storage buffer は、struct 直接でも（runtime）配列の要素でも対象外として返す。
        let block_id = match pointee_def {
            TypeDef::Array { elem, .. } | TypeDef::RuntimeArray { elem } => elem,
            _ => pointee,
        };
        if matches!(types.get(block_id), Some(TypeDef::Struct { .. }))
            && decos.get(block_id).is_some_and(|d| d.buffer_block)
        {
            return Err(unsupported(UnsupportedShape::LegacyBufferBlock));
        }
        return Ok(None);
    }

    let (set, binding) = decos
        .get(&var.id)
        .and_then(|d| d.set.zip(d.binding))
        .ok_or(SpirvExtractError::MissingDecoration { id: var.id })?;

    let members = match pointee_def {
        TypeDef::Struct { members } => members,
        TypeDef::Array { elem, .. } | TypeDef::RuntimeArray { elem }
            if matches!(types.get(elem), Some(TypeDef::Struct { .. })) =>
        {
            return Err(unsupported(UnsupportedShape::BlockArray));
        }
        _ => return Err(unsupported(UnsupportedShape::OtherType)),
    };
    if !decos.get(pointee).is_some_and(|d| d.block) {
        return Err(unsupported(UnsupportedShape::BlockShape));
    }
    let [member_id] = members.as_slice() else {
        return Err(unsupported(UnsupportedShape::BlockShape));
    };
    let member_id = *member_id;
    if member_offsets
        .get(&(*pointee, 0))
        .is_some_and(|&offset| offset != 0)
    {
        return Err(unsupported(UnsupportedShape::NonZeroMemberOffset));
    }

    let (elem_id, len_id) = match lookup(types, member_id)? {
        TypeDef::Array { elem, len_id } => (*elem, Some(*len_id)),
        TypeDef::RuntimeArray { elem } => (*elem, None),
        _ => return Err(unsupported(UnsupportedShape::BlockShape)),
    };

    let (element, bits) = match lookup(types, elem_id)? {
        TypeDef::Float { bits } => (ScalarKind::Float { bits: *bits }, *bits),
        TypeDef::Int { bits, signed } => (
            ScalarKind::Int {
                bits: *bits,
                signed: *signed,
            },
            *bits,
        ),
        TypeDef::Bool => return Err(unsupported(UnsupportedShape::Bool)),
        TypeDef::VectorOrMatrix => return Err(unsupported(UnsupportedShape::VectorOrMatrix)),
        TypeDef::Struct { .. } => return Err(unsupported(UnsupportedShape::StructElement)),
        _ => return Err(unsupported(UnsupportedShape::OtherType)),
    };
    if bits == 0 || bits % 8 != 0 {
        return Err(unsupported(UnsupportedShape::BitWidth));
    }
    let element_bytes = bits / 8;

    let array_stride = decos.get(&member_id).and_then(|d| d.array_stride);
    if array_stride == Some(0) {
        return Err(unsupported(UnsupportedShape::ZeroStride));
    }
    if array_stride.is_some_and(|stride| stride < element_bytes) {
        return Err(unsupported(UnsupportedShape::StrideSmallerThanElement));
    }

    let byte_length = match len_id {
        None => ByteLength::Unsized,
        Some(len_id) => {
            let constant = constants
                .get(&len_id)
                .ok_or(unsupported(UnsupportedShape::LengthNotConstant))?;
            // 結果型は 32 ビットの整数型で、値は 1 語、正の値だけを長さとして受理する。
            // 32 ビット以外は語数・符号の解釈が変わるため受理しない。
            let TypeDef::Int { bits, signed } = lookup(types, constant.type_id)? else {
                return Err(unsupported(UnsupportedShape::LengthNotConstant));
            };
            if *bits != 32 || constant.value_words != 1 {
                return Err(unsupported(UnsupportedShape::LengthNotConstant));
            }
            let positive = if *signed {
                constant.value != 0 && constant.value >> 31 == 0
            } else {
                constant.value != 0
            };
            if !positive {
                return Err(unsupported(UnsupportedShape::InvalidLength));
            }
            let stride = array_stride.ok_or(unsupported(UnsupportedShape::MissingArrayStride))?;
            ByteLength::Fixed(
                u64::from(constant.value)
                    .checked_mul(u64::from(stride))
                    .ok_or(SpirvExtractError::SizeOverflow)?,
            )
        }
    };

    Ok(Some(ExtractedOperand {
        set,
        binding,
        element,
        element_bytes,
        array_stride,
        byte_length,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 命令 1 つを語の列にする（語数は自動で入れる）。
    fn inst(opcode: u16, ops: &[u32]) -> Vec<u32> {
        let mut v = vec![((ops.len() as u32 + 1) << 16) | u32::from(opcode)];
        v.extend_from_slice(ops);
        v
    }

    fn module(insts: &[Vec<u32>]) -> Vec<u32> {
        let mut w = vec![MAGIC, 0x0001_0000, 0, 100, 0];
        for i in insts {
            w.extend_from_slice(i);
        }
        w
    }

    /// 最小の storage buffer。id: 1=f32 2=配列 3=struct 4=pointer 5=変数 6=u32 7=長さ。
    fn buffer_insts(runtime: bool, len: u32, set: u32, binding: u32) -> Vec<Vec<u32>> {
        vec![
            inst(OP_DECORATE, &[3, DECORATION_BLOCK]),
            inst(OP_DECORATE, &[2, DECORATION_ARRAY_STRIDE, 4]),
            inst(OP_DECORATE, &[5, DECORATION_DESCRIPTOR_SET, set]),
            inst(OP_DECORATE, &[5, DECORATION_BINDING, binding]),
            inst(OP_TYPE_FLOAT, &[1, 32]),
            inst(OP_TYPE_INT, &[6, 32, 0]),
            inst(OP_CONSTANT, &[6, 7, len]),
            if runtime {
                inst(OP_TYPE_RUNTIME_ARRAY, &[2, 1])
            } else {
                inst(OP_TYPE_ARRAY, &[2, 1, 7])
            },
            inst(OP_TYPE_STRUCT, &[3, 2]),
            inst(OP_TYPE_POINTER, &[4, STORAGE_CLASS_STORAGE_BUFFER, 3]),
            inst(OP_VARIABLE, &[4, 5, STORAGE_CLASS_STORAGE_BUFFER]),
        ]
    }

    #[test]
    fn runtime_array_is_unsized() {
        let m = module(&buffer_insts(true, 0, 1, 2));
        let got = extract_storage_buffer_operands(&m).unwrap();
        assert_eq!(got.len(), 1);
        let x = got[0];
        assert_eq!((x.set, x.binding), (1, 2));
        assert_eq!(x.element, ScalarKind::Float { bits: 32 });
        assert_eq!(x.element_bytes, 4);
        assert_eq!(x.array_stride, Some(4));
        assert_eq!(x.byte_length, ByteLength::Unsized);
    }

    #[test]
    fn fixed_array_has_fixed_length() {
        let m = module(&buffer_insts(false, 16, 0, 0));
        let got = extract_storage_buffer_operands(&m).unwrap();
        assert_eq!(got[0].byte_length, ByteLength::Fixed(64));
    }

    #[test]
    fn rejects_malformed_input() {
        let good = module(&buffer_insts(true, 0, 0, 0));
        assert_eq!(
            extract_storage_buffer_operands(&good[..3]),
            Err(SpirvExtractError::TooShort { words: 3 })
        );
        let mut bad = good.clone();
        bad[0] = 0xdead_beef;
        assert!(matches!(
            extract_storage_buffer_operands(&bad),
            Err(SpirvExtractError::BadMagic { .. })
        ));
        bad[0] = MAGIC_SWAPPED;
        assert_eq!(
            extract_storage_buffer_operands(&bad),
            Err(SpirvExtractError::ByteOrderReversed)
        );
        let mut zero = module(&[]);
        zero.push(0);
        assert!(matches!(
            extract_storage_buffer_operands(&zero),
            Err(SpirvExtractError::ZeroWordCount { .. })
        ));
        let cut = &good[..good.len() - 1];
        assert!(matches!(
            extract_storage_buffer_operands(cut),
            Err(SpirvExtractError::TruncatedInstruction { .. })
        ));
    }

    #[test]
    fn rejects_bool_element() {
        let m = module(&[
            inst(OP_DECORATE, &[3, DECORATION_BLOCK]),
            inst(OP_DECORATE, &[5, DECORATION_DESCRIPTOR_SET, 0]),
            inst(OP_DECORATE, &[5, DECORATION_BINDING, 0]),
            inst(OP_TYPE_BOOL, &[1]),
            inst(OP_TYPE_RUNTIME_ARRAY, &[2, 1]),
            inst(OP_TYPE_STRUCT, &[3, 2]),
            inst(OP_TYPE_POINTER, &[4, STORAGE_CLASS_STORAGE_BUFFER, 3]),
            inst(OP_VARIABLE, &[4, 5, STORAGE_CLASS_STORAGE_BUFFER]),
        ]);
        assert_eq!(
            extract_storage_buffer_operands(&m),
            Err(SpirvExtractError::Unsupported {
                id: 5,
                shape: UnsupportedShape::Bool
            })
        );
    }

    #[test]
    fn rejects_missing_binding_and_duplicate_binding() {
        let mut insts = buffer_insts(true, 0, 0, 0);
        let without_binding: Vec<Vec<u32>> = insts
            .iter()
            .filter(|i| i.get(2) != Some(&DECORATION_BINDING))
            .cloned()
            .collect();
        assert_eq!(
            extract_storage_buffer_operands(&module(&without_binding)),
            Err(SpirvExtractError::MissingDecoration { id: 5 })
        );

        // 同じ（set, binding）を持つ 2 つ目の変数（id 8）を足す。
        insts.push(inst(OP_DECORATE, &[8, DECORATION_DESCRIPTOR_SET, 0]));
        insts.push(inst(OP_DECORATE, &[8, DECORATION_BINDING, 0]));
        insts.push(inst(OP_VARIABLE, &[4, 8, STORAGE_CLASS_STORAGE_BUFFER]));
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            Err(SpirvExtractError::DuplicateBinding { set: 0, binding: 0 })
        );
    }

    #[test]
    fn image_variable_in_uniform_constant_is_skipped() {
        // id: 20=OpTypeImage 21=pointer(UniformConstant) 22=変数。画像は `TypeDef` に入らない型。
        let mut insts = buffer_insts(true, 0, 0, 0);
        insts.push(inst(25, &[20, 1, 1, 1, 0, 0, 2, 0, 0]));
        insts.push(inst(OP_TYPE_POINTER, &[21, 0, 20]));
        insts.push(inst(OP_VARIABLE, &[21, 22, 0]));
        let got = extract_storage_buffer_operands(&module(&insts)).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].set, 0);
    }

    fn unsupported(shape: UnsupportedShape) -> Result<Vec<ExtractedOperand>, SpirvExtractError> {
        Err(SpirvExtractError::Unsupported { id: 5, shape })
    }

    /// `buffer_insts` の命令のうち、条件に合うものを置き換える。
    fn replace_inst(insts: &mut [Vec<u32>], opcode: u16, ops: &[u32], new: Vec<u32>) {
        let target = inst(opcode, ops);
        let slot = insts.iter_mut().find(|i| **i == target).unwrap();
        *slot = new;
    }

    #[test]
    fn fixed_array_without_stride_is_rejected() {
        let insts: Vec<Vec<u32>> = buffer_insts(false, 16, 0, 0)
            .into_iter()
            .filter(|i| i.get(2) != Some(&DECORATION_ARRAY_STRIDE))
            .collect();
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            unsupported(UnsupportedShape::MissingArrayStride)
        );
    }

    #[test]
    fn stride_smaller_than_element_is_rejected() {
        let mut insts = buffer_insts(false, 16, 0, 0);
        replace_inst(
            &mut insts,
            OP_DECORATE,
            &[2, DECORATION_ARRAY_STRIDE, 4],
            inst(OP_DECORATE, &[2, DECORATION_ARRAY_STRIDE, 2]),
        );
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            unsupported(UnsupportedShape::StrideSmallerThanElement)
        );
    }

    #[test]
    fn nonzero_member_offset_is_rejected_and_zero_is_accepted() {
        let mut insts = buffer_insts(false, 16, 0, 0);
        insts.push(inst(OP_MEMBER_DECORATE, &[3, 0, DECORATION_OFFSET, 0]));
        assert!(extract_storage_buffer_operands(&module(&insts)).is_ok());
        let mut insts = buffer_insts(false, 16, 0, 0);
        insts.push(inst(OP_MEMBER_DECORATE, &[3, 0, DECORATION_OFFSET, 16]));
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            unsupported(UnsupportedShape::NonZeroMemberOffset)
        );
    }

    #[test]
    fn length_constant_must_be_positive_integer() {
        // 浮動小数点型（id 1）の定数。
        let mut insts = buffer_insts(false, 16, 0, 0);
        replace_inst(
            &mut insts,
            OP_CONSTANT,
            &[6, 7, 16],
            inst(OP_CONSTANT, &[1, 7, 16]),
        );
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            unsupported(UnsupportedShape::LengthNotConstant)
        );
        // 未定義の結果型。
        let mut insts = buffer_insts(false, 16, 0, 0);
        replace_inst(
            &mut insts,
            OP_CONSTANT,
            &[6, 7, 16],
            inst(OP_CONSTANT, &[99, 7, 16]),
        );
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            Err(SpirvExtractError::UnknownId { id: 99 })
        );
        // 符号付き 32 ビットの -1、および 0。
        for value in [u32::MAX, 0] {
            let mut insts = buffer_insts(false, value, 0, 0);
            replace_inst(
                &mut insts,
                OP_TYPE_INT,
                &[6, 32, 0],
                inst(OP_TYPE_INT, &[6, 32, 1]),
            );
            assert_eq!(
                extract_storage_buffer_operands(&module(&insts)),
                unsupported(UnsupportedShape::InvalidLength)
            );
        }
        // 符号なしの 0 も長さにならない。
        assert_eq!(
            extract_storage_buffer_operands(&module(&buffer_insts(false, 0, 0, 0))),
            unsupported(UnsupportedShape::InvalidLength)
        );
    }

    #[test]
    fn length_constant_must_be_32_bit() {
        // 64 ビット整数型の定数は、1 語でも長さとして受理しない。
        for signed in [0, 1] {
            let mut insts = buffer_insts(false, 16, 0, 0);
            replace_inst(
                &mut insts,
                OP_TYPE_INT,
                &[6, 32, 0],
                inst(OP_TYPE_INT, &[6, 64, signed]),
            );
            assert_eq!(
                extract_storage_buffer_operands(&module(&insts)),
                unsupported(UnsupportedShape::LengthNotConstant)
            );
        }
    }

    /// 旧形式（`Uniform` + `BufferBlock`）の変数。id: 1=f32 2=配列 3=struct 4=pointer 5=変数。
    fn legacy_insts(pointee_array: Option<bool>) -> Vec<Vec<u32>> {
        let mut insts = vec![
            inst(OP_DECORATE, &[3, DECORATION_BUFFER_BLOCK]),
            inst(OP_TYPE_FLOAT, &[1, 32]),
            inst(OP_TYPE_RUNTIME_ARRAY, &[2, 1]),
            inst(OP_TYPE_STRUCT, &[3, 2]),
        ];
        let pointee = match pointee_array {
            None => 3,
            Some(runtime) => {
                insts.push(inst(OP_TYPE_INT, &[6, 32, 0]));
                insts.push(inst(OP_CONSTANT, &[6, 7, 2]));
                insts.push(if runtime {
                    inst(OP_TYPE_RUNTIME_ARRAY, &[8, 3])
                } else {
                    inst(OP_TYPE_ARRAY, &[8, 3, 7])
                });
                8
            }
        };
        insts.push(inst(OP_TYPE_POINTER, &[4, STORAGE_CLASS_UNIFORM, pointee]));
        insts.push(inst(OP_VARIABLE, &[4, 5, STORAGE_CLASS_UNIFORM]));
        insts
    }

    #[test]
    fn legacy_buffer_block_is_rejected_including_arrays() {
        for shape in [None, Some(false), Some(true)] {
            assert_eq!(
                extract_storage_buffer_operands(&module(&legacy_insts(shape))),
                unsupported(UnsupportedShape::LegacyBufferBlock),
                "{shape:?}"
            );
        }
    }

    #[test]
    fn duplicate_id_across_kinds_is_rejected() {
        // 変数 id（5）を型として再定義する。
        let mut insts = buffer_insts(true, 0, 0, 0);
        insts.push(inst(OP_TYPE_BOOL, &[5]));
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            Err(SpirvExtractError::DuplicateDefinition { id: 5 })
        );
        // 型 id（4）を変数として再利用する。
        let mut insts = buffer_insts(true, 0, 0, 0);
        insts.push(inst(OP_VARIABLE, &[4, 4, STORAGE_CLASS_STORAGE_BUFFER]));
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            Err(SpirvExtractError::DuplicateDefinition { id: 4 })
        );
        // 定数 id（7）を型として再利用する。
        let mut insts = buffer_insts(true, 0, 0, 0);
        insts.push(inst(OP_TYPE_BOOL, &[7]));
        assert_eq!(
            extract_storage_buffer_operands(&module(&insts)),
            Err(SpirvExtractError::DuplicateDefinition { id: 7 })
        );
    }
}
