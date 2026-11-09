# CUDA の背後実装の宣言を照らしてからのパイプラインの生成（TASK-457）の所在と本リポの現状

TASK-457 の作業のうち、`ComputeOps::create_pipeline_checked` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-457。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `ComputeOps::create_pipeline_checked`（既定の本体。`CudaDevice` は上書きしない） | `grep -n 'create_pipeline_checked' crates/core/src/execution/compute.rs` |
| 実装のファイル | `crates/core/src/execution/compute.rs` | 同上 |
| cfg | feature なし | `crates/core/src/execution/compute.rs` |
| 試験 | `crates/core/tests/compute_create_pipeline_checked.rs`（device に触らない） | `cargo test -p fandhe-silicon-core --test compute_create_pipeline_checked` |

## 宣言を照らしてから作る口（1/4）

引数は `input`・`layout`・`declared`。既定の本体の順は `validate_declared_operands(declared, layout)` → `create_pipeline(input, layout)`。

| 入力 | 返す値 |
| ---- | ------ |
| 宣言の件数が `layout.slots` と違う | 先頭が `C1` の `InvalidArgument`。`create_pipeline` を呼ばない |
| 照らしを通る | `create_pipeline` を 1 回呼び、結果を値のまま返す |

試験:

- `declared_count_mismatch_does_not_call_create_pipeline`
- `matching_declaration_calls_create_pipeline_once_and_returns_as_is`
- `reachable_through_dyn_compute_ops`
- `create_pipeline_checked_signature_is_pinned`

確かめ方: `cargo test -p fandhe-silicon-core --test compute_create_pipeline_checked`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | ComputeOps に宣言を照らしてから作る口を足す | #4851 |
