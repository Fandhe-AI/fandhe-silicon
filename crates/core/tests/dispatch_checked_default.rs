//! `CommandQueue::dispatch_checked` の既定の本体を、背後実装を使わない最小の queue で
//! 外の crate から呼ぶ統合テスト（TASK-569 のポインタ）。
//!
//! 束縛のハンドルの `size()` を超える束縛は先頭 `C4:` の `InvalidArgument` で、
//! `max_buffer_bytes()` を超える宣言は先頭 `C8:` の `InvalidArgument` で拒否され、
//! そのとき `dispatch_ex` は呼ばれないこと、収まる入力と上限ちょうどの入力は
//! `Ok(())` で `dispatch_ex` が 1 回呼ばれることを固定する。機材には触れない。

use std::sync::Mutex;

use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{
    Binding, CommandEncoder, CommandQueue, ConstantBinding, DType, ImportedSemaphoreHandle,
    OperandSpec, PipelineHandle, SubmitToken, ThreadgroupMemory,
};
use fandhe_silicon_core::memory::{BufferHandle, BufferUsage};

/// `dispatch_ex` の呼び出し回数を数え、`max_buffer_bytes` に欄の値を返す最小の queue。
/// `dispatch_checked` は上書きしない。
struct Minimal {
    limit: u64,
    calls: Mutex<u32>,
}

impl Minimal {
    fn new(limit: u64) -> Self {
        Self {
            limit,
            calls: Mutex::new(0),
        }
    }

    fn calls(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

impl CommandQueue for Minimal {
    fn create_encoder(&self) -> CommandEncoder {
        CommandEncoder::__backend_new(1, None)
    }

    fn submit(
        &self,
        _commands: CommandEncoder,
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(0, 0, 0))
    }

    fn submit_many(
        &self,
        _commands: Vec<CommandEncoder>,
        _after: &[SubmitToken],
        _wait: &[ImportedSemaphoreHandle],
    ) -> Result<SubmitToken, ContractError> {
        Ok(SubmitToken::__backend_new(0, 0, 0))
    }

    fn poll(&self, _token: SubmitToken) -> CompletionState {
        CompletionState::Complete
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState) + Send>) {
        callback(CompletionState::Complete);
    }

    #[cfg(target_arch = "wasm32")]
    fn on_complete(&self, _token: SubmitToken, callback: Box<dyn FnOnce(CompletionState)>) {
        callback(CompletionState::Complete);
    }

    #[cfg(feature = "native-blocking-wait")]
    fn wait_blocking(
        &self,
        _token: SubmitToken,
        _timeout: Option<std::time::Duration>,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn dispatch_ex(
        &self,
        _encoder: &mut CommandEncoder,
        _pipeline: &PipelineHandle,
        _bindings: &[Binding<'_>],
        _consts: &[ConstantBinding<'_>],
        _dispatch_dims: [u32; 3],
        _threads_per_threadgroup: [u32; 3],
        _threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        *self.calls.lock().unwrap() += 1;
        Ok(())
    }

    fn max_buffer_bytes(&self) -> u64 {
        self.limit
    }
}

/// 大きさ `size` バイトのバッファのハンドル。
fn buffer(id: u64, size: u64) -> BufferHandle {
    BufferHandle::__backend_new(0, id, 1, size, BufferUsage::STORAGE)
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
    q: &Minimal,
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

/// 束縛のハンドルの大きさを超える宣言は `C4:` で拒否され、`dispatch_ex` は呼ばれない（BIND-53）。
#[test]
fn dispatch_checked_rejects_binding_larger_than_buffer_size_with_c4() {
    let q = Minimal::new(4096);
    let buf = buffer(1, 16);
    let bindings = [Binding::new(0, &buf, 0)];
    let declared = [spec(0, DType::F32, &[100])];
    assert_invalid_prefix(run(&q, &bindings, &declared), "C4:");
    assert_eq!(q.calls(), 0);
}

/// 必要バイト数が上限を 1 バイト超える宣言は `C8:` で拒否され、`dispatch_ex` は呼ばれない（BIND-53）。
#[test]
fn dispatch_checked_rejects_one_byte_over_max_buffer_bytes_with_c8() {
    let q = Minimal::new(4000);
    let declared = [spec(0, DType::Bool, &[4001])];
    assert_invalid_prefix(run(&q, &[], &declared), "C8:");
    assert_eq!(q.calls(), 0);
}

/// 必要バイト数が上限ちょうどの宣言は通り、`dispatch_ex` が 1 回呼ばれる（BIND-53）。
#[test]
fn dispatch_checked_accepts_required_bytes_equal_to_max_buffer_bytes() {
    let q = Minimal::new(4000);
    let declared = [spec(0, DType::Bool, &[4000])];
    assert_eq!(run(&q, &[], &declared), Ok(()));
    assert_eq!(q.calls(), 1);
}

/// ハンドルの大きさに収まる束縛は通り、`dispatch_ex` が 1 回呼ばれる（BIND-53）。
#[test]
fn dispatch_checked_accepts_binding_that_fits_buffer_size() {
    let q = Minimal::new(4096);
    let buf = buffer(1, 16);
    let bindings = [Binding::new(0, &buf, 0)];
    let declared = [spec(0, DType::F32, &[4])];
    assert_eq!(run(&q, &bindings, &declared), Ok(()));
    assert_eq!(q.calls(), 1);
}

/// C4 の容量は大きさから offset を引いた残りで、offset 16 は通り offset 20 は拒否される（BIND-53）。
#[test]
fn dispatch_checked_c4_uses_size_minus_offset() {
    let q = Minimal::new(4096);
    let buf = buffer(1, 32);
    let declared = [spec(0, DType::F32, &[4])];

    let fits = [Binding::new(0, &buf, 16)];
    assert_eq!(run(&q, &fits, &declared), Ok(()));
    assert_eq!(q.calls(), 1);

    let over = [Binding::new(0, &buf, 20)];
    assert_invalid_prefix(run(&q, &over, &declared), "C4:");
    assert_eq!(q.calls(), 1);
}

/// C4 は束縛ごとにその slot のハンドルの大きさで照らし、小さい slot 0 が拒否される（BIND-53）。
#[test]
fn dispatch_checked_c4_uses_each_binding_handle_size() {
    let q = Minimal::new(4096);
    let (small, large) = (buffer(1, 16), buffer(2, 400));
    let declared = [spec(0, DType::F32, &[100]), spec(1, DType::F32, &[100])];

    let mixed = [Binding::new(0, &small, 0), Binding::new(1, &large, 0)];
    let result = run(&q, &mixed, &declared);
    assert_invalid_prefix(result.clone(), "C4:");
    assert!(
        matches!(&result, Err(ContractError::InvalidArgument(m)) if m.contains("slot 0")),
        "expected slot 0 in message, got {result:?}"
    );
    assert_eq!(q.calls(), 0);

    let both_large = [Binding::new(0, &large, 0), Binding::new(1, &large, 0)];
    assert_eq!(run(&q, &both_large, &declared), Ok(()));
    assert_eq!(q.calls(), 1);
}
