//! device の形の列（buffer）を渡した入口が buffer の中身を変えないことの試験（TASK-919）。
//!
//! feature `native-blocking-wait` のビルドにだけ在る（`Cargo.toml` の `[[test]]`）。

mod common;

use common::Scripted;
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_exec::{AtomicAddOperands, Exec, F16FmaOperands, Outcome};

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
