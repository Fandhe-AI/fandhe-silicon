//! `ext-interop` つきの Metal の device の拡張アクセサ（相互運用・物理 GPU の識別子）の確認
//! （TASK-195 のポインタ。DEV-79・DEV-216。同一 GPU の判定は DEV-76）。
//!
//! Metal の `Device` は `external_interop()`・`physical_gpu_id()` を上書きしないので、どちらも
//! `Device` の既定の `None` を返す。この値と、`None` が同一 GPU の判定で `Undeterminable` の拒否に
//! なり、取り込みが呼ばれないことを、Metal の device を実際に作って確かめる。
//!
//! device に触る試験は `#[ignore]` を付け、専用の実機の回でだけ流す。device に触らない試験は
//! どの OS でも流れる。

use fandhe_silicon_core::error::{ContractError, IdentityMismatchReason};
use fandhe_silicon_core::interop::{PhysicalGpuId, check_same_physical_gpu};

fn undeterminable() -> ContractError {
    ContractError::DeviceIdentityMismatch {
        reason: IdentityMismatchReason::Undeterminable,
    }
}

/// 識別子が取れない側が 1 つでもあれば、`None` は一致にならず `Undeterminable` で拒まれる。
#[test]
fn a_missing_identity_is_never_a_match() {
    let id = PhysicalGpuId {
        device_uuid: [1; 16],
        driver_uuid: None,
    };
    assert_eq!(
        check_same_physical_gpu(None, Some(&id)).unwrap_err(),
        undeterminable()
    );
    assert_eq!(
        check_same_physical_gpu(Some(&id), None).unwrap_err(),
        undeterminable()
    );
    assert_eq!(
        check_same_physical_gpu(None, None).unwrap_err(),
        undeterminable()
    );
}

#[cfg(target_os = "macos")]
mod device {
    use super::undeterminable;
    use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
    use fandhe_silicon_core::capability::BackendKind;
    use fandhe_silicon_core::device::{
        Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
    };
    use fandhe_silicon_core::error::ContractError;
    use fandhe_silicon_core::execution::{ImportedSemaphoreHandle, SubmitToken};
    use fandhe_silicon_core::interop::{
        ExternalInterop, ExternalMemoryHandle, ExternalSemaphoreHandle, PhysicalGpuId,
        check_same_physical_gpu, import_memory_checked,
    };
    use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
    use std::cell::Cell;

    fn new_device() -> Box<dyn Device> {
        match MetalDeviceProvider::request(DeviceRequest {
            backend: BackendKind::Metal,
            profile: Profile::ComputeOnly,
            ownership: DeviceOwnership::ProductIndependent,
        }) {
            Ok(d) => d,
            Err(e) => panic!("request failed: {e:?}"),
        }
    }

    /// `import_memory` の呼び出しを数えるスタブ。Metal の device は `ExternalInterop` を持たないので、
    /// 取り込み前の拒否が取り込みを呼ばないことは、このスタブを `import_memory_checked` に渡して確かめる。
    struct CountingInterop {
        import_calls: Cell<u32>,
    }

    impl ExternalInterop for CountingInterop {
        fn export_memory(&self, _: &BufferHandle) -> Result<ExternalMemoryHandle, ContractError> {
            unimplemented!()
        }

        fn import_memory(
            &self,
            external: ExternalMemoryHandle,
            byte_len: u64,
            usage: BufferUsage,
        ) -> Result<BufferHandle, ContractError> {
            self.import_calls.set(self.import_calls.get() + 1);
            Ok(BufferHandle::__backend_new(
                1,
                external.__backend_id(),
                1,
                byte_len,
                usage,
            ))
        }

        fn export_semaphore(
            &self,
            _: SubmitToken,
        ) -> Result<ExternalSemaphoreHandle, ContractError> {
            unimplemented!()
        }

        fn import_semaphore(
            &self,
            _: ExternalSemaphoreHandle,
        ) -> Result<ImportedSemaphoreHandle, ContractError> {
            unimplemented!()
        }
    }

    fn other_gpu() -> PhysicalGpuId {
        PhysicalGpuId {
            device_uuid: [7; 16],
            driver_uuid: Some([9; 16]),
        }
    }

    /// DEV-79・DEV-216: `Box<dyn Device>` を通して呼ぶと、どちらも既定の `None`。
    #[test]
    #[ignore = "touches the Metal device; run only in a dedicated device session"]
    fn metal_device_reports_no_interop_and_no_physical_gpu_id() {
        let device = new_device();
        assert!(device.external_interop().is_none());
        assert!(device.physical_gpu_id().is_none());
    }

    /// DEV-216・DEV-76: Metal の device の識別子（`None`）は、どちらの側に置いても `Undeterminable`。
    #[test]
    #[ignore = "touches the Metal device; run only in a dedicated device session"]
    fn metal_physical_gpu_id_is_undeterminable_in_the_same_gpu_check() {
        let device = new_device();
        let own = device.physical_gpu_id();
        let other = other_gpu();
        assert_eq!(
            check_same_physical_gpu(own.as_ref(), Some(&other)).unwrap_err(),
            undeterminable()
        );
        assert_eq!(
            check_same_physical_gpu(Some(&other), own.as_ref()).unwrap_err(),
            undeterminable()
        );
        assert_eq!(
            check_same_physical_gpu(own.as_ref(), own.as_ref()).unwrap_err(),
            undeterminable()
        );
    }

    /// DEV-216・DEV-76: 取り込み先を Metal の device の識別子（`None`）にすると、
    /// export 元の識別子が `Some` でも拒否で、`import_memory` は 0 回。
    #[test]
    #[ignore = "touches the Metal device; run only in a dedicated device session"]
    fn import_into_a_metal_target_is_rejected_before_import_memory() {
        let device = new_device();
        let interop = CountingInterop {
            import_calls: Cell::new(0),
        };
        let handle = ExternalMemoryHandle::__backend_new(5, false, Some(other_gpu()));
        let err = import_memory_checked(
            &interop,
            device.physical_gpu_id().as_ref(),
            handle,
            32,
            BufferUsage::STORAGE,
        )
        .unwrap_err();
        assert_eq!(err, undeterminable());
        assert_eq!(interop.import_calls.get(), 0);
    }
}
