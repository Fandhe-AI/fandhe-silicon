# CUDA の背後実装の request の組み合わせの検査と、ほかの背後実装の provider を呼ばないことの試験（TASK-290）の所在と本リポの現状

TASK-290 の作業のうち、CUDA の背後実装の `DeviceProvider::request` の組み合わせの検査の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-290。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験のファイル | `crates/contract/tests/backend_cuda_request_combinations.rs` | 同ファイル |
| `[[test]]` の `required-features` | `backend-cuda`・`fault-injection` | `crates/contract/Cargo.toml` |
| cfg | `not(target_arch = "wasm32")` | 同ファイルの先頭 |
| 試験 | 2 本 | 下の「確かめ方」 |
| `DeviceUnavailable` の試験 | feature `fault-injection` の差し込みの provider で確かめる | `crates/contract/tests/backend_cuda_fault.rs` |
| request の検査の試験（TASK-160） | `crates/contract/tests/backend_cuda_request.rs` | 同ファイル |

## 組み合わせの検査の試験（1/3）

試験は device に触らない入口だけを使う。入口は、検査で拒まれる要求を渡した `CudaDeviceProvider::request` と、差し込みの 5 つの provider の `request`（feature `fault-injection`）。

| 試験 | 入口 | 入力 | 確かめること |
| ---- | ---- | ---- | ------------ |
| `other_backends_with_graphics_or_cpu_only_are_invalid_combinations` | `CudaDeviceProvider` と差し込みの 5 つ | `Metal`・`Vulkan`・`Dx12Compute` と `Graphics`・`CpuOnly`（`ProductIndependent`）の全組 | どれも `InvalidCombination` |
| `the_cuda_source_names_no_other_backend_provider` | なし | `crates/contract/src/backend_cuda.rs` のソース | `MetalDeviceProvider`・`VulkanDeviceProvider`・`backend_metal`・`backend_vulkan` を含まない |

エラーは値の種類だけ（`matches!`）で見る。

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_request_combinations
```

2 本を流す。

## ほかの背後実装の provider を呼ばないことの試験（2/3）

CUDA の `request` の失敗の後に、Vulkan の provider（macOS かつ `backend-metal` のビルドでは Metal の provider も）の `request` が呼ばれないことを、`request_call_count`（TASK-277 で足した口）で照らす。device に触らない入口だけを使う。

| 試験 | 入口 | 入力 | 確かめること |
| ---- | ---- | ---- | ------------ |
| `cuda_request_failures_call_no_other_backend_provider`（`crates/contract/tests/backend_cuda_request_no_other_provider.rs`） | `CudaDriverMissingProvider`・`CudaDeviceProvider`・`VulkanDeviceProvider`（macOS かつ `backend-metal` では `MetalDeviceProvider`） | CUDA の provider に `Cuda`・`Vulkan` の要求（fail-fast の模擬 3 回と縮退の模擬 2 回）、続けて Vulkan・Metal の provider に `Cuda` の要求（検査で拒まれる） | CUDA・Vulkan の数が `[0, 0]` → `[5, 0]` → `[5, 1]`。macOS かつ `backend-metal` のビルドでは Metal の数が 0 → 0 → 1 |

- `[[test]]` の `required-features`: `backend-cuda`・`backend-vulkan`・`fault-injection`（`crates/contract/Cargo.toml`）
- cfg は `not(target_arch = "wasm32")`。Metal の数は macOS かつ `backend-metal` のビルドでだけ照らす
- 数は値そのもので照らし、エラーの照合は値の種類だけ（`matches!`）

確かめ方。

- Linux: `cargo test -p fandhe-silicon-contract --features backend-cuda,backend-vulkan,fault-injection --test backend_cuda_request_no_other_provider`
- macOS: `cargo test -p fandhe-silicon-contract --all-features --test backend_cuda_request_no_other_provider`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 組み合わせの検査の試験 | #4148 |
| 2/3 | ほかの背後実装の provider を呼ばないことの試験 | #4149 |
