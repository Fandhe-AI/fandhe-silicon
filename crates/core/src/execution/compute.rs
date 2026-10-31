//! compute の dispatch に渡す公開の型（パイプラインの印・バッファの束縛・定数の束縛・共有メモリの指定）。
//!
//! `CommandQueue::dispatch` / `dispatch_ex`（`submit` モジュール）の引数として、
//! 上（`fandhe-silicon-upper`）・下-2（`fandhe-silicon-exec`）が組み立て、下-1 の背後実装が受け取る。
//! 値は純粋なデータで、背後実装の物を持たない。作成時の検査・下位の API へ渡す口は無い。
//! 背後実装だけが使う `__backend_new` は検査をせず、渡された値をそのまま写す。
//! パイプラインの slot の並びを持つ `BindingLayout` と、パイプライン生成の口 `ComputeOps` も置く。
//! `ComputeOps` は trait の宣言だけで、実装は持たない。
//! 対応づけ: TASK-769、TASK-415、COMP-25、COMP-26・COMP-27・COMP-30・COMP-34、BIND-1・BIND-2・BIND-3（ポインタのみ）。

use crate::error::ContractError;
use crate::execution::PipelineInput;
use crate::memory::BufferHandle;

/// 生成済みの compute パイプラインを指す印。
///
/// 背後実装が `__backend_new` で作り、`__backend_id`・`__backend_device_id` で読む。
/// `Clone`・`Copy` を持たない。外の crate は構造体式で作れない。
///
/// ```
/// use fandhe_silicon_core::execution::PipelineHandle;
/// let p = PipelineHandle::__backend_new(3, 1);
/// assert_eq!(p.__backend_id(), 3);
/// assert_eq!(p.__backend_device_id(), 1);
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::execution::PipelineHandle;
/// let _ = PipelineHandle { id: 3, device_id: 1 };
/// ```
#[derive(Debug)]
pub struct PipelineHandle {
    id: u64,
    device_id: u64,
}

impl PipelineHandle {
    /// 背後実装向けの組み立ての口。検査はしない。
    #[doc(hidden)]
    pub fn __backend_new(id: u64, device_id: u64) -> Self {
        Self { id, device_id }
    }

    /// 背後実装向けの読み出しの口（パイプラインの識別子）。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// 背後実装向けの読み出しの口（所属する device の識別子）。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }
}

/// バッファ 1 件の束縛。スロット番号・バッファ・先頭からの byte オフセットを持つ。
///
/// 外の crate は構造体式で作れない。`new` で作る。検査はしない。
///
/// ```
/// use fandhe_silicon_core::execution::Binding;
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let b = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let x = Binding::new(2, &b, 16);
/// assert_eq!((x.slot, x.offset), (2, 16));
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::execution::Binding;
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let b = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let _ = Binding { slot: 2, buffer: &b, offset: 16 };
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Binding<'a> {
    /// 束縛先のスロット番号。
    pub slot: u32,
    /// 束縛するバッファ。
    pub buffer: &'a BufferHandle,
    /// バッファの先頭からの byte オフセット。
    pub offset: u64,
}

impl<'a> Binding<'a> {
    /// スロット・バッファ・オフセットから束縛を作る。検査はしない。
    pub fn new(slot: u32, buffer: &'a BufferHandle, offset: u64) -> Self {
        Self {
            slot,
            buffer,
            offset,
        }
    }
}

/// 定数（小さな値の bytes）1 件の束縛。
///
/// 外の crate は構造体式で作れない。`new` で作る。検査はしない。
///
/// ```
/// use fandhe_silicon_core::execution::ConstantBinding;
/// let c = ConstantBinding::new(1, &[1, 2, 3, 4]);
/// assert_eq!((c.slot, c.bytes), (1, &[1u8, 2, 3, 4][..]));
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::execution::ConstantBinding;
/// let _ = ConstantBinding { slot: 1, bytes: &[1, 2, 3, 4] };
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ConstantBinding<'a> {
    /// 束縛先のスロット番号。
    pub slot: u32,
    /// 定数の bytes。
    pub bytes: &'a [u8],
}

impl<'a> ConstantBinding<'a> {
    /// スロットと bytes から定数の束縛を作る。検査はしない。
    pub fn new(slot: u32, bytes: &'a [u8]) -> Self {
        Self { slot, bytes }
    }
}

/// スレッドグループ共有メモリ 1 件の指定。番号と byte 数を持つ。
///
/// 外の crate は構造体式で作れない。`new` で作る。検査はしない。
///
/// ```
/// use fandhe_silicon_core::execution::ThreadgroupMemory;
/// let t = ThreadgroupMemory::new(0, 256);
/// assert_eq!((t.index, t.bytes), (0, 256));
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::execution::ThreadgroupMemory;
/// let _ = ThreadgroupMemory { index: 0, bytes: 256 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ThreadgroupMemory {
    /// 共有メモリの番号。
    pub index: u32,
    /// 共有メモリの byte 数。
    pub bytes: u32,
}

impl ThreadgroupMemory {
    /// 番号と byte 数から指定を作る。検査はしない。
    pub fn new(index: u32, bytes: u32) -> Self {
        Self { index, bytes }
    }
}

/// パイプラインの作成に渡す slot の番号の並びを持つ値。
///
/// 外の crate は構造体式で作れない。`new` で作る。`new` は検査をせず、重複や並びもそのまま持つ。
/// `ComputeOps::create_pipeline` の引数として渡る。
///
/// ```
/// use fandhe_silicon_core::execution::BindingLayout;
/// let l = BindingLayout::new(vec![0, 1, 4]);
/// assert_eq!(l.slots, vec![0, 1, 4]);
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::execution::BindingLayout;
/// let _ = BindingLayout { slots: vec![0, 1, 4] };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingLayout {
    /// slot 番号の並び。
    pub slots: Vec<u32>,
}

impl BindingLayout {
    /// slot の並びから作る。検査はしない。
    pub fn new(slots: Vec<u32>) -> Self {
        Self { slots }
    }
}

/// compute パイプラインの生成の口。
///
/// 上（`fandhe-silicon-upper`）・下-2（`fandhe-silicon-exec`）が成果物と `BindingLayout` を渡して呼び、
/// 下-1 の背後実装が実装する。既定の本体を持つメソッドは無く、`create_pipeline` は必ず実装する。
///
/// `create_pipeline` を持たない実装は拒まれる。
///
/// ```compile_fail,E0046
/// use fandhe_silicon_core::execution::ComputeOps;
/// struct C;
/// impl ComputeOps for C {}
/// ```
///
/// 持てば通る。
///
/// ```
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_core::execution::{BindingLayout, ComputeOps, PipelineHandle, PipelineInput};
/// struct C;
/// impl ComputeOps for C {
///     fn create_pipeline(
///         &self,
///         _input: PipelineInput<'_>,
///         _layout: &BindingLayout,
///     ) -> Result<PipelineHandle, ContractError> {
///         Ok(PipelineHandle::__backend_new(1, 1))
///     }
/// }
/// ```
pub trait ComputeOps {
    /// 成果物と slot の並びからパイプラインを作る。検査と生成の中身は実装が決める。
    fn create_pipeline(
        &self,
        input: PipelineInput<'_>,
        layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError>;
}
