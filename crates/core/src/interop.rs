//! 別の API・別のプロセスと資源を共有する相互運用の公開の型と、その操作の trait。
//!
//! feature `ext-interop` の裏にあり、feature を外した構成には名前が現れない。
//! 背後実装（下-1）が `ExternalInterop` を実装し、上と下-2 は `Device::external_interop` で
//! `&dyn ExternalInterop` を得て呼ぶ。ここには型・trait の宣言と既定の本体だけを置き、SDK は呼ばない。
//!
//! 外部ハンドルの型は `Clone`・`Copy` を実装せず、`import_*` に値で渡して消費する。
//! 同じハンドルを 2 回取り込む書き方と、複製して 2 か所へ取り込む書き方は、コンパイルで拒まれる
//! （各型の rustdoc の `compile_fail`）。
//! 各ハンドルの `__backend_*` は背後実装向けの口だが、safe なので誰でも呼べる。
//! この crate はハンドルの欄の値を検査しない。受けた背後実装が、欄の値を信用せず自分の記録と照合してから使うことを要請する。
//! 拡張の操作には既定の本体があり、上書きしない型では `ContractError::UnsupportedExtensionDefault { op }` を返す。

use crate::error::ContractError;
use crate::execution::{ImportedSemaphoreHandle, SubmitToken};
use crate::memory::{BufferHandle, BufferUsage};

mod same_gpu;
pub use same_gpu::{check_same_physical_gpu, import_memory_checked};

/// 物理 GPU の識別子。同じ物理 GPU かどうかを背後実装をまたいで照らすために `Device::physical_gpu_id` が返す。
///
/// `driver_uuid` は、背後実装が値を持たないとき `None`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalGpuId {
    /// device の UUID（16 バイト）。
    pub device_uuid: [u8; 16],
    /// driver の UUID（16 バイト）。値を持たない背後実装では `None`。
    pub driver_uuid: Option<[u8; 16]>,
}

/// 外部メモリのハンドル。`export_memory` が作り、`import_memory` が値で受け取って消費する。
///
/// 運ぶ値はすべて非公開（識別子・専用の確保かどうか・export 元の物理 GPU の識別子、unix では fd）。
/// `Clone`・`Copy` を実装しない。fd は `OwnedFd` で持つので、取り込まれずに drop されると閉じられる。
/// 取り込みに成功した背後実装は `__backend_take_fd` で fd を取り出して所有を移す。取り出した後のハンドルは fd を持たない。
/// fd を持たないハンドル（取り出し済み、または fd を載せずに作った値）も型の上では作れる。この crate は fd の有無を検査しないので、取り込む背後実装が確かめること。
///
/// 同じハンドルを 2 回取り込む書き方は、所有権の移動のエラーでコンパイルできない（INTEROP-44 のポインタ）。
///
/// ```compile_fail,E0382
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
/// # use fandhe_silicon_core::interop::{ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle};
/// # use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// # struct Backend;
/// # impl ExternalInterop for Backend {
/// #     fn export_memory(&self, _: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> { unimplemented!() }
/// #     fn import_memory(&self, _: ExternalMemoryHandle, n: u64, u: BufferUsage) -> Result<BufferHandle, ContractError> {
/// #         Ok(BufferHandle::__backend_new(1, 1, 1, n, u))
/// #     }
/// #     fn export_semaphore(&self, _: SubmitToken) -> Result<ExternalSemaphoreHandle, ContractError> { unimplemented!() }
/// #     fn import_semaphore(&self, _: ExternalSemaphoreHandle) -> Result<ImportedSemaphoreHandle, ContractError> { unimplemented!() }
/// # }
/// let backend = Backend;
/// let handle = ExternalMemoryHandle::__backend_new(1, false, None);
/// let _first = backend.import_memory(handle, 16, BufferUsage::STORAGE);
/// let _second = backend.import_memory(handle, 16, BufferUsage::STORAGE);
/// ```
///
/// 同じ書き方でも 1 回だけの取り込みは通る。
///
/// ```
/// # use fandhe_silicon_core::error::ContractError;
/// # use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
/// # use fandhe_silicon_core::interop::{ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle};
/// # use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
/// # struct Backend;
/// # impl ExternalInterop for Backend {
/// #     fn export_memory(&self, _: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> { unimplemented!() }
/// #     fn import_memory(&self, _: ExternalMemoryHandle, n: u64, u: BufferUsage) -> Result<BufferHandle, ContractError> {
/// #         Ok(BufferHandle::__backend_new(1, 1, 1, n, u))
/// #     }
/// #     fn export_semaphore(&self, _: SubmitToken) -> Result<ExternalSemaphoreHandle, ContractError> { unimplemented!() }
/// #     fn import_semaphore(&self, _: ExternalSemaphoreHandle) -> Result<ImportedSemaphoreHandle, ContractError> { unimplemented!() }
/// # }
/// let backend = Backend;
/// let handle = ExternalMemoryHandle::__backend_new(1, false, None);
/// let imported = backend.import_memory(handle, 16, BufferUsage::STORAGE).expect("import");
/// assert_eq!(imported.size(), 16);
/// ```
#[derive(Debug)]
pub struct ExternalMemoryHandle {
    id: u64,
    #[cfg(unix)]
    fd: Option<std::os::fd::OwnedFd>,
    dedicated: bool,
    source_gpu: Option<PhysicalGpuId>,
}

impl ExternalMemoryHandle {
    /// 背後実装がハンドルを作る。fd は持たない状態で作り、unix では `__backend_with_fd` で載せる。
    ///
    /// `source_gpu` は export 元の `Device::physical_gpu_id` の値。safe で誰でも値を作れるため、この crate は欄の値を検査しない。受ける背後実装が自分の記録と照合することを要請する。
    #[doc(hidden)]
    pub fn __backend_new(id: u64, dedicated: bool, source_gpu: Option<PhysicalGpuId>) -> Self {
        Self {
            id,
            #[cfg(unix)]
            fd: None,
            dedicated,
            source_gpu,
        }
    }

    /// fd を載せたハンドルを返す。ハンドルが fd の所有を持つ。
    #[doc(hidden)]
    #[cfg(unix)]
    pub fn __backend_with_fd(mut self, fd: std::os::fd::OwnedFd) -> Self {
        self.fd = Some(fd);
        self
    }

    /// fd を取り出して所有を移す。以後このハンドルは fd を持たず、2 回目は `None`。
    #[doc(hidden)]
    #[cfg(unix)]
    pub fn __backend_take_fd(&mut self) -> Option<std::os::fd::OwnedFd> {
        self.fd.take()
    }

    /// 識別子を読む。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// export 元が専用の確保で確保したかを読む。
    #[doc(hidden)]
    pub fn __backend_dedicated(&self) -> bool {
        self.dedicated
    }

    /// export 元の物理 GPU の識別子を読む。値を持たなければ `None`。
    #[doc(hidden)]
    pub fn __backend_source_gpu(&self) -> Option<&PhysicalGpuId> {
        self.source_gpu.as_ref()
    }
}

/// 外部セマフォのハンドル。`export_semaphore` などが作り、`import_semaphore` 系が値で受け取って消費する。
///
/// 運ぶ値はすべて非公開（識別子・timeline かどうか、unix では fd）。`Clone`・`Copy` を実装しない。
/// fd の所有と取り出しは `ExternalMemoryHandle` と同じ。
/// timeline かどうかは `__backend_timeline` で読める。取り込みの操作との対応はこの crate では検査せず、取り込む背後実装が確かめること。
///
/// 複製して 2 か所へ取り込む書き方は、`clone` が存在しないためコンパイルできない（INTEROP-45 のポインタ）。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::interop::ExternalSemaphoreHandle;
///
/// let handle = ExternalSemaphoreHandle::__backend_new(1, true);
/// let _copy = handle.clone();
/// ```
#[derive(Debug)]
pub struct ExternalSemaphoreHandle {
    id: u64,
    #[cfg(unix)]
    fd: Option<std::os::fd::OwnedFd>,
    timeline: bool,
}

impl ExternalSemaphoreHandle {
    /// 背後実装がハンドルを作る。fd は持たない状態で作り、unix では `__backend_with_fd` で載せる。
    #[doc(hidden)]
    pub fn __backend_new(id: u64, timeline: bool) -> Self {
        Self {
            id,
            #[cfg(unix)]
            fd: None,
            timeline,
        }
    }

    /// fd を載せたハンドルを返す。ハンドルが fd の所有を持つ。
    #[doc(hidden)]
    #[cfg(unix)]
    pub fn __backend_with_fd(mut self, fd: std::os::fd::OwnedFd) -> Self {
        self.fd = Some(fd);
        self
    }

    /// fd を取り出して所有を移す。以後このハンドルは fd を持たず、2 回目は `None`。
    #[doc(hidden)]
    #[cfg(unix)]
    pub fn __backend_take_fd(&mut self) -> Option<std::os::fd::OwnedFd> {
        self.fd.take()
    }

    /// 識別子を読む。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// timeline のセマフォなら `true`、binary なら `false`。
    #[doc(hidden)]
    pub fn __backend_timeline(&self) -> bool {
        self.timeline
    }
}

/// 外部 image のハンドル。`export_image` 系が作り、`import_image` が値で受け取って消費する。
///
/// 運ぶ値はすべて非公開（識別子・専用の確保かどうか、unix では fd）。`Clone`・`Copy` を実装しない。
/// fd の所有と取り出しは `ExternalMemoryHandle` と同じ。
#[derive(Debug)]
pub struct ExternalImageHandle {
    id: u64,
    #[cfg(unix)]
    fd: Option<std::os::fd::OwnedFd>,
    dedicated: bool,
}

impl ExternalImageHandle {
    /// 背後実装がハンドルを作る。fd は持たない状態で作り、unix では `__backend_with_fd` で載せる。
    #[doc(hidden)]
    pub fn __backend_new(id: u64, dedicated: bool) -> Self {
        Self {
            id,
            #[cfg(unix)]
            fd: None,
            dedicated,
        }
    }

    /// fd を載せたハンドルを返す。ハンドルが fd の所有を持つ。
    #[doc(hidden)]
    #[cfg(unix)]
    pub fn __backend_with_fd(mut self, fd: std::os::fd::OwnedFd) -> Self {
        self.fd = Some(fd);
        self
    }

    /// fd を取り出して所有を移す。以後このハンドルは fd を持たず、2 回目は `None`。
    #[doc(hidden)]
    #[cfg(unix)]
    pub fn __backend_take_fd(&mut self) -> Option<std::os::fd::OwnedFd> {
        self.fd.take()
    }

    /// 識別子を読む。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }

    /// export 元が専用の確保で確保したかを読む。
    #[doc(hidden)]
    pub fn __backend_dedicated(&self) -> bool {
        self.dedicated
    }
}

/// export 元が保持する image のハンドル。
///
/// 取り込み側へ渡すハンドルとは別で、`Clone`・`Copy` を実装する。消費されない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportedImageHandle {
    id: u64,
}

impl ExportedImageHandle {
    /// 背後実装がハンドルを作る。
    #[doc(hidden)]
    pub fn __backend_new(id: u64) -> Self {
        Self { id }
    }

    /// 識別子を読む。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }
}

/// 取り込んだ image を指すハンドル。`import_image` が返す。`Clone`・`Copy` を実装しない。
#[derive(Debug)]
pub struct ImportedImageHandle {
    id: u64,
}

impl ImportedImageHandle {
    /// 背後実装がハンドルを作る。
    #[doc(hidden)]
    pub fn __backend_new(id: u64) -> Self {
        Self { id }
    }

    /// 識別子を読む。
    #[doc(hidden)]
    pub fn __backend_id(&self) -> u64 {
        self.id
    }
}

/// 共有する image の画素の形式。グラフィックスの形式の型とは独立に持つ。
///
/// `#[non_exhaustive]` なので、後から variant を足しても外の crate の `match` は壊れない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExternalImageFormat {
    /// 8 ビット × 4 チャネルの正規化整数（R, G, B, A の順）。
    Rgba8Unorm,
    /// 8 ビット × 4 チャネルの正規化整数（B, G, R, A の順）。
    Bgra8Unorm,
    /// 32 ビット浮動小数点 × 4 チャネル。
    Rgba32Float,
}

/// 共有する image のメモリの並び。
///
/// `#[non_exhaustive]` なので、後から variant を足せる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExternalImageTiling {
    /// 実装が選ぶ最適な並び。
    Optimal,
    /// 行ごとに連続した並び。
    Linear,
}

/// 共有する image の作成直後の配置。
///
/// `#[non_exhaustive]` なので、後から variant を足せる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExternalImageLayout {
    /// 未定義。内容は保たれない。
    Undefined,
    /// 汎用。
    General,
    /// 転送の読み出し元。
    TransferSrc,
    /// 転送の書き込み先。
    TransferDst,
}

/// 専用の確保の要否の申告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DedicatedInfo {
    /// 専用の確保にするとき `true`。
    pub dedicated: bool,
}

/// `export_image`・`import_image` に渡す image の記述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalImageDescriptor {
    /// 画素の形式。
    pub format: ExternalImageFormat,
    /// 幅・高さ・深さ。
    pub extent: [u32; 3],
    /// メモリの並び。
    pub tiling: ExternalImageTiling,
    /// 作成直後の配置。
    pub initial_layout: ExternalImageLayout,
    /// 専用の確保の申告。申告しないとき `None`。
    pub dedicated: Option<DedicatedInfo>,
}

/// 別の API・別のプロセスと資源を共有する操作の trait。背後実装が実装し、`Device::external_interop` から届く。
///
/// 必須の操作は `export_memory`・`import_memory`・`export_semaphore`・`import_semaphore` の 4 つ。
/// それ以外の操作は既定の本体を持ち、上書きしない型では呼ぶと `ContractError::UnsupportedExtensionDefault { op }`
/// （`op` は呼んだ操作の名前）を返す。値や資源は作らず、暗黙の代替もしない。
/// 取り込みの操作は外部ハンドルを値で受け取る。渡したハンドルは `Err` のときも呼び手に戻らない（型の事実）。
pub trait ExternalInterop {
    /// `EXPORTABLE` で確保したバッファを外部メモリのハンドルにする。
    fn export_memory(&self, buffer: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError>;

    /// 外部メモリのハンドルを取り込み、`byte_len`・`usage` のバッファを返す。`external` は値で渡され、戻らない。
    fn import_memory(
        &self,
        external: ExternalMemoryHandle,
        byte_len: u64,
        usage: BufferUsage,
    ) -> Result<BufferHandle, ContractError>;

    /// 投入のトークンに対応する binary のセマフォを外部セマフォのハンドルにする。
    fn export_semaphore(
        &self,
        token: SubmitToken,
    ) -> Result<ExternalSemaphoreHandle, ContractError>;

    /// binary の外部セマフォのハンドルを取り込み、投入に結べるハンドルを返す。`external` は値で渡され、戻らない。
    fn import_semaphore(
        &self,
        external: ExternalSemaphoreHandle,
    ) -> Result<ImportedSemaphoreHandle, ContractError>;

    /// image を外部 image のハンドルにする。既定は `UnsupportedExtensionDefault { op: "export_image" }`。
    fn export_image(
        &self,
        _desc: ExternalImageDescriptor,
    ) -> Result<ExternalImageHandle, ContractError> {
        Err(ContractError::UnsupportedExtensionDefault { op: "export_image" })
    }

    /// 外部 image のハンドルを取り込む。`external` は値で渡され、戻らない。既定は `UnsupportedExtensionDefault { op: "import_image" }`。
    fn import_image(
        &self,
        _external: ExternalImageHandle,
        _desc: ExternalImageDescriptor,
    ) -> Result<ImportedImageHandle, ContractError> {
        Err(ContractError::UnsupportedExtensionDefault { op: "import_image" })
    }

    /// timeline のセマフォを外部セマフォのハンドルにし、export 時点の値と対で返す。
    /// 既定は `UnsupportedExtensionDefault { op: "export_semaphore_timeline" }`。
    fn export_semaphore_timeline(
        &self,
        _token: SubmitToken,
    ) -> Result<(ExternalSemaphoreHandle, u64), ContractError> {
        Err(ContractError::UnsupportedExtensionDefault {
            op: "export_semaphore_timeline",
        })
    }

    /// timeline の外部セマフォのハンドルを取り込む。`external` は値で渡され、戻らない。
    /// 既定は `UnsupportedExtensionDefault { op: "import_semaphore_timeline" }`。
    fn import_semaphore_timeline(
        &self,
        _external: ExternalSemaphoreHandle,
        _wait_value: u64,
    ) -> Result<ImportedSemaphoreHandle, ContractError> {
        Err(ContractError::UnsupportedExtensionDefault {
            op: "import_semaphore_timeline",
        })
    }
}
