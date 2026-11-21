//! CUDA の背後実装で、模擬のロスト（feature `fault-injection` の差し込み）の後の操作の拒否と、
//! 新しい `request` の device での往復を公開の `Device` から照らす試験（TASK-188 のポインタ）。
//!
//! どの試験も device に触るので `#[ignore]` を付ける。どの試験も本体を別のスレッドで流し、上限の時間を過ぎたら失敗とする。
#![cfg(not(target_arch = "wasm32"))]

use fandhe_silicon_contract::backend_cuda::CudaDeviceProvider;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::{
    Device, DeviceOwnership, DeviceProvider, DeviceRequest, Profile,
};
use fandhe_silicon_core::error::{
    CompletionState, CompletionStateEx, ContractError, DeviceLostReason,
};
use fandhe_silicon_core::execution::{
    Binding, BindingLayout, CommandEncoder, PipelineHandle, PipelineInput,
};
use fandhe_silicon_core::fault::{HookFault, inject};
use fandhe_silicon_core::memory::BufferUsage;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// 試験 1 本の上限。
const LIMIT: Duration = Duration::from_secs(30);

/// 各スレッドが `out[0]` に 1 を足す。`out` の先頭の 1 語だけを使う。
const COUNT: &str = "extern \"C\" __global__ void count(unsigned int* out) { atomicAdd(out, 1u); }";

const LEN: u64 = 16;

/// `download_async` の callback が受けた結果を溜める入れ物。
type Received = Arc<Mutex<Vec<Result<Vec<u8>, ContractError>>>>;

/// `body` を別のスレッドで流し、`LIMIT` を過ぎたら失敗とする。`body` の panic はそのまま送り直す。
fn within(body: impl FnOnce() + Send + 'static) {
    let (sender, receiver) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        body();
        let _ = sender.send(());
    });
    match receiver.recv_timeout(LIMIT) {
        Ok(()) => {
            let _ = handle.join();
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
            panic!("the test body ended without reporting");
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("the test body did not end within {LIMIT:?}")
        }
    }
}

fn new_device() -> Box<dyn Device> {
    match CudaDeviceProvider::request(DeviceRequest {
        backend: BackendKind::Cuda,
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

/// 公開の `Device` から `COUNT` のパイプラインを作る。配置は slot 0 だけ。
fn count_pipeline(device: &dyn Device) -> PipelineHandle {
    device
        .compute()
        .expect("a CUDA device must expose compute()")
        .create_pipeline(
            PipelineInput::CudaC {
                source: COUNT,
                entry_point: "count",
            },
            &BindingLayout::new(vec![0]),
        )
        .expect("create_pipeline")
}

// DEV-141
/// 公開の `Device` から、ロストの後に並べた操作が `DeviceLost`（`poll`・`poll_ex` は `Lost`）を返し、
/// 2 回繰り返しても同じ。dispatch を記録した記録先の `submit`・`submit_many` は `Unsupported` ではなく `DeviceLost` になる。
#[test]
#[ignore = "requires a CUDA device"]
fn the_listed_operations_are_lost_through_the_public_device() {
    within(|| {
        let device = new_device();
        let memory = device.memory();
        let queue = device.queue();
        let a = memory.allocate(LEN, rw()).expect("allocate a");
        let b = memory.allocate(LEN, rw()).expect("allocate b");
        memory.upload(&a, 0, &[7; LEN as usize]).expect("upload");
        let issued = queue.submit(queue.create_encoder(), &[]).expect("submit");
        queue.wait_blocking(issued, None).expect("wait_blocking");
        let pipeline = count_pipeline(device.as_ref());
        let mut recorded: Vec<CommandEncoder> = (0..4)
            .map(|_| {
                let mut enc = queue.create_encoder();
                queue
                    .dispatch(&mut enc, &pipeline, &[Binding::new(0, &b, 0)], [4, 1, 1])
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
            let got: Received = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&got);
            memory.download_async(&a, 0, LEN, Box::new(move |r| sink.lock().unwrap().push(r)));
            assert_eq!(
                *got.lock().unwrap(),
                [Err(ContractError::DeviceLost { generation })]
            );
            let started = Instant::now();
            assert_eq!(
                queue.wait_blocking(issued, Some(Duration::from_secs(5))),
                lost()
            );
            assert!(started.elapsed() < Duration::from_secs(5));
            assert_eq!(
                queue.dispatch(empty, &pipeline, &[Binding::new(0, &b, 0)], [4, 1, 1]),
                lost()
            );
            assert_eq!(
                memory.copy_buffer_to_buffer(empty, &a, 0, &b, 0, LEN),
                lost()
            );
            assert_eq!(memory.clear_buffer(empty, &a, 0, LEN), lost());
            assert_eq!(queue.submit(recorded.remove(0), &[]).map(|_| ()), lost());
            assert_eq!(
                queue
                    .submit_many(vec![recorded.remove(0)], &[], &[])
                    .map(|_| ()),
                lost()
            );
            assert_eq!(queue.poll(issued), CompletionState::Lost);
            assert_eq!(queue.poll_ex(issued), CompletionStateEx::Lost);
            assert_eq!(device.generation(), generation);
        }
    });
}

// DEV-147
/// ロストの後に `request` で得た新しい device は、確保・`upload`・コピーを記録した記録先の `submit`・
/// 読み戻しが通り、ロストした device の `allocate` は `DeviceLost` のまま。
#[test]
#[ignore = "requires a CUDA device"]
fn a_new_request_after_the_loss_round_trips_through_a_copy() {
    within(|| {
        let lost_device = new_device();
        lose(lost_device.as_ref());
        let lost_generation = lost_device.generation();

        let fresh = new_device();
        assert!(fresh.generation() > lost_generation);

        let memory = fresh.memory();
        let queue = fresh.queue();
        let src = memory.allocate(LEN, rw()).expect("allocate src");
        let dst = memory.allocate(LEN, rw()).expect("allocate dst");
        let bytes: Vec<u8> = (0..LEN as u8).collect();
        memory.upload(&src, 0, &bytes).expect("upload");
        let mut enc = queue.create_encoder();
        memory
            .copy_buffer_to_buffer(&mut enc, &src, 0, &dst, 0, LEN)
            .expect("copy");
        let token = queue.submit(enc, &[]).expect("submit");
        queue
            .wait_blocking(token, Some(Duration::from_secs(10)))
            .expect("wait_blocking");
        assert_eq!(
            memory.download_blocking(&dst, 0, LEN).expect("download"),
            bytes
        );
        assert_eq!(
            lost_device.memory().allocate(LEN, rw()).map(|_| ()),
            Err(ContractError::DeviceLost {
                generation: lost_device.generation()
            })
        );
    });
}
