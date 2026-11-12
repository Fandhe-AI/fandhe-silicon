//! `CommandQueue::dispatch_checked` の既定の本体を、背後実装を使わない最小の queue で
//! 外の crate から呼ぶ統合テスト（TASK-569 のポインタ）。
//!
//! 束縛のハンドルの `size()` を超える束縛は先頭 `C4:` の `InvalidArgument` で、
//! `max_buffer_bytes()` を超える宣言は先頭 `C8:` の `InvalidArgument` で拒否され、
//! そのとき `dispatch_ex` は呼ばれないこと、収まる入力と上限ちょうどの入力は
//! `Ok(())` で `dispatch_ex` が 1 回呼ばれることを固定する。ほかの検査（C8 の長さ 0・あふれ、
//! C7・C5・C6）でも `dispatch_ex` の前に拒否されること、上限が queue の `max_buffer_bytes()` から
//! 来て C8 が C4 より先に照らされること、通った入力の引数と `dispatch_ex` の結果がそのまま渡り
//! 返ること、`dyn CommandQueue` 越しに呼べることも固定する。機材には触れない。

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

/// `dispatch_ex` が受けた引数の写し。識別子と値だけを持つ。
#[derive(Debug, PartialEq)]
struct Call {
    encoder: u64,
    pipeline: (u64, u64),
    /// (slot, offset, バッファの識別子)
    bindings: Vec<(u32, u64, u64)>,
    /// (slot, bytes の長さ)
    consts: Vec<(u32, usize)>,
    dims: [u32; 3],
    tpt: [u32; 3],
    tg_mem: Vec<ThreadgroupMemory>,
}

/// `dispatch_ex` の引数を積み、欄で決めた結果を返す queue。`dispatch_checked` は上書きしない。
struct Recording {
    limit: u64,
    calls: Mutex<Vec<Call>>,
    result: Result<(), ContractError>,
}

impl Recording {
    fn new(limit: u64, result: Result<(), ContractError>) -> Self {
        Self {
            limit,
            calls: Mutex::new(Vec::new()),
            result,
        }
    }
}

impl CommandQueue for Recording {
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
        encoder: &mut CommandEncoder,
        pipeline: &PipelineHandle,
        bindings: &[Binding<'_>],
        consts: &[ConstantBinding<'_>],
        dispatch_dims: [u32; 3],
        threads_per_threadgroup: [u32; 3],
        threadgroup_memory: &[ThreadgroupMemory],
    ) -> Result<(), ContractError> {
        self.calls.lock().unwrap().push(Call {
            encoder: encoder.__backend_id(),
            pipeline: (pipeline.__backend_id(), pipeline.__backend_device_id()),
            bindings: bindings
                .iter()
                .map(|b| (b.slot, b.offset, b.buffer.__backend_id()))
                .collect(),
            consts: consts.iter().map(|c| (c.slot, c.bytes.len())).collect(),
            dims: dispatch_dims,
            tpt: threads_per_threadgroup,
            tg_mem: threadgroup_memory.to_vec(),
        });
        self.result.clone()
    }

    fn max_buffer_bytes(&self) -> u64 {
        self.limit
    }
}

/// 定数を渡して `dispatch_checked` を呼ぶ（起動の大きさは固定・共有メモリなし）。
fn run_with_consts(
    q: &dyn CommandQueue,
    bindings: &[Binding<'_>],
    consts: &[ConstantBinding<'_>],
    declared: &[OperandSpec<'_>],
) -> Result<(), ContractError> {
    let mut encoder = q.create_encoder();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    q.dispatch_checked(
        &mut encoder,
        &pipeline,
        bindings,
        consts,
        declared,
        [1, 1, 1],
        [1, 1, 1],
        &[],
    )
}

/// C8 の長さ 0・あふれ、C7、C5、C6 の違反は、それぞれの検査の `InvalidArgument` で拒否され、
/// `dispatch_ex` は呼ばれない（BIND-53）。
#[test]
fn dispatch_checked_rejects_other_checks_before_dispatch_ex() {
    let q = Minimal::new(1 << 40);

    assert_invalid_prefix(run(&q, &[], &[spec(0, DType::F32, &[0])]), "C8:");
    assert_invalid_prefix(run(&q, &[], &[spec(0, DType::F32, &[u64::MAX])]), "C8:");

    let buf = buffer(1, 16);
    let stray = [Binding::new(9, &buf, 0)];
    assert_invalid_prefix(run(&q, &stray, &[spec(0, DType::F32, &[4])]), "C7:");

    let big = buffer(2, 64);
    let misaligned = [Binding::new(0, &big, 4)];
    let aligned = [OperandSpec {
        slot: 0,
        dtype: DType::F32,
        shape: &[4],
        stride: &[1],
        alignment: 16,
    }];
    assert_invalid_prefix(run(&q, &misaligned, &aligned), "C5:");

    let bytes = [0u8; 8];
    let consts = [ConstantBinding::new(0, &bytes)];
    let result = run_with_consts(&q, &[], &consts, &[spec(0, DType::U32, &[3])]);
    assert_invalid_prefix(result, "C6:");

    assert_eq!(q.calls(), 0);
}

/// 上限は queue の `max_buffer_bytes()` から来て、C8 は C4 より先に照らされる（BIND-53）。
#[test]
fn dispatch_checked_checks_in_order_with_queue_limit() {
    let buf = buffer(1, 16);
    let bindings = [Binding::new(0, &buf, 0)];
    let declared = [spec(0, DType::Bool, &[4001])];

    let tight = Minimal::new(4000);
    assert_invalid_prefix(run(&tight, &bindings, &declared), "C8:");
    assert_eq!(tight.calls(), 0);

    let loose = Minimal::new(4001);
    assert_invalid_prefix(run(&loose, &bindings, &declared), "C4:");
    assert_eq!(loose.calls(), 0);
}

/// 通った入力の引数は、変えずに `dispatch_ex` へ 1 回だけ渡る（BIND-53）。
#[test]
fn dispatch_checked_passes_arguments_to_dispatch_ex_unchanged() {
    let q = Recording::new(4096, Ok(()));
    let mut encoder = CommandEncoder::__backend_new(77, None);
    let pipeline = PipelineHandle::__backend_new(9, 4);
    let (b0, b3) = (buffer(10, 64), buffer(11, 64));
    let bindings = [Binding::new(0, &b0, 0), Binding::new(3, &b3, 32)];
    let bytes = [1u8, 2, 3, 4];
    let consts = [ConstantBinding::new(5, &bytes)];
    let declared = [
        spec(0, DType::F32, &[4]),
        spec(3, DType::F32, &[4]),
        spec(5, DType::U32, &[1]),
    ];
    let tg_mem = [ThreadgroupMemory::new(0, 256)];

    let result = q.dispatch_checked(
        &mut encoder,
        &pipeline,
        &bindings,
        &consts,
        &declared,
        [3, 2, 1],
        [8, 4, 1],
        &tg_mem,
    );
    assert_eq!(result, Ok(()));

    let calls = q.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        Call {
            encoder: 77,
            pipeline: (9, 4),
            bindings: vec![(0, 0, 10), (3, 32, 11)],
            consts: vec![(5, 4)],
            dims: [3, 2, 1],
            tpt: [8, 4, 1],
            tg_mem: vec![ThreadgroupMemory::new(0, 256)],
        }
    );
}

/// `dispatch_ex` の `Err` は値のまま返る（BIND-53）。
#[test]
fn dispatch_checked_returns_dispatch_ex_error_unchanged() {
    let errors = [
        ContractError::DeviceMismatch,
        ContractError::InvalidArgument("rejected by dispatch_ex".into()),
    ];
    for err in errors {
        let q = Recording::new(4096, Err(err.clone()));
        let buf = buffer(1, 16);
        let bindings = [Binding::new(0, &buf, 0)];
        let declared = [spec(0, DType::F32, &[4])];
        let result = run_with_consts(&q, &bindings, &[], &declared);
        assert_eq!(result, Err(err));
        assert_eq!(q.calls.lock().unwrap().len(), 1);
    }
}

/// `&dyn CommandQueue` 越しでも既定の本体が検査し、拒否では `dispatch_ex` が呼ばれない（BIND-53）。
#[test]
fn dispatch_checked_reaches_through_dyn_command_queue() {
    let q = Minimal::new(4096);
    let dynq: &dyn CommandQueue = &q;
    let buf = buffer(1, 16);
    let bindings = [Binding::new(0, &buf, 0)];
    let declared = [spec(0, DType::F32, &[100])];
    assert_invalid_prefix(run_with_consts(dynq, &bindings, &[], &declared), "C4:");
    assert_eq!(q.calls(), 0);
}

/// `dispatch_checked` の引数と戻りの型を固定する。
#[test]
fn dispatch_checked_return_type_is_pinned() {
    // 引数と戻りの型を 1 つの関数ポインタの型に書き出すため、型の複雑さの lint を許す。
    #[allow(clippy::type_complexity)]
    let f: fn(
        &Minimal,
        &mut CommandEncoder,
        &PipelineHandle,
        &[Binding<'_>],
        &[ConstantBinding<'_>],
        &[OperandSpec<'_>],
        [u32; 3],
        [u32; 3],
        &[ThreadgroupMemory],
    ) -> Result<(), ContractError> = <Minimal as CommandQueue>::dispatch_checked;
    let q = Minimal::new(4096);
    let mut encoder = q.create_encoder();
    let pipeline = PipelineHandle::__backend_new(1, 1);
    let result = f(
        &q,
        &mut encoder,
        &pipeline,
        &[],
        &[],
        &[spec(0, DType::Bool, &[4])],
        [1, 1, 1],
        [1, 1, 1],
        &[],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(q.calls(), 1);
}
