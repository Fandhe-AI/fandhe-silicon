//! 束縛の宣言・束縛の組・束縛の組の配置・パイプラインの配置の型。
//!
//! 3 段で受け渡す束縛の型を置く。値は純粋なデータで、背後実装の物を持たない。
//! 値を作る操作・作成時の検査・下位の API へ渡す口はこのモジュールに無い。
//! 背後実装（下-1）だけが使う `__backend_new` は検査をせず、渡された値をそのまま写す。
//! 対応づけ: TASK-419、BIND-5・BIND-11・BIND-12・BIND-13（ポインタのみ）。

use core::num::NonZeroU64;
use core::ops::{BitOr, BitOrAssign};

use crate::memory::BufferHandle;

// ---- 束縛の宣言 ----

/// 束縛の宣言 1 件。番号・可視性・種類を持つ。
///
/// 外の crate は構造体式で作れない。`new` で作る。欄は `pub` で読み書きできる。
///
/// ```
/// use fandhe_silicon_core::binding::{
///     BindingDeclaration, BindingType, BufferBindingType, ShaderStageSet,
/// };
/// let d = BindingDeclaration::new(
///     0,
///     ShaderStageSet::COMPUTE,
///     BindingType::buffer(BufferBindingType::Uniform),
/// );
/// assert_eq!(d.binding, 0);
/// ```
///
/// 構造体式は拒まれる。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::binding::{
///     BindingDeclaration, BindingType, BufferBindingType, ShaderStageSet,
/// };
/// let _ = BindingDeclaration {
///     binding: 0,
///     visibility: ShaderStageSet::COMPUTE,
///     ty: BindingType::buffer(BufferBindingType::Uniform),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BindingDeclaration {
    /// 束縛の番号。
    pub binding: u32,
    /// この束縛が見える段の集合。
    pub visibility: ShaderStageSet,
    /// 束縛の種類。
    pub ty: BindingType,
}

impl BindingDeclaration {
    /// 番号・可視性・種類から宣言を作る。検査はしない。
    pub fn new(binding: u32, visibility: ShaderStageSet, ty: BindingType) -> Self {
        Self {
            binding,
            visibility,
            ty,
        }
    }
}

/// 束縛の種類。変種は `Buffer` だけ。
///
/// `_` の腕のない `match` は拒まれる。
///
/// ```compile_fail,E0004
/// use fandhe_silicon_core::binding::{BindingType, BufferBindingType};
/// let t = BindingType::buffer(BufferBindingType::Uniform);
/// match t {
///     BindingType::Buffer { .. } => (),
/// }
/// ```
///
/// 変種の構造体式は拒まれる。`buffer` で作る。
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::binding::{BindingType, BufferBindingType};
/// let _ = BindingType::Buffer { ty: BufferBindingType::Uniform };
/// ```
///
/// ```
/// use fandhe_silicon_core::binding::{BindingType, BufferBindingType};
/// let t = BindingType::buffer(BufferBindingType::Uniform);
/// assert!(matches!(t, BindingType::Buffer { .. }));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BindingType {
    /// バッファの束縛。
    #[non_exhaustive]
    Buffer {
        /// バッファの束縛の種類。
        ty: BufferBindingType,
    },
}

impl BindingType {
    /// バッファの束縛の種類を作る。外の crate が `Buffer` を組み立てる口。
    pub const fn buffer(ty: BufferBindingType) -> Self {
        Self::Buffer { ty }
    }
}

/// バッファの束縛の種類。読み書きの別は `Storage` の中に置く。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BufferBindingType {
    /// uniform バッファ。
    Uniform,
    /// storage バッファ。
    Storage {
        /// 読み取り専用なら `true`。
        read_only: bool,
    },
}

/// 可視性（段の集合）。内部のビットは非公開で、ビット列から値は作れない。
///
/// ```
/// use fandhe_silicon_core::binding::ShaderStageSet;
/// assert!((ShaderStageSet::VERTEX | ShaderStageSet::FRAGMENT).contains(ShaderStageSet::VERTEX));
/// ```
///
/// ```compile_fail,E0423
/// use fandhe_silicon_core::binding::ShaderStageSet;
/// let _ = ShaderStageSet(1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderStageSet(u32);

impl ShaderStageSet {
    /// どの段も含まない。
    pub const NONE: Self = Self(0);
    /// 頂点段。
    pub const VERTEX: Self = Self(1 << 0);
    /// フラグメント段。
    pub const FRAGMENT: Self = Self(1 << 1);
    /// compute 段。
    pub const COMPUTE: Self = Self(1 << 2);
    /// task 段。
    pub const TASK: Self = Self(1 << 3);
    /// mesh 段。
    pub const MESH: Self = Self(1 << 4);
    /// レイ生成段。
    pub const RAY_GENERATION: Self = Self(1 << 5);
    /// any-hit 段。
    pub const ANY_HIT: Self = Self(1 << 6);
    /// closest-hit 段。
    pub const CLOSEST_HIT: Self = Self(1 << 7);
    /// miss 段。
    pub const MISS: Self = Self(1 << 8);
    /// 頂点段とフラグメント段の和。
    pub const VERTEX_FRAGMENT: Self = Self(Self::VERTEX.0 | Self::FRAGMENT.0);

    /// どの段も含まないとき `true`。
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// `other` の段をすべて含むとき `true`。
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for ShaderStageSet {
    type Output = Self;

    /// 2 つの集合の和を返す。
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ShaderStageSet {
    /// 段を足す。
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// ---- 束縛の組の配置 ----

/// 束縛の組の配置を作る操作に渡す記述子。宣言の並びを借りる。
///
/// ```
/// use fandhe_silicon_core::binding::BindGroupLayoutDescriptor;
/// let d = BindGroupLayoutDescriptor::new(&[]);
/// assert!(d.entries.is_empty());
/// ```
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::binding::BindGroupLayoutDescriptor;
/// let _ = BindGroupLayoutDescriptor { entries: &[] };
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BindGroupLayoutDescriptor<'a> {
    /// 宣言の並び。
    pub entries: &'a [BindingDeclaration],
}

impl<'a> BindGroupLayoutDescriptor<'a> {
    /// 宣言の並びから記述子を作る。
    pub fn new(entries: &'a [BindingDeclaration]) -> Self {
        Self { entries }
    }
}

/// 束縛の組の配置。device の識別子と宣言の並びだけを持つ純粋なデータ。
///
/// 欄は非公開で、構造体式は拒まれる。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::binding::BindGroupLayout;
/// let _ = BindGroupLayout { device_id: 0, entries: Vec::new() };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BindGroupLayout {
    device_id: u64,
    entries: Vec<BindingDeclaration>,
}

impl BindGroupLayout {
    /// 宣言の並びを返す。`__backend_new` に渡された値をそのまま返す。背後実装を呼ばず、失敗しない。
    pub fn entries(&self) -> &[BindingDeclaration] {
        &self.entries
    }

    /// 背後実装が配置を作る。記述子の宣言を写す。検査はしない。
    ///
    /// 背後実装（下-1）だけが使う口。safe で誰でも値を作れるため、
    /// 背後実装はこの値を自分の記録で照らしてから下位の API に渡すこと。
    #[doc(hidden)]
    pub fn __backend_new(device_id: u64, desc: &BindGroupLayoutDescriptor<'_>) -> Self {
        Self {
            device_id,
            entries: desc.entries.to_vec(),
        }
    }

    /// 背後実装が device の識別子を読む。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }
}

// ---- パイプラインの配置 ----

/// パイプラインの配置を作る操作に渡す記述子。添字がグループ番号で、`None` は空き。
///
/// ```
/// use fandhe_silicon_core::binding::PipelineLayoutDescriptor;
/// let d = PipelineLayoutDescriptor::new(&[], 0);
/// assert_eq!(d.immediate_size, 0);
/// ```
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::binding::PipelineLayoutDescriptor;
/// let _ = PipelineLayoutDescriptor { bind_group_layouts: &[], immediate_size: 0 };
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PipelineLayoutDescriptor<'a> {
    /// グループ番号ごとの束縛の組の配置。`None` は空き。
    pub bind_group_layouts: &'a [Option<&'a BindGroupLayout>],
    /// immediate データの大きさ（バイト）。
    pub immediate_size: u32,
}

impl<'a> PipelineLayoutDescriptor<'a> {
    /// 配置の並びと immediate データの大きさから記述子を作る。
    pub fn new(bind_group_layouts: &'a [Option<&'a BindGroupLayout>], immediate_size: u32) -> Self {
        Self {
            bind_group_layouts,
            immediate_size,
        }
    }
}

/// パイプラインの配置。束縛の組の配置の写しと `immediate_size` を持つ純粋なデータ。寿命の引数を持たない。
///
/// 欄は非公開で、構造体式は拒まれる。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::binding::PipelineLayout;
/// let _ = PipelineLayout { device_id: 0, bind_group_layouts: Vec::new(), immediate_size: 0 };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineLayout {
    device_id: u64,
    bind_group_layouts: Vec<Option<BindGroupLayout>>,
    immediate_size: u32,
}

impl PipelineLayout {
    /// グループ番号ごとの束縛の組の配置を返す。`None` は空き。渡された値をそのまま返す。
    pub fn bind_group_layouts(&self) -> &[Option<BindGroupLayout>] {
        &self.bind_group_layouts
    }

    /// immediate データの大きさ（バイト）を返す。渡された値をそのまま返す。
    pub fn immediate_size(&self) -> u32 {
        self.immediate_size
    }

    /// 背後実装が配置を作る。各配置を複製して写す。検査はしない。
    ///
    /// 背後実装（下-1）だけが使う口。safe で誰でも値を作れるため、
    /// 背後実装はこの値を自分の記録で照らしてから下位の API に渡すこと。
    #[doc(hidden)]
    pub fn __backend_new(device_id: u64, desc: &PipelineLayoutDescriptor<'_>) -> Self {
        Self {
            device_id,
            bind_group_layouts: desc.bind_group_layouts.iter().map(|l| l.cloned()).collect(),
            immediate_size: desc.immediate_size,
        }
    }

    /// 背後実装が device の識別子を読む。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }
}

// ---- 束縛の組 ----

/// 束縛の資源。変種は `Buffer` だけ。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BindingResource<'a> {
    /// 単体のバッファの束縛。
    Buffer(BufferBinding<'a>),
}

/// バッファの束縛。`size` が `None` なら `offset` から末尾まで。大きさ 0 は型で表せない。
///
/// ```
/// use core::num::NonZeroU64;
/// use fandhe_silicon_core::binding::BufferBinding;
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let b = BufferBinding::new(&h, 0, NonZeroU64::new(4));
/// assert_eq!(b.offset, 0);
/// ```
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::binding::BufferBinding;
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let _ = BufferBinding { buffer: &h, offset: 0, size: None };
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BufferBinding<'a> {
    /// 束縛するバッファ。
    pub buffer: &'a BufferHandle,
    /// 束縛の開始位置（バイト）。
    pub offset: u64,
    /// 束縛する大きさ（バイト）。`None` は `offset` から末尾まで。
    pub size: Option<NonZeroU64>,
}

impl<'a> BufferBinding<'a> {
    /// バッファ・開始位置・大きさから束縛を作る。検査はしない。
    pub fn new(buffer: &'a BufferHandle, offset: u64, size: Option<NonZeroU64>) -> Self {
        Self {
            buffer,
            offset,
            size,
        }
    }
}

/// 束縛の組の項目 1 件。番号と資源を持つ。
///
/// ```
/// use fandhe_silicon_core::binding::{BindGroupEntry, BindingResource, BufferBinding};
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let e = BindGroupEntry::new(0, BindingResource::Buffer(BufferBinding::new(&h, 0, None)));
/// assert_eq!(e.binding, 0);
/// ```
///
/// ```compile_fail,E0639
/// use fandhe_silicon_core::binding::{BindGroupEntry, BindingResource, BufferBinding};
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let _ = BindGroupEntry {
///     binding: 0,
///     resource: BindingResource::Buffer(BufferBinding::new(&h, 0, None)),
/// };
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BindGroupEntry<'a> {
    /// 束縛の番号。
    pub binding: u32,
    /// 束縛する資源。
    pub resource: BindingResource<'a>,
}

impl<'a> BindGroupEntry<'a> {
    /// 番号と資源から項目を作る。検査はしない。
    pub fn new(binding: u32, resource: BindingResource<'a>) -> Self {
        Self { binding, resource }
    }
}

/// 束縛の組。配置と資源を借用する値で、`Clone`・`Copy` を持たない。
///
/// 欄は非公開で、構造体式は拒まれる。
///
/// ```compile_fail,E0451
/// use fandhe_silicon_core::binding::{BindGroup, BindGroupLayout};
/// fn f(layout: &BindGroupLayout) {
///     let _ = BindGroup { device_id: 0, layout, entries: Vec::new() };
/// }
/// ```
///
/// 複製は拒まれる。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::binding::BindGroup;
/// fn f(g: BindGroup<'_>) {
///     let _ = g.clone();
/// }
/// ```
///
/// 組が生きている間は資源を値で手放せない。
///
/// ```compile_fail,E0505
/// use fandhe_silicon_core::binding::*;
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let layout = BindGroupLayout::__backend_new(7, &BindGroupLayoutDescriptor::new(&[]));
/// let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let entries = [BindGroupEntry::new(0, BindingResource::Buffer(BufferBinding::new(&h, 0, None)))];
/// let group = BindGroup::__backend_new(7, &layout, &entries);
/// drop(h);
/// let _ = group.entries();
/// ```
///
/// 陽性対照: 組を使い終えてから手放す書き方は通る。
///
/// ```
/// use fandhe_silicon_core::binding::*;
/// use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// let layout = BindGroupLayout::__backend_new(7, &BindGroupLayoutDescriptor::new(&[]));
/// let h = BufferHandle::__backend_new(0, 1, 7, 64, BufferUsage::STORAGE);
/// let entries = [BindGroupEntry::new(0, BindingResource::Buffer(BufferBinding::new(&h, 0, None)))];
/// let group = BindGroup::__backend_new(7, &layout, &entries);
/// let _ = group.entries();
/// drop(group);
/// drop(h);
/// ```
///
/// 組が生きている間は配置も手放せない。
///
/// ```compile_fail,E0597
/// use fandhe_silicon_core::binding::*;
/// let group;
/// {
///     let layout = BindGroupLayout::__backend_new(7, &BindGroupLayoutDescriptor::new(&[]));
///     group = BindGroup::__backend_new(7, &layout, &[]);
/// }
/// let _ = group.entries();
/// ```
#[derive(Debug)]
pub struct BindGroup<'a> {
    device_id: u64,
    layout: &'a BindGroupLayout,
    entries: Vec<BindGroupEntry<'a>>,
}

impl<'a> BindGroup<'a> {
    /// 借用している配置を返す。渡された参照をそのまま返す。
    pub fn layout(&self) -> &'a BindGroupLayout {
        self.layout
    }

    /// 項目の並びを返す。渡された値をそのまま返す。
    pub fn entries(&self) -> &[BindGroupEntry<'a>] {
        &self.entries
    }

    /// 背後実装が束縛の組を作る。`entries` を写し、呼び手の slice は借用し続けない。検査はしない。
    ///
    /// 背後実装（下-1）だけが使う口。safe で誰でも値を作れるため、
    /// 背後実装はこの値を自分の記録で照らしてから下位の API に渡すこと。
    #[doc(hidden)]
    pub fn __backend_new(
        device_id: u64,
        layout: &'a BindGroupLayout,
        entries: &[BindGroupEntry<'a>],
    ) -> Self {
        Self {
            device_id,
            layout,
            entries: entries.to_vec(),
        }
    }

    /// 背後実装が device の識別子を読む。
    #[doc(hidden)]
    pub fn __backend_device_id(&self) -> u64 {
        self.device_id
    }
}
