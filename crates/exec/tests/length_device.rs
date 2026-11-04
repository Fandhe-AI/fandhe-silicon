//! device の形の列（buffer）を渡した入口が、長さ 0・出力の数 0・上限超過・バイト数のあふれで
//! `InvalidArgument` を返し、確保・読み出しの数えと能力の問合せが増えず、buffer の中身が変わらないことの試験
//! （TASK-924）。feature `native-blocking-wait` のビルドにだけ在る（`Cargo.toml` の `[[test]]`）。
//! 置き場の説明は docs/exec-length-check.md。

mod common;

use common::Scripted;
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OpId, Outcome, SubgroupSumF32Operands,
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

/// 入口を device の形の buffer で 1 回呼ぶ。`m` は `atomic_add` だけが使う。
fn call(
    exec: &Exec<'_>,
    op: OpId,
    n: u64,
    m: u64,
    bufs: &mut [BufferHandle],
) -> Result<Outcome, ContractError> {
    let mut it = bufs.iter_mut();
    match op {
        OpId::F16Fma => {
            let (a, b, c, out) = (
                it.next().unwrap(),
                it.next().unwrap(),
                it.next().unwrap(),
                it.next().unwrap(),
            );
            exec.f16_fma(None, n, F16FmaOperands::Device { a, b, c, out })
        }
        OpId::SubgroupSumU32 => {
            let (x, out) = (it.next().unwrap(), it.next().unwrap());
            exec.subgroup_sum_u32(None, n, SubgroupSumU32Operands::Device { x, out })
        }
        OpId::SubgroupSumF32 => {
            let (x, out) = (it.next().unwrap(), it.next().unwrap());
            exec.subgroup_sum_f32(None, n, SubgroupSumF32Operands::Device { x, out })
        }
        OpId::AtomicAdd => {
            let (x, out) = (it.next().unwrap(), it.next().unwrap());
            exec.atomic_add(None, n, m, AtomicAddOperands::Device { x, out })
        }
        _ => unreachable!(),
    }
}

/// 拒否される入力 (入口, n, m, 上限)。
fn rejected_inputs() -> Vec<(OpId, u64, u64, u64)> {
    let ops = [
        OpId::F16Fma,
        OpId::SubgroupSumU32,
        OpId::SubgroupSumF32,
        OpId::AtomicAdd,
    ];
    let mut v = Vec::new();
    for op in ops {
        let m = u64::from(op == OpId::AtomicAdd);
        let over = if op == OpId::F16Fma { 33 } else { 17 };
        v.push((op, 0, m, 1 << 28));
        v.push((op, over, m, 64));
        v.push((op, u64::MAX, m, u64::MAX));
    }
    v.push((OpId::AtomicAdd, 4, 0, 1 << 28));
    v
}

#[test]
fn device_form_rejects_lengths_without_touching_buffers() {
    for (op, n, m, limit) in rejected_inputs() {
        let dev = Scripted::bare().with_max_buffer_bytes(limit);
        let exec = Exec::deny_by_default(&dev);
        let count = if op == OpId::F16Fma { 4 } else { 2 };
        let mut bufs: Vec<BufferHandle> = (1..=count).map(|i| alloc(&dev, i)).collect();

        let before = dev.memory_calls();
        let r = call(&exec, op, n, m, &mut bufs);
        let after = dev.memory_calls();

        let tag = format!("{op:?} {n} {m} {limit}");
        assert!(
            matches!(r, Err(ContractError::InvalidArgument(_))),
            "{tag}: {r:?}"
        );
        assert_eq!(before, after, "{tag}");
        assert_eq!(dev.query_ex_calls(), 0, "{tag}");
        for (i, h) in bufs.iter().enumerate() {
            assert_eq!(
                read(&dev, h),
                vec![i as u8 + 1; LEN as usize],
                "{tag} buffer {i}"
            );
        }
        for h in bufs {
            dev.memory().free(h);
        }
    }
}

#[test]
fn device_form_passes_one_before_byte_overflow() {
    let dev = Scripted::bare().with_max_buffer_bytes(u64::MAX);
    let exec = Exec::deny_by_default(&dev);
    let mut bufs: Vec<BufferHandle> = (1..=4).map(|i| alloc(&dev, i)).collect();

    let before = dev.memory_calls();
    let r = call(&exec, OpId::F16Fma, u64::MAX / 2, 0, &mut bufs);
    let after = dev.memory_calls();

    assert!(
        !matches!(r, Err(ContractError::InvalidArgument(_))),
        "{r:?}"
    );
    assert_eq!(dev.query_ex_calls(), 1);
    assert_eq!(before, after);
    for (i, h) in bufs.iter().enumerate() {
        assert_eq!(read(&dev, h), vec![i as u8 + 1; LEN as usize], "buffer {i}");
    }
    for h in bufs {
        dev.memory().free(h);
    }
}

#[test]
fn device_form_subgroup_passes_one_before_byte_overflow() {
    for op in [OpId::SubgroupSumU32, OpId::SubgroupSumF32] {
        let dev = Scripted::bare().with_max_buffer_bytes(u64::MAX);
        let exec = Exec::deny_by_default(&dev);
        let mut bufs: Vec<BufferHandle> = (1..=2).map(|i| alloc(&dev, i)).collect();

        let before = dev.memory_calls();
        let r = call(&exec, op, u64::MAX / 4, 0, &mut bufs);
        let after = dev.memory_calls();

        let tag = format!("{op:?}");
        assert!(
            !matches!(r, Err(ContractError::InvalidArgument(_))),
            "{tag}: {r:?}"
        );
        assert_eq!(dev.query_ex_calls(), 1, "{tag}");
        assert_eq!(before, after, "{tag}");
        for (i, h) in bufs.iter().enumerate() {
            assert_eq!(
                read(&dev, h),
                vec![i as u8 + 1; LEN as usize],
                "{tag} buffer {i}"
            );
        }
        for h in bufs {
            dev.memory().free(h);
        }
    }
}

#[test]
fn device_form_atomic_passes_one_before_byte_overflow() {
    for (n, m) in [(u64::MAX / 4, 1), (1, u64::MAX / 4)] {
        let dev = Scripted::bare().with_max_buffer_bytes(u64::MAX);
        let exec = Exec::deny_by_default(&dev);
        let mut bufs: Vec<BufferHandle> = (1..=2).map(|i| alloc(&dev, i)).collect();

        let before = dev.memory_calls();
        let r = call(&exec, OpId::AtomicAdd, n, m, &mut bufs);
        let after = dev.memory_calls();

        let tag = format!("{n} {m}");
        assert!(
            !matches!(r, Err(ContractError::InvalidArgument(_))),
            "{tag}: {r:?}"
        );
        assert_eq!(dev.query_ex_calls(), 1, "{tag}");
        assert_eq!(before, after, "{tag}");
        for (i, h) in bufs.iter().enumerate() {
            assert_eq!(
                read(&dev, h),
                vec![i as u8 + 1; LEN as usize],
                "{tag} buffer {i}"
            );
        }
        for h in bufs {
            dev.memory().free(h);
        }
    }
}
