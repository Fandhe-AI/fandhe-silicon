//! device の形の列（buffer）を渡した入口が buffer の中身を変えないことの試験（TASK-919）。
//! `f16_fma`・`atomic_add`・`subgroup_sum_u32`・`subgroup_sum_f32` を扱う。
//!
//! feature `native-blocking-wait` のビルドにだけ在る（`Cargo.toml` の `[[test]]`）。

mod common;

use common::Scripted;
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OptIn, Outcome, SubgroupSumF32Operands,
    SubgroupSumU32Operands,
};

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

/// 長さ `len` バイトの buffer を `fill` で埋めて作る。
fn alloc_sized(dev: &Scripted, len: u64, fill: u8) -> BufferHandle {
    let mem = dev.memory();
    let h = mem.allocate(len, BufferUsage::STORAGE).unwrap();
    mem.upload(&h, 0, &vec![fill; len as usize]).unwrap();
    h
}

#[test]
fn cpu_route_subgroup_sums_with_device_operands_are_unsupported_and_leave_buffers() {
    // 2 ブロックになる要素数。出力は 256 要素ごとに 1 つ。
    let n: u64 = 257;
    let out_len: u64 = n.div_ceil(256);
    // 入口 0: `subgroup_sum_u32`。入口 1: `subgroup_sum_f32`。
    for entry in 0..2 {
        let op = if entry == 0 {
            "subgroup_sum_u32"
        } else {
            "subgroup_sum_f32"
        };
        // 組 0: `Exec::new` に印を渡す。組 1: `Exec::deny_by_default` に呼び出しごとの上書きで渡す。
        for mode in 0..2 {
            let dev = Scripted::bare();
            let (exec, over) = if mode == 0 {
                (Exec::new(&dev, CPU_ONLY), None)
            } else {
                (Exec::deny_by_default(&dev), Some(CPU_ONLY))
            };
            let (x_bytes, out_bytes) = (n * 4, out_len * 4);
            let mut x = alloc_sized(&dev, x_bytes, 1);
            let mut out = alloc_sized(&dev, out_bytes, 2);
            let (memory_before, queue_before) = (dev.memory_calls(), dev.queue_calls());
            let r = if entry == 0 {
                exec.subgroup_sum_u32(
                    over,
                    n,
                    SubgroupSumU32Operands::Device {
                        x: &mut x,
                        out: &mut out,
                    },
                )
            } else {
                exec.subgroup_sum_f32(
                    over,
                    n,
                    SubgroupSumF32Operands::Device {
                        x: &mut x,
                        out: &mut out,
                    },
                )
            };
            let ok = match &r {
                Err(ContractError::Unsupported { op: got, .. }) => *got == op,
                _ => false,
            };
            assert!(ok, "entry {entry} mode {mode}: {r:?}");
            assert_eq!(dev.query_ex_calls(), 1, "entry {entry} mode {mode}");
            assert_eq!(
                dev.memory_calls(),
                memory_before,
                "entry {entry} mode {mode}"
            );
            assert_eq!(dev.queue_calls(), queue_before, "entry {entry} mode {mode}");
            let mem = dev.memory();
            assert_eq!(
                mem.download_blocking(&x, 0, x_bytes).unwrap(),
                vec![1u8; x_bytes as usize],
                "entry {entry} mode {mode}: x"
            );
            assert_eq!(
                mem.download_blocking(&out, 0, out_bytes).unwrap(),
                vec![2u8; out_bytes as usize],
                "entry {entry} mode {mode}: out"
            );
            mem.free(x);
            mem.free(out);
        }
    }
}
