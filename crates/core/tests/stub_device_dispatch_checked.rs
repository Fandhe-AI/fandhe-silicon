//! feature `test-support` の裏の試験用の device の queue で、`CommandQueue::dispatch_checked` の
//! 既定の本体を外の crate から公開のパスだけで呼ぶ統合テスト（TASK-569 のポインタ）。
//!
//! queue の上限が device の `limits()` と一致すること、確保したハンドルの大きさを超える束縛が
//! 先頭 `C4:` で、上限を 1 バイト超える宣言が先頭 `C8:` で拒否されること、通った入力では
//! `dispatch_ex` の結果がそのまま返ることを固定する。機材には触れない。

use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{Binding, CommandQueue, DType, OperandSpec, PipelineHandle};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_core::stub_device::StubDeviceProvider;

const BACKENDS: [BackendKind; 3] = [BackendKind::Metal, BackendKind::Vulkan, BackendKind::Cuda];

/// 指定の `BackendKind` の試験用の device を作る。
fn device_for(backend: BackendKind) -> Box<dyn Device> {
    StubDeviceProvider::request(DeviceRequest {
        backend,
        profile: Profile::ComputeOnly,
        ownership: DeviceOwnership::Shared,
    })
    .expect("device")
}

/// 1 次元の宣言（stride 1・alignment 0）。
fn spec<'a>(slot: u32, dtype: DType, shape: &'a [u64]) -> OperandSpec<'a> {
    OperandSpec {
        slot,
        dtype,
        shape,
        stride: &[1],
        alignment: 0,
    }
}

/// 定数・共有メモリなしで `dispatch_checked` を呼ぶ。
fn run(
    q: &dyn CommandQueue,
    bindings: &[Binding<'_>],
    declared: &[OperandSpec<'_>],
) -> Result<(), ContractError> {
    let mut encoder = q.create_encoder();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    q.dispatch_checked(
        &mut encoder,
        &pipeline,
        bindings,
        &[],
        declared,
        [1, 1, 1],
        [1, 1, 1],
        &[],
    )
}

/// 結果が `InvalidArgument` で、文言が `prefix` で始まることを確かめる。
fn assert_invalid_prefix(result: Result<(), ContractError>, prefix: &str) {
    assert!(
        matches!(&result, Err(ContractError::InvalidArgument(m)) if m.starts_with(prefix)),
        "expected InvalidArgument starting with {prefix:?}, got {result:?}"
    );
}

/// queue の上限は device の `limits()` と一致する（BIND-53）。
#[test]
fn stub_queue_max_buffer_bytes_matches_device_limits() {
    for backend in BACKENDS {
        let d = device_for(backend);
        assert_eq!(
            d.queue().max_buffer_bytes(),
            d.limits().max_buffer_bytes,
            "{backend:?}"
        );
    }
}

/// 確保した大きさを超える束縛は `C4:` で拒否される（BIND-53）。
#[test]
fn stub_queue_rejects_binding_larger_than_allocated_size_with_c4() {
    for backend in BACKENDS {
        let d = device_for(backend);
        let buf = d
            .memory()
            .allocate(16, BufferUsage::STORAGE)
            .expect("allocate");
        {
            let bindings = [Binding::new(0, &buf, 0)];
            let declared = [spec(0, DType::F32, &[100])];
            assert_invalid_prefix(run(d.queue(), &bindings, &declared), "C4:");
        }
        d.memory().free(buf);
    }
}

/// 上限を 1 バイト超える宣言は `C8:` で拒否され、上限ちょうどは検査を通る（BIND-53）。
#[test]
fn stub_queue_rejects_one_byte_over_limit_with_c8() {
    for backend in BACKENDS {
        let d = device_for(backend);
        let limit = d.limits().max_buffer_bytes;
        let over = limit.checked_add(1).expect("limit + 1");

        let over_shape = [over];
        let declared = [spec(0, DType::Bool, &over_shape)];
        assert_invalid_prefix(run(d.queue(), &[], &declared), "C8:");

        let exact_shape = [limit];
        let exact = [spec(0, DType::Bool, &exact_shape)];
        assert_eq!(
            run(d.queue(), &[], &exact),
            Err(ContractError::Unsupported {
                op: "dispatch_ex",
                backend
            })
        );
    }
}

/// 通った入力では `dispatch_ex` の結果がそのまま返る（BIND-53）。
#[test]
fn stub_queue_passes_through_dispatch_ex_result() {
    for backend in BACKENDS {
        let d = device_for(backend);
        let q = d.queue();
        let buf = d
            .memory()
            .allocate(16, BufferUsage::STORAGE)
            .expect("allocate");
        {
            let bindings = [Binding::new(0, &buf, 0)];
            let declared = [spec(0, DType::F32, &[4])];
            let checked = run(q, &bindings, &declared);

            let mut encoder = q.create_encoder();
            let pipeline = PipelineHandle::__backend_new(1, 1);
            let direct = q.dispatch_ex(
                &mut encoder,
                &pipeline,
                &bindings,
                &[],
                [1, 1, 1],
                [1, 1, 1],
                &[],
            );
            assert_eq!(checked, direct);
        }
        d.memory().free(buf);
    }
}

/// C4 の容量はハンドルの大きさで決まる（BIND-53）。
#[test]
fn stub_queue_c4_uses_handle_size() {
    let d = device_for(BackendKind::Vulkan);
    let buf = BufferHandle::__backend_new(0, 1, 1, 4, BufferUsage::STORAGE);
    let bindings = [Binding::new(0, &buf, 0)];
    let declared = [spec(0, DType::F32, &[4])];
    assert_invalid_prefix(run(d.queue(), &bindings, &declared), "C4:");
}
