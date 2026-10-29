//! 同一の物理 GPU の判定と、判定を通してから外部メモリを取り込む入口。
//!
//! 上と下-2 は `check_same_physical_gpu` で 2 つの識別子を照らし、背後実装の取り込み
//! （`ExternalInterop::import_memory`）の前に拒否できる。`import_memory_checked` は、
//! 取り込み先の識別子とハンドルが運ぶ export 元の識別子を照らし、一致したときだけ
//! `import_memory` を呼ぶ。どちらも SDK を呼ばない純粋な関数で、機材を要しない。

use super::{ExternalInterop, ExternalMemoryHandle, PhysicalGpuId};
use crate::error::{ContractError, IdentityMismatchReason};
use crate::memory::{BufferHandle, BufferUsage};

/// 2 つの物理 GPU の識別子が同じ物理 GPU を指すかを照らす。
///
/// 規則（fail-closed）:
/// - どちらかが `None` なら判定できないので `Undeterminable` で拒む。`None` を一致とはしない。
/// - `device_uuid` が 1 バイトでも違えば `Mismatch`。
/// - `device_uuid` が同じで、両方の `driver_uuid` が `Some` かつ値が違えば `Mismatch`。
/// - `driver_uuid` が片方でも `None` なら `driver_uuid` は比べず、`device_uuid` の一致で `Ok(())`。
///   この組では driver の違いを検出しない。
/// - `device_uuid` は 16 バイトの比較だけで、全ゼロでも一致として扱う（値の妥当性は見ない）。
///
/// 戻りは `ContractError::DeviceIdentityMismatch { reason }`。呼び出し元は `Err` のとき取り込みを呼ばない。
pub fn check_same_physical_gpu(
    left: Option<&PhysicalGpuId>,
    right: Option<&PhysicalGpuId>,
) -> Result<(), ContractError> {
    let (Some(left), Some(right)) = (left, right) else {
        return Err(ContractError::DeviceIdentityMismatch {
            reason: IdentityMismatchReason::Undeterminable,
        });
    };
    let driver_differs = matches!(
        (left.driver_uuid, right.driver_uuid),
        (Some(a), Some(b)) if a != b
    );
    if left.device_uuid != right.device_uuid || driver_differs {
        return Err(ContractError::DeviceIdentityMismatch {
            reason: IdentityMismatchReason::Mismatch,
        });
    }
    Ok(())
}

/// export 元と取り込み先の識別子を照らしてから `interop.import_memory` を呼ぶ入口。
///
/// `target_gpu` は取り込み先 device の `Device::physical_gpu_id` の値、照合の相手は
/// `handle` が運ぶ export 元の値。`source_gpu` は export 元が付けた値で、この関数は真正性を確かめない。
/// `check_same_physical_gpu` が `Err` のときは
/// `interop.import_memory` を呼ばずにその `Err` を返す。ハンドルは値で受けて消費するので、
/// 拒否のときも呼び手へは戻らず、fd はハンドルとともに閉じる。
/// この関数を使わずに `ExternalInterop::import_memory` を直接呼ぶ呼び出しは、判定されない。
pub fn import_memory_checked(
    interop: &dyn ExternalInterop,
    target_gpu: Option<&PhysicalGpuId>,
    handle: ExternalMemoryHandle,
    byte_len: u64,
    usage: BufferUsage,
) -> Result<BufferHandle, ContractError> {
    check_same_physical_gpu(handle.__backend_source_gpu(), target_gpu)?;
    interop.import_memory(handle, byte_len, usage)
}
