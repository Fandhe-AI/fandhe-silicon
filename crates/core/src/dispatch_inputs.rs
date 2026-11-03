//! dispatch の引数の検査に当てる名前つきの入力の組と、入力から契約の型の値を作る関数。
//!
//! feature `test-support` の裏にあり、試験だけが使う。背後実装・ファイル・時刻には触れない。
//! バッファの確保は呼び手が渡したメモリ操作の口で行う。
//! 入力の一覧 [`DISPATCH_INPUTS`] の各行が、どの検査（`validate_declared_operands` と
//! `validate_dispatch_operands` の C1・C3〜C8）に当たるかを [`ExpectedCheck`] で持つ。
//! ポインタ: `docs/spec/05-tasks.md` の TASK-2944。

use crate::error::ContractError;
use crate::execution::{
    Binding, BindingLayout, ConstantBinding, DType, OperandBinding, OperandConstant, OperandSpec,
};
use crate::memory::{BufferHandle, BufferUsage, MemoryOps};

/// 入力が当たる検査。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpectedCheck {
    /// どの検査にも当たらず `Ok(())` を返す。
    Pass,
    /// C1 に当たる。
    C1,
    /// C3 に当たる。
    C3,
    /// C4 に当たる。
    C4,
    /// C5 に当たる。
    C5,
    /// C6 に当たる。
    C6,
    /// C7 に当たる。
    C7,
    /// C8 に当たる。
    C8,
    /// dispatch_dims に 0 を含む入力。C1・C3〜C8 の検査には当たらない。
    ZeroDispatchDims,
    /// dispatch_dims が上限を超える入力。C1・C3〜C8 の検査には当たらない。
    DispatchDimsOverLimit,
}

impl ExpectedCheck {
    /// 拒否の文言の接頭辞。C1・C3〜C8 以外は `None`。
    #[must_use]
    pub const fn prefix(self) -> Option<&'static str> {
        match self {
            Self::C1 => Some("C1:"),
            Self::C3 => Some("C3:"),
            Self::C4 => Some("C4:"),
            Self::C5 => Some("C5:"),
            Self::C6 => Some("C6:"),
            Self::C7 => Some("C7:"),
            Self::C8 => Some("C8:"),
            Self::Pass | Self::ZeroDispatchDims | Self::DispatchDimsOverLimit => None,
        }
    }
}

/// 確保するバッファ 1 件と、その束縛の slot・offset。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferSpec {
    /// 束縛の番号。
    pub slot: u32,
    /// 確保するバイト数。
    pub byte_len: u64,
    /// 束縛の offset（バイト）。
    pub offset: u64,
}

/// 定数 1 件の slot とバイト列。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstantSpec {
    /// 束縛の番号。
    pub slot: u32,
    /// 定数のバイト列。
    pub bytes: &'static [u8],
}

/// dispatch_dims の指定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimsSpec {
    /// 固定の値。
    Fixed([u32; 3]),
    /// 呼び手が渡した上限を超える値（[`dispatch_dims_over_limit`] で作る）。
    OverLimit,
}

/// 上限 `max` を超える dispatch_dims を返す。
///
/// 軸 0 が `max[0] + 1`、軸 1・2 が 1。`max[0]` が `u32::MAX` なら `None`。
#[must_use]
pub fn dispatch_dims_over_limit(max: [u32; 3]) -> Option<[u32; 3]> {
    max[0].checked_add(1).map(|x| [x, 1, 1])
}

/// 名前つきの入力 1 件。
#[derive(Debug, Clone, Copy)]
pub struct DispatchInput {
    /// 入力の名前。
    pub name: &'static str,
    /// 当たる検査。
    pub expected: ExpectedCheck,
    /// 引数の宣言。
    pub declared: &'static [OperandSpec<'static>],
    /// 配置の slot の並び。
    pub layout_slots: &'static [u32],
    /// 確保するバッファと束縛。
    pub buffers: &'static [BufferSpec],
    /// 定数。
    pub constants: &'static [ConstantSpec],
    /// dispatch_dims の指定。
    pub dims: DimsSpec,
}

impl DispatchInput {
    /// 配置を作る。
    #[must_use]
    pub fn layout(&self) -> BindingLayout {
        BindingLayout::new(self.layout_slots.to_vec())
    }

    /// `buffers` の並びでバッファを確保する。途中で失敗したら確保済みを解放してから `Err` を返す。
    pub fn allocate_buffers(
        &self,
        memory: &dyn MemoryOps,
    ) -> Result<Vec<BufferHandle>, ContractError> {
        let mut handles = Vec::with_capacity(self.buffers.len());
        for spec in self.buffers {
            match memory.allocate(spec.byte_len, BufferUsage::STORAGE) {
                Ok(h) => handles.push(h),
                Err(e) => {
                    for h in handles {
                        memory.free(h);
                    }
                    return Err(e);
                }
            }
        }
        Ok(handles)
    }

    /// 確保したバッファから束縛の組を作る。長さが違えば短いほうまで。
    #[must_use]
    pub fn bindings<'b>(&self, handles: &'b [BufferHandle]) -> Vec<Binding<'b>> {
        self.buffers
            .iter()
            .zip(handles)
            .map(|(spec, h)| Binding::new(spec.slot, h, spec.offset))
            .collect()
    }

    /// 定数の束縛の組を作る。
    #[must_use]
    pub fn constants(&self) -> Vec<ConstantBinding<'static>> {
        self.constants
            .iter()
            .map(|c| ConstantBinding::new(c.slot, c.bytes))
            .collect()
    }

    /// 検査に渡す束縛（`buffers` と同じ並び）を作る。
    #[must_use]
    pub fn operand_bindings(&self) -> Vec<OperandBinding> {
        self.buffers
            .iter()
            .map(|b| OperandBinding {
                slot: b.slot,
                offset: b.offset,
            })
            .collect()
    }

    /// 検査に渡す定数を作る。
    #[must_use]
    pub fn operand_constants(&self) -> Vec<OperandConstant<'static>> {
        self.constants
            .iter()
            .map(|c| OperandConstant {
                slot: c.slot,
                bytes: c.bytes,
            })
            .collect()
    }

    /// dispatch_dims を返す。`OverLimit` は `max` から [`dispatch_dims_over_limit`] で作る。
    #[must_use]
    pub fn dispatch_dims(&self, max: [u32; 3]) -> Option<[u32; 3]> {
        match self.dims {
            DimsSpec::Fixed(d) => Some(d),
            DimsSpec::OverLimit => dispatch_dims_over_limit(max),
        }
    }
}

/// 名前つきの入力の一覧。
pub const DISPATCH_INPUTS: [DispatchInput; 13] = [
    DispatchInput {
        name: "pass_base",
        expected: ExpectedCheck::Pass,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[4],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 16,
            offset: 0,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "pass_aligned_offset",
        expected: ExpectedCheck::Pass,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[4],
            stride: &[1],
            alignment: 16,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 1024,
            offset: 16,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "pass_constant_len",
        expected: ExpectedCheck::Pass,
        declared: &[OperandSpec {
            slot: 3,
            dtype: DType::U32,
            shape: &[3],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[3],
        buffers: &[],
        constants: &[ConstantSpec {
            slot: 3,
            bytes: &[0u8; 12],
        }],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c1_layout_count",
        expected: ExpectedCheck::C1,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[4],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[0, 1],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 16,
            offset: 0,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c3_stride_dims",
        expected: ExpectedCheck::C3,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[4],
            stride: &[1, 1],
            alignment: 4,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 16,
            offset: 0,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c4_capacity",
        expected: ExpectedCheck::C4,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[100],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 16,
            offset: 0,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c4_offset_past_end",
        expected: ExpectedCheck::C4,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[100],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 4,
            offset: 1000,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c5_alignment",
        expected: ExpectedCheck::C5,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[4],
            stride: &[1],
            alignment: 16,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 1024,
            offset: 4,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c6_constant_len",
        expected: ExpectedCheck::C6,
        declared: &[OperandSpec {
            slot: 3,
            dtype: DType::U32,
            shape: &[3],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[3],
        buffers: &[],
        constants: &[ConstantSpec {
            slot: 3,
            bytes: &[0u8; 8],
        }],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c7_undeclared_slot",
        expected: ExpectedCheck::C7,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[4],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 5,
            byte_len: 16,
            offset: 0,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "c8_zero_length",
        expected: ExpectedCheck::C8,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[0],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 4,
            offset: 0,
        }],
        constants: &[],
        dims: DimsSpec::Fixed([1, 1, 1]),
    },
    DispatchInput {
        name: "zero_dispatch_dims",
        expected: ExpectedCheck::ZeroDispatchDims,
        declared: &[
            OperandSpec {
                slot: 0,
                dtype: DType::F32,
                shape: &[1],
                stride: &[1],
                alignment: 4,
            },
            OperandSpec {
                slot: 1,
                dtype: DType::F32,
                shape: &[1],
                stride: &[1],
                alignment: 4,
            },
            OperandSpec {
                slot: 2,
                dtype: DType::F32,
                shape: &[1],
                stride: &[1],
                alignment: 4,
            },
            OperandSpec {
                slot: 3,
                dtype: DType::U32,
                shape: &[1],
                stride: &[1],
                alignment: 4,
            },
        ],
        layout_slots: &[0, 1, 2, 3],
        buffers: &[
            BufferSpec {
                slot: 0,
                byte_len: 4,
                offset: 0,
            },
            BufferSpec {
                slot: 1,
                byte_len: 4,
                offset: 0,
            },
            BufferSpec {
                slot: 2,
                byte_len: 4,
                offset: 0,
            },
        ],
        constants: &[ConstantSpec {
            slot: 3,
            bytes: &[0u8; 4],
        }],
        dims: DimsSpec::Fixed([0, 1, 1]),
    },
    DispatchInput {
        name: "dispatch_dims_over_limit",
        expected: ExpectedCheck::DispatchDimsOverLimit,
        declared: &[OperandSpec {
            slot: 0,
            dtype: DType::F32,
            shape: &[4],
            stride: &[1],
            alignment: 4,
        }],
        layout_slots: &[0],
        buffers: &[BufferSpec {
            slot: 0,
            byte_len: 16,
            offset: 0,
        }],
        constants: &[],
        dims: DimsSpec::OverLimit,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_inputs_names_are_unique() {
        for (i, a) in DISPATCH_INPUTS.iter().enumerate() {
            for b in &DISPATCH_INPUTS[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
    }

    const ALL: [ExpectedCheck; 10] = [
        ExpectedCheck::Pass,
        ExpectedCheck::C1,
        ExpectedCheck::C3,
        ExpectedCheck::C4,
        ExpectedCheck::C5,
        ExpectedCheck::C6,
        ExpectedCheck::C7,
        ExpectedCheck::C8,
        ExpectedCheck::ZeroDispatchDims,
        ExpectedCheck::DispatchDimsOverLimit,
    ];

    #[test]
    fn dispatch_inputs_cover_every_expected_check() {
        for e in ALL {
            assert!(DISPATCH_INPUTS.iter().any(|i| i.expected == e), "{e:?}");
        }
    }

    #[test]
    fn dispatch_inputs_prefix_values() {
        let want = [
            None,
            Some("C1:"),
            Some("C3:"),
            Some("C4:"),
            Some("C5:"),
            Some("C6:"),
            Some("C7:"),
            Some("C8:"),
            None,
            None,
        ];
        for (e, w) in ALL.into_iter().zip(want) {
            assert_eq!(e.prefix(), w, "{e:?}");
        }
    }

    #[test]
    fn dispatch_inputs_over_limit_values() {
        assert_eq!(dispatch_dims_over_limit([0, 0, 0]), Some([1, 1, 1]));
        assert_eq!(dispatch_dims_over_limit([u32::MAX, 1, 1]), None);
        assert_eq!(dispatch_dims_over_limit([7, 8, 9]), Some([8, 1, 1]));
    }
}
