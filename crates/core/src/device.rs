//! device の要求・プロファイル・所有形態・ネイティブのハンドルの型。
//!
//! 上・下-2 が `DeviceRequest` を組み立て、下-1 の背後実装が受け取って device を作る。
//! 型の持ち主は TASK-136（ポインタ）。
//!
//! 加えて、背後実装が実装する device の口 `Device` と、それを得る入口 `DeviceProvider`、診断の値 `DeviceDiagnostics` を置く
//! （TASK-158 のポインタ）。`Device` は投入の窓口 `queue` を持つ。compute の口・ロストの通知の method は持たない（TASK-588・TASK-592・TASK-769 のポインタ）。

use core::any::Any;
use core::fmt;

use crate::capability::{BackendKind, Capabilities};
use crate::error::ContractError;
use crate::execution::CommandQueue;
use crate::memory::MemoryOps;

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

/// device の診断の値。`Device::diagnostics` が返す。
///
/// 診断出力やログの見出しに使う読み取り専用の値で、背後実装が device を作るときの情報を文字列で持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDiagnostics {
    /// アダプタ（物理 device）の名前。
    pub adapter_name: String,
    /// 背後実装の名前。
    pub backend_name: &'static str,
}

/// 背後実装が実装する device の口。
///
/// 上と下-2 は `DeviceProvider::request` で `Box<dyn Device>` を得て、能力の問合せ（`Capabilities`）と
/// メモリ操作（`memory`）・投入の窓口（`queue`）を呼ぶ。この段の `Device` は、compute の口・ロストの通知の method を持たない
/// （TASK-769 のポインタ）。ここに無い method を前提にしない。
/// 対応する項目のポインタ: DEV-1・DEV-16。
pub trait Device: Capabilities {
    /// この device の背後実装の種類。
    fn backend(&self) -> BackendKind;

    /// この device を作るときに指定された所有形態。
    fn ownership(&self) -> &DeviceOwnership;

    /// device の世代。背後実装ごとにプロセスの中で 1 から単調に増える値で、device を作るときに 1 つ取る。
    fn generation(&self) -> u64;

    /// この device のメモリ操作の口。
    fn memory(&self) -> &dyn MemoryOps;

    /// この device の投入の窓口。記録先の作成・投入・完了の確認はここから行う（CMD-1 のポインタ）。
    fn queue(&self) -> &dyn CommandQueue;

    /// 診断の値。
    fn diagnostics(&self) -> DeviceDiagnostics;
}

/// `DeviceRequest` から device を作る入口。背後実装が実装する。
///
/// 上・下-2 が背後実装の型を選んで `request` を呼ぶ。対応する項目のポインタ: DEV-67・DEV-68、TASK-265。
pub trait DeviceProvider {
    /// 要求から device を作る。
    ///
    /// - 要求の組み合わせの検査に当たると、device を作る前に `ContractError::InvalidCombination` を返す。
    /// - device を作り終えるまでの失敗は `ContractError::DeviceUnavailable` で返す。
    /// - 失敗しても別の背後実装へ切り替えない。要求した背後実装で作れなければエラーを返す。
    ///
    /// エラーに添える文字列の文言は契約にしない。試験は値の種類（`ContractError` の variant）で照らす。
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError>;
}
