//! device の形の列（buffer）を渡した入口が buffer の中身を変えないことの試験（TASK-919）。
//!
//! feature `native-blocking-wait` のビルドにだけ在る（`Cargo.toml` の `[[test]]`）。

mod common;

use common::Scripted;
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_exec::{AtomicAddOperands, Exec, F16FmaOperands, OptIn, Outcome};

const LEN: u64 = 16;

fn alloc(dev: &Scripted, fill: u8) -> BufferHandle {
    let mem = dev.memory();
    let h = mem.allocate(LEN, BufferUsage::STORAGE).unwrap();
    mem.upload(&h, 0, &[fill; LEN as usize]).unwrap();
    h
}

fn read(dev: &Scripted, h: &BufferHandle) -> Vec<u8> {
    dev.memory().download_blocking(h, 0, LEN).unwrap()
}

#[test]
fn unavailable_leaves_device_buffers_unchanged() {
    let dev = Scripted::bare();
    let exec = Exec::deny_by_default(&dev);

    let mut bufs: Vec<BufferHandle> = (1..=6).map(|i| alloc(&dev, i)).collect();
    let mut it = bufs.iter_mut();
    let (a, b, c, out) = (
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
    );
    let r = exec.f16_fma(None, 8, F16FmaOperands::Device { a, b, c, out });
    assert!(matches!(r, Ok(Outcome::Unavailable { .. })));
    let (x, out) = (it.next().unwrap(), it.next().unwrap());
    let r = exec.atomic_add(None, 4, 1, AtomicAddOperands::Device { x, out });
    assert!(matches!(r, Ok(Outcome::Unavailable { .. })));

    for (i, h) in bufs.iter().enumerate() {
        assert_eq!(read(&dev, h), vec![i as u8 + 1; LEN as usize], "buffer {i}");
    }
    for h in bufs {
        dev.memory().free(h);
    }
}

/// 経路が `Route::Cpu` に決まる印（`allow_cpu` だけを立てる）。
const CPU_ONLY: OptIn = OptIn {
    allow_gpu_emulated: false,
    allow_cpu: true,
};

#[test]
fn cpu_route_with_device_operands_is_unsupported_and_leaves_buffers() {
    // 組 0: `Exec::new` に印を渡す。組 1: `Exec::deny_by_default` に呼び出しごとの上書きで渡す。
    for mode in 0..2 {
        let dev = Scripted::bare();
        let (exec, over) = if mode == 0 {
            (Exec::new(&dev, CPU_ONLY), None)
        } else {
            (Exec::deny_by_default(&dev), Some(CPU_ONLY))
        };
        let mut bufs: Vec<BufferHandle> = (1..=4).map(|i| alloc(&dev, i)).collect();
        let (memory_before, queue_before) = (dev.memory_calls(), dev.queue_calls());
        let mut it = bufs.iter_mut();
        let (a, b, c, out) = (
            it.next().unwrap(),
            it.next().unwrap(),
            it.next().unwrap(),
            it.next().unwrap(),
        );
        let r = exec.f16_fma(over, 8, F16FmaOperands::Device { a, b, c, out });
        assert!(
            matches!(r, Err(ContractError::Unsupported { op: "f16_fma", .. })),
            "mode {mode}: {r:?}"
        );
        assert_eq!(dev.query_ex_calls(), 1, "mode {mode}");
        assert_eq!(dev.memory_calls(), memory_before, "mode {mode}");
        assert_eq!(dev.queue_calls(), queue_before, "mode {mode}");
        for (i, h) in bufs.iter().enumerate() {
            assert_eq!(read(&dev, h), vec![i as u8 + 1; LEN as usize], "buffer {i}");
        }
        for h in bufs {
            dev.memory().free(h);
        }
    }
}
