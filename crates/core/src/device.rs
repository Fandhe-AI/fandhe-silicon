//! device の要求・プロファイル・所有形態・ネイティブのハンドルの型。
//!
//! 上・下-2 が `DeviceRequest` を組み立て、下-1 の背後実装が受け取って device を作る。
//! 型の持ち主は TASK-136（ポインタ）。`Device` トレイトなどは別の持ち分でここには置かない。

use core::any::Any;
use core::fmt;

use crate::capability::BackendKind;

/// device の要求。背後実装・プロファイル・所有形態の 3 つだけを持つ。
#[derive(Debug)]
pub struct DeviceRequest {
    /// 要求する背後実装。
    pub backend: BackendKind,
    /// 要求するプロファイル。
    pub profile: Profile,
    /// device の所有形態。
    pub ownership: DeviceOwnership,
}

/// device のプロファイル。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Profile {
    /// CPU だけ。
    CpuOnly,
    /// compute だけ。
    ComputeOnly,
    /// グラフィックスを含む。
    Graphics,
}

/// device の所有形態。
#[derive(Debug)]
#[non_exhaustive]
pub enum DeviceOwnership {
    /// 製品が独立して持つ。
    ProductIndependent,
    /// 他と共有する。
    Shared,
    /// 外から取り込んだネイティブの device を使う。
    Imported {
        /// 取り込むネイティブのハンドル。
        native: NativeDeviceHandle,
    },
}

/// 背後実装のネイティブの device のハンドルを型を消して包む。
///
/// 作る口は safe な `new`。読む口は背後実装（下-1）だけが使う前提の隠した関数で、上・下-2 は使わない。
pub struct NativeDeviceHandle {
    inner: Box<dyn Any + Send + Sync>,
}

impl NativeDeviceHandle {
    /// 値を包んでハンドルを作る。
    pub fn new<T: Any + Send + Sync>(value: T) -> Self {
        Self {
            inner: Box::new(value),
        }
    }

    /// 背後実装が中身を型を指定して取り出す。型が違えば `None`。
    #[doc(hidden)]
    pub fn __backend_downcast_ref<T: Any>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }
}

impl fmt::Debug for NativeDeviceHandle {
    /// 中身は出さない。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeDeviceHandle(..)")
    }
}
