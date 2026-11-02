//! 上の層の device と queue（`lib.rs` から `pub use` される）。
//!
//! `&dyn Device` を借用で包み、`Queue` の各操作は呼ぶたびに `device.queue()` で得た
//! 契約の `CommandQueue` の操作をちょうど 1 回呼んで、結果を値のまま返す。

use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::GpuTiming;

use crate::encoder::{CommandBuffer, CommandEncoder, SubmitId};

/// 契約の `Device` を借用で包む上の層の入口。持つのは `&dyn Device` だけ。
///
/// ```
/// use fandhe_silicon_core::device::Device;
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_upper::UpperDevice;
///
/// fn probe(dev: &dyn Device) -> Result<(), ContractError> {
///     let q = UpperDevice::new(dev).queue();
///     let id = q.submit(q.create_command_encoder().finish())?;
///     let _ = q.gpu_timing(id);
///     Ok(())
/// }
/// ```
pub struct UpperDevice<'d> {
    device: &'d dyn Device,
}

impl<'d> UpperDevice<'d> {
    /// `device` を借用で包む。
    pub fn new(device: &'d dyn Device) -> Self {
        Self { device }
    }

    /// 投入の窓口を返す。失敗しない。
    pub fn queue(&self) -> Queue<'d> {
        Queue {
            device: self.device,
        }
    }
}

/// 記録先の作成・投入・待ち・時刻の読み取りの窓口。持つのは `&dyn Device` だけ。
///
/// `native-blocking-wait` を有効にしないビルドには `wait_blocking` が無い。
///
#[cfg_attr(not(feature = "native-blocking-wait"), doc = "```compile_fail,E0599")]
#[cfg_attr(feature = "native-blocking-wait", doc = "```")]
#[doc = "use fandhe_silicon_upper::{Queue, SubmitId};"]
#[doc = ""]
#[doc = "fn probe(q: &Queue<'_>, id: SubmitId) {"]
#[doc = "    let _ = q.wait_blocking(id, None);"]
#[doc = "}"]
#[doc = "```"]
pub struct Queue<'d> {
    device: &'d dyn Device,
}

impl<'d> Queue<'d> {
    /// 契約の `create_encoder` を 1 回呼び、返った値を包んで返す。
    pub fn create_command_encoder(&self) -> CommandEncoder<'d> {
        CommandEncoder::from_contract(self.device.queue().create_encoder())
    }

    /// 契約の `submit(encoder, &[])` を 1 回呼ぶ。`Ok` の印は `SubmitId` に包み、`Err` は値のまま返す。
    pub fn submit(&self, buffer: CommandBuffer<'d>) -> Result<SubmitId, ContractError> {
        self.device
            .queue()
            .submit(buffer.into_contract(), &[])
            .map(SubmitId::from_token)
    }

    /// 契約の `wait_blocking(token, timeout)` を 1 回呼び、結果を値のまま返す。
    #[cfg(feature = "native-blocking-wait")]
    pub fn wait_blocking(
        &self,
        id: SubmitId,
        timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        self.device.queue().wait_blocking(id.token(), timeout)
    }

    /// 契約の `gpu_timing(token)` を 1 回呼び、値のまま返す。ほかの操作は呼ばない。
    pub fn gpu_timing(&self, id: SubmitId) -> GpuTiming {
        self.device.queue().gpu_timing(id.token())
    }
}
