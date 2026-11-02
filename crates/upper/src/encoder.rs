//! 上の層の記録先・投入前の値・投入の印（`lib.rs` から `pub use` される）。
//!
//! 契約の `CommandEncoder`・`SubmitToken`（`fandhe-silicon-core`）を所有で包む薄い型。
//! `device.rs` の `Queue` が作成・投入・時刻の読み取りで受け渡す。

use core::marker::PhantomData;

use fandhe_silicon_core::execution::{CommandEncoder as ContractEncoder, SubmitToken};

/// 契約の記録先を値で持ち回す記録先。`Queue::create_command_encoder` が作る。
///
/// `finish` は自身を消費するので、`finish` の後に同じ値を使うコードはコンパイルできない。
///
/// ```compile_fail,E0382
/// use fandhe_silicon_core::device::Device;
/// use fandhe_silicon_upper::UpperDevice;
///
/// fn probe(dev: &dyn Device) {
///     let q = UpperDevice::new(dev).queue();
///     let enc = q.create_command_encoder();
///     let _a = enc.finish();
///     let _b = enc.finish();
/// }
/// ```
///
/// `finish` の前の記録先は `Queue::submit` に渡せない。
///
/// ```compile_fail,E0308
/// use fandhe_silicon_core::device::Device;
/// use fandhe_silicon_upper::UpperDevice;
///
/// fn probe(dev: &dyn Device) {
///     let q = UpperDevice::new(dev).queue();
///     let enc = q.create_command_encoder();
///     let _ = q.submit(enc);
/// }
/// ```
#[must_use]
#[derive(Debug)]
pub struct CommandEncoder<'d> {
    inner: ContractEncoder,
    _device: PhantomData<&'d ()>,
}

impl<'d> CommandEncoder<'d> {
    /// 契約の記録先を包む。`Queue::create_command_encoder` から呼ぶ。
    pub(crate) fn from_contract(inner: ContractEncoder) -> Self {
        Self {
            inner,
            _device: PhantomData,
        }
    }

    /// 記録を終え、投入前の値を返す。中の値を移すだけで、契約の操作は呼ばず、失敗しない。
    pub fn finish(self) -> CommandBuffer<'d> {
        CommandBuffer {
            inner: self.inner,
            _device: PhantomData,
        }
    }
}

/// `finish` で得た投入前の値。公開の操作は持たず、`Queue::submit` に渡す。
///
/// 投入すると消費されるので、同じ値を 2 回投入するコードはコンパイルできない。
///
/// ```compile_fail,E0382
/// use fandhe_silicon_core::device::Device;
/// use fandhe_silicon_upper::UpperDevice;
///
/// fn probe(dev: &dyn Device) {
///     let q = UpperDevice::new(dev).queue();
///     let buf = q.create_command_encoder().finish();
///     let _ = q.submit(buf);
///     let _ = q.submit(buf);
/// }
/// ```
#[must_use]
#[derive(Debug)]
pub struct CommandBuffer<'d> {
    inner: ContractEncoder,
    _device: PhantomData<&'d ()>,
}

impl CommandBuffer<'_> {
    /// 投入のために契約の記録先を取り出す。`Queue::submit` から呼ぶ。
    pub(crate) fn into_contract(self) -> ContractEncoder {
        self.inner
    }
}

/// 契約の投入の印を包んだ値。`Queue::submit` が返し、時刻の読み取り（と待ち）に渡す。
///
/// 取り出す口・作る口は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmitId {
    token: SubmitToken,
}

impl SubmitId {
    /// 契約の印を包む。`Queue::submit` から呼ぶ。
    pub(crate) fn from_token(token: SubmitToken) -> Self {
        Self { token }
    }

    /// 契約の印を取り出す。`Queue` の待ち・時刻の読み取りから呼ぶ。
    pub(crate) fn token(self) -> SubmitToken {
        self.token
    }
}
