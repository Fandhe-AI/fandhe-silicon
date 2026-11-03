# 描画の操作の引数と戻りの型を固定する試験（TASK-908）の所在と本リポの現状

TASK-908 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
事実はリポジトリのルートで流したコマンドの出力から書く。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-908。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 署名の型の試験（1/2）

### 所在

| 項目 | 事実 |
| ---- | ---- |
| 試験 | `crates/core/tests/graphics_signatures.rs`（外の crate の統合テスト。公開のパスだけを使う） |
| 登録 | `crates/core/Cargo.toml` の末尾の `[[test]]`（`name = "graphics_signatures"`・`required-features = ["graphics"]`） |
| 固定の方法 | `type G = dyn GraphicsOps;` の上で `<G as GraphicsOps>::<操作>` を関数ポインタの型へ強制する |
| 操作の数 | `GraphicsOps` の操作は `destroy_offscreen_target` を含めて 11。この試験が固定するのはそのうち 10 |
| 固定しない操作 | `destroy_offscreen_target`（所在は [texture-view-destroy-copy.md](texture-view-destroy-copy.md)・TASK-372 のポインタ） |

### 固定する型

| 操作 | 引数と戻りの型 |
| ---- | -------------- |
| `create_render_pipeline` | `fn(&G, PipelineInput<'_>, PipelineInput<'_>, &BindingLayout) -> Result<PipelineHandle, ContractError>` |
| `create_render_pipeline_with_depth` | `fn(&G, PipelineInput<'_>, PipelineInput<'_>, &BindingLayout, DepthFormat) -> Result<PipelineHandle, ContractError>` |
| `create_offscreen_target` | `fn(&G, u32, u32, PixelFormat) -> Result<RenderTargetHandle, ContractError>` |
| `create_offscreen_target_with_depth` | `fn(&G, u32, u32, PixelFormat, DepthConfig) -> Result<(RenderTargetHandle, RenderTargetHandle), ContractError>` |
| `create_render_encoder` | `fn(&G, &RenderTargetHandle) -> RenderEncoder` |
| `create_render_encoder_with_depth` | `fn(&G, &RenderTargetHandle, &RenderTargetHandle, [f32; 4]) -> RenderEncoder` |
| `draw` | `fn(&G, &mut RenderEncoder, &PipelineHandle, &BufferHandle, u32) -> Result<(), ContractError>` |
| `draw_ex` | `fn(&G, &mut RenderEncoder, &PipelineHandle, &BufferHandle, u32, &[ConstantBinding<'_>]) -> Result<(), ContractError>` |
| `submit_render` | `fn(&G, RenderEncoder, &[ImportedSemaphoreHandle]) -> Result<SubmitToken, ContractError>` |
| `copy_target_to_buffer` | `fn(&G, &mut CommandEncoder, &RenderTargetHandle, &BufferHandle, u64) -> Result<(), ContractError>` |
| `RenderEncoder::__backend_rejected` | `fn(&RenderEncoder) -> Option<&ContractError>` |

### 試験の名前と固定する操作

| 試験 | 固定する操作 |
| ---- | ------------ |
| `pipeline_and_target_creation_signatures` | `create_render_pipeline`・`create_render_pipeline_with_depth`・`create_offscreen_target`・`create_offscreen_target_with_depth`。`PixelFormat` の 2 値が `PixelFormat` として書けること |
| `render_encoder_creation_returns_encoder` | `create_render_encoder`・`create_render_encoder_with_depth`・`__backend_rejected`。`__backend_new_rejected` で組んだ値の `__backend_rejected()` が `Some(&ContractError::DeviceMismatch)` であること |
| `submit_render_wait_signature` | `submit_render` |
| `draw_signatures_return_result` | `draw`・`draw_ex` |
| `draw_error_values_are_contract_error` | `Result<(), ContractError>` に `InvalidArgument(String)` と `DeviceMismatch` の `Err` が書けること |
| `submit_render_takes_encoder_by_value` | `submit_render`（第 2 引数が `RenderEncoder` の値） |
| `copy_target_to_buffer_signature` | `copy_target_to_buffer` |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features graphics --test graphics_signatures
```

出力は `Running tests/graphics_signatures.rs` に続けて `7 passed; 0 failed; 0 ignored`。

### 流れる組

| 組 | 流れるか |
| -- | -------- |
| `make verify`（`--all-features`） | 流れる |
| `make build-profiles` の core の `graphics` だけの組・`test-support,graphics` の組 | 流れる |
| feature なしの `cargo test -p fandhe-silicon-core` | 流れない（`Running tests/graphics_signatures.rs` の行が無い） |
| `make wasm-verify` の core の wasm32 型検査 | 型検査に乗る |
