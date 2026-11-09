# CUDA の背後実装の device の口の到達（TASK-166）の所在と本リポの現状

TASK-166 の作業のうち、CUDA の背後実装の device から compute の口へ届く所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-166。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `impl Device for CudaDevice` の `compute()` | `grep -c 'fn compute(' crates/contract/src/backend_cuda.rs` |
| 試験 | `crates/contract/tests/backend_cuda_reach.rs`（どの試験も device に触る） | 下の「確かめ方」 |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")`（`crates/contract/src/lib.rs`） | 同上 |

## compute の口（1/3）

- `CudaDevice` の `Device::compute()` は `Some(self)` を返す
- `create_pipeline` の実装の所在は [cuda-create-pipeline.md](./cuda-create-pipeline.md)（TASK-803）
- `Device::compute()` の既定の本体の所在は [device-compute.md](./device-compute.md)（TASK-2956）

| 試験 | 確かめること |
| ---- | ------------ |
| `compute_only_device_has_compute_and_no_graphics` | `ComputeOnly`・`ProductIndependent` の device で `backend()` が `Cuda`、`compute()` が `Some`、`graphics()` が `None`（`graphics()` の行は feature `graphics` のビルドだけ） |
| `create_pipeline_through_compute_returns_a_handle` | `compute()` の `Some` から `create_pipeline` が `Ok(PipelineHandle)` を返す |

どちらも `#[ignore = "requires a CUDA device"]` を付ける。

確かめ方:

```bash
grep -c 'fn compute(' crates/contract/src/backend_cuda.rs
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_reach
cargo test -p fandhe-silicon-contract --features backend-cuda,graphics --test backend_cuda_reach
```

1 行目は `1`、2・3 行目は `0 passed; 0 failed; 2 ignored`。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | device から compute の口を返す | #3738 |
| 2/3 | 確保と記録先の作成へ届く device の要る試験を ignore 付きで足す | #3739 |
| 3/3 | 完了の条件の確かめ | #3740 |
