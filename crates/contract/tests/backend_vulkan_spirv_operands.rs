//! SPIR-V からオペランドの宣言を取り出す検証用の道具を、別 crate から公開の入口だけで確かめる統合テスト（spec TASK-98）。
//!
//! 走るのは feature `backend-vulkan` と `test-support` の両方が有効な組だけ（`Cargo.toml` の `required-features`）。
//! 入力はこのファイルの中で語の列として組み立てる。バイナリのファイル・外部の道具・GPU には依らない。
//! 命令・装飾・storage class の番号は Khronos の公開の SPIR-V 仕様のもので、道具の内部の定数には依らず独立に持つ。
//! 置き場・ゲートの説明は `docs/vulkan-operand-extraction.md`。
//! spec との対応は `docs/spec/05-tasks.md` の TASK-98 のポインタのみ。

#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_vulkan::spirv_operands::{
    ByteLength, ExtractedOperand, ScalarKind, SpirvExtractError, UnsupportedShape,
    extract_storage_buffer_operands,
};

// SPIR-V 仕様のバイナリ形式（ヘッダー 5 語。命令の先頭の語は上位 16 ビットが語数、下位 16 ビットが opcode）。
const MAGIC: u32 = 0x0723_0203;
const MAGIC_REVERSED: u32 = 0x0302_2307;
const HEADER_WORDS: usize = 5;

const OP_NAME: u16 = 5;
const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;
const OP_EXECUTION_MODE: u16 = 16;
const OP_CAPABILITY: u16 = 17;
const OP_TYPE_VOID: u16 = 19;
const OP_TYPE_BOOL: u16 = 20;
const OP_TYPE_INT: u16 = 21;
const OP_TYPE_FLOAT: u16 = 22;
const OP_TYPE_VECTOR: u16 = 23;
const OP_TYPE_MATRIX: u16 = 24;
const OP_TYPE_ARRAY: u16 = 28;
const OP_TYPE_RUNTIME_ARRAY: u16 = 29;
const OP_TYPE_STRUCT: u16 = 30;
const OP_TYPE_POINTER: u16 = 32;
const OP_TYPE_FUNCTION: u16 = 33;
const OP_CONSTANT: u16 = 43;
const OP_SPEC_CONSTANT: u16 = 50;
const OP_FUNCTION: u16 = 54;
const OP_FUNCTION_END: u16 = 56;
const OP_VARIABLE: u16 = 59;
const OP_DECORATE: u16 = 71;
const OP_MEMBER_DECORATE: u16 = 72;
const OP_DECORATION_GROUP: u16 = 73;
const OP_GROUP_DECORATE: u16 = 74;
const OP_GROUP_MEMBER_DECORATE: u16 = 75;
const OP_LABEL: u16 = 248;
const OP_RETURN: u16 = 253;

const DECO_BLOCK: u32 = 2;
const DECO_BUFFER_BLOCK: u32 = 3;
const DECO_ARRAY_STRIDE: u32 = 6;
const DECO_NON_WRITABLE: u32 = 24;
const DECO_BINDING: u32 = 33;
const DECO_DESCRIPTOR_SET: u32 = 34;
const DECO_OFFSET: u32 = 35;

const SC_UNIFORM_CONSTANT: u32 = 0;
const SC_INPUT: u32 = 1;
const SC_UNIFORM: u32 = 2;
const SC_OUTPUT: u32 = 3;
const SC_WORKGROUP: u32 = 4;
const SC_PRIVATE: u32 = 6;
const SC_FUNCTION: u32 = 7;
const SC_PUSH_CONSTANT: u32 = 9;
const SC_STORAGE_BUFFER: u32 = 12;

type Inst = Vec<u32>;

/// 命令 1 つを語の列にする（語数は自動で入れる）。
fn inst(opcode: u16, ops: &[u32]) -> Inst {
    let mut v = vec![((ops.len() as u32 + 1) << 16) | u32::from(opcode)];
    v.extend_from_slice(ops);
    v
}

fn opcode_of(i: &Inst) -> u16 {
    (i[0] & 0xFFFF) as u16
}

fn module(insts: &[Inst]) -> Vec<u32> {
    let mut w = vec![MAGIC, 0x0001_0000, 0, 1000, 0];
    for i in insts {
        w.extend_from_slice(i);
    }
    w
}

/// 命令の列から、opcode と先頭のオペランドが合うものを取り除く。
fn remove(insts: &mut Vec<Inst>, opcode: u16, ops_prefix: &[u32]) {
    let before = insts.len();
    insts.retain(|i| !(opcode_of(i) == opcode && i[1..].starts_with(ops_prefix)));
    assert!(insts.len() < before, "removal target not found");
}

/// 命令の列のうち、opcode と先頭のオペランドが合うものを差し替える。
fn replace(insts: &mut [Inst], opcode: u16, ops_prefix: &[u32], new: Inst) {
    let slot = insts
        .iter_mut()
        .find(|i| opcode_of(i) == opcode && i[1..].starts_with(ops_prefix))
        .expect("replacement target not found");
    *slot = new;
}

/// 要素の型。
#[derive(Clone, Copy)]
enum Elem {
    Float(u32),
    Int(u32, bool),
}

/// 配列の長さの指定。
#[derive(Clone, Copy)]
enum Len {
    Runtime,
    Fixed { value: u32, signed: bool },
}

/// buffer 1 つの指定。引数を増やさないための束ね。
#[derive(Clone, Copy)]
struct Buf {
    elem: Elem,
    len: Len,
    stride: Option<u32>,
    set: u32,
    binding: u32,
    /// id の起点。複数の buffer を 1 つのモジュールに入れるときは重ならない値にする。
    base: u32,
}

impl Buf {
    /// f32 の runtime array（stride 4）、set 0・binding 0。
    fn f32_runtime() -> Self {
        Buf {
            elem: Elem::Float(32),
            len: Len::Runtime,
            stride: Some(4),
            set: 0,
            binding: 0,
            base: 0,
        }
    }

    /// f32 の固定長（符号なし 32 ビットの長さ）、stride 4。
    fn f32_fixed(len: u32) -> Self {
        Buf {
            len: Len::Fixed {
                value: len,
                signed: false,
            },
            ..Self::f32_runtime()
        }
    }

    fn at(self, set: u32, binding: u32, base: u32) -> Self {
        Buf {
            set,
            binding,
            base,
            ..self
        }
    }

    // id の割り当て: +1 要素の型、+2 配列の型、+3 struct、+4 pointer、+5 変数、+6 長さの整数型、+7 長さの定数。
    fn elem_id(&self) -> u32 {
        self.base + 1
    }
    fn array_id(&self) -> u32 {
        self.base + 2
    }
    fn struct_id(&self) -> u32 {
        self.base + 3
    }
    fn ptr_id(&self) -> u32 {
        self.base + 4
    }
    fn var_id(&self) -> u32 {
        self.base + 5
    }
    fn len_ty_id(&self) -> u32 {
        self.base + 6
    }
    fn len_id(&self) -> u32 {
        self.base + 7
    }

    /// 道具が読む形の storage buffer 1 つ分の命令（装飾 → 型 → 定数 → 変数の順）。
    fn insts(&self) -> Vec<Inst> {
        let mut v = vec![
            inst(OP_DECORATE, &[self.struct_id(), DECO_BLOCK]),
            inst(OP_MEMBER_DECORATE, &[self.struct_id(), 0, DECO_OFFSET, 0]),
            inst(OP_DECORATE, &[self.var_id(), DECO_DESCRIPTOR_SET, self.set]),
            inst(OP_DECORATE, &[self.var_id(), DECO_BINDING, self.binding]),
        ];
        if let Some(stride) = self.stride {
            v.push(inst(
                OP_DECORATE,
                &[self.array_id(), DECO_ARRAY_STRIDE, stride],
            ));
        }
        v.push(match self.elem {
            Elem::Float(bits) => inst(OP_TYPE_FLOAT, &[self.elem_id(), bits]),
            Elem::Int(bits, signed) => {
                inst(OP_TYPE_INT, &[self.elem_id(), bits, u32::from(signed)])
            }
        });
        match self.len {
            Len::Runtime => v.push(inst(
                OP_TYPE_RUNTIME_ARRAY,
                &[self.array_id(), self.elem_id()],
            )),
            Len::Fixed { value, signed } => {
                v.push(inst(
                    OP_TYPE_INT,
                    &[self.len_ty_id(), 32, u32::from(signed)],
                ));
                v.push(inst(OP_CONSTANT, &[self.len_ty_id(), self.len_id(), value]));
                v.push(inst(
                    OP_TYPE_ARRAY,
                    &[self.array_id(), self.elem_id(), self.len_id()],
                ));
            }
        }
        v.push(inst(OP_TYPE_STRUCT, &[self.struct_id(), self.array_id()]));
        v.push(inst(
            OP_TYPE_POINTER,
            &[self.ptr_id(), SC_STORAGE_BUFFER, self.struct_id()],
        ));
        v.push(inst(
            OP_VARIABLE,
            &[self.ptr_id(), self.var_id(), SC_STORAGE_BUFFER],
        ));
        v
    }
}

fn extract(insts: &[Inst]) -> Result<Vec<ExtractedOperand>, SpirvExtractError> {
    extract_storage_buffer_operands(&module(insts))
}

/// 単一 buffer（変数の id は 5）の命令を作り、差し替えてから取り出した結果を返す。
fn extract_mutated(
    buf: Buf,
    mutate: impl FnOnce(&mut Vec<Inst>),
) -> Result<Vec<ExtractedOperand>, SpirvExtractError> {
    let mut insts = buf.insts();
    mutate(&mut insts);
    extract(&insts)
}

fn unsupported(id: u32, shape: UnsupportedShape) -> SpirvExtractError {
    SpirvExtractError::Unsupported { id, shape }
}

fn operand(
    set: u32,
    binding: u32,
    element: ScalarKind,
    element_bytes: u32,
    array_stride: Option<u32>,
    byte_length: ByteLength,
) -> ExtractedOperand {
    ExtractedOperand {
        set,
        binding,
        element,
        element_bytes,
        array_stride,
        byte_length,
    }
}

// ---------------------------------------------------------------------------
// A. 別 crate からの到達と、期待値との照合
// ---------------------------------------------------------------------------

#[test]
fn tool_is_reachable_from_another_crate() {
    assert_eq!(extract(&[]), Ok(vec![]));
}

#[test]
fn extracted_operands_equal_expected_declarations() {
    let bufs = [
        Buf {
            elem: Elem::Int(32, false),
            len: Len::Fixed {
                value: 5,
                signed: false,
            },
            stride: Some(4),
            ..Buf::f32_runtime()
        }
        .at(1, 3, 10),
        Buf {
            stride: None,
            ..Buf::f32_runtime()
        }
        .at(0, 2, 20),
        Buf {
            elem: Elem::Float(64),
            len: Len::Fixed {
                value: 3,
                signed: false,
            },
            stride: Some(16),
            ..Buf::f32_runtime()
        }
        .at(0, 0, 30),
        Buf {
            elem: Elem::Int(8, true),
            stride: Some(1),
            ..Buf::f32_runtime()
        }
        .at(2, 0, 40),
        Buf {
            elem: Elem::Int(64, true),
            len: Len::Fixed {
                value: 1,
                signed: true,
            },
            stride: Some(8),
            ..Buf::f32_runtime()
        }
        .at(1, 0, 50),
    ];
    let insts: Vec<Inst> = bufs.iter().flat_map(Buf::insts).collect();
    let expected = vec![
        operand(
            0,
            0,
            ScalarKind::Float { bits: 64 },
            8,
            Some(16),
            ByteLength::Fixed(48),
        ),
        operand(
            0,
            2,
            ScalarKind::Float { bits: 32 },
            4,
            None,
            ByteLength::Unsized,
        ),
        operand(
            1,
            0,
            ScalarKind::Int {
                bits: 64,
                signed: true,
            },
            8,
            Some(8),
            ByteLength::Fixed(8),
        ),
        operand(
            1,
            3,
            ScalarKind::Int {
                bits: 32,
                signed: false,
            },
            4,
            Some(4),
            ByteLength::Fixed(20),
        ),
        operand(
            2,
            0,
            ScalarKind::Int {
                bits: 8,
                signed: true,
            },
            1,
            Some(1),
            ByteLength::Unsized,
        ),
    ];
    assert_eq!(extract(&insts), Ok(expected));
}

#[test]
fn order_is_by_set_then_binding() {
    let pairs = [
        (1, 0),
        (0, 1),
        (0, 0),
        (1, 1),
        (0, u32::MAX),
        (0, 5),
        (2, 5),
        (1, 5),
    ];
    let insts: Vec<Inst> = pairs
        .iter()
        .enumerate()
        .flat_map(|(k, &(s, b))| Buf::f32_runtime().at(s, b, 10 * (k as u32 + 1)).insts())
        .collect();
    let got = extract(&insts).unwrap();
    let got_pairs: Vec<(u32, u32)> = got.iter().map(|x| (x.set, x.binding)).collect();
    let mut sorted = pairs.to_vec();
    sorted.sort_unstable();
    assert_eq!(got_pairs, sorted);
    assert_eq!(got_pairs.len(), pairs.len());
}

#[test]
fn compiler_like_module_yields_only_storage_buffers() {
    let storage = Buf::f32_runtime().at(0, 1, 10);
    let mut insts = vec![
        inst(OP_CAPABILITY, &[1]),
        inst(OP_MEMORY_MODEL, &[0, 1]),
        inst(OP_ENTRY_POINT, &[5, 100, 0x6e69_616d, 0, storage.var_id()]),
        inst(OP_EXECUTION_MODE, &[100, 17, 64, 1, 1]),
        inst(OP_NAME, &[100, 0x6e69_616d, 0]),
        inst(OP_TYPE_VOID, &[101]),
        inst(OP_TYPE_FUNCTION, &[102, 101]),
        // 対象でない変数（Input / Private / PushConstant）。
        inst(OP_TYPE_POINTER, &[103, SC_INPUT, storage.elem_id()]),
        inst(OP_VARIABLE, &[103, 104, SC_INPUT]),
        inst(OP_TYPE_POINTER, &[105, SC_PRIVATE, storage.elem_id()]),
        inst(OP_VARIABLE, &[105, 106, SC_PRIVATE]),
        inst(
            OP_TYPE_POINTER,
            &[107, SC_PUSH_CONSTANT, storage.struct_id()],
        ),
        inst(OP_VARIABLE, &[107, 108, SC_PUSH_CONSTANT]),
    ];
    insts.extend(storage.insts());
    insts.extend([
        inst(OP_FUNCTION, &[101, 100, 0, 102]),
        inst(OP_LABEL, &[109]),
        inst(OP_RETURN, &[]),
        inst(OP_FUNCTION_END, &[]),
    ]);
    assert_eq!(
        extract(&insts),
        Ok(vec![operand(
            0,
            1,
            ScalarKind::Float { bits: 32 },
            4,
            Some(4),
            ByteLength::Unsized,
        )])
    );
}

#[test]
fn variables_sharing_types_are_each_extracted() {
    let first = Buf::f32_runtime().at(0, 0, 0);
    let mut insts = first.insts();
    // 2 つ目の block は同じ配列の型（要素の型・stride を含む）を共有する。
    insts.extend([
        inst(OP_DECORATE, &[13, DECO_BLOCK]),
        inst(OP_MEMBER_DECORATE, &[13, 0, DECO_OFFSET, 0]),
        inst(OP_TYPE_STRUCT, &[13, first.array_id()]),
        inst(OP_TYPE_POINTER, &[14, SC_STORAGE_BUFFER, 13]),
        inst(OP_VARIABLE, &[14, 15, SC_STORAGE_BUFFER]),
        inst(OP_DECORATE, &[15, DECO_DESCRIPTOR_SET, 0]),
        inst(OP_DECORATE, &[15, DECO_BINDING, 1]),
    ]);
    // 3 つ目の変数は 1 つ目と同じ pointer 型を共有する。
    insts.extend([
        inst(OP_VARIABLE, &[first.ptr_id(), 16, SC_STORAGE_BUFFER]),
        inst(OP_DECORATE, &[16, DECO_DESCRIPTOR_SET, 0]),
        inst(OP_DECORATE, &[16, DECO_BINDING, 2]),
    ]);
    let got = extract(&insts).unwrap();
    assert_eq!(got.len(), 3);
    for (k, x) in got.iter().enumerate() {
        assert_eq!((x.set, x.binding), (0, k as u32));
        assert_eq!(x.element, ScalarKind::Float { bits: 32 });
        assert_eq!(x.byte_length, ByteLength::Unsized);
    }
}

#[test]
fn scalar_kinds_and_element_bytes() {
    let cases = [
        (Elem::Float(16), ScalarKind::Float { bits: 16 }, 2),
        (Elem::Float(32), ScalarKind::Float { bits: 32 }, 4),
        (Elem::Float(64), ScalarKind::Float { bits: 64 }, 8),
        (
            Elem::Int(8, true),
            ScalarKind::Int {
                bits: 8,
                signed: true,
            },
            1,
        ),
        (
            Elem::Int(8, false),
            ScalarKind::Int {
                bits: 8,
                signed: false,
            },
            1,
        ),
        (
            Elem::Int(16, true),
            ScalarKind::Int {
                bits: 16,
                signed: true,
            },
            2,
        ),
        (
            Elem::Int(16, false),
            ScalarKind::Int {
                bits: 16,
                signed: false,
            },
            2,
        ),
        (
            Elem::Int(32, true),
            ScalarKind::Int {
                bits: 32,
                signed: true,
            },
            4,
        ),
        (
            Elem::Int(32, false),
            ScalarKind::Int {
                bits: 32,
                signed: false,
            },
            4,
        ),
        (
            Elem::Int(64, true),
            ScalarKind::Int {
                bits: 64,
                signed: true,
            },
            8,
        ),
        (
            Elem::Int(64, false),
            ScalarKind::Int {
                bits: 64,
                signed: false,
            },
            8,
        ),
    ];
    for (k, &(elem, kind, bytes)) in cases.iter().enumerate() {
        let buf = Buf {
            elem,
            stride: Some(bytes),
            ..Buf::f32_runtime()
        };
        let got = extract(&buf.insts()).unwrap();
        assert_eq!(
            got,
            vec![operand(0, 0, kind, bytes, Some(bytes), ByteLength::Unsized)],
            "case {k}"
        );
    }
}

// ---------------------------------------------------------------------------
// B. 決まらない量
// ---------------------------------------------------------------------------

#[test]
fn runtime_array_is_unsized_never_zero() {
    for stride in [Some(4), None] {
        let buf = Buf {
            stride,
            ..Buf::f32_runtime()
        };
        let got = extract(&buf.insts()).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].byte_length, ByteLength::Unsized);
        assert_ne!(got[0].byte_length, ByteLength::Fixed(0));
        assert_eq!(got[0].array_stride, stride);
    }
}

#[test]
fn fixed_length_is_count_times_stride() {
    let three = Buf {
        stride: Some(8),
        ..Buf::f32_fixed(3)
    };
    assert_eq!(
        extract(&three.insts()).unwrap()[0].byte_length,
        ByteLength::Fixed(24)
    );
    let one = Buf {
        stride: Some(8),
        ..Buf::f32_fixed(1)
    };
    assert_eq!(
        extract(&one.insts()).unwrap()[0].byte_length,
        ByteLength::Fixed(8)
    );
}

#[test]
fn largest_length_and_stride_do_not_overflow() {
    let buf = Buf {
        stride: Some(u32::MAX),
        ..Buf::f32_fixed(u32::MAX)
    };
    let got = extract(&buf.insts()).unwrap();
    assert_eq!(
        got[0].byte_length,
        ByteLength::Fixed(18_446_744_065_119_617_025)
    );
}

#[test]
fn signed_length_boundaries() {
    let with = |value: u32, signed: bool| {
        let buf = Buf {
            len: Len::Fixed { value, signed },
            ..Buf::f32_runtime()
        };
        extract(&buf.insts())
    };
    let invalid = Err(unsupported(5, UnsupportedShape::InvalidLength));
    assert_eq!(
        with(0x7FFF_FFFF, true).unwrap()[0].byte_length,
        ByteLength::Fixed(0x7FFF_FFFF * 4)
    );
    assert_eq!(with(0x8000_0000, true), invalid);
    assert_eq!(with(u32::MAX, true), invalid);
    assert_eq!(
        with(0x8000_0000, false).unwrap()[0].byte_length,
        ByteLength::Fixed(0x8000_0000 * 4)
    );
    assert_eq!(with(0, true), invalid);
    assert_eq!(with(0, false), invalid);
}

#[test]
fn length_that_is_not_a_plain_constant_is_rejected() {
    let not_constant = Err(unsupported(5, UnsupportedShape::LengthNotConstant));
    let buf = Buf::f32_fixed(4);
    // 長さの id が未定義（`UnknownId` ではなくこの形で返る、現状の振る舞い）。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_TYPE_ARRAY,
            &[2],
            inst(OP_TYPE_ARRAY, &[2, 1, 99])
        )),
        not_constant
    );
    // 長さの id が型を指す。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_TYPE_ARRAY,
            &[2],
            inst(OP_TYPE_ARRAY, &[2, 1, 1])
        )),
        not_constant
    );
    // spec constant。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_CONSTANT,
            &[6],
            inst(OP_SPEC_CONSTANT, &[6, 7, 4])
        )),
        not_constant
    );
    // 値が 2 語。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_CONSTANT,
            &[6],
            inst(OP_CONSTANT, &[6, 7, 4, 0])
        )),
        not_constant
    );
    // 64 ビットの整数型。
    assert_eq!(
        extract_mutated(buf, |v| {
            replace(v, OP_TYPE_INT, &[6], inst(OP_TYPE_INT, &[6, 64, 0]));
            replace(v, OP_CONSTANT, &[6], inst(OP_CONSTANT, &[6, 7, 4, 0]));
        }),
        not_constant
    );
    // 整数型でない結果型（f32）。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_CONSTANT,
            &[6],
            inst(OP_CONSTANT, &[1, 7, 4])
        )),
        not_constant
    );
}

#[test]
fn stride_boundaries() {
    let fixed = |stride: Option<u32>| {
        extract(
            &Buf {
                stride,
                ..Buf::f32_fixed(2)
            }
            .insts(),
        )
    };
    assert_eq!(
        fixed(Some(0)),
        Err(unsupported(5, UnsupportedShape::ZeroStride))
    );
    assert_eq!(
        fixed(Some(3)),
        Err(unsupported(5, UnsupportedShape::StrideSmallerThanElement))
    );
    assert_eq!(fixed(Some(4)).unwrap()[0].byte_length, ByteLength::Fixed(8));
    assert_eq!(
        fixed(Some(5)).unwrap()[0].byte_length,
        ByteLength::Fixed(10)
    );
    assert_eq!(
        fixed(None),
        Err(unsupported(5, UnsupportedShape::MissingArrayStride))
    );
    let runtime = |stride: u32| {
        extract(
            &Buf {
                stride: Some(stride),
                ..Buf::f32_runtime()
            }
            .insts(),
        )
    };
    assert_eq!(
        runtime(0),
        Err(unsupported(5, UnsupportedShape::ZeroStride))
    );
    assert_eq!(
        runtime(3),
        Err(unsupported(5, UnsupportedShape::StrideSmallerThanElement))
    );
}

// ---------------------------------------------------------------------------
// C. 切れた入力・不正な語数
// ---------------------------------------------------------------------------

#[test]
fn inputs_shorter_than_header() {
    for n in 0..HEADER_WORDS {
        // 先頭の語がマジックでなくても、長さの検査が先に効く。
        assert_eq!(
            extract_storage_buffer_operands(&vec![0xdead_beef; n]),
            Err(SpirvExtractError::TooShort { words: n })
        );
        let mut with_magic = vec![0; n];
        if let Some(first) = with_magic.first_mut() {
            *first = MAGIC;
        }
        assert_eq!(
            extract_storage_buffer_operands(&with_magic),
            Err(SpirvExtractError::TooShort { words: n })
        );
    }
    // バイト順が逆でも、5 語に満たなければ長さの検査が先。
    assert_eq!(
        extract_storage_buffer_operands(&[MAGIC_REVERSED, 0, 0, 0]),
        Err(SpirvExtractError::TooShort { words: 4 })
    );
}

#[test]
fn magic_is_checked_after_length() {
    assert_eq!(
        extract_storage_buffer_operands(&[0xdead_beef, 0, 0, 0, 0]),
        Err(SpirvExtractError::BadMagic)
    );
    assert_eq!(
        extract_storage_buffer_operands(&[0, 0, 0, 0, 0]),
        Err(SpirvExtractError::BadMagic)
    );
    assert_eq!(
        extract_storage_buffer_operands(&[MAGIC_REVERSED, 0, 0, 0, 0]),
        Err(SpirvExtractError::ByteOrderReversed)
    );
}

#[test]
fn every_prefix_is_classified_without_panic() {
    let insts: Vec<Inst> = [
        Buf::f32_runtime().at(0, 0, 10),
        Buf {
            stride: Some(8),
            ..Buf::f32_fixed(3)
        }
        .at(0, 1, 20),
        Buf {
            elem: Elem::Int(16, true),
            stride: Some(2),
            ..Buf::f32_runtime()
        }
        .at(1, 0, 30),
    ]
    .iter()
    .flat_map(Buf::insts)
    .collect();
    let full = module(&insts);
    let total = extract_storage_buffer_operands(&full).unwrap().len();
    assert_eq!(total, 3);

    // 命令の先頭の語番号（テスト側の命令の列から計算する）。
    let mut starts = Vec::new();
    let mut pos = HEADER_WORDS;
    for i in &insts {
        starts.push((pos, i.len()));
        pos += i.len();
    }
    assert_eq!(pos, full.len());

    for len in 0..=full.len() {
        let got = extract_storage_buffer_operands(&full[..len]);
        if len < HEADER_WORDS {
            assert_eq!(got, Err(SpirvExtractError::TooShort { words: len }));
            continue;
        }
        let containing = starts
            .iter()
            .find(|&&(start, count)| start < len && len < start + count);
        match containing {
            None => {
                let ok = got.unwrap_or_else(|e| panic!("boundary prefix {len} failed: {e}"));
                assert!(ok.len() <= total, "prefix {len}");
            }
            Some(&(start, count)) => assert_eq!(
                got,
                Err(SpirvExtractError::TruncatedInstruction {
                    offset: start,
                    word_count: count,
                    remaining: len - start,
                }),
                "prefix {len}"
            ),
        }
    }
}

#[test]
fn zero_word_count_reports_its_offset() {
    let mut w = module(&Buf::f32_runtime().insts());
    let offset = w.len();
    w.push(0);
    assert_eq!(
        extract_storage_buffer_operands(&w),
        Err(SpirvExtractError::ZeroWordCount { offset })
    );
    // opcode が非 0 でも語数が 0 なら同じ。
    let mut w = module(&[]);
    w.push(u32::from(OP_CONSTANT));
    assert_eq!(
        extract_storage_buffer_operands(&w),
        Err(SpirvExtractError::ZeroWordCount { offset: 5 })
    );
}

#[test]
fn word_count_beyond_the_module() {
    // 語数 0xFFFF で、残りは 2 語。
    let w = module(&[vec![(0xFFFF << 16) | u32::from(OP_TYPE_INT), 1]]);
    assert_eq!(
        extract_storage_buffer_operands(&w),
        Err(SpirvExtractError::TruncatedInstruction {
            offset: 5,
            word_count: 65535,
            remaining: 2
        })
    );
    // 語数が残りより 1 大きい。
    let w = module(&[vec![(5 << 16) | u32::from(OP_TYPE_INT), 1, 32, 0]]);
    assert_eq!(
        extract_storage_buffer_operands(&w),
        Err(SpirvExtractError::TruncatedInstruction {
            offset: 5,
            word_count: 5,
            remaining: 4
        })
    );
    // 語数が残りちょうど。
    let w = module(&[vec![(4 << 16) | u32::from(OP_TYPE_INT), 1, 32, 0]]);
    assert_eq!(extract_storage_buffer_operands(&w), Ok(vec![]));
}

#[test]
fn too_few_operands_for_each_instruction_the_tool_reads() {
    // （opcode, 要るオペランド数, 与える語の並び）。足りない数だけ後ろを落とす。
    let cases: [(u16, &[u32]); 14] = [
        (OP_TYPE_BOOL, &[1]),
        (OP_TYPE_INT, &[1, 32, 0]),
        (OP_TYPE_FLOAT, &[1, 32]),
        (OP_TYPE_VECTOR, &[1, 2, 4]),
        (OP_TYPE_MATRIX, &[1, 2, 4]),
        (OP_TYPE_ARRAY, &[1, 2, 3]),
        (OP_TYPE_RUNTIME_ARRAY, &[1, 2]),
        (OP_TYPE_STRUCT, &[1]),
        (OP_TYPE_POINTER, &[1, SC_STORAGE_BUFFER, 2]),
        (OP_CONSTANT, &[1, 2, 3]),
        (OP_VARIABLE, &[1, 2, SC_STORAGE_BUFFER]),
        (OP_DECORATE, &[1, DECO_BINDING, 0]),
        (OP_MEMBER_DECORATE, &[1, 0, DECO_OFFSET, 0]),
        (OP_DECORATION_GROUP, &[1]),
    ];
    let mut checked = 0;
    for (opcode, full) in cases {
        for n in 0..full.len() {
            assert_eq!(
                extract(&[inst(opcode, &full[..n])]),
                Err(SpirvExtractError::MissingOperands { offset: 5, opcode }),
                "opcode {opcode} with {n} operands"
            );
            checked += 1;
        }
    }
    // 装飾グループの展開命令も、対象の id が無ければ同じ形で返る。
    for opcode in [OP_GROUP_DECORATE, OP_GROUP_MEMBER_DECORATE] {
        assert_eq!(
            extract(&[inst(opcode, &[])]),
            Err(SpirvExtractError::MissingOperands { offset: 5, opcode })
        );
        checked += 1;
    }
    assert!(checked > 0);

    // 値の要らない装飾と、知らない装飾は 2 オペランドで受理される。
    for deco in [DECO_BLOCK, DECO_BUFFER_BLOCK, DECO_NON_WRITABLE, 9999] {
        assert_eq!(extract(&[inst(OP_DECORATE, &[1, deco])]), Ok(vec![]));
    }
    // 知らないメンバーの装飾は 3 オペランドで受理される。
    assert_eq!(
        extract(&[inst(OP_MEMBER_DECORATE, &[1, 0, DECO_NON_WRITABLE])]),
        Ok(vec![])
    );
}

#[test]
fn single_word_corruption_never_panics() {
    let insts: Vec<Inst> = [
        Buf::f32_runtime().at(0, 0, 10),
        Buf {
            stride: Some(8),
            ..Buf::f32_fixed(3)
        }
        .at(0, 1, 20),
        Buf {
            elem: Elem::Int(64, false),
            stride: Some(8),
            ..Buf::f32_runtime()
        }
        .at(1, 0, 30),
    ]
    .iter()
    .flat_map(Buf::insts)
    .collect();
    let good = module(&insts);
    let fixed_values = [0, 1, 2, 12, 0x7FFF_FFFF, 0x8000_0000, 0x0001_0000, u32::MAX];
    let mut runs = 0usize;
    for pos in 0..good.len() {
        let orig = good[pos];
        let mut values = fixed_values.to_vec();
        values.push(orig | 0xFFFF_0000);
        values.push(orig.wrapping_add(0x0001_0000));
        values.push(orig & 0xFFFF);
        for v in values {
            let mut w = good.clone();
            w[pos] = v;
            runs += 1;
            match extract_storage_buffer_operands(&w) {
                Ok(ops) => {
                    for pair in ops.windows(2) {
                        assert!(
                            (pair[0].set, pair[0].binding) < (pair[1].set, pair[1].binding),
                            "pos {pos} value {v:#x}"
                        );
                    }
                }
                Err(e) => {
                    let text = e.to_string();
                    assert!(!text.is_empty());
                    assert!(text.is_ascii() && !text.contains('\n'), "{text}");
                }
            }
        }
    }
    assert!(runs > good.len());
}

// ---------------------------------------------------------------------------
// D. 不正な参照・重複
// ---------------------------------------------------------------------------

#[test]
fn undefined_ids_are_reported() {
    let buf = Buf::f32_fixed(4);
    // 変数の pointer 型。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_VARIABLE,
            &[4],
            inst(OP_VARIABLE, &[99, 5, SC_STORAGE_BUFFER])
        )),
        Err(SpirvExtractError::UnknownId { id: 99 })
    );
    // pointer の指す型。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_TYPE_POINTER,
            &[4],
            inst(OP_TYPE_POINTER, &[4, SC_STORAGE_BUFFER, 98])
        )),
        Err(SpirvExtractError::UnknownId { id: 98 })
    );
    // メンバーの型。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_TYPE_STRUCT,
            &[3],
            inst(OP_TYPE_STRUCT, &[3, 97])
        )),
        Err(SpirvExtractError::UnknownId { id: 97 })
    );
    // 配列の要素の型。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_TYPE_ARRAY,
            &[2],
            inst(OP_TYPE_ARRAY, &[2, 96, 7])
        )),
        Err(SpirvExtractError::UnknownId { id: 96 })
    );
    // 定数の結果型。
    assert_eq!(
        extract_mutated(buf, |v| replace(
            v,
            OP_CONSTANT,
            &[6],
            inst(OP_CONSTANT, &[95, 7, 4])
        )),
        Err(SpirvExtractError::UnknownId { id: 95 })
    );
}

#[test]
fn variable_type_that_is_not_a_pointer() {
    assert_eq!(
        extract_mutated(Buf::f32_runtime(), |v| replace(
            v,
            OP_VARIABLE,
            &[4],
            inst(OP_VARIABLE, &[1, 5, SC_STORAGE_BUFFER])
        )),
        Err(SpirvExtractError::WrongKind { id: 1 })
    );
}

#[test]
fn missing_set_or_binding() {
    let missing = Err(SpirvExtractError::MissingDecoration { id: 5 });
    assert_eq!(
        extract_mutated(Buf::f32_runtime(), |v| remove(
            v,
            OP_DECORATE,
            &[5, DECO_DESCRIPTOR_SET]
        )),
        missing
    );
    assert_eq!(
        extract_mutated(Buf::f32_runtime(), |v| remove(
            v,
            OP_DECORATE,
            &[5, DECO_BINDING]
        )),
        missing
    );
    assert_eq!(
        extract_mutated(Buf::f32_runtime(), |v| {
            remove(v, OP_DECORATE, &[5, DECO_DESCRIPTOR_SET]);
            remove(v, OP_DECORATE, &[5, DECO_BINDING]);
        }),
        missing
    );
}

#[test]
fn duplicate_decorations_are_rejected() {
    let buf = Buf::f32_runtime();
    let dup = |extra: Inst| extract_mutated(buf, |v| v.push(extra));
    assert_eq!(
        dup(inst(OP_DECORATE, &[5, DECO_BINDING, 1])),
        Err(SpirvExtractError::DuplicateDefinition { id: 5 })
    );
    assert_eq!(
        dup(inst(OP_DECORATE, &[5, DECO_DESCRIPTOR_SET, 1])),
        Err(SpirvExtractError::DuplicateDefinition { id: 5 })
    );
    assert_eq!(
        dup(inst(OP_DECORATE, &[2, DECO_ARRAY_STRIDE, 8])),
        Err(SpirvExtractError::DuplicateDefinition { id: 2 })
    );
    assert_eq!(
        dup(inst(OP_MEMBER_DECORATE, &[3, 0, DECO_OFFSET, 0])),
        Err(SpirvExtractError::DuplicateDefinition { id: 3 })
    );
    // 型・定数・変数の id の重複も同じ形で拒否される。
    assert_eq!(
        dup(inst(OP_TYPE_FLOAT, &[1, 32])),
        Err(SpirvExtractError::DuplicateDefinition { id: 1 })
    );
    assert_eq!(
        dup(inst(OP_VARIABLE, &[4, 1, SC_STORAGE_BUFFER])),
        Err(SpirvExtractError::DuplicateDefinition { id: 1 })
    );
}

#[test]
fn duplicate_binding_reports_set_and_binding() {
    let insts: Vec<Inst> = [
        Buf::f32_runtime().at(2, 7, 10),
        Buf::f32_runtime().at(2, 7, 20),
    ]
    .iter()
    .flat_map(Buf::insts)
    .collect();
    assert_eq!(
        extract(&insts),
        Err(SpirvExtractError::DuplicateBinding { set: 2, binding: 7 })
    );
}

#[test]
fn storage_class_mismatch_in_both_directions() {
    let mismatch = Err(SpirvExtractError::StorageClassMismatch { id: 5 });
    // pointer が Uniform、変数が StorageBuffer。
    assert_eq!(
        extract_mutated(Buf::f32_runtime(), |v| replace(
            v,
            OP_TYPE_POINTER,
            &[4],
            inst(OP_TYPE_POINTER, &[4, SC_UNIFORM, 3])
        )),
        mismatch
    );
    // pointer が StorageBuffer、変数が Uniform。
    assert_eq!(
        extract_mutated(Buf::f32_runtime(), |v| replace(
            v,
            OP_VARIABLE,
            &[4],
            inst(OP_VARIABLE, &[4, 5, SC_UNIFORM])
        )),
        mismatch
    );
}

// ---------------------------------------------------------------------------
// E. 対象外の形（`UnsupportedShape` は non_exhaustive なので、表は手で保つ）
// ---------------------------------------------------------------------------

/// 変数の id が 5 の単一 buffer を差し替え、`Unsupported { id: 5, shape }` になることを確かめる。
fn assert_unsupported(
    label: &str,
    buf: Buf,
    shape: UnsupportedShape,
    mutate: impl FnOnce(&mut Vec<Inst>),
) {
    assert_eq!(
        extract_mutated(buf, mutate),
        Err(unsupported(5, shape)),
        "{label}"
    );
}

#[test]
fn unsupported_element_shapes_are_reported() {
    let r = Buf::f32_runtime();
    assert_unsupported("bool", r, UnsupportedShape::Bool, |v| {
        replace(v, OP_TYPE_FLOAT, &[1], inst(OP_TYPE_BOOL, &[1]));
    });
    assert_unsupported("vector", r, UnsupportedShape::VectorOrMatrix, |v| {
        replace(v, OP_TYPE_FLOAT, &[1], inst(OP_TYPE_VECTOR, &[1, 8, 4]));
    });
    assert_unsupported("matrix", r, UnsupportedShape::VectorOrMatrix, |v| {
        replace(v, OP_TYPE_FLOAT, &[1], inst(OP_TYPE_MATRIX, &[1, 8, 4]));
    });
    assert_unsupported("struct element", r, UnsupportedShape::StructElement, |v| {
        replace(v, OP_TYPE_FLOAT, &[1], inst(OP_TYPE_STRUCT, &[1, 8]));
    });
    for (label, elem) in [
        ("float 0", Elem::Float(0)),
        ("float 12", Elem::Float(12)),
        ("int 0", Elem::Int(0, false)),
        ("int 1", Elem::Int(1, true)),
        ("int 12", Elem::Int(12, false)),
    ] {
        assert_unsupported(label, Buf { elem, ..r }, UnsupportedShape::BitWidth, |_| {});
    }
}

#[test]
fn unsupported_block_shapes_are_reported() {
    let r = Buf::f32_runtime();
    // block の配列（固定長・runtime）。
    assert_unsupported(
        "block runtime array",
        r,
        UnsupportedShape::BlockArray,
        |v| {
            v.push(inst(OP_TYPE_RUNTIME_ARRAY, &[8, 3]));
            replace(
                v,
                OP_TYPE_POINTER,
                &[4],
                inst(OP_TYPE_POINTER, &[4, SC_STORAGE_BUFFER, 8]),
            );
        },
    );
    assert_unsupported(
        "block fixed array",
        Buf::f32_fixed(2),
        UnsupportedShape::BlockArray,
        |v| {
            v.push(inst(OP_TYPE_ARRAY, &[8, 3, 7]));
            replace(
                v,
                OP_TYPE_POINTER,
                &[4],
                inst(OP_TYPE_POINTER, &[4, SC_STORAGE_BUFFER, 8]),
            );
        },
    );
    // block の形。
    assert_unsupported(
        "no Block decoration",
        r,
        UnsupportedShape::BlockShape,
        |v| {
            remove(v, OP_DECORATE, &[3, DECO_BLOCK]);
        },
    );
    assert_unsupported("no member", r, UnsupportedShape::BlockShape, |v| {
        replace(v, OP_TYPE_STRUCT, &[3], inst(OP_TYPE_STRUCT, &[3]));
    });
    assert_unsupported("two members", r, UnsupportedShape::BlockShape, |v| {
        replace(v, OP_TYPE_STRUCT, &[3], inst(OP_TYPE_STRUCT, &[3, 2, 2]));
    });
    assert_unsupported(
        "member is not an array",
        r,
        UnsupportedShape::BlockShape,
        |v| {
            replace(v, OP_TYPE_STRUCT, &[3], inst(OP_TYPE_STRUCT, &[3, 1]));
        },
    );
    // メンバーの Offset。
    assert_unsupported(
        "non-zero offset",
        r,
        UnsupportedShape::NonZeroMemberOffset,
        |v| {
            replace(
                v,
                OP_MEMBER_DECORATE,
                &[3],
                inst(OP_MEMBER_DECORATE, &[3, 0, DECO_OFFSET, 16]),
            );
        },
    );
    assert_unsupported("no offset", r, UnsupportedShape::MissingMemberOffset, |v| {
        remove(v, OP_MEMBER_DECORATE, &[3]);
    });
}

#[test]
fn unsupported_other_types_are_reported() {
    let r = Buf::f32_runtime();
    // 指す型が scalar。
    assert_unsupported("pointee is scalar", r, UnsupportedShape::OtherType, |v| {
        replace(
            v,
            OP_TYPE_POINTER,
            &[4],
            inst(OP_TYPE_POINTER, &[4, SC_STORAGE_BUFFER, 1]),
        );
    });
    // 指す型が配列の配列、struct でない要素の配列。
    assert_unsupported("array of array", r, UnsupportedShape::OtherType, |v| {
        v.push(inst(OP_TYPE_RUNTIME_ARRAY, &[8, 2]));
        replace(
            v,
            OP_TYPE_POINTER,
            &[4],
            inst(OP_TYPE_POINTER, &[4, SC_STORAGE_BUFFER, 8]),
        );
    });
    assert_unsupported("array of scalar", r, UnsupportedShape::OtherType, |v| {
        v.push(inst(OP_TYPE_RUNTIME_ARRAY, &[8, 1]));
        replace(
            v,
            OP_TYPE_POINTER,
            &[4],
            inst(OP_TYPE_POINTER, &[4, SC_STORAGE_BUFFER, 8]),
        );
    });
    // メンバーの配列の要素が配列。
    assert_unsupported(
        "member array of array",
        r,
        UnsupportedShape::OtherType,
        |v| {
            v.push(inst(OP_TYPE_RUNTIME_ARRAY, &[9, 1]));
            replace(
                v,
                OP_TYPE_RUNTIME_ARRAY,
                &[2],
                inst(OP_TYPE_RUNTIME_ARRAY, &[2, 9]),
            );
        },
    );
}

#[test]
fn unsupported_legacy_buffer_block_is_reported() {
    let legacy = |v: &mut Vec<Inst>| {
        remove(v, OP_DECORATE, &[3, DECO_BLOCK]);
        v.push(inst(OP_DECORATE, &[3, DECO_BUFFER_BLOCK]));
        replace(
            v,
            OP_TYPE_POINTER,
            &[4],
            inst(OP_TYPE_POINTER, &[4, SC_UNIFORM, 3]),
        );
        replace(v, OP_VARIABLE, &[4], inst(OP_VARIABLE, &[4, 5, SC_UNIFORM]));
    };
    assert_unsupported(
        "uniform + BufferBlock",
        Buf::f32_runtime(),
        UnsupportedShape::LegacyBufferBlock,
        legacy,
    );
    assert_unsupported(
        "uniform + BufferBlock array",
        Buf::f32_runtime(),
        UnsupportedShape::LegacyBufferBlock,
        |v| {
            legacy(v);
            v.push(inst(OP_TYPE_RUNTIME_ARRAY, &[8, 3]));
            replace(
                v,
                OP_TYPE_POINTER,
                &[4],
                inst(OP_TYPE_POINTER, &[4, SC_UNIFORM, 8]),
            );
        },
    );
}

#[test]
fn unsupported_length_and_stride_shapes_are_reported() {
    assert_unsupported(
        "zero stride",
        Buf {
            stride: Some(0),
            ..Buf::f32_runtime()
        },
        UnsupportedShape::ZeroStride,
        |_| {},
    );
    assert_unsupported(
        "missing stride",
        Buf {
            stride: None,
            ..Buf::f32_fixed(2)
        },
        UnsupportedShape::MissingArrayStride,
        |_| {},
    );
    assert_unsupported(
        "stride smaller than element",
        Buf {
            stride: Some(2),
            ..Buf::f32_fixed(2)
        },
        UnsupportedShape::StrideSmallerThanElement,
        |_| {},
    );
    assert_unsupported(
        "zero length",
        Buf::f32_fixed(0),
        UnsupportedShape::InvalidLength,
        |_| {},
    );
    assert_unsupported(
        "length is a spec constant",
        Buf::f32_fixed(2),
        UnsupportedShape::LengthNotConstant,
        |v| replace(v, OP_CONSTANT, &[6], inst(OP_SPEC_CONSTANT, &[6, 7, 2])),
    );
}

#[test]
fn decoration_groups_are_rejected_with_the_group_id() {
    let group = 20;
    let cases = [
        inst(OP_DECORATION_GROUP, &[group]),
        inst(OP_GROUP_DECORATE, &[group, 5]),
        inst(OP_GROUP_MEMBER_DECORATE, &[group, 3, 0]),
    ];
    for c in cases {
        let mut insts = Buf::f32_runtime().insts();
        insts.insert(0, c);
        assert_eq!(
            extract(&insts),
            Err(unsupported(group, UnsupportedShape::DecorationGroup))
        );
    }
}

// ---------------------------------------------------------------------------
// F. エラーにせず対象から外す変数
// ---------------------------------------------------------------------------

#[test]
fn non_storage_buffer_variables_are_skipped() {
    // push constant は、現状は対象に含めない。
    let others = [
        SC_UNIFORM_CONSTANT,
        SC_INPUT,
        SC_OUTPUT,
        SC_WORKGROUP,
        SC_PRIVATE,
        SC_FUNCTION,
        SC_PUSH_CONSTANT,
    ];
    let mut insts = Vec::new();
    for (k, class) in others.into_iter().enumerate() {
        let ptr = 200 + 2 * k as u32;
        insts.push(inst(OP_TYPE_POINTER, &[ptr, class, 1]));
        insts.push(inst(OP_VARIABLE, &[ptr, ptr + 1, class]));
    }
    // Uniform + Block（旧形式でない uniform buffer）。
    insts.extend([
        inst(OP_DECORATE, &[300, DECO_BLOCK]),
        inst(OP_TYPE_FLOAT, &[301, 32]),
        inst(OP_TYPE_STRUCT, &[300, 301]),
        inst(OP_TYPE_POINTER, &[302, SC_UNIFORM, 300]),
        inst(OP_VARIABLE, &[302, 303, SC_UNIFORM]),
    ]);
    assert_eq!(extract(&insts), Ok(vec![]));

    // storage buffer と混ぜたら、storage buffer だけが返る。
    let storage = Buf::f32_runtime().at(0, 3, 10);
    let mut mixed = insts.clone();
    mixed.extend(storage.insts());
    let got = extract(&mixed).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].set, got[0].binding), (0, 3));
}

// ---------------------------------------------------------------------------
// G. エラーの型
// ---------------------------------------------------------------------------

#[test]
fn errors_are_std_errors_with_ascii_display() {
    let shapes = [
        UnsupportedShape::Bool,
        UnsupportedShape::VectorOrMatrix,
        UnsupportedShape::StructElement,
        UnsupportedShape::BlockArray,
        UnsupportedShape::LegacyBufferBlock,
        UnsupportedShape::BlockShape,
        UnsupportedShape::BitWidth,
        UnsupportedShape::LengthNotConstant,
        UnsupportedShape::ZeroStride,
        UnsupportedShape::MissingArrayStride,
        UnsupportedShape::StrideSmallerThanElement,
        UnsupportedShape::NonZeroMemberOffset,
        UnsupportedShape::MissingMemberOffset,
        UnsupportedShape::InvalidLength,
        UnsupportedShape::DecorationGroup,
        UnsupportedShape::OtherType,
    ];
    let mut errors = vec![
        SpirvExtractError::TooShort { words: 3 },
        SpirvExtractError::BadMagic,
        SpirvExtractError::ByteOrderReversed,
        SpirvExtractError::ZeroWordCount { offset: 5 },
        SpirvExtractError::TruncatedInstruction {
            offset: 5,
            word_count: 9,
            remaining: 2,
        },
        SpirvExtractError::MissingOperands {
            offset: 5,
            opcode: 21,
        },
        SpirvExtractError::UnknownId { id: 4 },
        SpirvExtractError::StorageClassMismatch { id: 4 },
        SpirvExtractError::WrongKind { id: 4 },
        SpirvExtractError::MissingDecoration { id: 4 },
        SpirvExtractError::DuplicateDefinition { id: 4 },
        SpirvExtractError::DuplicateBinding { set: 1, binding: 2 },
    ];
    errors.extend(
        shapes
            .into_iter()
            .map(|shape| SpirvExtractError::Unsupported { id: 4, shape }),
    );
    for e in errors {
        let boxed: Box<dyn std::error::Error> = Box::new(e.clone());
        let text = boxed.to_string();
        assert!(!text.is_empty());
        assert!(text.is_ascii(), "{text}");
        assert!(!text.contains('\n'), "{text}");
        assert_eq!(e.clone(), e);
    }
}
