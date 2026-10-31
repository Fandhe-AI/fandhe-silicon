//! `BindingLayout` と `ComputeOps` の形の確認（TASK-769・TASK-415 のポインタ。COMP-25）。
//!
//! 呼び出しを記録する実装で、`create_pipeline` が渡された `PipelineInput` と `BindingLayout` を
//! そのまま受け取り、返した `Ok(PipelineHandle)`・`Err(..)` がそのまま戻ること、
//! `&dyn ComputeOps` として呼べること、`BindingLayout` が値をそのまま持つことを固定する。機材には触れない。

use std::sync::Mutex;

use fandhe_silicon_core::error::ContractError;
use fandhe_silicon_core::execution::{BindingLayout, ComputeOps, PipelineHandle, PipelineInput};

/// 受け取った引数の写し。(入口名, ソース, slots)。
type Call = (String, String, Vec<u32>);

/// 呼び出しを記録し、決めた結果を返す実装。
struct Recording {
    calls: Mutex<Vec<Call>>,
    /// `Some((id, device_id))` なら `Ok`、`None` なら `Err`。
    ok: Option<(u64, u64)>,
}

impl Recording {
    fn new(ok: Option<(u64, u64)>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            ok,
        }
    }
}

impl ComputeOps for Recording {
    fn create_pipeline(
        &self,
        input: PipelineInput<'_>,
        layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        let (source, entry) = match input {
            PipelineInput::Wgsl {
                source,
                entry_point,
            } => (source.to_string(), entry_point.to_string()),
            other => panic!("unexpected input: {other:?}"),
        };
        self.calls
            .lock()
            .unwrap()
            .push((entry, source, layout.slots.clone()));
        match self.ok {
            Some((id, dev)) => Ok(PipelineHandle::__backend_new(id, dev)),
            None => Err(ContractError::InvalidArgument("bad".into())),
        }
    }
}

fn wgsl() -> PipelineInput<'static> {
    PipelineInput::Wgsl {
        source: "fn main() {}",
        entry_point: "main",
    }
}

#[test]
fn passes_input_and_layout_through() {
    let r = Recording::new(Some((5, 2)));
    let layout = BindingLayout::new(vec![0, 3, 1]);
    let h = r.create_pipeline(wgsl(), &layout).unwrap();
    assert_eq!((h.__backend_id(), h.__backend_device_id()), (5, 2));
    assert_eq!(
        *r.calls.lock().unwrap(),
        vec![(
            "main".to_string(),
            "fn main() {}".to_string(),
            vec![0, 3, 1]
        )]
    );
}

#[test]
fn returns_err_unchanged() {
    let r = Recording::new(None);
    let e = r
        .create_pipeline(wgsl(), &BindingLayout::new(vec![]))
        .unwrap_err();
    assert_eq!(e, ContractError::InvalidArgument("bad".into()));
}

#[test]
fn callable_as_dyn() {
    let r = Recording::new(Some((1, 1)));
    let d: &dyn ComputeOps = &r;
    let h = d
        .create_pipeline(wgsl(), &BindingLayout::new(vec![7]))
        .unwrap();
    assert_eq!(h.__backend_id(), 1);
    assert_eq!(
        *r.calls.lock().unwrap(),
        vec![("main".to_string(), "fn main() {}".to_string(), vec![7])]
    );
}

#[test]
fn binding_layout_keeps_slots_as_given() {
    assert_eq!(BindingLayout::new(vec![2, 0, 5]).slots, vec![2, 0, 5]);
    assert!(BindingLayout::new(vec![]).slots.is_empty());
    assert_eq!(BindingLayout::new(vec![1, 1, 0, 1]).slots, vec![1, 1, 0, 1]);
}

#[test]
fn binding_layout_clone_and_eq() {
    let a = BindingLayout::new(vec![0, 1]);
    let b = a.clone();
    assert_eq!(a, b);
    assert_ne!(a, BindingLayout::new(vec![1, 0]));
}
