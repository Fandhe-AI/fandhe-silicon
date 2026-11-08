# 成果物のキャッシュキーの口（TASK-501）の所在と本リポの現状

`ComputeOps` が成果物のキャッシュキーを返す口の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-501。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/core/src/execution/compute.rs` の `ComputeOps::pipeline_cache_key`（TASK-769 のポインタ） | `grep -n 'pipeline_cache_key' crates/core/src/execution/compute.rs` |
| キーの型 | `crates/core/src/execution.rs` の `CacheKey`（TASK-470 のポインタ） | `grep -n 'pub struct CacheKey' crates/core/src/execution.rs` |
| 試験 | `crates/core/tests/compute_ops_shape.rs` の `default_pipeline_cache_key_is_none` と、`compute.rs` の doctest | 下の「キャッシュキーの口の既定（1/4）」 |

## キャッシュキーの口の既定（1/4）

- シグネチャは `fn pipeline_cache_key(&self, _input: &PipelineInput<'_>, _bounds_checked: bool) -> Option<CacheKey>`。
- 既定の本体は入力を読まずに `None` を返す。パイプラインは作らない。
- 上書きしない実装は `None` を返す。今は試験用の device の compute の口がそうである。
- 試験 `default_pipeline_cache_key_is_none` は、`Msl` と `Wgsl` の 2 入力・`bounds_checked` の 2 値で `None` になることと、`create_pipeline` が呼ばれないことを確かめる。
- 確かめ方: `cargo test -p fandhe-silicon-core --test compute_ops_shape`・`cargo test -p fandhe-silicon-core --doc execution::compute`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | `ComputeOps` に `pipeline_cache_key` の口を足す | #4957 |
