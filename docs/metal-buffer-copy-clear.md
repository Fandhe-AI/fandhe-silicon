# バッファのコピーとクリアの記録の操作と Metal の背後実装（TASK-610）の所在と本リポの現状

`MemoryOps` の、記録先へバッファ間のコピーとバッファのクリアを記録する 2 つの操作の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-610。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 操作の口 | `crates/core/src/memory/ops.rs` の `MemoryOps` の 2 つのメソッド `copy_buffer_to_buffer`・`clear_buffer` | `grep -n 'fn copy_buffer_to_buffer\|fn clear_buffer' crates/core/src/memory/ops.rs` |
| 試験用の device | `crates/core/src/stub_device.rs` の `StubMemory` | 同ファイル |
| 背後実装 | `crates/contract/src/backend_cuda.rs`・`backend_vulkan.rs`・`backend_metal.rs` の `impl MemoryOps` | 各ファイル |
| 試験 | `crates/core/tests/memory_copy_clear_stub.rs`（feature `test-support`） | 下の「確かめ方」 |

## 記録の操作の口（1/4）

`MemoryOps` に、既定の本体を持たない必須のメソッドを 2 つ持つ。feature・target の cfg は付かない。

```rust
fn copy_buffer_to_buffer(
    &self,
    encoder: &mut CommandEncoder,
    src: &BufferHandle,
    src_offset: u64,
    dst: &BufferHandle,
    dst_offset: u64,
    size: u64,
) -> Result<(), ContractError>;

fn clear_buffer(
    &self,
    encoder: &mut CommandEncoder,
    buffer: &BufferHandle,
    offset: u64,
    size: u64,
) -> Result<(), ContractError>;
```

試験用の device・CUDA・Vulkan・Metal の本体は、引数を使わずに `ContractError::Unsupported` を返す。`op` の値は `"copy_buffer_to_buffer"`・`"clear_buffer"`。
試験用の device の `backend` は、要求した背後実装の値。

足した試験は `copy_and_clear_on_the_stub_return_unsupported_and_record_nothing`。2 つの操作が `Unsupported` を返し、記録先の `Debug` 出力が操作の前後で変わらず `recorded: 0` のままであることを照らす。

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --features test-support --test memory_copy_clear_stub
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | MemoryOps にバッファのコピーとクリアの記録の操作を足す | #5220 |
