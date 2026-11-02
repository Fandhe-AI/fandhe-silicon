//! 上の層の compute のパス（`lib.rs` から `pub use` される）。
//!
//! `encoder.rs` の `CommandEncoder::begin_compute_pass` が契約の記録先を受け渡して始め、
//! `end` で記録先へ戻す。

use core::marker::PhantomData;

use fandhe_silicon_core::execution::CommandEncoder as ContractEncoder;

use crate::encoder::CommandEncoder;

/// `CommandEncoder::begin_compute_pass` で始めた compute のパス。契約の記録先を値で持つ。
///
/// パスを始めた後は元の記録先を使えない。
///
/// ```compile_fail,E0382
/// use fandhe_silicon_core::device::Device;
/// use fandhe_silicon_upper::UpperDevice;
///
/// fn probe(dev: &dyn Device) {
///     let q = UpperDevice::new(dev).queue();
///     let enc = q.create_command_encoder();
///     let _pass = enc.begin_compute_pass();
///     let _buf = enc.finish();
/// }
/// ```
///
/// `end` で記録先へ戻し、`finish` して投入できる。
///
/// ```no_run
/// use fandhe_silicon_core::device::Device;
/// use fandhe_silicon_core::error::ContractError;
/// use fandhe_silicon_upper::UpperDevice;
///
/// fn probe(dev: &dyn Device) -> Result<(), ContractError> {
///     let q = UpperDevice::new(dev).queue();
///     let buf = q.create_command_encoder().begin_compute_pass().end().finish();
///     let _ = q.submit(buf)?;
///     Ok(())
/// }
/// ```
#[must_use]
#[derive(Debug)]
pub struct ComputePass<'d> {
    inner: ContractEncoder,
    _device: PhantomData<&'d ()>,
}

impl<'d> ComputePass<'d> {
    /// 契約の記録先を包む。`CommandEncoder::begin_compute_pass` から呼ぶ。
    pub(crate) fn from_contract(inner: ContractEncoder) -> Self {
        Self {
            inner,
            _device: PhantomData,
        }
    }

    /// パスを終え、記録先へ戻す。中の値を移すだけで、契約の操作は呼ばず、失敗しない。
    pub fn end(self) -> CommandEncoder<'d> {
        CommandEncoder::from_contract(self.inner)
    }
}
