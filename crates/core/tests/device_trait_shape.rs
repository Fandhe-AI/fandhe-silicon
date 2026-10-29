//! `Device`・`DeviceProvider`・`DeviceDiagnostics` の形を別の crate から固定する公開テスト（TASK-158。DEV-1・DEV-16・DEV-67・DEV-68 のポインタのみ）。
//!
//! 試験用のダミーの device と provider はこのファイルの中だけにあり、公開 API ではない。機材には触れない。
//! 確かめるもの: `Box<dyn Device>` を作れること（オブジェクト安全）、`request` の戻り値の型、
//! 診断の値の読み取りと `Clone`・`PartialEq`、`Device` 越しに `Capabilities` の method と `memory()` を呼べること。

use fandhe_silicon_core::capability::{
    BackendKind, Capabilities, CapabilityEvidence, CapabilityLevel, CapabilityQuery, FeatureId,
    LimitsTable,
};
use fandhe_silicon_core::device::{
    Device, DeviceDiagnostics, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage, MemoryOps};

const LIMITS: LimitsTable = LimitsTable {
    max_buffer_bytes: 1 << 20,
    max_dispatch_dims: [64, 64, 64],
    max_texture_size: 2048,
};

/// 受けた引数をそのままハンドルへ渡す最小のメモリ操作。記憶域は確保しない。
struct DummyMemory;

impl MemoryOps for DummyMemory {
    fn allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError> {
        Ok(BufferHandle::__backend_new(0, 1, 1, byte_len, usage))
    }

    fn upload(&self, _: &BufferHandle, _: u64, _: &[u8]) -> Result<(), ContractError> {
        Ok(())
    }

    #[cfg(feature = "native-blocking-wait")]
    fn download_blocking(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
    ) -> Result<Vec<u8>, ContractError> {
        Ok(Vec::new())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>) + Send>,
    ) {
        callback(Ok(Vec::new()));
    }

    #[cfg(target_arch = "wasm32")]
    fn download_async(
        &self,
        _: &BufferHandle,
        _: u64,
        _: u64,
        callback: Box<dyn FnOnce(Result<Vec<u8>, ContractError>)>,
    ) {
        callback(Ok(Vec::new()));
    }

    fn free(&self, _: BufferHandle) {}
}

/// 要求の所有形態をそのまま持つダミーの device。
struct DummyDevice {
    ownership: DeviceOwnership,
    memory: DummyMemory,
}

impl Capabilities for DummyDevice {
    fn query(&self, _: FeatureId) -> CapabilityLevel {
        CapabilityLevel::S1
    }

    fn limits(&self) -> &LimitsTable {
        &LIMITS
    }
}

impl Device for DummyDevice {
    fn backend(&self) -> BackendKind {
        BackendKind::Metal
    }

    fn ownership(&self) -> &DeviceOwnership {
        &self.ownership
    }

    fn generation(&self) -> u64 {
        1
    }

    fn memory(&self) -> &dyn MemoryOps {
        &self.memory
    }

    fn diagnostics(&self) -> DeviceDiagnostics {
        DeviceDiagnostics {
            adapter_name: "dummy-adapter".to_string(),
            backend_name: "dummy-backend",
        }
    }
}

/// 組み合わせが成り立たない要求と、作れない要求を値の種類で返し分けるダミーの provider。
struct DummyProvider;

impl DeviceProvider for DummyProvider {
    fn request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError> {
        match (req.profile, req.ownership) {
            (Profile::CpuOnly, DeviceOwnership::Shared) => {
                Err(ContractError::InvalidCombination("dummy".to_string()))
            }
            (Profile::Graphics, _) => Err(ContractError::DeviceUnavailable("dummy".to_string())),
            (_, ownership) => Ok(Box::new(DummyDevice {
                ownership,
                memory: DummyMemory,
            })),
        }
    }
}

fn request(profile: Profile, ownership: DeviceOwnership) -> DeviceRequest {
    DeviceRequest {
        backend: BackendKind::Metal,
        profile,
        ownership,
    }
}

#[test]
fn request_returns_boxed_dyn_device() {
    // 関数ポインタへの代入で、戻り値の型が `Result<Box<dyn Device>, ContractError>` であることを固定する。
    let f: fn(DeviceRequest) -> Result<Box<dyn Device>, ContractError> = DummyProvider::request;
    let device: Box<dyn Device> = f(request(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    ))
    .expect("device");
    assert_eq!(device.backend(), BackendKind::Metal);
    assert_eq!(device.generation(), 1);
    assert!(matches!(
        device.ownership(),
        DeviceOwnership::ProductIndependent
    ));
}

#[test]
fn request_failures_are_distinguished_by_variant() {
    let invalid = DummyProvider::request(request(Profile::CpuOnly, DeviceOwnership::Shared));
    assert!(matches!(invalid, Err(ContractError::InvalidCombination(_))));
    let unavailable = DummyProvider::request(request(
        Profile::Graphics,
        DeviceOwnership::ProductIndependent,
    ));
    assert!(matches!(
        unavailable,
        Err(ContractError::DeviceUnavailable(_))
    ));
}

#[test]
fn diagnostics_fields_read_clone_and_compare() {
    let device = DummyProvider::request(request(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    ))
    .expect("device");
    let d = device.diagnostics();
    let DeviceDiagnostics {
        adapter_name,
        backend_name,
    } = d.clone();
    let _: String = adapter_name;
    let _: &'static str = backend_name;
    assert_eq!(d, d.clone());
    let other = DeviceDiagnostics {
        adapter_name: "other".to_string(),
        backend_name: d.backend_name,
    };
    assert_ne!(d, other);
    let other_backend = DeviceDiagnostics {
        backend_name: "other-backend",
        ..d.clone()
    };
    assert_ne!(d, other_backend);
    assert_eq!(d.adapter_name, "dummy-adapter");
    assert_eq!(d.backend_name, "dummy-backend");
}

#[test]
fn device_exposes_capabilities_and_memory() {
    let device = DummyProvider::request(request(
        Profile::ComputeOnly,
        DeviceOwnership::ProductIndependent,
    ))
    .expect("device");
    let feature = FeatureId("any");
    assert_eq!(device.query(feature), CapabilityLevel::S1);
    assert_eq!(device.limits(), &LIMITS);
    assert_eq!(
        device.query_ex(feature),
        CapabilityQuery {
            level: CapabilityLevel::S1,
            unsupported_reason: None,
            evidence: None::<CapabilityEvidence>,
        }
    );
    let buf = device
        .memory()
        .allocate(16, BufferUsage::STORAGE)
        .expect("allocate");
    assert_eq!(buf.size(), 16);
    device.memory().free(buf);
}
