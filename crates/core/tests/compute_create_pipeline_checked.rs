//! `ComputeOps::create_pipeline_checked` の既定の本体の動きの確認（TASK-457 のポインタ）。
//!
//! 呼び出しを記録する実装で、宣言の照らしが `create_pipeline` より先に走ること、
//! 通れば `create_pipeline` が 1 回だけ呼ばれて結果が値のまま戻ることを固定する。device には触れない。

use std::sync::Mutex;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{
    BindingLayout, ComputeOps, DType, OperandSpec, PipelineHandle, PipelineInput,
};

/// `create_pipeline` に渡った slots を記録する実装。
struct Recording {
    calls: Mutex<Vec<Vec<u32>>>,
    ok: bool,
}

impl Recording {
    fn new(ok: bool) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            ok,
        }
    }
}

impl ComputeOps for Recording {
    fn create_pipeline(
        &self,
        _input: PipelineInput<'_>,
        layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        self.calls.lock().unwrap().push(layout.slots.clone());
        if self.ok {
            Ok(PipelineHandle::__backend_new(7, 3))
        } else {
            Err(ContractError::InvalidArgument("inner".into()))
        }
    }
}

fn wgsl() -> PipelineInput<'static> {
    PipelineInput::Wgsl {
        source: "fn main() {}",
        entry_point: "main",
    }
}

fn spec<'a>(slot: u32, shape: &'a [u64], stride: &'a [i64]) -> OperandSpec<'a> {
    OperandSpec {
        slot,
        dtype: DType::F32,
        shape,
        stride,
        alignment: 4,
    }
}

fn starts<T>(r: &Result<T, ContractError>, name: &str) -> bool {
    matches!(r, Err(ContractError::InvalidArgument(m)) if m.starts_with(name))
}

/// 件数が違う宣言は `create_pipeline` を呼ばずに `C1` で返る（BIND-60）。
#[test]
fn declared_count_mismatch_does_not_call_create_pipeline() {
    let ops = Recording::new(true);
    let declared = [spec(0, &[4], &[1])];
    let r = ops.create_pipeline_checked(wgsl(), &BindingLayout::new(vec![0, 1]), &declared);
    assert!(starts(&r, "C1"));
    assert!(ops.calls.lock().unwrap().is_empty());
}

#[test]
fn matching_declaration_calls_create_pipeline_once_and_returns_as_is() {
    let ops = Recording::new(true);
    let declared = [spec(0, &[4], &[1]), spec(1, &[2, 2], &[2, 1])];
    let r = ops
        .create_pipeline_checked(wgsl(), &BindingLayout::new(vec![0, 1]), &declared)
        .unwrap();
    assert_eq!((r.__backend_id(), r.__backend_device_id()), (7, 3));
    assert_eq!(*ops.calls.lock().unwrap(), vec![vec![0, 1]]);
}

#[test]
fn reachable_through_dyn_compute_ops() {
    let ops = Recording::new(true);
    let dynref: &dyn ComputeOps = &ops;
    let r = dynref.create_pipeline_checked(wgsl(), &BindingLayout::new(vec![]), &[]);
    assert!(r.is_ok());
    assert_eq!(ops.calls.lock().unwrap().len(), 1);
}

#[test]
fn create_pipeline_checked_signature_is_pinned() {
    let _f: for<'a, 'b, 'c, 'd> fn(
        &'a Recording,
        PipelineInput<'b>,
        &'c BindingLayout,
        &'d [OperandSpec<'d>],
    ) -> Result<PipelineHandle, ContractError> = <Recording as ComputeOps>::create_pipeline_checked;
}
