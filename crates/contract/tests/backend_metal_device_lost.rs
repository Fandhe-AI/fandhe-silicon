//! Metal の背後実装で、模擬のロスト（feature `fault-injection` の差し込み）の後の操作の拒否と、
//! 新しい `request` の device での往復の確認（TASK-186 のポインタ）。
//!
//! device に触るので `#[ignore]` を付け、`--ignored` を付けたときだけ走る。macOS 以外ではこのファイルは 0 件になる。

#![cfg(target_os = "macos")]

use fandhe_silicon_contract::backend_metal::MetalDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, DeviceLostReason,
};
use fandhe_silicon_core::execution::{
    Binding, BindingLayout, CommandEncoder, DType, OperandSpec, PipelineHandle, PipelineInput,
};
use fandhe_silicon_core::fault::{HookFault, inject};
use fandhe_silicon_core::memory::BufferUsage;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const COPY_KERNEL: &str = "#include <metal_stdlib>\n\
    using namespace metal;\n\
    kernel void copy_u32(device const uint* src [[buffer(0)]],\n\
                            device uint* dst [[buffer(1)]],\n\
                            uint gid [[thread_position_in_grid]]) {\n\
        dst[gid] = src[gid];\n\
    }\n";

const LEN: u64 = 16;

/// `download_async` の callback が受けた結果を溜める入れ物。
type Received = Arc<Mutex<Vec<Result<Vec<u8>, ContractError>>>>;

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

fn rw() -> BufferUsage {
    BufferUsage::STORAGE | BufferUsage::UPLOAD | BufferUsage::DOWNLOAD
}

fn lose(device: &dyn Device) {
    inject(
        device.fault_hooks(),
        HookFault::DeviceLost(DeviceLostReason::Unknown),
    )
    .expect("loss");
}

/// 公開の `Device` から `COPY_KERNEL` のパイプラインを作る。
fn copy_pipeline(device: &dyn Device) -> PipelineHandle {
    device
        .compute()
        .expect("a Metal device must expose compute()")
        .create_pipeline(
            PipelineInput::Msl {
                source: COPY_KERNEL,
                entry_point: "copy_u32",
            },
            &BindingLayout::new(vec![0, 1]),
        )
        .expect("create_pipeline")
}

/// DEV-141 のポインタ。公開の `Device` から、ロストの後の 8 操作と同じ入口の 5 操作が `DeviceLost` を返し、
/// 2 回繰り返しても同じ。dispatch を記録した記録先の `submit` は、製品のビルドでも `Unsupported` ではなく `DeviceLost` になる。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn the_listed_operations_are_lost_through_the_public_device() {
    let device = new_device();
    let memory = device.memory();
    let queue = device.queue();
    let a = memory.allocate(LEN, rw()).expect("allocate a");
    let b = memory.allocate(LEN, rw()).expect("allocate b");
    memory.upload(&a, 0, &[7; LEN as usize]).expect("upload");
    let issued = queue.submit(queue.create_encoder(), &[]).expect("submit");
    queue.wait_blocking(issued, None).expect("wait_blocking");
    let pipeline = copy_pipeline(device.as_ref());
    let mut recorded: Vec<CommandEncoder> = (0..4)
        .map(|_| {
            let mut enc = queue.create_encoder();
            queue
                .dispatch(
                    &mut enc,
                    &pipeline,
                    &[Binding::new(0, &a, 0), Binding::new(1, &b, 0)],
                    [4, 1, 1],
                )
                .expect("dispatch");
            enc
        })
        .collect();
    let mut empties = [queue.create_encoder(), queue.create_encoder()];

    lose(device.as_ref());
    let generation = device.generation();
    let lost = || Err(ContractError::DeviceLost { generation });

    for empty in &mut empties {
        assert_eq!(memory.allocate(LEN, rw()).map(|_| ()), lost());
        assert_eq!(memory.upload(&a, 0, &[7; LEN as usize]), lost());
        assert_eq!(memory.download_blocking(&a, 0, LEN).map(|_| ()), lost());
        let started = Instant::now();
        assert_eq!(
            queue.wait_blocking(issued, Some(Duration::from_secs(5))),
            lost()
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(
            queue.dispatch(
                empty,
                &pipeline,
                &[Binding::new(0, &a, 0), Binding::new(1, &b, 0)],
                [4, 1, 1],
            ),
            lost()
        );
        assert_eq!(queue.submit(recorded.remove(0), &[]).map(|_| ()), lost());

        let got: Received = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&got);
        memory.download_async(&a, 0, LEN, Box::new(move |r| sink.lock().unwrap().push(r)));
        assert_eq!(
            *got.lock().unwrap(),
            [Err(ContractError::DeviceLost { generation })]
        );
        assert_eq!(
            queue
                .submit_many(vec![recorded.remove(0)], &[], &[])
                .map(|_| ()),
            lost()
        );
        let shape = [4_u64];
        let stride = [1_i64];
        let declared = [
            OperandSpec {
                slot: 0,
                dtype: DType::U32,
                shape: &shape,
                stride: &stride,
                alignment: 4,
            },
            OperandSpec {
                slot: 1,
                dtype: DType::U32,
                shape: &shape,
                stride: &stride,
                alignment: 4,
            },
        ];
        assert_eq!(
            queue.dispatch_checked(
                empty,
                &pipeline,
                &[Binding::new(0, &a, 0), Binding::new(1, &b, 0)],
                &[],
                &declared,
                [4, 1, 1],
                [1, 1, 1],
                &[],
            ),
            lost()
        );
        assert_eq!(
            memory.copy_buffer_to_buffer(empty, &a, 0, &b, 0, LEN),
            lost()
        );
        assert_eq!(memory.clear_buffer(empty, &a, 0, LEN), lost());
        assert_eq!(queue.poll(issued), CompletionState::Lost);
        assert_eq!(queue.poll_ex(issued), CompletionStateEx::Lost);
        assert_eq!(device.generation(), generation);
    }
}

/// DEV-147 のポインタ。ロストの後に `request` で得た新しい device は、確保・`upload`・空の記録先の `submit`・
/// 読み戻しが通り、ロストした device の `allocate` は `DeviceLost` のまま。
#[test]
#[ignore = "touches the Metal device; run only in a dedicated device session"]
fn a_new_request_after_the_loss_round_trips_through_an_empty_submit() {
    let lost_device = new_device();
    lose(lost_device.as_ref());
    let lost_generation = lost_device.generation();

    let fresh = new_device();
    assert!(fresh.generation() > lost_generation);

    let memory = fresh.memory();
    let queue = fresh.queue();
    let handle = memory.allocate(LEN, rw()).expect("allocate");
    let bytes: Vec<u8> = (0..LEN as u8).collect();
    memory.upload(&handle, 0, &bytes).expect("upload");
    let token = queue.submit(queue.create_encoder(), &[]).expect("submit");
    queue
        .wait_blocking(token, Some(Duration::from_secs(10)))
        .expect("wait_blocking");
    assert_eq!(
        memory.download_blocking(&handle, 0, LEN).expect("download"),
        bytes
    );
    assert_eq!(
        lost_device.memory().allocate(LEN, rw()).map(|_| ()),
        Err(ContractError::DeviceLost {
            generation: lost_device.generation()
        })
    );
}
