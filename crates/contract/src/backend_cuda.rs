//! CUDA の背後実装（下-1）。`backend-cuda` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! 現状は骨格のみで、チップのバインディング crate への依存は未導入（依存の追加はユーザー承認制）。
//! wasm32 ではコンパイル対象から外す（`lib.rs` の `cfg`）。
//!
//! spec との対応: TASK-5（ポインタ表記）。
//!
//! `CudaDeviceProvider` は CUDA の `DeviceProvider` の入口で、device を作る前に要求を検査する
//! （TASK-160 のポインタ）。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;

/// この provider が受け持つ背後実装。
const BACKEND: BackendKind = BackendKind::Cuda;

/// 要求の組み合わせを device を作る前に照らす。`DeviceProvider::request` から呼ばれる。
///
/// 照らす順は、背後実装、プロファイル、所有形態。最初に当たったものを返す。
/// `Graphics` は `graphics` feature の有無によらず拒む。
fn check_request(req: &DeviceRequest) -> Result<(), ContractError> {
    if req.backend != BACKEND {
        return Err(ContractError::InvalidCombination(format!(
            "CUDA provider cannot serve backend {:?}",
            req.backend
        )));
    }
    match req.profile {
        Profile::ComputeOnly => {}
        Profile::Graphics | Profile::CpuOnly => {
            return Err(ContractError::InvalidCombination(format!(
                "backend {BACKEND:?} does not serve profile {:?}",
                req.profile
            )));
        }
        _ => {
            return Err(ContractError::InvalidCombination(
                "unknown device profile".into(),
            ));
        }
    }
    match req.ownership {
        DeviceOwnership::ProductIndependent => Ok(()),
        DeviceOwnership::Shared => Err(ContractError::Unsupported {
            op: "request(DeviceOwnership::Shared)",
            backend: BACKEND,
        }),
        DeviceOwnership::Imported { .. } => Err(ContractError::Unsupported {
            op: "request(DeviceOwnership::Imported)",
            backend: BACKEND,
        }),
        _ => Err(ContractError::InvalidCombination(
            "unknown device ownership".into(),
        )),
    }
}

/// CUDA の背後実装の `DeviceProvider`。
///
/// `request` は、背後実装が CUDA でない・`Graphics`・`CpuOnly` を `InvalidCombination` で、
/// `Shared`・`Imported` を `Unsupported` で拒む。
#[derive(Debug, Clone, Copy, Default)]
pub struct CudaDeviceProvider;

impl DeviceProvider for CudaDeviceProvider {
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        check_request(&req)?;
        Err(ContractError::DeviceUnavailable(
            "CudaContext::new(0): CUDA driver is not available in this build".into(),
        ))
    }
}
