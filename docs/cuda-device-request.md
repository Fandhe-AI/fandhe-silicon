# CUDA の背後実装の device の取得・診断・確保（TASK-160）の所在と本リポの現状

TASK-160 の作業のうち、CUDA の背後実装の `DeviceProvider::request` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-160。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 公開の型 | `CudaDeviceProvider` | `grep -n 'pub ' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | `crates/contract/tests/backend_cuda_request.rs`。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_request` |

## request の検査（1/5）

`request` が要求を照らす順（device を作る前。最初に当たったものを返す）:

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 背後実装が CUDA でない | `InvalidCombination` |
| 2 | `Graphics`・`CpuOnly` | `InvalidCombination` |
| 3 | `Shared`・`Imported` | `Unsupported { backend: Cuda, .. }` |

- 背後実装と profile の拒否は、所有形態の拒否より先に見る
- `Graphics` は contract の feature `graphics` の有無によらず拒む

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | request の検査と provider の入口 | #3718 |
