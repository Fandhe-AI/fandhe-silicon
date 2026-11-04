//! 経路が決まった入口が返す `ContractError::Unsupported` の `op`・`backend` 欄の試験（TASK-919）。
//!
//! 公開のパスだけを使い、7 つの入口が返す欄の値（入口の関数名と device の背後実装）を照らす。

mod common;

use common::Scripted;
use fandhe_silicon_core::capability::BackendKind;
use fandhe_silicon_core::device::Device;
use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_exec::{
    AtomicAddOperands, Exec, F16FmaOperands, OptIn, Outcome, SubgroupSumF32Operands,
    SubgroupSumU32Operands,
};

const CPU_OK: Option<OptIn> = Some(OptIn {
    allow_gpu_emulated: false,
    allow_cpu: true,
});

const GPU_OK: Option<OptIn> = Some(OptIn {
    allow_gpu_emulated: true,
    allow_cpu: false,
});

fn expect_fields(r: Result<Outcome, ContractError>, op: &str, backend: BackendKind) {
    match r {
        Err(ContractError::Unsupported {
            op: got_op,
            backend: got_backend,
        }) => {
            assert_eq!(got_op, op);
            assert_eq!(got_backend, backend);
        }
        _ => panic!("expected Unsupported for {op}"),
    }
}

#[test]
fn unsupported_carries_entry_name_and_device_backend() {
    let dev = Scripted::bare();
    let backend = dev.backend();
    assert_eq!(backend, BackendKind::Metal);
    let exec = Exec::deny_by_default(&dev);
    let (h16, hu, hf) = ([1u16; 2], [1u32; 2], [1.0f32; 2]);
    let (mut o16, mut ou, mut of, mut oa) = ([0u16; 2], [0u32; 1], [0.0f32; 1], [0.0f32; 2]);
    expect_fields(
        exec.f16_fma(
            GPU_OK,
            2,
            F16FmaOperands::Host {
                a: &h16,
                b: &h16,
                c: &h16,
                out: &mut o16,
            },
        ),
        "f16_fma",
        backend,
    );
    expect_fields(
        exec.subgroup_sum_u32(
            GPU_OK,
            2,
            SubgroupSumU32Operands::Host {
                x: &hu,
                out: &mut ou,
            },
        ),
        "subgroup_sum_u32",
        backend,
    );
    expect_fields(
        exec.subgroup_sum_f32(
            GPU_OK,
            2,
            SubgroupSumF32Operands::Host {
                x: &hf,
                out: &mut of,
            },
        ),
        "subgroup_sum_f32",
        backend,
    );
    expect_fields(
        exec.atomic_add(
            CPU_OK,
            2,
            1,
            AtomicAddOperands::Host {
                x: &hf,
                out: &mut oa,
            },
        ),
        "atomic_add",
        backend,
    );
    expect_fields(
        exec.external_interop_shared_memory(None),
        "external_interop_shared_memory",
        backend,
    );
    expect_fields(
        exec.hardware_ray_tracing(None),
        "hardware_ray_tracing",
        backend,
    );
    expect_fields(
        exec.unified_memory_sharing(None),
        "unified_memory_sharing",
        backend,
    );
}

#[test]
fn f16_fma_cpu_route_runs() {
    let dev = Scripted::bare();
    let exec = Exec::deny_by_default(&dev);
    let h16 = [1u16; 2];
    let mut o16 = [0u16; 2];
    let r = exec.f16_fma(
        CPU_OK,
        2,
        F16FmaOperands::Host {
            a: &h16,
            b: &h16,
            c: &h16,
            out: &mut o16,
        },
    );
    assert!(matches!(r, Ok(Outcome::Ran { .. })));
}

#[test]
fn subgroup_sum_u32_cpu_route_runs() {
    let dev = Scripted::bare();
    let exec = Exec::deny_by_default(&dev);
    let hu = [1u32; 2];
    let mut ou = [0u32; 1];
    let r = exec.subgroup_sum_u32(
        CPU_OK,
        2,
        SubgroupSumU32Operands::Host {
            x: &hu,
            out: &mut ou,
        },
    );
    assert!(matches!(r, Ok(Outcome::Ran { .. })));
}

#[test]
fn subgroup_sum_f32_cpu_route_runs() {
    let dev = Scripted::bare();
    let exec = Exec::deny_by_default(&dev);
    let hf = [1.0f32; 2];
    let mut of = [0.0f32; 1];
    let r = exec.subgroup_sum_f32(
        CPU_OK,
        2,
        SubgroupSumF32Operands::Host {
            x: &hf,
            out: &mut of,
        },
    );
    assert!(matches!(r, Ok(Outcome::Ran { .. })));
}
