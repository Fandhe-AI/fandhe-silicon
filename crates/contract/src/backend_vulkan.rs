//! Vulkan の背後実装（下-1）。`backend-vulkan` feature で有効になる。
//!
//! この背後実装の `unsafe` / FFI はこのモジュールに閉じ込め、外へは safe な API だけを出す。
//! 現状は骨格のみで、チップのバインディング crate への依存は未導入（依存の追加はユーザー承認制）。
//! wasm32 ではコンパイル対象から外す（`lib.rs` の `cfg`）。
//! 子モジュール `spirv_operands` は検証用の道具で、feature `test-support` の裏にだけ公開される
//! （TASK-98。説明は `docs/vulkan-operand-extraction.md`）。
//!
//! 公開の入口 `VulkanDeviceProvider` は Vulkan の `DeviceProvider` で、device を作る前に要求の
//! 組み合わせを照らす（TASK-159 のポインタ。説明は `docs/vulkan-device-request.md`）。
//!
//! spec との対応: TASK-5（ポインタ表記）。

#[cfg(feature = "test-support")]
pub mod spirv_operands;

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;

/// この provider が受け持つ背後実装。
const BACKEND: BackendKind = BackendKind::Vulkan;

/// device を作る前に要求を照らす。背後実装 → プロファイル → 所有形態の順で、最初に当たったものを返す。
///
/// `VulkanDeviceProvider::request` から呼ばれ、native の資源には触れない。
fn check_request(req: &DeviceRequest) -> Result<(), ContractError> {
    if req.backend != BACKEND {
        return Err(ContractError::InvalidCombination(format!(
            "Vulkan provider cannot serve backend {:?}",
            req.backend
        )));
    }
    match req.profile {
        Profile::ComputeOnly => {}
        Profile::Graphics => {
            #[cfg(not(feature = "graphics"))]
            return Err(ContractError::InvalidCombination(
                "profile Graphics requires the `graphics` feature of the contract crate".into(),
            ));
        }
        Profile::CpuOnly => {
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

/// Vulkan の `DeviceProvider` の入口。
///
/// Vulkan でない背後実装・`CpuOnly`・feature `graphics` の無いビルドの `Graphics` は
/// `InvalidCombination`、`Shared`・`Imported` は `Unsupported` で拒む。
#[derive(Debug, Clone, Copy, Default)]
pub struct VulkanDeviceProvider;

impl DeviceProvider for VulkanDeviceProvider {
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        check_request(&req)?;
        // device の取得は次の段が置き換える。
        Err(ContractError::DeviceUnavailable(
            "Entry::load: Vulkan loader is not available in this build".into(),
        ))
    }
}
