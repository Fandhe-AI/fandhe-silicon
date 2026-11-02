//! 上の層のバッファの包み（`lib.rs` から `pub use` される）。
//!
//! 契約の `BufferHandle`（`fandhe-silicon-core`）を値で持つ。契約の操作は呼ばない。

use fandhe_silicon_core::memory::BufferHandle;

/// 契約の `BufferHandle` を所有で包む値。
///
/// 複製できない。
///
/// ```compile_fail,E0599
/// use fandhe_silicon_core::memory::BufferHandle;
/// use fandhe_silicon_upper::Buffer;
///
/// fn probe(h: BufferHandle) {
///     let b = Buffer::from_handle(h);
///     let _c = b.clone();
/// }
/// ```
///
/// 取り出しは `into_handle` で 1 回だけ。
///
/// ```compile_fail,E0382
/// use fandhe_silicon_core::memory::BufferHandle;
/// use fandhe_silicon_upper::Buffer;
///
/// fn probe(h: BufferHandle) {
///     let b = Buffer::from_handle(h);
///     let _a = b.into_handle();
///     let _b = b.into_handle();
/// }
/// ```
///
/// 包んだ handle は借りて読め、取り出して返せる。
///
/// ```no_run
/// use fandhe_silicon_core::memory::BufferHandle;
/// use fandhe_silicon_upper::Buffer;
///
/// fn probe(h: BufferHandle) -> BufferHandle {
///     let mut b = Buffer::from_handle(h);
///     let _ = b.handle().size();
///     let _ = b.handle_mut();
///     b.into_handle()
/// }
/// ```
#[derive(Debug)]
pub struct Buffer {
    handle: BufferHandle,
}

impl Buffer {
    /// handle を包む。値を移すだけで、契約の操作は呼ばず、失敗しない。
    pub fn from_handle(handle: BufferHandle) -> Self {
        Self { handle }
    }

    /// 包んだ handle を借りる。
    pub fn handle(&self) -> &BufferHandle {
        &self.handle
    }

    /// 包んだ handle を可変で借りる。
    pub fn handle_mut(&mut self) -> &mut BufferHandle {
        &mut self.handle
    }

    /// 包んだ handle を取り出す。自身を消費するので 1 回だけ呼べる。
    pub fn into_handle(self) -> BufferHandle {
        self.handle
    }
}
